# Yeravich

English | [简体中文](README.zh-CN.md)

A Chinese–English translation app that stays in the background. Select or copy text, press Alt+Q to translate it, and press Esc to hide the window when you are done. On macOS, use Option+Q.

Yeravich uses bilibili's free [Index Translate](https://github.com/bilibili/Index-Translate#free-public-api) service by default. No API key is required. Translation requires an internet connection, and your source text is sent to the service.

![Yeravich interface](https://github.com/user-attachments/assets/7ab96e45-3120-4ebe-a80d-636a22e5abe9)

Desktop behavior has currently been tested only on Hyprland and macOS (Apple Silicon).

## Usage

Start Yeravich, then press the shortcut while using another app. Selected or copied text is inserted into the left panel and translated automatically. The translation appears on the right. Click **Copy** to copy it.

You can also type or paste text manually. Press Enter to translate, or Shift+Enter to insert a line break. The ⇄ button at the top switches the translation direction; the language menu in the upper-right corner changes the interface language. Chinese system locales default to Chinese → English; other locales default to English → Chinese.

| Action | Hyprland | macOS |
| --- | --- | --- |
| Show the window and translate | Alt+Q (requires a binding) | Option+Q |
| Hide the window | Esc | Esc |
| Quit the app | Ctrl+Q | Cmd+Q |
| Translate the input | Enter | Enter |
| Insert a line break | Shift+Enter | Shift+Enter |

Except for the shortcut that shows the window, these keys apply while working in Yeravich.

Closing the window has the same effect as pressing Esc: the app stays running, and both the source text and translation are preserved. Click **Quit** or press Ctrl+Q / Cmd+Q to stop the app. You will need to start it again before using the global shortcut.

Pressing Alt+Q / Option+Q while editing in Yeravich preserves your current text. The tray menu's **Show Yeravich** action also restores the window without reading new text.

## Hyprland

Use the Linux x86_64 AppImage. Published packages are available under [Releases](https://github.com/zonowry/yeravich/releases). For manual builds, go to **Actions → AppImage → workflow run → Artifacts → Yeravich-linux-x86_64**.

Download the `.AppImage` and its matching `.sha256` file. Extract the archive first if you downloaded it from Artifacts. For version 0.0.1:

```sh
sha256sum --check Yeravich-0.0.1-x86_64.AppImage.sha256
chmod +x Yeravich-0.0.1-x86_64.AppImage
./Yeravich-0.0.1-x86_64.AppImage
```

If your system cannot mount the AppImage, add `--appimage-extract-and-run` to the launch command.

Global shortcuts require `xdg-desktop-portal` and `xdg-desktop-portal-hyprland`. The AppImage includes the tool needed to read the clipboard. If you build and run from source, install `wl-clipboard` as well.

From the repository directory, install the desktop entry and icon:

```sh
install -Dm644 data/com.zonowry.yeravich.desktop "${XDG_DATA_HOME:-$HOME/.local/share}/applications/com.zonowry.yeravich.desktop"
install -Dm644 data/com.zonowry.yeravich.svg "${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps/com.zonowry.yeravich.svg"
```

In the installed `.desktop` file, replace `Exec=yeravich` with the absolute path to your AppImage. Enclose the path in double quotes if it contains spaces.

Add an Alt+Q binding to `~/.config/hypr/hyprland.lua`:

```lua
hl.bind("ALT + Q", hl.dsp.global("com.zonowry.yeravich:show"))
```

Save the file and run `hyprctl reload config-only`. If you still use `hyprland.conf`, use this binding instead:

```ini
bind = ALT, Q, global, com.zonowry.yeravich:show
```

Reload the older configuration with `hyprctl reload`. Resolve any existing Alt+Q binding conflict first.

The `global` action shows an already running Yeravich process, so start the app before using it.

Both the text selection and clipboard can supply the input. Yeravich prefers the source whose contents have changed; if both have changed, the selection takes priority. If the imported text is not what you intended, paste the correct text into the input panel and press Enter.

## macOS

For Apple Silicon Macs, use the DMG with `macos-arm64` in its filename. Published packages are available under [Releases](https://github.com/zonowry/yeravich/releases). For manual builds, go to **Actions → DMG → workflow run → Artifacts → Yeravich-macos-arm64**.

Open the DMG, drag Yeravich.app into Applications, then launch it from Applications. Extract the archive first if you downloaded it from Artifacts.

The app is currently self-signed and has not been notarized by Apple. macOS may block the first launch. After verifying the download's source, open **System Settings → Privacy & Security** and click **Open Anyway**. See [Apple's instructions](https://support.apple.com/en-us/102445).

To translate selected text without copying it first, grant Accessibility access:

1. Click **Selection access** at the top of Yeravich.
2. Enable Yeravich in **System Settings → Privacy & Security → Accessibility**. If it is not listed, click `+` and add Yeravich.app from Applications.
3. Select text in another app and press Option+Q.

Yeravich reads the selection before bringing its window to the front. It first requests selected text, then tries the standard macOS selection-range interface, and retains text-marker support for Zen / Firefox web pages. If no selection can be read and clipboard text is used, a notice explains the fallback.

You can use the app without this permission: copy text with Cmd+C, then press Option+Q. Use the same approach for apps that do not expose their selected text.

If selection capture stops working after updating or moving the app, remove the old Accessibility entry, add the current Yeravich.app, and grant access again.

## Configuration

The first launch creates a configuration file. Existing files are preserved:

| Platform | Path |
| --- | --- |
| Hyprland | `~/.config/yeravich/config.toml` |
| macOS | `~/Library/Application Support/yeravich/config.toml` |

If `XDG_CONFIG_HOME` is set, the Hyprland configuration is stored under `yeravich/config.toml` in that directory.

The default configuration is shown below and in [config.example.toml](config.example.toml):

```toml
[shortcuts]
show_window = "Alt+Q"

[translation]
endpoint = "https://index-translate.bilibili.com/v1/chat/completions"
model = "Index-Translate-35B-A3B"
# Set this when using a service that requires an API key:
# api_key = "your-api-key"
```

To change the shortcut, edit `show_window`, for example to `"Ctrl+Shift+T"`. On macOS, `Alt` means Option. On Hyprland, also update the desktop binding shown above.

To use another translation service, set its full Chat Completions-compatible endpoint URL, model, and API key if required.

Quit and restart the app after changing the configuration. Hiding or closing the window does not reload it.

## Running from source

Install Rust and the system dependencies for your platform, then run from the repository directory:

```sh
cargo run --locked
```

Developer documentation is available in Chinese: [Rust reading and practice](docs/rust-learning.md), [building and verification](docs/rust-learning.md#构建与验证), and [the release process](docs/rust-learning.md#自动发布-release).
