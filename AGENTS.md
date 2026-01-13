# Agent Guidelines: Local Exchange

Welcome to the **Local Exchange (localex)** repository. This document outlines the technical environment, coding standards, and operational procedures for agentic assistants.

## 1. Project Overview
Local Exchange is a Peer-to-Peer (P2P) file sharing and communication system written primarily in Rust, with an Android client using Kotlin.
- **Core Stack**: Rust, libp2p (networking), Tokio (async), Sea-ORM (database), Ratatui (TUI), UniFFI (FFI bindings).
- **Architecture**: Multi-package Cargo workspace.

## 2. Build, Lint, and Test Commands

### General Rust Commands
- **Build**: `cargo build` (workspace) or `cargo build -p <package_name>`
- **Lint**: `cargo clippy --workspace -- -D warnings`
- **Format**: `cargo fmt --all`
- **Test (All)**: `cargo test`
- **Test (Single)**: `cargo test -p <package_name> --lib <module_path>::<test_name>`

### Android & Bindings
- **Generate Kotlin Bindings**: `make build-android-bindgen`
- **Build Android App**: `./gradlew assembleDebug` (from `/android` directory)

## 3. Code Style Guidelines

### Rust Conventions
- **Naming**:
    - Modules/Files: `snake_case` (e.g., `src/auth.rs`)
    - Structs/Enums/Traits: `PascalCase` (e.g., `DaemonPeer`)
    - Functions/Variables: `snake_case` (e.g., `get_peer_id`)
- **Imports**:
    - Group imports by source: `std`, external crates, workspace packages, then local `crate::`.
    - Use block imports for clarity: `use common::{auth::Request, peer::Peer};`
- **Types**:
    - Prefer `anyhow::Result` for application logic (`daemon`, `tui`).
    - Use `thiserror` for library boundaries and FFI (e.g., `packages/android`).
    - Use `Arc<RwLock<T>>` for shared state; `tokio::sync` for async primitives.
- **Error Handling**:
    - Avoid `unwrap()` in production logic unless success is invariants-guaranteed.
    - Provide context to errors: `context("failed to bind to port")?`.

### Kotlin/Android Conventions
- Follow standard Kotlin style (PascalCase for classes, camelCase for methods/properties).
- UI is built with **Jetpack Compose**.

## 4. Testing Expectations
- **Unit Tests**: Place in the same file using `#[cfg(test)] mod tests { ... }`.
- **Integration Tests**: Place in `tests/` directory at crate root (create if missing).
- **Requirement**: All new features must include unit tests. Run `cargo test` before submitting changes.

## 5. Operation Procedures
- **Pre-commit**: Run `cargo clippy` and `cargo fmt`.
- **LSP**: Use `lsp_diagnostics` to verify code health before finishing tasks.
- **FFI**: When modifying `packages/android` or types used in FFI, remember to regenerate bindings with `make build-android-bindgen`.

---
*Note: This file is intended for AI agents. Maintain it with clear, actionable technical instructions.*
