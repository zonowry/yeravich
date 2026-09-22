# Yeravich

Yeravich is an early-stage cross-platform desktop translation client built with Rust and iced. It supports translation through an OpenAI-compatible Chat Completions service, with a settings page for the API address, API key, model, and connection testing. Selection capture, shortcuts, tray integration, and startup integration are still to be connected.

## Project structure

The workspace follows the application/capability separation used by [COSMIC Settings](https://github.com/pop-os/cosmic-settings). Yeravich uses upstream iced directly. Pages remain modules inside the application until they need an independent crate boundary.

```text
Cargo.toml                    Workspace dependencies, metadata, and lints
Cargo.lock                    Locked application dependencies
mise.toml                     Local Rust toolchain selection
.github/workflows/ci.yml       Workspace checks
yeravich-app/
    Cargo.toml                Desktop application (default workspace member)
    src/
        main.rs               iced startup and window settings
        app.rs                Application composition and message routing
        controllers/          Translation/settings actions and presentation state
        pages/                Translation and settings views
        platform/config.rs    Persistent public configuration
        platform/secrets.rs   Plaintext credential storage for the prototype
        platform/storage.rs   Platform config directory and atomic file writes
        task_scope.rs         Owned cancellation and stale-result protection
crates/
    yeravich-core/
        src/                  Translation, configuration, provider capabilities
        tests/                UI-independent capability and configuration tests
```

The default fixed language pair is `en -> zh-CN`. Public configuration contains opaque credential references. For this prototype, API keys are stored in a separate local plaintext file, as described below. Keys remain hidden in UI message diagnostics and error reports.

## Application flow

```text
iced message -> page/controller -> yeravich-core capability -> UI state -> view
```

`app.rs` routes page messages. The translation page owns iced editor state and builds widgets without I/O. Its controller calls core capabilities and owns a task scope; iced `Task` drives the returned futures and delivers completion messages. Replacing an operation cancels the previous future. Completion IDs prevent already-queued stale results from changing the current state. Dropping the controller cancels all remaining work.

`yeravich-core` has no dependency on iced or platform event loops. Platform adapters belong in the application and implement core traits. The core owns the Chat Completions HTTP adapter and settings capabilities; iced runs the futures on its Tokio executor. Future long-lived event sources can use iced subscriptions when those integrations are added.

The translation page supports multiline input, language swapping, busy/error states, and scrollable translation output. Switching pages preserves the translation draft. Connection tests are cancelled when the settings fields change, another test starts, or the user leaves settings; stale completions are ignored. Saving temporarily disables editing and navigation.

## Provider settings

Open **Settings** and enter:

- **API address**: an HTTP/HTTPS base URL such as `https://api.example.com/v1`, a custom prefix, or a full `/chat/completions` endpoint. A bare host defaults to `/v1/chat/completions`. URLs containing credentials, query parameters, or fragments are rejected.
- **API key**: optional for unauthenticated local services. After saving, the field stays empty; leaving it blank retains the saved key. Enter a new key to replace it, or select the removal checkbox to remove it when saving.
- **Model**: the exact model ID accepted by the provider.

**Test connection** sends a short, non-streaming completion with the draft settings and reports success/latency or a categorized error. It may consume provider quota. It does not save changes. The request follows the [OpenAI Chat Completions format](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create); service-specific extensions are not required. HTTP redirects are not followed, and raw error response bodies are not displayed.

**Save** persists the configuration and activates the provider for translation. A successful connection test is not required to save. Connection checks time out after 20 seconds and translations after 60 seconds.

Files are stored in the platform configuration directory: `$XDG_CONFIG_HOME/yeravich` on Linux, or `~/.config/yeravich` when that variable is unset. macOS and Windows use the location supplied by `directories::ProjectDirs`.

- `config.toml`: API address, model, language pair, and opaque credential reference.
- `credentials.json`: plaintext API keys indexed by those references. No system keyring is required.

Writes replace complete files atomically. A save that fails does not replace the active configuration. Malformed configuration files produce a startup error instead of silently resetting settings. Changing the credential storage later only requires replacing the `SecretStore` implementation.

## Build and test

The workspace requires Rust 1.98 or newer. `mise.toml` selects the latest Rust release for local development; CI uses Rust 1.98.1 with rustfmt and Clippy. The GUI uses iced 0.14 with its default GPU/software renderers and Wayland/X11 support, plus advanced text shaping for multilingual content.

On Debian/Ubuntu, install the Linux build prerequisites:

```sh
sudo apt-get install build-essential pkg-config libwayland-dev libxkbcommon-dev
```

Run the application from the workspace root:

```sh
cargo run
# Equivalent: cargo run -p yeravich-app
```

Run all workspace checks:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo check --workspace --all-targets --all-features
```

Core-only tests can run without building the GUI:

```sh
cargo test -p yeravich-core
```

Application ID: `com.zonowry.yeravich`. Suggested future shortcut: `Super+Shift+Y`.
