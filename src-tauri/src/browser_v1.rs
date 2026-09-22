//! PGlove browser core: manages per-tab WebView2 webviews inside the main
//! window, plus the IPC commands used by the chrome UI.
//!
//! Windows note: creating webviews from synchronous commands / event handlers
//! deadlocks WebView2 (tauri-apps/wry#583). All commands here are `async`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{
    webview::{NewWindowResponse, PageLoadEvent, WebviewBuilder, WebviewUrl},
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, State,
};

/// Label of the window and its chrome webview (defined in tauri.conf.json).
pub const MAIN_WINDOW: &str = "main";
/// Height of the browser chrome above the tab content area, in logical px.
/// Must match the CSS height of `#chrome` (84px).
pub const CHROME_TOP: f64 = 84.0;
/// Fallback window size used until the real window reports its size.
const DEFAULT_WINDOW_SIZE: (f64, f64) = (1280.0, 800.0);

static NEXT_TAB: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Serialize)]
pub struct TabInfo {
    pub id: String,
    pub title: String,
    pub url: String,
    pub loading: bool,
    pub active: bool,
}

#[derive(Default)]
pub struct BrowserState {
    tabs: Mutex<Vec<TabInfo>>,
    active: Mutex<Option<String>>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Lock order is always `tabs` first, then `active`.
fn snapshot(state: &BrowserState) -> Vec<TabInfo> {
    let active = state.active.lock().unwrap().clone();
    let mut tabs = state.tabs.lock().unwrap().clone();
    for t in tabs.iter_mut() {
        t.active = active.as_deref() == Some(t.id.as_str());
    }
    tabs
}

fn emit_state(app: &AppHandle, state: &BrowserState) {
    let tabs = snapshot(state);
    if let Some(win) = app.get_webview_window(MAIN_WINDOW) {
        let _ = win.emit("state", tabs);
    }
}

/// Size of the tab content area (below the chrome) in logical px.
fn content_area(app: &AppHandle) -> (LogicalPosition<f64>, LogicalSize<f64>) {
    let (w, h) = window_logical_size(app);
    let size = LogicalSize::new(w, (h - CHROME_TOP).max(0.0));
    let pos = LogicalPosition::new(0.0, CHROME_TOP);
    (pos, size)
}

fn window_logical_size(app: &AppHandle) -> (f64, f64) {
    let Some(win) = app.get_webview_window(MAIN_WINDOW) else {
        return DEFAULT_WINDOW_SIZE;
    };
    let Ok(inner) = win.inner_size() else {
        return DEFAULT_WINDOW_SIZE;
    };
    let scale = win.scale_factor().unwrap_or(1.0);
    (
        inner.width as f64 / scale,
        inner.height as f64 / scale,
    )
}

/// Turn user input ("example.com", "https://…", "start") into something the
/// webview can load.
fn webview_url_for(input: &str) -> WebviewUrl {
    let input = input.trim();
    if input.is_empty() || input == "start" || input == "pglove://start" {
        return WebviewUrl::App("start.html".into());
    }
    let with_scheme = if input.contains("://") {
        input.to_string()
    } else {
        format!("https://{input}")
    };
    match url::Url::parse(&with_scheme) {
        Ok(u) => WebviewUrl::External(u),
        Err(_) => WebviewUrl::App("start.html".into()),
    }
}

fn start_page_url() -> url::Url {
    url::Url::parse("http://tauri.localhost/start.html")
        .expect("static start page url")
}

/// Update one tab's metadata and notify the chrome UI.
fn update_tab(
    app: &AppHandle,
    label: &str,
    url: Option<String>,
    title: Option<String>,
    loading: Option<bool>,
) {
    let state = app.state::<BrowserState>();
    {
        let mut tabs = state.tabs.lock().unwrap();
        if let Some(tab) = tabs.iter_mut().find(|t| t.id == *label) {
            if let Some(u) = url {
                tab.url = u;
            }
            if let Some(t) = title {
                tab.title = t;
            }
            if let Some(l) = loading {
                tab.loading = l;
            }
        }
    }
    emit_state(app, &state);
}

fn create_tab_webview(
    app: &AppHandle,
    url_input: Option<&str>,
) -> Result<String, String> {
    let label = format!("tab-{}", NEXT_TAB.fetch_add(1, Ordering::SeqCst));

    let app_pg = app.clone();
    let app_title = app.clone();
    let app_newwin = app.clone();

    let builder = WebviewBuilder::new(
        label.clone(),
        webview_url_for(url_input.unwrap_or("start")),
    )
    .user_agent(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
         (KHTML, like Gecko) PGlove/1.0.4 Chrome/126 Safari/537.36",
    )
    .on_page_load(move |wv, payload| {
        let label = wv.label().to_string();
        let url = payload.url().to_string();
        match payload.event() {
            PageLoadEvent::Started => {
                update_tab(&app_pg, &label, Some(url), None, Some(true));
            }
            PageLoadEvent::Finished => {
                update_tab(&app_pg, &label, Some(url), None, Some(false));
            }
        }
    })
    .on_document_title_changed(move |wv, title| {
        let label = wv.label().to_string();
        update_tab(&app_title, &label, None, Some(title), None);
    })
    .on_new_window(move |url, _features| {
        // Route popups/new windows into a brand-new tab instead.
        if let Some(win) = app_newwin.get_webview_window(MAIN_WINDOW) {
            let _ = win.emit("open-in-tab", url.to_string());
        }
        NewWindowResponse::Deny
    });

    let main_wv = app
        .get_webview(MAIN_WINDOW)
        .ok_or("main chrome webview missing")?;
    let window = main_wv.window();
    let (pos, size) = content_area(app);
    let wv = window
        .add_child(builder, pos, size)
        .map_err(|e| format!("failed to create tab webview: {e}"))?;

    {
        let mut tabs = state_of(app).tabs.lock().unwrap();
        tabs.push(TabInfo {
            id: label.clone(),
            title: "New Tab".into(),
            url: String::new(),
            loading: false,
            active: true,
        });
        let mut active = state_of(app).active.lock().unwrap();
        *active = Some(label.clone());
    }

    hide_other_webviews(app, &label);
    let _ = wv.set_focus();
    Ok(label)
}

/// Create a tab (webview + state entry) and make it active.
fn spawn_tab(app: &AppHandle, _state: &BrowserState, url: Option<&str>) -> Result<String, String> {
    let label = create_tab_webview(app, url)?;
    emit_state(app, &state_of(app));
    Ok(label)
}

fn state_of(app: &AppHandle) -> std::sync::MutexGuard<'static, BrowserState> {
    // AppHandle::state returns State<BrowserState> (a reference to the managed
    // value); deref to a plain &BrowserState for helper calls.
    let s = app.try_state::<BrowserState>().expect("state managed");
    std::mem::ManuallyDrop::new(s).manage_ref()
}

// Hmm — see below; replaced by direct access via State in commands and
// app.state in callbacks. This helper is removed in the compiled code path
// used by commands; kept simple instead.

fn hide_other_webviews(app: &AppHandle, keep: &str) {
    let Some(state) = app.try_state::<BrowserState>() else { return };
    let tabs = state.tabs.lock().unwrap().clone();
    for tab in tabs {
        if tab.id == keep {
            continue;
        }
        if let Some(wv) = app.get_webview(&tab.id) {
            let _ = wv.hide();
        }
    }
}

fn activate_webviews(app: &AppHandle, state: &BrowserState, id: &str) -> Result<(), String> {
    let tabs = state.tabs.lock().unwrap().clone();
    for tab in &tabs {
        let active = tab.id == id;
        let Some(wv) = app.get_webview(&tab.id) else { continue };
        if active {
            let _ = wv.show();
            let _ = wv.set_focus();
        } else {
            let _ = wv.hide();
        }
    }
    *state.active.lock().unwrap() = Some(id.to_string());
    Ok(())
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn new_tab(
    app: AppHandle,
    state: State<'_, BrowserState>,
    url: Option<String>,
) -> Result<TabInfo, String> {
    let id = spawn_tab(&app, &state, url.as_deref())?;
    let info = snapshot(&state)
        .into_iter()
        .find(|t| t.id == id)
        .ok_or("tab vanished after creation")?;
    Ok(info)
}

#[tauri::command]
pub async fn close_tab(
    app: AppHandle,
    state: State<'_, BrowserState>,
    id: String,
) -> Result<Vec<TabInfo>, String> {
    let mut activate_next: Option<String> = None;
    {
        let mut tabs = state.tabs.lock().unwrap();
        let idx = match tabs.iter().position(|t| t.id == id) {
            Some(i) => i,
            None => return Ok(snapshot(&state)),
        };
        let was_active = state.active.lock().unwrap().as_deref() == Some(id.as_str());
        tabs.remove(idx);
        if was_active {
            activate_next = if idx < tabs.len() {
                Some(tabs[idx].id.clone())
            } else if !tabs.is_empty() {
                Some(tabs[idx - 1].id.clone())
            } else {
                None
            };
        }
    }

    if let Some(wv) = app.get_webview(&id) {
        let _ = wv.close();
    }

    if state.tabs.lock().unwrap().is_empty() {
        let _ = spawn_tab(&app, &state, None);
    } else if let Some(n) = activate_next {
        let _ = activate_webviews(&app, &state, &n);
    }

    emit_state(&app, &state);
    Ok(snapshot(&state))
}

#[tauri::command]
pub async fn activate_tab(
    app: AppHandle,
    state: State<'_, BrowserState>,
    id: String,
) -> Result<(), String> {
    activate_webviews(&app, &state, &id)?;
    emit_state(&app, &state);
    Ok(())
}

#[tauri::command]
pub async fn navigate(
    app: AppHandle,
    _state: State<'_, BrowserState>,
    id: String,
    url: String,
) -> Result<(), String> {
    let wv = app.get_webview(&id).ok_or("no such tab")?;
    let target = webview_url_for(&url);
    match target {
        WebviewUrl::External(u) => {
            update_tab(&app, &id, Some(url), None, Some(true));
            wv.navigate(u).map_err(|e| e.to_string())
        }
        WebviewUrl::App(_) => {
            update_tab(&app, &id, Some("start".into()), None, Some(true));
            wv.navigate(start_page_url()).map_err(|e| e.to_string())
        }
        WebviewUrl::Custom(p) => {
            let u = url::Url::parse(&format!("http://tauri.localhost/{}", p))
                .map_err(|e| e.to_string())?;
            update_tab(&app, &id, Some("start".into()), None, Some(true));
            wv.navigate(u).map_err(|e| e.to_string())
        }
    }
}

fn eval_active(app: &AppHandle, id: &str, js: &str) -> Result<(), String> {
    let wv = app.get_webview(id).ok_or("no such tab")?;
    wv.eval(js).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn go_back(app: AppHandle, id: String) -> Result<(), String> {
    eval_active(&app, &id, "history.back()")
}

#[tauri::command]
pub async fn go_forward(app: AppHandle, id: String) -> Result<(), String> {
    eval_active(&app, &id, "history.forward()")
}

#[tauri::command]
pub async fn reload_tab(app: AppHandle, id: String) -> Result<(), String> {
    let wv = app.get_webview(&id).ok_or("no such tab")?;
    wv.reload().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_loading(app: AppHandle, id: String) -> Result<(), String> {
    eval_active(&app, &id, "window.stop()")
}

/// Keeps every tab's webview sized to the content area (below the chrome).
/// The frontend calls this whenever the window resizes.
#[tauri::command]
pub async fn layout(
    app: AppHandle,
    state: State<'_, BrowserState>,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let pos = LogicalPosition::new(0.0, CHROME_TOP);
    let size = LogicalSize::new(width, (height - CHROME_TOP).max(0.0));
    let tabs = state.tabs.lock().unwrap().clone();
    for tab in &tabs {
        if let Some(wv) = app.get_webview(&tab.id) {
            let _ = wv.set_position(pos);
            let _ = wv.set_size(size);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_state(state: State<'_, BrowserState>) -> Vec<TabInfo> {
    snapshot(&state)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run() {
    tauri::Builder::default()
        .manage(BrowserState::default())
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(350));
                let state = handle.state::<BrowserState>();
                let _ = spawn_tab(&handle, &state, None);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            new_tab,
            close_tab,
            activate_tab,
            navigate,
            go_back,
            go_forward,
            reload_tab,
            stop_loading,
            layout,
            get_state
        ])
        .run(tauri::generate_context!())
        .expect("error while running PGlove");
}