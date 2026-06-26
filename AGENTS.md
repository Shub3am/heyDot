# Hey Dot

Local-first voice and screen assistant for macOS (Windows later). Rust core, Tauri 2 shell, React UI.

## Modules

- `app/`: Tauri app, windows and UI. See `app/AGENTS.md`.
- `crates/dot-settings/`: settings file and API keys. See `crates/dot-settings/AGENTS.md`.
- `site/`: marketing landing page (Vite + React, deployed on Vercel from `site/`). Independent of the app.
- `docs/specs/`, `docs/plans/`: design specs and implementation plans per phase.
- `hey_dot.py`, `requirements.txt`: hackathon prototype, kept until Phase 1 reaches parity, then deleted. Tag `v0-hackathon` preserves it.

## Run

    pnpm -C app install
    pnpm -C app tauri dev

## Test

    pnpm -C app build        # must run before any cargo command: tauri needs app/dist
    pnpm -C app test
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

## Build

    pnpm -C app tauri build --target universal-apple-darwin --bundles app,dmg

Output: `target/universal-apple-darwin/release/bundle/`. Builds are unsigned until an Apple Developer ID is added.

## Repo-wide rules

- Toolchains are pinned: `rust-toolchain.toml` and `packageManager` in `app/package.json`.
- The Cargo workspace root is the repo root; release profile settings live there, not in member crates.
- Every module folder has an `AGENTS.md`; update it in the same commit when its purpose, boundaries or invariants change.
- License: Apache-2.0.
