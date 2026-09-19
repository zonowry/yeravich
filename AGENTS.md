# Yeravich engineering rules

- Keep application flow unidirectional: `Model -> Event -> pure reducer -> Effect`.
- `view` functions and reducers must not perform network, filesystem, clipboard, D-Bus, or other I/O.
- Represent long-lived event sources as iced `Subscription`s. Interpret short-lived `Effect`s as iced `Task`s in the desktop assembly crate.
- Keep platform-private types out of `yeravich-core`. Hyprland support is a transparent, replaceable fallback and must never be selected before standard interfaces fail.
- Never put API keys or secret values in logs, TOML, `Debug` output, fixtures, or snapshots. Configuration contains opaque secret references only.
- Before submitting changes, run `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo check --workspace --all-targets --all-features`.
