# Yeravich engineering rules

- Keep application flow simple: `iced message -> page/controller -> core capability -> UI state -> view`.
- iced views are declarative. Widgets emit user intent; views must not perform network, filesystem, clipboard, D-Bus, or other I/O themselves.
- Own asynchronous work in an explicit scope. Replacing an operation cancels its previous task, and dropping the scope cancels all remaining work.
- Keep iced and platform event loops in the top-level `yeravich-app` crate; `crates/yeravich-core` exposes directly callable product capabilities and must not depend on a UI toolkit.
- Keep the application shell in `app.rs`, page presentation in `pages/`, controllers in `controllers/`, and platform adapters in `platform/`. Add separate crates only for capabilities with an independent boundary.
- Keep platform-private types out of `yeravich-core`. Hyprland support is a transparent, replaceable fallback and must never be selected before standard interfaces fail.
- For this prototype, API keys may be stored in plaintext in the local `credentials.json` file. Keep them out of logs, `Debug` output, fixtures, and snapshots. `config.toml` contains opaque secret references only; access credentials through the `SecretStore` interface.
- Before submitting changes, run `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo check --workspace --all-targets --all-features`.
