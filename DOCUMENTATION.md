# YouTube Desktop

A lightweight desktop app wrapper for YouTube built with Tauri. Direct, clean, no bloat.

## Quick Start

### Prerequisites
- **Node.js** (v18+)
- **Rust** (install via [rustup.rs](https://rustup.rs))
- **Tauri CLI** (install globally: `npm install -g @tauri-apps/cli@latest`)
- **~5GB free disk space** (Rust debug build artifacts are large)

### Dev Mode
```bash
npm install
npm run tauri:dev
```

### Production Build
```bash
npm run tauri:build
```

Output: `.msi` installer + `.exe` in `src-tauri/target/release/bundle/`

---

## Architecture

- **Tauri 2.x** — Rust-based desktop runtime. Provides window, webview, OS APIs.
- **Direct navigation** — The window URL points to `https://www.youtube.com` directly. No iframe, no React layer, no build pipeline.
- **Minimal** — No frontend framework. Compiles in ~2min on first run, ~5sec after.

### Why not iframe + React?
Rejected early. YouTube sends `X-Frame-Options: DENY` which blocks cross-origin iframe access entirely. Direct webview navigation sidesteps this — we're modifying the document we're already inside.

---

## Project Structure

```
YouTube/
├── dist/
│   └── index.html                 # Unused placeholder (Tauri requires it)
├── src-tauri/
│   ├── tauri.conf.json            # Tauri config (window, URL, icons)
│   ├── Cargo.toml                 # Rust dependencies
│   ├── build.rs                   # Tauri build script
│   ├── icons/                     # App icons (generated via: npx tauri icon <image.png>)
│   └── src/
│       └── main.rs                # Rust entry point
├── package.json                   # npm scripts
└── DOCUMENTATION.md               # (you are here)
```

---

## Known Gotchas

- **Windows URL handler requires elevated permissions** — Registry writes for `youtube.com` handler may fail on restricted accounts. The app will continue to function normally if registration fails; clicking YouTube links just won’t open in the app.
- **`shell-open` is not a valid Tauri 2 feature flag** — remove it from `Cargo.toml` if it reappears.
- **`Cargo.lock` caching** — if Cargo keeps reading stale deps, delete `src-tauri/Cargo.lock` and retry.
- **Application Control / Smart App Control** — Windows may block unsigned Rust build scripts. Run from `C:\Dev\` or a non-Desktop path if this hits. Desktop path (`C:\Users\...\Desktop\`) can trigger policy blocks on some machines.
- **Disk space** — First build needs ~4GB free. Delete `src-tauri/target/` to reclaim space between sessions if needed.
- **Avast / AV interference** — Not the culprit for `os error 4551`. That's Windows Smart App Control or WDAC.

---

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| **Ctrl+Shift+Space** | Play / Pause YouTube (works even when window is minimized/in background) |

## URL Handler Registration (Windows)

The app automatically registers itself as the default handler for `youtube.com` URLs on Windows startup. This means:
- YouTube links clicked anywhere on your system will open in this app
- The app executable and registry key are set up automatically on first run
- No manual configuration needed

---

## Current Status
- [x] Tauri scaffold complete
- [x] Window navigates directly to youtube.com
- [x] Icons generated and placed in `src-tauri/icons/`
- [x] App boots successfully
- [x] Production build succeeds — `.msi` + `.exe` generated ✓
- [x] System tray integration (minimize, quick controls, global hotkeys) ✓
- [x] YouTube URL handler registration on Windows ✓
- [ ] Further feature development (see roadmap below)

---

## Roadmap

### Window / UX Behaviour
- [ ] Always-on-top mode (picture-in-picture style, stays above other windows)
- [ ] Custom window chrome / frameless window
- [ ] Remember window size and position between sessions
- [ ] Keyboard shortcuts for common actions (fullscreen, mute, next video)

### Features the Browser Can't Easily Do
- [x] System tray integration (minimize to tray, quick controls: play/pause, next video)
- [x] Global hotkeys (pause/play YouTube even when the window is in the background — **Ctrl+Shift+Space**)
- [ ] Auto-start with Windows

### Content
- [ ] Block/hide elements via injected CSS (ads, Shorts shelf, sidebar recommendations)

---

## Next

Pick an item from the roadmap and we'll build it.
