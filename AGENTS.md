# Yeravich engineering rules

- Keep application flow simple: `Slint callback -> controller -> core capability -> UI state snapshot`.
- Slint markup is declarative. Callbacks forward user intent to Rust and must not perform network, filesystem, clipboard, D-Bus, or other I/O themselves.
- Own asynchronous work in an explicit scope. Replacing an operation cancels its previous task, and dropping the scope cancels all remaining work.
- Keep Slint and platform event loops in `yeravich-app`; `yeravich-core` exposes directly callable product capabilities and must not depend on a UI toolkit.
- Keep platform-private types out of `yeravich-core`. Hyprland support is a transparent, replaceable fallback and must never be selected before standard interfaces fail.
- Never put API keys or secret values in logs, TOML, `Debug` output, fixtures, or snapshots. Configuration contains opaque secret references only.
- Before submitting changes, run `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo check --workspace --all-targets --all-features`.
