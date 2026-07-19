# Installation

CourseLib ships as a desktop app for **macOS** and **Windows**. Linux builds are not published yet.

## Download

1. Open the [latest release](https://github.com/ririyad/courselib/releases/latest).
2. Download the asset for your platform:
    - **macOS:** arm64 (Apple Silicon) or x64 (Intel)
    - **Windows:** x64 NSIS installer (`.exe`)
3. Open / install the app using the notes below.

!!! warning "Builds are currently unsigned"
    Apple notarization and Windows code signing are not configured yet. Expect Gatekeeper or SmartScreen warnings on first open. That is expected for these builds — not malware.

=== "macOS"

    If Gatekeeper blocks the app:

    1. **Right-click → Open** the app (or DMG), then confirm **Open**, **or**
    2. Go to **System Settings → Privacy & Security** and allow the blocked app.

    You can also clear quarantine after download:

    ```bash
    xattr -dr com.apple.quarantine /path/to/CourseLib.app
    ```

=== "Windows"

    The installer is **unsigned**. SmartScreen may show **Windows protected your PC** / Unknown Publisher:

    1. Choose **More info**
    2. Choose **Run anyway**

    WebView2 is bootstrapped on first install if it is missing (recent Windows 10/11 usually already have it).

## First launch

On first use CourseLib creates a default vault under your Documents folder (`CourseLib Vault`). You can change the vault folder later from **Vault settings** on the library home.

## Build from source (developers)

### Requirements

- **Node.js** `^20.19.0` or `>=22.12.0` (npm)
- **Rust** toolchain (see `src-tauri/Cargo.toml` rust-version)
- Platform deps for [Tauri 2](https://v2.tauri.app/start/prerequisites/):
    - **macOS:** Xcode Command Line Tools
    - **Windows:** Visual Studio C++ Build Tools (MSVC) + Windows SDK, and the [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

### Quick start

```bash
# Install frontend dependencies
npm install

# Run the desktop app (starts Vite + Tauri)
npm run tauri -- dev
```

### Useful commands

| Command | Purpose |
|---------|---------|
| `npm run tauri -- dev` | Develop with hot reload |
| `npm run build` | Build static frontend → `build/` |
| `npm run tauri -- build` | Package release desktop app (local) |
| `npm run tauri -- build --debug` | Package debug desktop app |
| `npm run validate` | Rust tests + frontend build + Tauri debug build |
| `cd src-tauri && cargo test` | Backend unit tests only |
