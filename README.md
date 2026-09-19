# Yeravich

Yeravich is an early-stage desktop translation overlay for Wayland/Hyprland. This repository currently contains the architecture skeleton: shared domain contracts, a pure Elm-style reducer, configuration types, replaceable platform boundaries, a minimal iced settings window, and a layer-shell preview. It intentionally does **not** yet register shortcuts, read real selections, call translation APIs, store secrets, or manage startup/tray integration.

## Workspace

- `yeravich-core`: platform-independent domain types, reducer, effects, configuration, and ports.
- `yeravich-ui`: platform-independent iced views.
- `yeravich-platform-wayland`: standard XDG Portal, AT-SPI, and PRIMARY-selection boundaries.
- `yeravich-compat-hyprland`: optional compositor-specific fallback boundary.
- `yeravich-desktop-wayland`: effect assembly and Wayland executables.

The default fixed language pair is `en -> zh-CN`. Provider credentials are represented only by opaque secret-store references; secret values must never be serialized.

## Build and test

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo check --workspace --all-targets --all-features
```

Run the ordinary placeholder settings window with:

```sh
cargo run -p yeravich-desktop-wayland
```

Run the windowless layer-shell event-loop scaffold with:

```sh
cargo run -p yeravich-desktop-wayland --bin yeravich-daemon
```

On a compositor supporting `wlr-layer-shell`, run the centered overlay preview with:

```sh
cargo run -p yeravich-desktop-wayland --example layer_shell_preview
```

The preview uses the active output, overlay layer, and an exclusive zone of zero. Press its Close button to exit.

## Toolchain note

The workspace MSRV is Rust 1.90. `ashpd` 0.13 requires Rust 1.92, so the unimplemented portal adapter is temporarily pinned to the latest compatible 0.12 line. Upgrade it to the planned 0.13 line when the workspace MSRV is raised; the core port is intentionally insulated from this change.

Application ID: `com.zonowry.yeravich`. Suggested future shortcut: `Super+Shift+Y`.
