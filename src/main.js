// PGlove chrome UI logic — talks to the Rust backend over Tauri IPC.
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

const $ = (s) => document.querySelector(s);

const tabstrip = $('#tabstrip');
const addressInput = $('#address');
const addressGhost = $('#address-ghost');
const reloadBtn = $('#btn-reload');
const reloadIcon = $('#reload-icon');
const stopIcon = $('#stop-icon');
const loadingBall = $('#loading-ball');
const menu = $('#menu');
const aboutOverlay = $('#about-overlay');
const aboutClose = $('#about-close');

/** @type {import('@tauri-apps/api/core').InvokeArgs & { id: string; title: string; url: string; loading: boolean; active: boolean }[]} */
let tabs = [];
const byId = new Map();
let activeId = null;

const isStartUrl = (u) => !u || u === 'start' || /tauri\.localhost|^pglove:\/\//.test(u);

function displayUrl(u) {
  if (isStartUrl(u)) return '';
  return u.replace(/^https?:\/\//, '').replace(/\/$/, '');
}

// ------------------------------------------------------------------ render

function render(list) {
  tabs = list;
  byId.clear();
  for (const t of list) byId.set(t.id, t);
  const active = list.find((t) => t.active);
  activeId = active ? active.id : null;

  tabstrip.innerHTML = '';
  for (const t of list) {
    const el = document.createElement('div');
    el.className = 'tab' + (t.active ? ' active' : '');
    el.dataset.id = t.id;
    el.title = t.url && !isStartUrl(t.url) ? t.url : t.title || 'New Tab';

    const dot = document.createElement('span');
    dot.className = 'tab-dot';
    el.appendChild(dot);

    if (t.loading) {
      const ld = document.createElement('span');
      ld.className = 'tab-loading';
      el.appendChild(ld);
    }

    const title = document.createElement('span');
    title.className = 'tab-title';
    title.textContent = t.title || 'New Tab';
    el.appendChild(title);

    const close = document.createElement('button');
    close.className = 'tab-close';
    close.textContent = '\u00d7';
    close.title = 'Close tab (Ctrl+W)';
    close.addEventListener('click', (e) => {
      e.stopPropagation();
      closeTab(t.id);
    });
    el.appendChild(close);

    el.addEventListener('click', () => activateTab(t.id));
    el.addEventListener('auxclick', (e) => {
      if (e.button === 1) closeTab(t.id);
    });
    tabstrip.appendChild(el);
  }

  const add = document.createElement('button');
  add.className = 'tab-add';
  add.textContent = '+';
  add.title = 'New tab (Ctrl+T)';
  add.addEventListener('click', () => newTab());
  tabstrip.appendChild(add);

  syncAddressBar();
  syncTitle();
  syncLoadingUI();
}

function syncAddressBar() {
  if (document.activeElement === addressInput) return;
  const t = activeId ? byId.get(activeId) : null;
  if (!t) {
    addressInput.value = '';
    addressGhost.hidden = true;
    return;
  }
  if (isStartUrl(t.url)) {
    addressInput.value = '';
    addressGhost.hidden = false;
  } else {
    addressInput.value = displayUrl(t.url);
    addressGhost.hidden = true;
  }
}

function syncTitle() {
  const t = activeId ? byId.get(activeId) : null;
  document.title = t ? `${t.title || 'New Tab'} — PGlove` : 'PGlove';
}

function syncLoadingUI() {
  const t = activeId ? byId.get(activeId) : null;
  const loading = !!t && !!t.loading;
  reloadIcon.hidden = loading;
  stopIcon.hidden = !loading;
  loadingBall.hidden = !loading;
  reloadBtn.title = loading ? 'Stop (Esc)' : 'Reload (Ctrl+R)';
}

// ------------------------------------------------------------------ actions

async function newTab(url) {
  try {
    await invoke('new_tab', { url: url ?? null });
  } catch (e) {
    console.error('new_tab failed', e);
  }
}

async function closeTab(id) {
  try {
    await invoke('close_tab', { id });
  } catch (e) {
    console.error('close_tab failed', e);
  }
}

async function activateTab(id) {
  try {
    await invoke('activate_tab', { id });
  } catch (e) {
    console.error('activate_tab failed', e);
  }
}

async function navigate(activeId, url) {
  if (!activeId) return;
  try {
    await invoke('navigate', { id: activeId, url });
  } catch (e) {
    console.error('navigate failed', e);
  }
}

async function goBack() {
  if (!activeId) return;
  await invoke('go_back', { id: activeId }).catch(() => {});
}

async function goForward() {
  if (!activeId) return;
  await invoke('go_forward', { id: activeId }).catch(() => {});
}

async function reloadActive() {
  if (!activeId) return;
  await invoke('reload_tab', { id: activeId }).catch(() => {});
}

async function stopActive() {
  if (!activeId) return;
  await invoke('stop_loading', { id: activeId }).catch(() => {});
}

// -------------------------------------------------------------- address bar

function resolveInput(raw) {
  const s = raw.trim();
  if (!s) return null;
  if (/^https?:\/\//i.test(s)) return s;
  if (/^(localhost|127\.0\.0\.1)(:\d+)?([/?#].*)?$/i.test(s)) return 'http://' + s;
  const withSpace = s.includes(' ');
  const looksLikeHost =
    !withSpace &&
    !s.endsWith('.') &&
    /^[a-z0-9.-]+(\.[a-z]{2,}|:[0-9]+)([/?#][\s\S]*)?$/i.test(s);
  if (looksLikeHost) return 'https://' + s;
  return 'https://duckduckgo.com/?q=' + encodeURIComponent(s);
}

addressInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    e.preventDefault();
    const url = resolveInput(addressInput.value);
    if (url) navigate(activeId, url);
    addressInput.blur();
  } else if (e.key === 'Escape') {
    addressInput.blur();
  }
});

addressInput.addEventListener('focus', () => {
  if (!addressInput.value) addressInput.select();
  else addressInput.select();
});

addressInput.addEventListener('blur', syncAddressBar);

// ---------------------------------------------------------------- toolbar

$('#btn-back').addEventListener('click', goBack);
$('#btn-forward').addEventListener('click', goForward);

reloadBtn.addEventListener('click', () => {
  if (stopIcon.hidden) reloadActive();
  else stopActive();
});

$('#btn-newtab').addEventListener('click', () => newTab());
$('#btn-menu').addEventListener('click', (e) => {
  e.stopPropagation();
  menu.hidden = !menu.hidden;
});
document.addEventListener('click', () => {
  menu.hidden = true;
});

menu.addEventListener('click', (e) => {
  const btn = e.target.closest('.menu-item');
  if (!btn) return;
  menu.hidden = true;
  const action = btn.dataset.action;
  if (action === 'new-tab') newTab();
  else if (action === 'start') navigate(activeId, 'start');
  else if (action === 'pglove-site') newTab('pglove.jo3.org');
  else if (action === 'github') newTab('github.com/TechAdityaBRO/PGlove');
  else if (action === 'about') aboutOverlay.hidden = false;
});

aboutClose.addEventListener('click', () => {
  aboutOverlay.hidden = true;
});
aboutOverlay.addEventListener('click', (e) => {
  if (e.target === aboutOverlay) aboutOverlay.hidden = true;
});

// -------------------------------------------------------------- keyboard

window.addEventListener('keydown', (e) => {
  const mod = e.ctrlKey || e.metaKey;
  if (mod && e.key.toLowerCase() === 't') {
    e.preventDefault();
    newTab();
  } else if (mod && e.key.toLowerCase() === 'w') {
    e.preventDefault();
    if (activeId) closeTab(activeId);
  } else if ((mod && e.key.toLowerCase() === 'l') || (e.altKey && e.key.toLowerCase() === 'd')) {
    e.preventDefault();
    addressInput.focus();
    addressInput.select();
  } else if (mod && e.key.toLowerCase() === 'r') {
    e.preventDefault();
    reloadActive();
  } else if (e.altKey && e.key === 'ArrowLeft') {
    e.preventDefault();
    goBack();
  } else if (e.altKey && e.key === 'ArrowRight') {
    e.preventDefault();
    goForward();
  } else if (e.key === 'Escape') {
    menu.hidden = true;
    if (activeId) stopActive();
  }
});

// --------------------------------------------------------------- resize

let layoutTimer = null;
window.addEventListener('resize', () => {
  clearTimeout(layoutTimer);
  layoutTimer = setTimeout(
    () => invoke('layout', { width: window.innerWidth, height: window.innerHeight }).catch(() => {}),
    60,
  );
});

// ----------------------------------------------------------------- events

listen('state', (e) => render(e.payload)).catch(() => {});
listen('open-in-tab', (e) => newTab(e.payload)).catch(() => {});

// ------------------------------------------------------------------- boot

(async function boot() {
  await invoke('layout', { width: window.innerWidth, height: window.innerHeight }).catch(() => {});
  try {
    render(await invoke('get_state'));
  } catch {
    /* backend still starting */
  }
  // The first tab is created shortly after app start; poll briefly if needed.
  for (let i = 0; i < 12 && tabs.length === 0; i++) {
    await new Promise((r) => setTimeout(r, 150));
    try {
      render(await invoke('get_state'));
    } catch {
      /* keep polling */
    }
  }
})();