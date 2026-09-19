# Yeravich

Yeravich is an early-stage cross-platform desktop translation client built with Rust and Slint. It currently contains the application shell and the core capability boundary. Real translation providers, selection capture, credential storage, shortcuts, tray integration, and startup integration are still to be connected.

## Workspace

- `yeravich-core`: directly callable product capabilities: provider management, translation orchestration, configuration, and safe secret references. It has no UI dependency.
- `yeravich-app`: the Slint UI, presentation state, cancellable task scope, and future platform adapters.

The default fixed language pair is `en -> zh-CN`. Provider credentials are represented only by opaque secret-store references; secret values must never be serialized.

The application deliberately avoids a global reducer. Its flow is:

```text
Slint callback -> Rust controller -> yeravich-core capability -> UI state snapshot
```

The `.slint` file derives presentation from one `UiState` value. Rust callbacks launch owned, cancellable operations. Starting another operation of the same kind cancels the previous one, and closing the application drops the task scope and cancels all children.

## Build and test

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo check --workspace --all-targets --all-features
```

Run the application with:

```sh
cargo run -p yeravich-app
```

## Toolchain note

The workspace MSRV is Rust 1.90. Slint is pinned to 1.16.1 because Slint 1.17 and newer require Rust 1.92. Upgrade Slint when the workspace toolchain is raised.

Application ID: `com.zonowry.yeravich`. Suggested future shortcut: `Super+Shift+Y`.
