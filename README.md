# YouTube Desktop

A lightweight, no-bloat desktop wrapper for YouTube. Watch in a native app window with system tray integration, global hotkeys, and zero tracking overhead.

## Features

- **Direct Navigation** — Points straight to youtube.com in a native window. No iframe, no intermediary.
- **System Tray** — Minimize to tray, quick controls (play/pause, next video, always-on-top) without focusing the window.
- **Global Hotkey** — **Ctrl+Shift+Space** to play/pause YouTube, even when the window is hidden or in the background.
- **URL Handler** — Click YouTube links anywhere on Windows, they open in this app.
- **Auto-Start** — Option to launch on Windows startup during installation.
- **Self-Signed & Code-Signed** — Installer is properly signed to avoid Windows security warnings.

## System Requirements

- **Windows 10** or later (x64)
- **WebView2** — Bundled by Tauri, installed automatically if missing

## Installation

Download the latest **YouTube-Desktop-Setup.exe** from [Releases](../../releases).

Run the installer:
1. Choose install location (default: `C:\Program Files\YouTube Desktop`)
2. Optionally create a desktop shortcut
3. Optionally enable auto-start on Windows startup
4. Click Install

That's it. The app launches automatically.

## Usage

### Starting the App
- Desktop shortcut, Start Menu → YouTube Desktop, or click any YouTube link on Windows.

### Controls

| Action | Method |
|--------|--------|
| Play / Pause | Ctrl+Shift+Space (global — works in background) |
| Show / Hide | Click tray icon or "Show" in tray menu |
| Always-on-Top | Right-click tray → "Always on Top" |
| Next Video | Right-click tray → "Next Video" |
| Quit | Right-click tray → "Quit" |

### Closing the App
- Click the X button → minimizes to tray (doesn't quit)
- Right-click tray → Quit to actually close

## Building from Source

### Prerequisites
- **Node.js** v18+
- **Rust** (install via [rustup.rs](https://rustup.rs))
- **Tauri CLI** (`npm install -g @tauri-apps/cli@latest`)
- **Inno Setup 6** (for installer: [jrsoftware.org](https://jrsoftware.org/isdl.php))
- **Windows SDK** (for signtool.exe)
- **~5GB free disk space** (Rust artifacts are large)

### Dev Build
```bash
npm install
npm run tauri:dev
```

App runs in debug mode with hot reload.

### Production Build & Installer

```bash
npm run tauri:build
npm run dist
```

This:
1. Builds the Rust binary
2. Signs the `.exe` with your code-signing certificate
3. Compiles the Inno Setup installer
4. Signs the `Setup.exe`

Output: `./installer/YouTube-Desktop-Setup.exe`

**First-time setup:**
You need a code-signing certificate. See [SIGNING.md](./SIGNING.md) for one-time cert generation.

## Project Structure

```
YouTube/
├── src-tauri/
│   ├── src/main.rs           # Rust entry point (tray, hotkeys, URL handler)
│   ├── tauri.conf.json       # Tauri config
│   ├── Cargo.toml            # Rust dependencies
│   ├── icons/                # App icons
│   └── target/               # Build artifacts
├── setup.iss                 # Inno Setup installer script
├── build-installer.ps1       # Build & sign helper (PowerShell)
├── package.json              # npm scripts
├── DOCUMENTATION.md          # Technical details
└── README.md                 # (you are here)
```

## Development Notes

### Why Direct Navigation?
YouTube blocks iframe access with `X-Frame-Options: DENY`. We bypass this by navigating the webview directly to youtube.com — the webview **is** YouTube, not hosting it.

### Tauri
Rust-based desktop runtime. Minimal footprint, small binary, fast startup.

### No Tracking
Requests go straight to YouTube. No proxies, no intermediaries, no tracking added by this app.

## Troubleshooting

**Smart App Control blocks the installer?**
- The app is self-signed, not from a certificate authority. This is expected.
- Run PowerShell as Administrator, set execution policy: `Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser`
- Or move the project folder to `C:\Dev\` instead of Desktop.

**"Setup.exe has no publisher" warning?**
- Normal for self-signed apps. Code signing only removes this if the cert is from a trusted CA.
- You can trust the cert manually: PowerShell → `Import-Certificate -FilePath .\signing\youtube-desktop.pfx -CertStoreLocation Cert:\CurrentUser\Root\`

**Global hotkey not working?**
- Some apps/games intercept Ctrl+Shift+Space. Try a different modifier in `src/main.rs` (line ~106).

## License

[MIT](./LICENSE)

## Credits

Built with:
- [Tauri](https://tauri.app) — Desktop runtime
- [Inno Setup](https://jrsoftware.org) — Installer
- [Windows SDK](https://developer.microsoft.com/windows) — Code signing

---

**Questions?** Open an issue. **Contributions?** PRs welcome.
