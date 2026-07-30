#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]
// ^ Hide the damn console window on Windows release builds
// also: "please god don't let a random terminal pop up and traumatize the user"

// drag in the tauri shit we actually need
// Manager → get windows by label; Menu/MenuItem → tray menu items; TrayIconBuilder → the little guy in the taskbar
// Image → for the icon because apparently you need to import that separately like a psychopath
use tauri::{
    Emitter,
    Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    image::Image,
    webview::WebviewBuilder,
    LogicalPosition, LogicalSize, WebviewUrl,
};
// and the global shortcut plugin — without this, ctrl+shift+space is just vibes
use tauri_plugin_global_shortcut::{Builder as ShortcutBuilder, GlobalShortcutExt, ShortcutState};
use serde::{Deserialize, Serialize};
use std::{fs, sync::Mutex, time::Duration};

// Height (in logical px) of the custom titlebar strip that lives at the top of the
// "main" webview. Tab webviews are children stacked below this, never overlapping it.
const TITLEBAR_H: f64 = 40.0;
const DEFAULT_TAB_URL: &str = "https://www.youtube.com/";
const SESSION_FILE: &str = "session.json";
const PROFILE_REGISTRY_FILE: &str = "profiles.json";
const HISTORY_FILE: &str = "history.json";
const BOOKMARKS_FILE: &str = "bookmarks.json";

// Injected into every tab webview. YouTube has its own right-click menu; this adds a
// minimal custom one just for links (video/channel/playlist anchors) with a single
// "Open link in new tab" action that calls back into Rust to spawn a real tab webview.
// Non-link right-clicks fall through to the page's native context menu untouched.
const CONTEXT_MENU_SCRIPT: &str = r#"
(function () {
  // A single uncaught exception anywhere below (e.g. touching the DOM before
  // it exists) used to silently abort the rest of this script, including the
  // fullscreen-detection listeners registered near the bottom — which is why
  // the app titlebar could fail to hide with no visible error anywhere.
  try {
  // Child webviews always receive Tauri's internal IPC bridge. The public API is
  // normally present too, but WebView2 can omit it for externally-created child
  // views; provide the narrow compatibility façade the rest of this script uses.
  if (!window.__TAURI__ && window.__TAURI_INTERNALS__?.invoke) {
    Object.defineProperty(window, '__TAURI__', {
      value: { core: { invoke: window.__TAURI_INTERNALS__.invoke } },
      configurable: false,
      writable: false
    });
  }
  let menu = null;
  let windowFullscreen = false;
  let queuePanel = null;
  function removeMenu() {
    if (menu) { menu.remove(); menu = null; }
  }
  function toggleMiniPlayer() {
    const video = document.querySelector('video.html5-main-video, video');
    if (!video || !document.pictureInPictureEnabled) return;
    if (document.pictureInPictureElement) {
      document.exitPictureInPicture().catch(function () {});
    } else {
      video.requestPictureInPicture().catch(function () {});
    }
  }
  function ensureQueuePanel() {
    if (queuePanel) return queuePanel;
    queuePanel = document.createElement('aside');
    queuePanel.id = 'ytd-desktop-queue';
    queuePanel.style.cssText = 'position:fixed;top:16px;right:16px;width:360px;max-height:calc(100vh - 32px);'
      + 'display:none;flex-direction:column;background:#212121;color:#f1f1f1;border:1px solid #444;'
      + 'border-radius:12px;box-shadow:0 8px 28px rgba(0,0,0,.55);z-index:2147483647;font:14px Roboto,Arial,sans-serif;overflow:hidden;';
    document.body.appendChild(queuePanel);
    return queuePanel;
  }
  function loadQueue() {
    if (!window.__TAURI__) return;
    window.__TAURI__.core.invoke('queue_list').then(function (items) {
      const panel = ensureQueuePanel();
      panel.replaceChildren();
      const header = document.createElement('div');
      header.style.cssText = 'display:flex;align-items:center;justify-content:space-between;padding:14px 16px;border-bottom:1px solid #444;font-weight:700;';
      header.append('Queue (' + items.length + ')');
      const clear = document.createElement('button');
      clear.textContent = 'Clear';
      clear.style.cssText = 'background:transparent;border:0;color:#8ab4f8;cursor:pointer;font:inherit;';
      clear.onclick = function () { window.__TAURI__.core.invoke('queue_clear').then(loadQueue); };
      header.appendChild(clear);
      panel.appendChild(header);
      const list = document.createElement('div');
      list.style.cssText = 'overflow:auto;padding:6px 0;';
      if (!items.length) {
        const empty = document.createElement('div');
        empty.textContent = 'Your queue is empty.';
        empty.style.cssText = 'padding:22px 16px;color:#aaa;text-align:center;';
        list.appendChild(empty);
      }
      items.forEach(function (item, index) {
        const row = document.createElement('div');
        row.style.cssText = 'display:flex;gap:8px;align-items:center;padding:10px 12px;';
        const play = document.createElement('button');
        play.textContent = '▶';
        play.title = 'Play now';
        const title = document.createElement('button');
        title.textContent = item.title || 'YouTube video';
        title.style.cssText = 'flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;background:transparent;border:0;color:#f1f1f1;text-align:left;cursor:pointer;font:inherit;';
        const up = document.createElement('button'); up.textContent = '↑'; up.title = 'Move up';
        const down = document.createElement('button'); down.textContent = '↓'; down.title = 'Move down';
        const remove = document.createElement('button'); remove.textContent = '×'; remove.title = 'Remove';
        [play, up, down, remove].forEach(function (button) {
          button.style.cssText = 'background:transparent;border:0;color:#ccc;cursor:pointer;font-size:16px;padding:3px;';
        });
        const playNow = function () { window.__TAURI__.core.invoke('queue_play', { index: index }); panel.style.display = 'none'; };
        play.onclick = playNow; title.onclick = playNow;
        up.onclick = function () { window.__TAURI__.core.invoke('queue_move', { index: index, offset: -1 }).then(loadQueue); };
        down.onclick = function () { window.__TAURI__.core.invoke('queue_move', { index: index, offset: 1 }).then(loadQueue); };
        remove.onclick = function () { window.__TAURI__.core.invoke('queue_remove', { index: index }).then(loadQueue); };
        row.append(play, title, up, down, remove);
        list.appendChild(row);
      });
      panel.appendChild(list);
    });
  }
  window.__YTD_QUEUE__ = {
    toggle: function () {
      const panel = ensureQueuePanel();
      panel.style.display = panel.style.display === 'flex' ? 'none' : 'flex';
      if (panel.style.display === 'flex') loadQueue();
    },
    add: function (url, title) {
      if (window.__TAURI__) window.__TAURI__.core.invoke('queue_add', { url: url, title: title || 'YouTube video' });
    }
  };
  const playbackStorageKey = 'ytd-desktop-playback';
  function playbackData() {
    try { return JSON.parse(localStorage.getItem(playbackStorageKey)) || { resume: {}, speeds: {} }; }
    catch (_) { return { resume: {}, speeds: {} }; }
  }
  function savePlaybackData(data) {
    try { localStorage.setItem(playbackStorageKey, JSON.stringify(data)); } catch (_) {}
  }
  function currentVideoKey() {
    try { return new URL(location.href).searchParams.get('v'); } catch (_) { return null; }
  }
  function currentChannelKey() {
    const channel = document.querySelector('ytd-watch-metadata #channel-name a[href], #owner a[href]');
    return channel && channel.href ? channel.href : 'default';
  }
  function activeVideo() {
    return document.querySelector('video.html5-main-video, video');
  }
  function rememberPlaybackPosition() {
    const video = activeVideo();
    const key = currentVideoKey();
    if (!video || !key || !Number.isFinite(video.currentTime)) return;
    const data = playbackData();
    if (video.ended || (Number.isFinite(video.duration) && video.duration - video.currentTime < 15)) {
      delete data.resume[key];
    } else {
      data.resume[key] = video.currentTime;
    }
    savePlaybackData(data);
  }
  function restorePlayback() {
    const video = activeVideo();
    const key = currentVideoKey();
    if (!video || !key || video.dataset.ytdDesktopPlaybackRestored) return;
    if (!Number.isFinite(video.duration) || video.duration <= 0) return;
    const data = playbackData();
    const savedPosition = data.resume[key];
    const savedSpeed = data.speeds[currentChannelKey()] || data.speeds.default;
    if (savedPosition && savedPosition > 5 && savedPosition < video.duration - 15) {
      video.currentTime = savedPosition;
    }
    if (savedSpeed) video.playbackRate = savedSpeed;
    video.dataset.ytdDesktopPlaybackRestored = 'true';
  }
  function changePlaybackSpeed(delta) {
    const video = activeVideo();
    if (!video) return;
    const rate = Math.max(.25, Math.min(3, Math.round((video.playbackRate + delta) * 100) / 100));
    video.playbackRate = rate;
    const data = playbackData();
    data.speeds[currentChannelKey()] = rate;
    savePlaybackData(data);
    const toast = document.createElement('div');
    toast.textContent = 'Playback speed: ' + rate + '×';
    toast.style.cssText = 'position:fixed;bottom:28px;left:50%;transform:translateX(-50%);padding:9px 14px;'
      + 'background:rgba(0,0,0,.85);color:#fff;border-radius:5px;font:14px Roboto,Arial,sans-serif;z-index:2147483647;';
    document.body.appendChild(toast);
    setTimeout(function () { toast.remove(); }, 1400);
  }
  window.__YTD_PLAYBACK__ = {
    speedUp: function () { changePlaybackSpeed(.25); },
    speedDown: function () { changePlaybackSpeed(-.25); },
    resetSpeed: function () { const video = activeVideo(); if (video) changePlaybackSpeed(1 - video.playbackRate); }
  };
  let lastRememberedSecond = -1;
  setInterval(restorePlayback, 1000);
  document.addEventListener('timeupdate', function (e) {
    const second = Math.floor(e.target.currentTime || 0);
    if (e.target.matches && e.target.matches('video.html5-main-video') && second - lastRememberedSecond >= 5) {
      lastRememberedSecond = second;
      rememberPlaybackPosition();
    }
  }, true);
  window.addEventListener('pagehide', rememberPlaybackPosition);
  document.addEventListener('visibilitychange', function () { if (document.hidden) rememberPlaybackPosition(); });
  const cleanViewStorageKey = 'ytd-desktop-clean-view';
  let cleanViewPanel = null;
  function cleanViewSettings() {
    try {
      return Object.assign({ shorts: false, comments: false, recommendations: false, distractionFree: false },
        JSON.parse(localStorage.getItem(cleanViewStorageKey)) || {});
    } catch (_) {
      return { shorts: false, comments: false, recommendations: false, distractionFree: false };
    }
  }
  function applyCleanView() {
    const settings = cleanViewSettings();
    const rules = [];
    if (settings.shorts) rules.push('ytd-rich-shelf-renderer[is-shorts],ytd-reel-shelf-renderer,ytd-shorts,ytd-rich-item-renderer:has(a[href^="/shorts/"]){display:none!important}');
    if (settings.comments) rules.push('#comments,ytd-comments{display:none!important}');
    if (settings.recommendations) rules.push('#secondary,ytd-watch-next-secondary-results-renderer{display:none!important}#primary{max-width:none!important}');
    if (settings.distractionFree) rules.push('#masthead-container,#guide,ytd-mini-guide-renderer,#chat,#secondary,#comments{display:none!important}#page-manager{margin-top:0!important}');
    let style = document.getElementById('ytd-desktop-clean-view-style');
    if (!style) {
      style = document.createElement('style');
      style.id = 'ytd-desktop-clean-view-style';
      document.documentElement.appendChild(style);
    }
    style.textContent = rules.join('\n');
  }
  function ensureCleanViewPanel() {
    if (cleanViewPanel) return cleanViewPanel;
    cleanViewPanel = document.createElement('aside');
    cleanViewPanel.style.cssText = 'position:fixed;top:16px;right:16px;width:300px;display:none;flex-direction:column;'
      + 'background:#212121;color:#f1f1f1;border:1px solid #444;border-radius:12px;box-shadow:0 8px 28px rgba(0,0,0,.55);'
      + 'z-index:2147483647;font:14px Roboto,Arial,sans-serif;overflow:hidden;';
    document.body.appendChild(cleanViewPanel);
    return cleanViewPanel;
  }
  function renderCleanViewPanel() {
    const panel = ensureCleanViewPanel();
    const settings = cleanViewSettings();
    panel.replaceChildren();
    const title = document.createElement('div');
    title.textContent = 'Clean View';
    title.style.cssText = 'padding:14px 16px;border-bottom:1px solid #444;font-weight:700;';
    panel.appendChild(title);
    [
      ['shorts', 'Hide Shorts'],
      ['comments', 'Hide comments'],
      ['recommendations', 'Hide recommendations'],
      ['distractionFree', 'Distraction-free layout']
    ].forEach(function (entry) {
      const row = document.createElement('label');
      row.style.cssText = 'display:flex;align-items:center;gap:10px;padding:11px 16px;cursor:pointer;';
      const checkbox = document.createElement('input');
      checkbox.type = 'checkbox';
      checkbox.checked = settings[entry[0]];
      checkbox.onchange = function () {
        const next = cleanViewSettings();
        next[entry[0]] = checkbox.checked;
        try { localStorage.setItem(cleanViewStorageKey, JSON.stringify(next)); } catch (_) {}
        applyCleanView();
      };
      row.append(checkbox, entry[1]);
      panel.appendChild(row);
    });
  }
  window.__YTD_CLEAN_VIEW__ = {
    toggle: function () {
      const panel = ensureCleanViewPanel();
      panel.style.display = panel.style.display === 'flex' ? 'none' : 'flex';
      if (panel.style.display === 'flex') renderCleanViewPanel();
    }
  };
  // This script runs at document-create time, before the parser has produced
  // <html> — document.documentElement is still null here. Defer until it
  // exists instead of crashing (which used to abort the rest of this IIFE,
  // silently skipping every listener registered below, including all of the
  // fullscreen detection that hides the app titlebar).
  (function applyCleanViewWhenReady() {
    if (document.documentElement) {
      applyCleanView();
    } else {
      setTimeout(applyCleanViewWhenReady, 0);
    }
  })();
  let libraryPanel = null;
  function ensureLibraryPanel() {
    if (libraryPanel) return libraryPanel;
    libraryPanel = document.createElement('aside');
    libraryPanel.style.cssText = 'position:fixed;top:16px;right:16px;width:390px;max-height:calc(100vh - 32px);display:none;flex-direction:column;'
      + 'background:#212121;color:#f1f1f1;border:1px solid #444;border-radius:12px;box-shadow:0 8px 28px rgba(0,0,0,.55);'
      + 'z-index:2147483647;font:14px Roboto,Arial,sans-serif;overflow:hidden;';
    document.body.appendChild(libraryPanel);
    return libraryPanel;
  }
  function formatBookmarkTime(seconds) {
    seconds = Math.floor(seconds || 0);
    return Math.floor(seconds / 60) + ':' + String(seconds % 60).padStart(2, '0');
  }
  function renderLibrary() {
    if (!window.__TAURI__) return;
    Promise.all([
      window.__TAURI__.core.invoke('bookmark_list'),
      window.__TAURI__.core.invoke('history_list')
    ]).then(function ([bookmarks, history]) {
      const panel = ensureLibraryPanel();
      panel.replaceChildren();
      const header = document.createElement('div');
      header.style.cssText = 'display:flex;align-items:center;justify-content:space-between;padding:14px 16px;border-bottom:1px solid #444;font-weight:700;';
      header.append('Bookmarks & Recent');
      const add = document.createElement('button');
      add.textContent = 'Bookmark current';
      add.style.cssText = 'background:transparent;border:0;color:#8ab4f8;cursor:pointer;font:inherit;';
      add.onclick = function () { window.__YTD_BOOKMARK__.add(); };
      header.appendChild(add);
      panel.appendChild(header);
      const search = document.createElement('input');
      search.placeholder = 'Search bookmarks and history';
      search.style.cssText = 'margin:12px;padding:9px 10px;background:#303030;border:1px solid #555;border-radius:5px;color:#fff;';
      panel.appendChild(search);
      const list = document.createElement('div');
      list.style.cssText = 'overflow:auto;padding:0 6px 8px;';
      function draw() {
        list.replaceChildren();
        const query = search.value.toLowerCase();
        const addSection = function (heading, items, bookmark) {
          const matching = items.map(function (item, index) { return { item: item, index: index }; }).filter(function (entry) {
            return !query || (entry.item.title || '').toLowerCase().includes(query);
          });
          if (!matching.length) return;
          const label = document.createElement('div');
          label.textContent = heading;
          label.style.cssText = 'padding:10px 10px 5px;color:#aaa;font-size:12px;font-weight:700;';
          list.appendChild(label);
          matching.forEach(function (entry) {
            const item = entry.item;
            const index = entry.index;
            const row = document.createElement('div');
            row.style.cssText = 'display:flex;align-items:center;gap:8px;padding:8px 10px;';
            const open = document.createElement('button');
            open.textContent = item.title || 'YouTube video';
            open.style.cssText = 'flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;background:transparent;border:0;color:#f1f1f1;text-align:left;cursor:pointer;font:inherit;';
            open.onclick = function () { location.href = item.url + (bookmark && item.timestamp ? (item.url.includes('?') ? '&t=' : '?t=') + Math.floor(item.timestamp) : ''); };
            row.appendChild(open);
            if (bookmark) {
              const stamp = document.createElement('span'); stamp.textContent = formatBookmarkTime(item.timestamp); stamp.style.color = '#aaa'; row.appendChild(stamp);
              const remove = document.createElement('button'); remove.textContent = '×'; remove.style.cssText = 'background:transparent;border:0;color:#ccc;cursor:pointer;font-size:18px;';
              remove.onclick = function () { window.__TAURI__.core.invoke('bookmark_remove', { index: index }).then(renderLibrary); };
              row.appendChild(remove);
            }
            list.appendChild(row);
          });
        };
        addSection('Bookmarks', bookmarks, true);
        addSection('Recent', history, false);
        if (!list.children.length) { const empty = document.createElement('div'); empty.textContent = 'No bookmarks or recent videos yet.'; empty.style.cssText = 'padding:20px;color:#aaa;text-align:center;'; list.appendChild(empty); }
      }
      search.oninput = draw;
      draw();
      panel.appendChild(list);
    });
  }
  window.__YTD_BOOKMARK__ = {
    add: function () {
      const video = activeVideo();
      if (window.__TAURI__) window.__TAURI__.core.invoke('bookmark_add', {
        url: location.href,
        title: document.title,
        timestamp: video ? video.currentTime : 0
      }).then(renderLibrary);
    }
  };
  window.__YTD_LIBRARY__ = {
    toggle: function () {
      const panel = ensureLibraryPanel();
      panel.style.display = panel.style.display === 'flex' ? 'none' : 'flex';
      if (panel.style.display === 'flex') renderLibrary();
    }
  };
  function reportFullscreen(fullscreen, playerFullscreen) {
    if (window.__TAURI__) {
      // Keep this separate from the state-report command. The titlebar must be
      // out of the way before WebView2/YouTube finish negotiating fullscreen.
      if (playerFullscreen) {
        window.__TAURI__.core.invoke('set_titlebar_hidden', { hidden: true }).catch(function () {});
      }
      window.__TAURI__.core.invoke('webview_fullscreen_changed', { fullscreen, playerFullscreen })
        .catch(function () {});
    }
  }
  function updateFullscreen() {
    const fullscreenElement = document.fullscreenElement
      || document.webkitFullscreenElement
      || document.mozFullScreenElement
      || document.msFullscreenElement
      || null;
    const player = document.querySelector('.html5-video-player');
    // YouTube sometimes uses its ytp-fullscreen state before (or instead of)
    // exposing a standard DOM fullscreen element in WebView2.
    const playerFullscreen = Boolean(
      (fullscreenElement && (
        fullscreenElement.matches?.('.html5-video-player, video')
        || fullscreenElement.querySelector?.('video')
      ))
      || player?.classList.contains('ytp-fullscreen')
    );
    reportFullscreen(Boolean(fullscreenElement) || playerFullscreen, playerFullscreen);
  }
  function findLink(e) {
    // Walk the DOM ancestry first (cheap, covers the common case).
    let el = e.target;
    while (el && el.tagName !== 'A') el = el.parentElement;
    if (el && el.href) return el;
    // YouTube's hover-preview overlay (the autoplay thumbnail on hover) can sit
    // visually on top of a thumbnail without being a DOM descendant of its <a> —
    // e.target then resolves to the overlay, not the link, and the walk above
    // misses it. elementsFromPoint sees everything actually stacked at the click
    // point regardless of subtree, so it catches the anchor underneath too.
    if (document.elementsFromPoint) {
      const stack = document.elementsFromPoint(e.clientX, e.clientY);
      for (const node of stack) {
        if (node.tagName === 'A' && node.href) return node;
        const anchor = node.closest && node.closest('a[href]');
        if (anchor) return anchor;
      }
    }
    return null;
  }
  function reportTabState() {
    if (window.__TAURI__ && window.__YTD_TAB_LABEL__) {
      window.__TAURI__.core.invoke('tab_navigated', {
        label: window.__YTD_TAB_LABEL__,
        url: location.href,
        title: document.title
      }).catch(function () {});
    }
  }
  function addMenuItem(label, action) {
    const item = document.createElement('button');
    item.textContent = label;
    item.style.cssText = 'display:block;width:100%;padding:8px 16px;background:transparent;border:0;'
      + 'color:#fff;text-align:left;font:13px Roboto,Arial,sans-serif;cursor:pointer;white-space:nowrap;';
    item.onmouseenter = function () { item.style.background = '#3a3a3a'; };
    item.onmouseleave = function () { item.style.background = 'transparent'; };
    item.onclick = function (ev) {
      ev.stopPropagation();
      removeMenu();
      action();
    };
    menu.appendChild(item);
  }
  document.addEventListener('contextmenu', function (e) {
    const el = findLink(e);
    e.preventDefault();
    removeMenu();
    menu = document.createElement('div');
    menu.style.cssText = 'position:fixed;left:' + e.clientX + 'px;top:' + e.clientY + 'px;'
      + 'background:#282828;color:#fff;padding:4px 0;border-radius:4px;'
      + 'box-shadow:0 2px 8px rgba(0,0,0,.5);z-index:2147483647;user-select:none;';
    if (el) {
      const href = el.href;
      addMenuItem('Open link in new tab', function () {
        if (window.__TAURI__) {
          window.__TAURI__.core.invoke('tab_new_url', { url: href });
        }
      });
      addMenuItem('Add to queue', function () {
        window.__YTD_QUEUE__.add(href, el.textContent.trim());
      });
    }
    addMenuItem(document.pictureInPictureElement ? 'Exit mini player' : 'Mini player', toggleMiniPlayer);
    addMenuItem(windowFullscreen ? 'Leave fullscreen' : 'Fullscreen', function () {
      if (window.__TAURI__) {
        window.__TAURI__.core.invoke('win_toggle_fullscreen');
      }
    });
    document.body.appendChild(menu);
    setTimeout(function () {
      document.addEventListener('click', removeMenu, { once: true });
      document.addEventListener('contextmenu', removeMenu, { once: true });
      document.addEventListener('scroll', removeMenu, { once: true });
    }, 0);
  }, true);

  document.addEventListener('keydown', function (e) {
    if (e.key === 'F11') {
      e.preventDefault();
      e.stopPropagation();
      if (window.__TAURI__) {
        window.__TAURI__.core.invoke('win_toggle_fullscreen').catch(function () {});
      }
      return;
    }
    if (e.ctrlKey && e.altKey && e.key.toLowerCase() === 't') {
      e.preventDefault();
      if (!e.repeat && window.__TAURI__) {
        window.__TAURI__.core.invoke('tab_toggle_theater');
      }
    }
    if (!e.ctrlKey && !e.altKey && !e.metaKey && e.key.toLowerCase() === 'f'
        && !/^(INPUT|TEXTAREA|SELECT)$/.test((e.target && e.target.tagName) || '')) {
      reportFullscreen(!document.fullscreenElement, true);
      setTimeout(updateFullscreen, 250);
    }
  }, true);

  document.addEventListener('pointerdown', function (e) {
    if (e.target.closest && e.target.closest('.ytp-fullscreen-button')) {
      // Hide the host chrome before YouTube completes its fullscreen animation.
      reportFullscreen(true, true);
      setTimeout(updateFullscreen, 250);
    }
  }, true);

  document.addEventListener('ended', function (e) {
    if (e.target.matches && e.target.matches('video.html5-main-video') && window.__TAURI__) {
      window.__TAURI__.core.invoke('queue_play_next').catch(function () {});
    }
  }, true);

  document.addEventListener('DOMContentLoaded', function () {
    reportTabState();
    const title = document.querySelector('title');
    if (title) {
      new MutationObserver(reportTabState).observe(title, {
        childList: true,
        subtree: true,
        characterData: true
      });
    }
  }, { once: true });
  window.addEventListener('load', reportTabState, { once: true });

  if (window.__TAURI__?.event?.listen) {
    window.__TAURI__.event.listen('window-fullscreen-changed', function (event) {
      windowFullscreen = Boolean(event.payload && event.payload.fullscreen);
    });
  }

  document.addEventListener('fullscreenchange', updateFullscreen);
  document.addEventListener('webkitfullscreenchange', updateFullscreen);
  document.addEventListener('mozfullscreenchange', updateFullscreen);
  document.addEventListener('MSFullscreenChange', updateFullscreen);
  // The standard fullscreen event is inconsistent in WebView2 for YouTube's
  // player. A lightweight reconciliation catches its ytp-fullscreen fallback.
  setInterval(updateFullscreen, 500);
  } catch (e) {
    console.error('[ytd] tab init script failed:', e);
  }
})();
"#;

#[derive(Clone, Serialize, Deserialize)]
struct TabInfo {
    label: String,
    title: String,
    url: String,
}

#[derive(Serialize, Deserialize)]
struct SavedSession {
    tabs: Vec<TabInfo>,
    active_index: Option<usize>,
}

#[derive(Clone, Serialize)]
struct QueueItem {
    title: String,
    url: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct HistoryItem {
    title: String,
    url: String,
    visited_at: u64,
}

#[derive(Clone, Serialize, Deserialize)]
struct Bookmark {
    title: String,
    url: String,
    timestamp: f64,
    created_at: u64,
}

#[derive(Clone, Serialize)]
struct FullscreenPayload {
    fullscreen: bool,
}

struct TabsState {
    tabs: Mutex<Vec<TabInfo>>,
    queue: Mutex<Vec<QueueItem>>,
    history: Mutex<Vec<HistoryItem>>,
    bookmarks: Mutex<Vec<Bookmark>>,
    active: Mutex<Option<String>>,
    counter: Mutex<u32>,
    player_fullscreen: Mutex<bool>,
    content_fullscreen: Mutex<bool>,
    window_fullscreen: Mutex<bool>,
    titlebar_hidden: Mutex<bool>,
    profiles: Mutex<Vec<String>>,
    active_profile: Mutex<String>,
}

impl Default for TabsState {
    fn default() -> Self {
        Self {
            tabs: Mutex::new(Vec::new()),
            queue: Mutex::new(Vec::new()),
            history: Mutex::new(Vec::new()),
            bookmarks: Mutex::new(Vec::new()),
            active: Mutex::new(None),
            counter: Mutex::new(0),
            player_fullscreen: Mutex::new(false),
            content_fullscreen: Mutex::new(false),
            window_fullscreen: Mutex::new(false),
            titlebar_hidden: Mutex::new(false),
            profiles: Mutex::new(vec!["Personal".into(), "Work".into(), "Study".into()]),
            active_profile: Mutex::new("Personal".into()),
        }
    }
}

#[derive(Clone, Serialize)]
struct TabsPayload {
    tabs: Vec<TabInfo>,
    active: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ProfileRegistry {
    profiles: Vec<String>,
    active: String,
}

#[derive(Clone, Serialize)]
struct ProfilesPayload {
    profiles: Vec<String>,
    active: String,
}

fn app_data_path(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    let directory = app.path().app_data_dir().ok()?;
    fs::create_dir_all(&directory).ok()?;
    Some(directory)
}

fn profile_directory(app: &tauri::AppHandle, state: &TabsState) -> Option<std::path::PathBuf> {
    let profile = state.active_profile.lock().unwrap().clone();
    let safe_name = profile.chars().filter(|character| character.is_ascii_alphanumeric() || *character == '-' || *character == '_').collect::<String>();
    let directory = app_data_path(app)?.join("profiles").join(if safe_name.is_empty() { "Personal" } else { &safe_name });
    fs::create_dir_all(&directory).ok()?;
    Some(directory)
}

fn session_path(app: &tauri::AppHandle, state: &TabsState) -> Option<std::path::PathBuf> {
    Some(profile_directory(app, state)?.join(SESSION_FILE))
}

fn unix_timestamp() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn save_profile_library(app: &tauri::AppHandle, state: &TabsState) {
    let Some(directory) = profile_directory(app, state) else { return };
    if let Ok(history) = serde_json::to_vec(&*state.history.lock().unwrap()) {
        let _ = fs::write(directory.join(HISTORY_FILE), history);
    }
    if let Ok(bookmarks) = serde_json::to_vec(&*state.bookmarks.lock().unwrap()) {
        let _ = fs::write(directory.join(BOOKMARKS_FILE), bookmarks);
    }
}

fn load_profile_library(app: &tauri::AppHandle, state: &TabsState) {
    let Some(directory) = profile_directory(app, state) else { return };
    let history = fs::read(directory.join(HISTORY_FILE)).ok().and_then(|data| serde_json::from_slice(&data).ok()).unwrap_or_default();
    let bookmarks = fs::read(directory.join(BOOKMARKS_FILE)).ok().and_then(|data| serde_json::from_slice(&data).ok()).unwrap_or_default();
    *state.history.lock().unwrap() = history;
    *state.bookmarks.lock().unwrap() = bookmarks;
}

fn save_profile_registry(app: &tauri::AppHandle, state: &TabsState) {
    let registry = ProfileRegistry {
        profiles: state.profiles.lock().unwrap().clone(),
        active: state.active_profile.lock().unwrap().clone(),
    };
    let Some(path) = app_data_path(app).map(|directory| directory.join(PROFILE_REGISTRY_FILE)) else { return };
    if let Ok(json) = serde_json::to_vec(&registry) {
        let _ = fs::write(path, json);
    }
}

fn load_profile_registry(app: &tauri::AppHandle, state: &TabsState) {
    let Some(path) = app_data_path(app).map(|directory| directory.join(PROFILE_REGISTRY_FILE)) else { return };
    let Ok(contents) = fs::read(path) else { return };
    let Ok(registry) = serde_json::from_slice::<ProfileRegistry>(&contents) else { return };
    if registry.profiles.is_empty() || !registry.profiles.iter().any(|profile| profile == &registry.active) {
        return;
    }
    *state.profiles.lock().unwrap() = registry.profiles;
    *state.active_profile.lock().unwrap() = registry.active;
}

fn save_session(app: &tauri::AppHandle, state: &TabsState) {
    let tabs = state.tabs.lock().unwrap().clone();
    let active = state.active.lock().unwrap().clone();
    let session = SavedSession {
        active_index: active.as_ref().and_then(|active| tabs.iter().position(|tab| &tab.label == active)),
        tabs,
    };
    let Some(path) = session_path(app, state) else { return };
    if let Ok(json) = serde_json::to_vec(&session) {
        let _ = fs::write(path, json);
    }
}

fn load_session(app: &tauri::AppHandle, state: &TabsState) -> Option<SavedSession> {
    let path = session_path(app, state)?;
    let contents = fs::read(path).ok()?;
    serde_json::from_slice(&contents).ok()
}

fn emit_tabs_updated(app: &tauri::AppHandle, state: &TabsState) {
    let payload = TabsPayload {
        tabs: state.tabs.lock().unwrap().clone(),
        active: state.active.lock().unwrap().clone(),
    };
    let _ = app.emit("tabs-updated", payload);
}

fn emit_player_fullscreen_state(app: &tauri::AppHandle, state: &TabsState) {
    let payload = FullscreenPayload {
        fullscreen: *state.player_fullscreen.lock().unwrap(),
    };
    let _ = app.emit("player-fullscreen-changed", payload);
}

fn emit_queue_updated(app: &tauri::AppHandle, state: &TabsState) {
    let _ = app.emit("queue-updated", state.queue.lock().unwrap().clone());
}

fn emit_history_updated(app: &tauri::AppHandle, state: &TabsState) {
    let _ = app.emit("history-updated", state.history.lock().unwrap().clone());
}

fn emit_bookmarks_updated(app: &tauri::AppHandle, state: &TabsState) {
    let _ = app.emit("bookmarks-updated", state.bookmarks.lock().unwrap().clone());
}

fn record_history(app: &tauri::AppHandle, state: &TabsState, url: String, title: String) {
    if !url.starts_with("https://www.youtube.com/") {
        return;
    }
    let mut history = state.history.lock().unwrap();
    history.retain(|item| item.url != url);
    history.insert(0, HistoryItem { title, url, visited_at: unix_timestamp() });
    history.truncate(500);
    drop(history);
    save_profile_library(app, state);
    emit_history_updated(app, state);
}

fn emit_window_fullscreen_state(app: &tauri::AppHandle, state: &TabsState) {
    let payload = FullscreenPayload {
        fullscreen: *state.window_fullscreen.lock().unwrap(),
    };
    let _ = app.emit("window-fullscreen-changed", payload);
}

fn emit_content_fullscreen_state(app: &tauri::AppHandle, state: &TabsState) {
    let payload = FullscreenPayload {
        fullscreen: *state.content_fullscreen.lock().unwrap(),
    };
    let _ = app.emit("content-fullscreen-changed", payload);
}

// Keep the native webview geometry and the HTML titlebar in one authoritative state.
// The frontend only renders this event; it never changes the layout optimistically.
fn set_titlebar_visibility(app: &tauri::AppHandle, state: &TabsState, hidden: bool) {
    let mut current = state.titlebar_hidden.lock().unwrap();
    if *current == hidden {
        // State can be unchanged while the parent webview has just reloaded or
        // transitioned into player fullscreen. Re-emit so it can resynchronise.
        drop(current);
        let _ = app.emit("titlebar-visibility-changed", FullscreenPayload { fullscreen: hidden });
        return;
    }
    *current = hidden;
    drop(current);
    resize_tabs(app, state);
    let _ = app.emit("titlebar-visibility-changed", FullscreenPayload { fullscreen: hidden });
}

// Fullscreen tabs are native child webviews, so DOM hover handlers can miss the
// pointer entirely during a resize/fullscreen transition. Track the cursor at the
// native window level instead. This is deliberately disabled for video-player
// fullscreen: that mode must have no application chrome at all.
fn track_fullscreen_titlebar(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(75));

        let state = app.state::<TabsState>();
        if !*state.window_fullscreen.lock().unwrap() || *state.player_fullscreen.lock().unwrap() {
            continue;
        }

        let Some(window) = app.get_window("main") else { continue };
        let (Ok(cursor), Ok(window_top), Ok(scale)) = (
            window.cursor_position(),
            window.inner_position(),
            window.scale_factor(),
        ) else {
            continue;
        };
        let relative_y = cursor.y - f64::from(window_top.y);
        let hidden = *state.titlebar_hidden.lock().unwrap();
        let reveal_zone = 4.0 * scale;
        let hide_below = (TITLEBAR_H + 8.0) * scale;

        if hidden && relative_y <= reveal_zone {
            set_titlebar_visibility(&app, &state, false);
        } else if !hidden && relative_y > hide_below {
            set_titlebar_visibility(&app, &state, true);
        }
    });
}

fn content_rect(app: &tauri::AppHandle, state: &TabsState) -> Option<(LogicalPosition<f64>, LogicalSize<f64>)> {
    let window = app.get_window("main")?;
    let size = window.inner_size().ok()?;
    let scale = window.scale_factor().unwrap_or(1.0);
    let w = size.width as f64 / scale;
    let h = size.height as f64 / scale;
    if *state.titlebar_hidden.lock().unwrap() {
        // Hidden chrome must not reserve space. The tab webview reports top-edge
        // pointer movement when the auto-hidden titlebar should reappear.
        Some((LogicalPosition::new(0.0, 0.0), LogicalSize::new(w, h)))
    } else {
        Some((LogicalPosition::new(0.0, TITLEBAR_H), LogicalSize::new(w, (h - TITLEBAR_H).max(0.0))))
    }
}

// Resize every known tab webview to fill the area below the titlebar. Cheap enough
// for a handful of tabs and keeps hidden tabs correctly sized when they're shown again.
fn resize_tabs(app: &tauri::AppHandle, state: &TabsState) {
    let Some((position, size)) = content_rect(app, state) else { return };
    for t in state.tabs.lock().unwrap().iter() {
        if let Some(wv) = app.get_webview(&t.label) {
            let _ = wv.set_position(position);
            let _ = wv.set_size(size);
        }
    }
}

fn switch_to_tab(app: &tauri::AppHandle, state: &TabsState, label: &str) {
    for t in state.tabs.lock().unwrap().iter() {
        if let Some(wv) = app.get_webview(&t.label) {
            if t.label == label {
                let _ = wv.show();
            } else {
                let _ = wv.hide();
            }
        }
    }
    *state.active.lock().unwrap() = Some(label.to_string());
}

#[tauri::command]
fn tabs_list(state: tauri::State<TabsState>) -> TabsPayload {
    TabsPayload {
        tabs: state.tabs.lock().unwrap().clone(),
        active: state.active.lock().unwrap().clone(),
    }
}

// add_child() blocks on a channel waiting for the main thread to build the webview
// (WebView2 needs the UI thread). Called directly from setup() this is fine — setup
// runs on the main thread before the event loop is pumping, so the dispatch resolves
// immediately. Called from an IPC command it must NOT run synchronously on the main
// thread (Tauri's dispatch can land it there), or it deadlocks waiting on a message
// only it could ever send — that's why the `tab_new` command wrapper below is async.
fn create_tab(app: &tauri::AppHandle, state: &TabsState, url: &str) -> Result<String, String> {
    let window = app.get_window("main").ok_or("no main window")?;
    let (position, size) = content_rect(app, state).ok_or("no content rect")?;

    let label = {
        let mut counter = state.counter.lock().unwrap();
        *counter += 1;
        format!("tab-{}", *counter)
    };

    let parsed_url = url.parse().map_err(|_| "invalid url".to_string())?;
    let tab_label = serde_json::to_string(&label).unwrap_or_default();
    let initialization_script = format!("window.__YTD_TAB_LABEL__ = {};\n{}", tab_label, CONTEXT_MENU_SCRIPT);
    let builder = WebviewBuilder::new(&label, WebviewUrl::External(parsed_url))
        .initialization_script(&initialization_script)
        .data_directory(profile_directory(app, state).ok_or("no profile directory")?);
    window
        .add_child(builder, position, size)
        .map_err(|e| e.to_string())?;

    state.tabs.lock().unwrap().push(TabInfo {
        label: label.clone(),
        title: "YouTube".into(),
        url: url.to_string(),
    });
    record_history(app, state, url.to_string(), "YouTube".into());

    switch_to_tab(app, state, &label);
    emit_tabs_updated(app, state);
    save_session(app, state);
    Ok(label)
}

#[tauri::command]
async fn tab_new(app: tauri::AppHandle, state: tauri::State<'_, TabsState>) -> Result<String, String> {
    create_tab(&app, &state, DEFAULT_TAB_URL)
}

#[tauri::command]
async fn tab_new_url(app: tauri::AppHandle, state: tauri::State<'_, TabsState>, url: String) -> Result<String, String> {
    create_tab(&app, &state, &url)
}

// async for the same reason as tab_new: the empty-tabs fallback below can call
// create_tab(), which must not run on the main thread.
#[tauri::command]
async fn tab_close(app: tauri::AppHandle, state: tauri::State<'_, TabsState>, label: String) -> Result<(), ()> {
    let was_active = state.active.lock().unwrap().as_deref() == Some(label.as_str());

    if let Some(wv) = app.get_webview(&label) {
        let _ = wv.close();
    }
    state.tabs.lock().unwrap().retain(|t| t.label != label);

    if was_active {
        let next = state.tabs.lock().unwrap().last().map(|t| t.label.clone());
        match next {
            Some(next_label) => switch_to_tab(&app, &state, &next_label),
            None => *state.active.lock().unwrap() = None,
        }
    }

    // No tabs left — give the user something to look at instead of a blank strip.
    if state.tabs.lock().unwrap().is_empty() {
        let _ = create_tab(&app, &state, DEFAULT_TAB_URL);
        return Ok(());
    }

    emit_tabs_updated(&app, &state);
    save_session(&app, &state);
    Ok(())
}

fn profiles_payload(state: &TabsState) -> ProfilesPayload {
    ProfilesPayload {
        profiles: state.profiles.lock().unwrap().clone(),
        active: state.active_profile.lock().unwrap().clone(),
    }
}

fn emit_profiles_updated(app: &tauri::AppHandle, state: &TabsState) {
    let _ = app.emit("profiles-updated", profiles_payload(state));
}

fn activate_profile(app: &tauri::AppHandle, state: &TabsState, profile: &str) -> Result<(), String> {
    if !state.profiles.lock().unwrap().iter().any(|candidate| candidate == profile) {
        return Err("unknown profile".into());
    }
    if *state.active_profile.lock().unwrap() == profile {
        emit_profiles_updated(app, state);
        return Ok(());
    }

    let labels = state.tabs.lock().unwrap().iter().map(|tab| tab.label.clone()).collect::<Vec<_>>();
    for label in labels {
        if let Some(webview) = app.get_webview(&label) {
            let _ = webview.close();
        }
    }
    state.tabs.lock().unwrap().clear();
    *state.active.lock().unwrap() = None;
    state.queue.lock().unwrap().clear();
    *state.active_profile.lock().unwrap() = profile.to_string();

    load_profile_library(app, state);
    restore_session(app, state);
    if state.tabs.lock().unwrap().is_empty() {
        let _ = create_tab(app, state, DEFAULT_TAB_URL);
    }
    save_profile_registry(app, state);
    emit_tabs_updated(app, state);
    emit_queue_updated(app, state);
    emit_profiles_updated(app, state);
    Ok(())
}

#[tauri::command]
fn profile_list(state: tauri::State<TabsState>) -> ProfilesPayload {
    profiles_payload(&state)
}

#[tauri::command]
fn profile_select(app: tauri::AppHandle, state: tauri::State<TabsState>, profile: String) -> Result<(), String> {
    activate_profile(&app, &state, &profile)
}

#[tauri::command]
fn profile_create(app: tauri::AppHandle, state: tauri::State<TabsState>, profile: String) -> Result<(), String> {
    let profile = profile.trim();
    if profile.is_empty() || profile.len() > 32 || !profile.chars().all(|character| character.is_ascii_alphanumeric() || character == ' ' || character == '-' || character == '_') {
        return Err("Use 1–32 letters, numbers, spaces, hyphens, or underscores.".into());
    }
    if !state.profiles.lock().unwrap().iter().any(|candidate| candidate.eq_ignore_ascii_case(profile)) {
        state.profiles.lock().unwrap().push(profile.to_string());
    }
    activate_profile(&app, &state, profile)
}

#[tauri::command]
fn tab_switch(app: tauri::AppHandle, state: tauri::State<TabsState>, label: String) {
    switch_to_tab(&app, &state, &label);
    emit_tabs_updated(&app, &state);
    save_session(&app, &state);
}

#[tauri::command]
fn tab_navigated(
    app: tauri::AppHandle,
    state: tauri::State<TabsState>,
    label: String,
    url: String,
    title: String,
) {
    let history_title = title.clone();
    let mut tabs = state.tabs.lock().unwrap();
    let Some(tab) = tabs.iter_mut().find(|tab| tab.label == label) else { return };
    let url_changed = tab.url != url;
    tab.url = url.clone();
    if !title.trim().is_empty() {
        tab.title = title;
    }
    drop(tabs);
    emit_tabs_updated(&app, &state);
    if url_changed {
        record_history(&app, &state, url, history_title);
    }
    save_session(&app, &state);
}

#[tauri::command]
fn queue_list(state: tauri::State<TabsState>) -> Vec<QueueItem> {
    state.queue.lock().unwrap().clone()
}

#[tauri::command]
fn queue_add(app: tauri::AppHandle, state: tauri::State<TabsState>, url: String, title: String) {
    state.queue.lock().unwrap().push(QueueItem { title, url });
    emit_queue_updated(&app, &state);
}

#[tauri::command]
fn queue_remove(app: tauri::AppHandle, state: tauri::State<TabsState>, index: usize) {
    let mut queue = state.queue.lock().unwrap();
    if index < queue.len() {
        queue.remove(index);
    }
    drop(queue);
    emit_queue_updated(&app, &state);
}

#[tauri::command]
fn queue_move(app: tauri::AppHandle, state: tauri::State<TabsState>, index: usize, offset: i32) {
    let mut queue = state.queue.lock().unwrap();
    let destination = index as i32 + offset;
    if index < queue.len() && destination >= 0 && (destination as usize) < queue.len() {
        queue.swap(index, destination as usize);
    }
    drop(queue);
    emit_queue_updated(&app, &state);
}

#[tauri::command]
fn queue_clear(app: tauri::AppHandle, state: tauri::State<TabsState>) {
    state.queue.lock().unwrap().clear();
    emit_queue_updated(&app, &state);
}

fn play_queue_item(app: &tauri::AppHandle, state: &TabsState, index: usize) {
    let item = {
        let mut queue = state.queue.lock().unwrap();
        if index >= queue.len() {
            return;
        }
        queue.remove(index)
    };
    if let Some(webview) = active_webview(app, state) {
        let url = serde_json::to_string(&item.url).unwrap_or_default();
        let _ = webview.eval(&format!("location.href = {};", url));
    }
    emit_queue_updated(app, state);
}

#[tauri::command]
fn queue_play(app: tauri::AppHandle, state: tauri::State<TabsState>, index: usize) {
    play_queue_item(&app, &state, index);
}

#[tauri::command]
fn queue_play_next(app: tauri::AppHandle, state: tauri::State<TabsState>) {
    play_queue_item(&app, &state, 0);
}

#[tauri::command]
fn queue_toggle(app: tauri::AppHandle, state: tauri::State<TabsState>) {
    if let Some(webview) = active_webview(&app, &state) {
        let _ = webview.eval("window.__YTD_QUEUE__?.toggle();");
    }
}

#[tauri::command]
fn clean_view_toggle(app: tauri::AppHandle, state: tauri::State<TabsState>) {
    if let Some(webview) = active_webview(&app, &state) {
        let _ = webview.eval("window.__YTD_CLEAN_VIEW__?.toggle();");
    }
}

#[tauri::command]
fn history_list(state: tauri::State<TabsState>) -> Vec<HistoryItem> {
    state.history.lock().unwrap().clone()
}

#[tauri::command]
fn bookmark_list(state: tauri::State<TabsState>) -> Vec<Bookmark> {
    state.bookmarks.lock().unwrap().clone()
}

#[tauri::command]
fn bookmark_add(app: tauri::AppHandle, state: tauri::State<TabsState>, url: String, title: String, timestamp: f64) {
    state.bookmarks.lock().unwrap().insert(0, Bookmark { title, url, timestamp, created_at: unix_timestamp() });
    save_profile_library(&app, &state);
    emit_bookmarks_updated(&app, &state);
}

#[tauri::command]
fn bookmark_remove(app: tauri::AppHandle, state: tauri::State<TabsState>, index: usize) {
    let mut bookmarks = state.bookmarks.lock().unwrap();
    if index < bookmarks.len() {
        bookmarks.remove(index);
    }
    drop(bookmarks);
    save_profile_library(&app, &state);
    emit_bookmarks_updated(&app, &state);
}

#[tauri::command]
fn library_panel_toggle(app: tauri::AppHandle, state: tauri::State<TabsState>) {
    if let Some(webview) = active_webview(&app, &state) {
        let _ = webview.eval("window.__YTD_LIBRARY__?.toggle();");
    }
}

#[tauri::command]
fn webview_fullscreen_changed(
    app: tauri::AppHandle,
    state: tauri::State<TabsState>,
    fullscreen: bool,
    player_fullscreen: bool,
) {
    *state.player_fullscreen.lock().unwrap() = fullscreen && player_fullscreen;
    *state.content_fullscreen.lock().unwrap() = fullscreen && !player_fullscreen;
    let hide_titlebar = *state.window_fullscreen.lock().unwrap()
        || *state.content_fullscreen.lock().unwrap()
        || *state.player_fullscreen.lock().unwrap();
    set_titlebar_visibility(&app, &state, hide_titlebar);
    emit_player_fullscreen_state(&app, &state);
    emit_content_fullscreen_state(&app, &state);
}

#[tauri::command]
fn set_titlebar_hidden(app: tauri::AppHandle, state: tauri::State<TabsState>, hidden: bool) {
    set_titlebar_visibility(&app, &state, hidden);
}

#[tauri::command]
fn show_titlebar(app: tauri::AppHandle, state: tauri::State<TabsState>) {
    if !*state.window_fullscreen.lock().unwrap() || *state.player_fullscreen.lock().unwrap() {
        return;
    }
    set_titlebar_visibility(&app, &state, false);
}

fn active_webview(app: &tauri::AppHandle, state: &TabsState) -> Option<tauri::Webview> {
    let label = state.active.lock().unwrap().clone()?;
    app.get_webview(&label)
}

#[tauri::command]
fn tab_back(app: tauri::AppHandle, state: tauri::State<TabsState>) {
    if let Some(wv) = active_webview(&app, &state) {
        let _ = wv.eval("history.back()");
    }
}

#[tauri::command]
fn tab_forward(app: tauri::AppHandle, state: tauri::State<TabsState>) {
    if let Some(wv) = active_webview(&app, &state) {
        let _ = wv.eval("history.forward()");
    }
}

#[tauri::command]
fn tab_reload(app: tauri::AppHandle, state: tauri::State<TabsState>) {
    if let Some(wv) = active_webview(&app, &state) {
        let _ = wv.eval("location.reload()");
    }
}

#[tauri::command]
fn tab_navigate(app: tauri::AppHandle, state: tauri::State<TabsState>, url: String) {
    if let Some(webview) = active_webview(&app, &state) {
        if let Ok(url) = serde_json::to_string(&url) {
            let _ = webview.eval(&format!("location.href = {};", url));
        }
    }
}

#[tauri::command]
fn tab_toggle_theater(app: tauri::AppHandle, state: tauri::State<TabsState>) {
    media_control(&app, &state, "theater");
}

#[tauri::command]
fn win_minimize(app: tauri::AppHandle) {
    if let Some(w) = app.get_window("main") {
        let _ = w.minimize();
    }
}

#[tauri::command]
fn win_toggle_maximize(app: tauri::AppHandle) {
    if let Some(w) = app.get_window("main") {
        let is_max = w.is_maximized().unwrap_or(false);
        if is_max {
            let _ = w.unmaximize();
        } else {
            let _ = w.maximize();
        }
    }
}

fn media_control(app: &tauri::AppHandle, state: &TabsState, action: &str) {
    let script = match action {
        "toggle" => "(function(){const v=document.querySelector('video.html5-main-video,video');if(v){v.paused?v.play():v.pause();}})();",
        "next" => "(function(){const b=document.querySelector('.ytp-next-button');if(b)b.click();})();",
        "previous" => "(function(){const b=document.querySelector('.ytp-prev-button');if(b)b.click();else{const v=document.querySelector('video.html5-main-video,video');if(v)v.currentTime=0;}})();",
        "seek_forward" => "(function(){const v=document.querySelector('video.html5-main-video,video');if(v)v.currentTime=Math.min(v.duration||Infinity,v.currentTime+10);})();",
        "seek_backward" => "(function(){const v=document.querySelector('video.html5-main-video,video');if(v)v.currentTime=Math.max(0,v.currentTime-10);})();",
        "volume_up" => "(function(){const v=document.querySelector('video.html5-main-video,video');if(v)v.volume=Math.min(1,v.volume+.05);})();",
        "volume_down" => "(function(){const v=document.querySelector('video.html5-main-video,video');if(v)v.volume=Math.max(0,v.volume-.05);})();",
        "mute" => "(function(){const v=document.querySelector('video.html5-main-video,video');if(v)v.muted=!v.muted;})();",
        "theater" => "(function(){const b=document.querySelector('.ytp-size-button');if(b)b.click();})();",
        "speed_up" => "window.__YTD_PLAYBACK__?.speedUp();",
        "speed_down" => "window.__YTD_PLAYBACK__?.speedDown();",
        "speed_reset" => "window.__YTD_PLAYBACK__?.resetSpeed();",
        _ => return,
    };
    if let Some(webview) = active_webview(app, state) {
        let _ = webview.eval(script);
    }
}

#[tauri::command]
fn win_toggle_fullscreen(app: tauri::AppHandle, state: tauri::State<TabsState>) {
    let Some(window) = app.get_window("main") else { return };
    let fullscreen = !window.is_fullscreen().unwrap_or(false);
    let _ = window.set_fullscreen(fullscreen);
    *state.window_fullscreen.lock().unwrap() = fullscreen;
    let hide_titlebar = fullscreen
        || *state.content_fullscreen.lock().unwrap()
        || *state.player_fullscreen.lock().unwrap();
    set_titlebar_visibility(&app, &state, hide_titlebar);
    emit_window_fullscreen_state(&app, &state);
}

#[tauri::command]
fn win_close(app: tauri::AppHandle) {
    if let Some(w) = app.get_window("main") {
        let _ = w.close();
    }
}

// Windows bullshit: register youtube.com:// so clicking YouTube links opens this app instead of
// whatever the fuck the default browser is
#[cfg(target_os = "windows")]
fn register_youtube_handler() {
    use winreg::RegKey;
    use winreg::enums::*;

    if let Ok(exe_path) = std::env::current_exe() {
        let exe_str = exe_path.to_string_lossy().to_string();
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        
        // Register youtube.com protocol — shove it into the registry like a barbarian
        if let Ok((youtube_key, _)) = hkcu.create_subkey(r"Software\Classes\youtube.com") {
            let _ = youtube_key.set_value("", &"URL:YouTube");
            let _ = youtube_key.set_value("URL Protocol", &"");
            
            if let Ok((shell_key, _)) = youtube_key.create_subkey(r"shell\open\command") {
                let command = format!(r#""{}\" \"%1\""#, exe_str);
                let _ = shell_key.set_value("", &command);
            }
        }
    }
}

fn main() {
    // Fire up the URL handler before anything else has a chance to fuck it up
    #[cfg(target_os = "windows")]
    register_youtube_handler();

    // build the app — order matters here: plugins first, then setup(), then run()
    // if you rearrange this and shit breaks, that's on you
    tauri::Builder::default()
        // register the global shortcut plugin — has to go here before setup() or shortcuts won't exist yet
        .plugin(
            ShortcutBuilder::new()
                .build()
        )
        .manage(TabsState::default())
        .invoke_handler(tauri::generate_handler![
            tabs_list,
            profile_list,
            profile_select,
            profile_create,
            tab_new,
            tab_new_url,
            tab_close,
            tab_switch,
            tab_navigated,
            queue_list,
            queue_add,
            queue_remove,
            queue_move,
            queue_clear,
            queue_play,
            queue_play_next,
            queue_toggle,
            clean_view_toggle,
            history_list,
            bookmark_list,
            bookmark_add,
            bookmark_remove,
            library_panel_toggle,
            webview_fullscreen_changed,
            set_titlebar_hidden,
            show_titlebar,
            tab_back,
            tab_forward,
            tab_reload,
            tab_navigate,
            tab_toggle_theater,
            win_minimize,
            win_toggle_maximize,
            win_toggle_fullscreen,
            win_close,
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();

            // Tray menu — the only way to control your addiction when you hide the window from yourself
            let show       = MenuItem::with_id(app, "show",        "Show",            true, None::<&str>)?;
            let pin        = MenuItem::with_id(app, "pin",         "Always on Top",   true, None::<&str>)?;
            let play_pause = MenuItem::with_id(app, "play_pause",  "Play / Pause",    true, None::<&str>)?;
            let next       = MenuItem::with_id(app, "next",        "Next Video",      true, None::<&str>)?;
            let quit       = MenuItem::with_id(app, "quit",        "Quit",            true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &pin, &play_pause, &next, &quit])?;

            // Load icon from bundled icons/
            let icon = Image::from_path("icons/icon.ico").unwrap_or_else(|_| app.default_window_icon().unwrap().clone());

            let _tray = TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .tooltip("YouTube Desktop")
                .on_tray_icon_event(|tray, event| {
                    // Only left-click activates the window. Right-click also fires
                    // Click here (in addition to natively opening the attached
                    // menu) — stealing focus for it made the OS treat the popup as
                    // deactivated and dismiss it immediately, so it only ever
                    // flashed on screen instead of staying open.
                    if let TrayIconEvent::Click { button: MouseButton::Left, .. } = event {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .on_menu_event(move |app, event| {
                    match event.id().as_ref() {
                        "show" => {
                            if let Some(w) = app.get_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                        "pin" => {
                            if let Some(w) = app.get_window("main") {
                                let current = w.is_always_on_top().unwrap_or(false);
                                let _ = w.set_always_on_top(!current);
                            }
                        }
                        "play_pause" => {
                            let state = app.state::<TabsState>();
                            media_control(app, &state, "toggle");
                        }
                        "next" => {
                            let state = app.state::<TabsState>();
                            media_control(app, &state, "next");
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    }
                })
                .build(app)?;

            // Register global shortcut for play/pause — works even when window is hidden/background
            // ctrl+shift+space because ctrl+space was taken by literally everything else on the planet
            app.global_shortcut()
                .on_shortcut("ctrl+shift+space", move |app, _shortcut, _event| {
                    // find the play button and click it like a ghost haunting youtube
                    let state = app.state::<TabsState>();
                    media_control(app, &state, "toggle");
                })
                // if this fails we just silently have no shortcut — not ideal but also not worth crashing over
                .expect("Failed to register global shortcut");

            // Hardware media keys are registered opportunistically: Windows may reserve
            // some of them for another player, so a failed individual registration must
            // not prevent the remaining controls from working.
            for (shortcut, action) in [
                ("MediaPlayPause", "toggle"),
                ("MediaTrackNext", "next"),
                ("MediaTrackPrevious", "previous"),
                ("ctrl+alt+right", "seek_forward"),
                ("ctrl+alt+left", "seek_backward"),
                ("ctrl+alt+up", "volume_up"),
                ("ctrl+alt+down", "volume_down"),
                ("ctrl+alt+m", "mute"),
                ("ctrl+alt+t", "theater"),
                ("ctrl+alt+2", "speed_up"),
                ("ctrl+alt+1", "speed_down"),
                ("ctrl+alt+0", "speed_reset"),
            ] {
                let action = action.to_string();
                let _ = app.global_shortcut().on_shortcut(shortcut, move |app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        let state = app.state::<TabsState>();
                        media_control(app, &state, &action);
                    }
                });
            }


            // Close-to-tray behavior: the X button hides instead of quitting. Minimize
            // stays a real, normal minimize (taskbar), not a tray-hide.
            let win_clone = window.clone();
            let app_handle = app.handle().clone();
            window.on_window_event(move |event| {
                match event {
                    // user clicked X — nope, we're just hiding. you're not getting rid of us that easily
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = win_clone.hide();
                    }
                    // window got resized (or un-minimized) — keep the tab webviews filling
                    // the area below the custom titlebar
                    tauri::WindowEvent::Resized(_) => {
                        let state = app_handle.state::<TabsState>();
                        let fullscreen = win_clone.is_fullscreen().unwrap_or(false);
                        let mut known_fullscreen = state.window_fullscreen.lock().unwrap();
                        if *known_fullscreen != fullscreen {
                            *known_fullscreen = fullscreen;
                            drop(known_fullscreen);
                            // WebView2 sometimes promotes an in-page player-fullscreen
                            // request into a real OS window resize, so this event can
                            // race the JS-side fullscreenchange report. Recompute from
                            // all three flags — mirroring win_toggle_fullscreen and
                            // webview_fullscreen_changed — instead of forwarding the
                            // raw window-only flag, which used to let a not-yet-reported
                            // player_fullscreen=true get clobbered by a stale `false`
                            // here, flashing the titlebar back on mid-transition.
                            let hide_titlebar = fullscreen
                                || *state.content_fullscreen.lock().unwrap()
                                || *state.player_fullscreen.lock().unwrap();
                            set_titlebar_visibility(&app_handle, &state, hide_titlebar);
                            emit_window_fullscreen_state(&app_handle, &state);
                        }
                        resize_tabs(&app_handle, &state);
                    }
                    // anything else? not our problem
                    _ => {}
                }
            });

            // Keep the app fullscreen titlebar responsive even when a child webview
            // owns focus (or has just moved to fill the window).
            track_fullscreen_titlebar(app.handle().clone());

            // Restore the last browsing session, falling back to a single home tab.
            let state = app.state::<TabsState>();
            load_profile_registry(&app.handle().clone(), &state);
            load_profile_library(&app.handle().clone(), &state);
            restore_session(&app.handle().clone(), &state);
            if state.tabs.lock().unwrap().is_empty() {
                let _ = create_tab(&app.handle().clone(), &state, DEFAULT_TAB_URL);
            }

            Ok(())
        })
        // generate_context! reads tauri.conf.json at compile time — don't ask me how, it's macro magic
        .run(tauri::generate_context!())
        // if this panics, something has gone catastrophically wrong and i'm sorry
        .expect("error while running tauri application");
}

fn restore_session(app: &tauri::AppHandle, state: &TabsState) {
    let Some(session) = load_session(app, state) else { return };
    if session.tabs.is_empty() {
        return;
    }

    for tab in &session.tabs {
        let url = if tab.url.starts_with("http://") || tab.url.starts_with("https://") {
            tab.url.as_str()
        } else {
            DEFAULT_TAB_URL
        };
        let _ = create_tab(app, state, url);
    }

    let labels = {
        let mut tabs = state.tabs.lock().unwrap();
        for (restored, saved) in tabs.iter_mut().zip(session.tabs.iter()) {
            restored.title = saved.title.clone();
        }
        tabs.iter().map(|tab| tab.label.clone()).collect::<Vec<_>>()
    };
    if let Some(active_index) = session.active_index.filter(|index| *index < labels.len()) {
        switch_to_tab(app, state, &labels[active_index]);
    }
    emit_tabs_updated(app, state);
    save_session(app, state);
}
