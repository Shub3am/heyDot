# Phase 0: Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the hackathon repo into a buildable Tauri 2 + Rust workspace with a license, the landing page moved to `site/`, CI, and routing docs, without changing any product behaviour yet.

**Architecture:** A Cargo workspace rooted at the repo root with one member for now (`app/src-tauri`); later crates join under `crates/`. The Tauri app lives in `app/` (React + TS + Vite, pnpm). CI on GitHub Actions macOS runners checks formatting, lint, tests, and builds an unsigned universal `.dmg` artifact.

**Tech Stack:** Rust 1.98 (edition 2024), Tauri 2.11 (`tauri` 2.11.x, `@tauri-apps/cli` 2.11.x), create-tauri-app 4.7.4 `react-ts` template, React 19, Vite 8, TypeScript 6 (template pin), vitest 5 + jsdom + Testing Library, pnpm 12.5.1, Node 26, GitHub Actions (`actions/checkout@v7`, `pnpm/action-setup@v6`, `actions/setup-node@v7`, `Swatinem/rust-cache@v2`, `actions/upload-artifact@v7`).

**Spec:** `docs/specs/2026-09-23-phase0-phase1-design.md` (section "Phase 0: foundation").

## Global Constraints

- License: `LICENSE` is the Apache-2.0 full text. No CLA, contribution docs or commercial terms in this phase.
- Bundle identifier `com.shub3am.heydot`; product name `Hey Dot`.
- Workspace root `Cargo.toml` at repo root, `resolver = "3"`, release profile lives at the root (Cargo ignores member profiles in a workspace).
- `rust-toolchain.toml` pins channel `1.98` with `rustfmt`, `clippy`, targets `aarch64-apple-darwin` and `x86_64-apple-darwin`.
- `app/package.json` has `"packageManager": "pnpm@12.5.1"`.
- No template demo code kept (no `greet` command, no opener plugin, no Vite/Tauri/React logos).
- `hey_dot.py` and `requirements.txt` stay untouched until Phase 1 parity.
- Commits: one logical change each, no co-author lines, nothing pushed until the user has seen test output.
- No em dashes in any written file.

## Review Focus

- Fresh clone, `cargo clippy` before the frontend is built: `tauri::generate_context!` fails if `app/dist` is missing. Expect CI and the docs to build the frontend first. Task 3 orders CI steps accordingly; Task 4 documents it.
- Vercel deploy after `frontend/` moves to `site/`: the Vercel project's Root Directory still says `frontend`, so the next deploy fails. Expect the user to be told to change it to `site`. Task 1 step and final report cover this.
- CI universal build needs both Rust targets installed: expect `rust-toolchain.toml` to make rustup install them on the runner. Task 3 verifies with a local universal build.
- pnpm version mismatch between local and CI: expect CI to read `packageManager` from `app/package.json`. Task 3 sets `package_json_file`.
- Build output path in a workspace: bundles land in the root `target/`, not `app/src-tauri/target/`. Expect the artifact upload path to be `target/universal-apple-darwin/release/bundle/dmg/*.dmg`. Task 3 asserts the file exists locally.

---

### Task 1: License, root ignore file, landing page moved to `site/`

**Files:**
- Create: `LICENSE`
- Create: `.gitignore`
- Move: `frontend/` -> `site/`

**Interfaces:**
- Consumes: nothing.
- Produces: `site/` as the landing page folder (later phases edit it there).

- [ ] **Step 1: Add the Apache-2.0 license text**

```bash
cd ~/Desktop/personal/heyDot
curl -fsSL https://www.apache.org/licenses/LICENSE-2.0.txt -o LICENSE
head -3 LICENSE
```
Expected: the first non-blank lines read `Apache License` and `Version 2.0, January 2004`.

- [ ] **Step 2: Commit the license**

```bash
git add LICENSE
git commit -m "Add Apache-2.0 license"
```

- [ ] **Step 3: Add the root ignore file**

Create `.gitignore`:
```gitignore
/target/
node_modules/
.DS_Store
```

- [ ] **Step 4: Move the landing page**

```bash
git mv frontend site
```

- [ ] **Step 5: Verify the landing page still builds from its new folder**

```bash
npm --prefix site ci && npm --prefix site run build
```
Expected: Vite prints `built in` and `site/dist/index.html` exists. `git status --short` shows no `site/dist` or `site/node_modules` (both ignored by `site/.gitignore`).

- [ ] **Step 6: Commit**

```bash
git add .gitignore site frontend
git commit -m "Move landing page from frontend/ to site/"
```

Note for the final report: the Vercel project Root Directory must be changed from `frontend` to `site` before the next deploy.

---

### Task 2: Tauri 2 app scaffold in a Cargo workspace

**Files:**
- Create: `Cargo.toml` (workspace root), `rust-toolchain.toml`
- Create (generated then edited): `app/` from create-tauri-app `react-ts`
- Modify: `app/package.json`, `app/src-tauri/Cargo.toml`, `app/src-tauri/tauri.conf.json`, `app/src-tauri/src/lib.rs`, `app/src-tauri/src/main.rs`, `app/src-tauri/capabilities/default.json`, `app/index.html`, `app/src/App.tsx`, `app/vite.config.ts`
- Delete: `app/README.md`, `app/src/App.css`, `app/src/assets/react.svg`, `app/public/tauri.svg`, `app/public/vite.svg`
- Test: `app/src/App.test.tsx`

**Interfaces:**
- Consumes: nothing.
- Produces: Rust package `hey-dot` (lib `hey_dot_lib`, fn `run()`), pnpm scripts `dev`, `build`, `test`, `tauri` in `app/`. Later tasks and phases add crates to `[workspace] members`.

- [ ] **Step 1: Generate the template**

```bash
cd ~/Desktop/personal/heyDot
pnpm dlx create-tauri-app@4.7.4 -y -m pnpm -t react-ts --identifier com.shub3am.heydot app
```
Expected: `app/` exists with `src/`, `src-tauri/`, `package.json`.

- [ ] **Step 2: Delete template demo files not imported by code**

```bash
rm app/README.md app/public/tauri.svg app/public/vite.svg
rmdir app/public
```
(`App.css` and `assets/react.svg` are imported by the template `App.tsx`, so they go in Step 9 once it is replaced.)

- [ ] **Step 3: Create the workspace root and toolchain pin**

`Cargo.toml`:
```toml
[workspace]
resolver = "3"
members = ["app/src-tauri"]

# Read the optimization guideline for more details: https://tauri.app/concept/size/#cargo-configuration
[profile.release]
codegen-units = 1
lto = true
opt-level = 3
panic = "abort"
strip = true
```

`rust-toolchain.toml`:
```toml
[toolchain]
channel = "1.98"
components = ["rustfmt", "clippy"]
targets = ["aarch64-apple-darwin", "x86_64-apple-darwin"]
```

- [ ] **Step 4: Rename the Rust package, drop the opener plugin and the member release profile**

`app/src-tauri/Cargo.toml` becomes:
```toml
[package]
name = "hey-dot"
version = "0.1.0"
description = "Local-first voice and screen assistant"
authors = ["Shubham Vishwakarma"]
license = "Apache-2.0"
edition = "2024"

[lib]
# The `_lib` suffix keeps the lib name distinct from the bin name, see https://github.com/rust-lang/cargo/issues/8519
name = "hey_dot_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

`app/src-tauri/src/lib.rs`:
```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

`app/src-tauri/src/main.rs`:
```rust
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    hey_dot_lib::run()
}
```

`app/src-tauri/capabilities/default.json`:
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Capability for the main window",
  "windows": ["main"],
  "permissions": ["core:default"]
}
```

- [ ] **Step 5: Set product name and window title**

In `app/src-tauri/tauri.conf.json` change `"productName": "app"` to `"productName": "Hey Dot"` and the window `"title": "app"` to `"title": "Hey Dot"`. Leave everything else as generated.

- [ ] **Step 6: Update the frontend package**

In `app/package.json`: set `"name": "hey-dot-app"`, add `"packageManager": "pnpm@12.5.1"`, add script `"test": "vitest run"`, remove the dependency `"@tauri-apps/plugin-opener"`. Then:
```bash
pnpm -C app install
pnpm -C app add -D vitest@5 jsdom @testing-library/react
```

In `app/index.html`: delete the `<link rel="icon" ...>` line and set `<title>Hey Dot</title>`.

- [ ] **Step 7: Write the failing UI test**

`app/src/App.test.tsx`:
```tsx
import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import App from "./App";

test("renders the app name", () => {
  render(<App />);
  expect(screen.getByRole("heading", { name: "Hey Dot" })).toBeTruthy();
});
```

Add the vitest environment to `app/vite.config.ts`: first line `/// <reference types="vitest/config" />`, and inside the returned config after `plugins: [react()],` add:
```ts
  test: {
    environment: "jsdom",
  },
```

- [ ] **Step 8: Run it to verify it fails**

Run: `pnpm -C app test`
Expected: FAIL, the template `App` renders "Welcome to Tauri + React", not a "Hey Dot" heading.

- [ ] **Step 9: Replace the template App**

`app/src/App.tsx`:
```tsx
function App() {
  return (
    <main>
      <h1>Hey Dot</h1>
    </main>
  );
}

export default App;
```

Then delete the files only the template App imported:
```bash
rm app/src/App.css app/src/assets/react.svg
rmdir app/src/assets
```

- [ ] **Step 10: Run all checks**

```bash
pnpm -C app test
pnpm -C app build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: vitest `1 passed`; Vite `built in`; fmt silent; clippy `Finished` with no warnings (and no "profiles for the non root package" warning); cargo test `test result: ok` (0 tests).

- [ ] **Step 11: Launch once to see the window**

Run: `pnpm -C app tauri dev`
Expected: a window titled "Hey Dot" showing the heading. Close it.

- [ ] **Step 12: Commit**

```bash
git add Cargo.toml Cargo.lock rust-toolchain.toml app
git commit -m "Scaffold Tauri 2 app in a Cargo workspace"
```

---

### Task 3: CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: scripts from Task 2 (`pnpm -C app build`, `pnpm -C app test`, `pnpm -C app tauri build`), `rust-toolchain.toml`.
- Produces: required checks `check` and `build` on PRs; artifact `hey-dot-macos-universal`.

- [ ] **Step 1: Write the workflow**

`.github/workflows/ci.yml`:
```yaml
name: CI

on:
  pull_request:
  push:
    branches: [main, rebuild]

jobs:
  check:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v7
      - uses: pnpm/action-setup@v6
        with:
          package_json_file: app/package.json
          cache: true
          cache_dependency_path: app/pnpm-lock.yaml
      - uses: actions/setup-node@v7
        with:
          node-version: 26
      - uses: Swatinem/rust-cache@v2
      - run: pnpm -C app install --frozen-lockfile
      # tauri::generate_context! needs app/dist to exist before any cargo command
      - run: pnpm -C app build
      - run: pnpm -C app test
      - run: cargo fmt --all --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace

  build:
    needs: check
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v7
      - uses: pnpm/action-setup@v6
        with:
          package_json_file: app/package.json
          cache: true
          cache_dependency_path: app/pnpm-lock.yaml
      - uses: actions/setup-node@v7
        with:
          node-version: 26
      - uses: Swatinem/rust-cache@v2
      - run: pnpm -C app install --frozen-lockfile
      - run: pnpm -C app tauri build --target universal-apple-darwin --bundles app,dmg
      - uses: actions/upload-artifact@v7
        with:
          name: hey-dot-macos-universal
          path: target/universal-apple-darwin/release/bundle/dmg/*.dmg
```

- [ ] **Step 2: Run the CI commands locally, in CI order, from a clean frontend**

```bash
rm -rf app/dist app/node_modules
pnpm -C app install --frozen-lockfile && pnpm -C app build && pnpm -C app test \
  && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
pnpm -C app tauri build --target universal-apple-darwin --bundles app,dmg
ls target/universal-apple-darwin/release/bundle/dmg/
lipo -archs "target/universal-apple-darwin/release/bundle/macos/Hey Dot.app/Contents/MacOS/"*
```
Expected: every check passes; the dmg folder lists `Hey Dot_0.1.0_universal.dmg`; `lipo` prints `x86_64 arm64`.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "Add CI workflow for checks and universal macOS build"
```

The workflow first runs on GitHub when the branch is pushed, which waits for the user's go-ahead.

---

### Task 4: Routing docs and README

**Files:**
- Create: `AGENTS.md`, `app/AGENTS.md`
- Modify: `README.md` (full rewrite)

**Interfaces:**
- Consumes: layout and commands from Tasks 1-3.
- Produces: root routing doc that later phases add one line to per new crate.

- [ ] **Step 1: Write the root routing doc**

`AGENTS.md`:
```markdown
# Hey Dot

Local-first voice and screen assistant for macOS (Windows later). Rust core, Tauri 2 shell, React UI.

## Modules

- `app/`: Tauri app, windows and UI. See `app/AGENTS.md`.
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
```

- [ ] **Step 2: Write the app module doc**

`app/AGENTS.md`:
```markdown
# app

Owns: the Tauri shell (`src-tauri/`) and the React UI (`src/`). Windows, tray, hotkeys and the IPC surface between UI and Rust live here.

Must not know about: model formats, audio processing, provider HTTP details. Those belong in `crates/` and are called from `src-tauri` only.

Entry points: `src-tauri/src/main.rs` -> `hey_dot_lib::run()` in `src-tauri/src/lib.rs`; UI starts at `src/main.tsx`.

Invariants and gotchas:
- `tauri::generate_context!` reads `dist/` at compile time, so `pnpm build` must run before `cargo clippy/test/build` on a fresh checkout.
- Capabilities in `src-tauri/capabilities/` gate every IPC command; a new command needs a permission entry.
- Bundle identifier `com.shub3am.heydot` is permanent once released; macOS permissions are keyed to it.

Called by: the user (launching the app) and CI (`.github/workflows/ci.yml`).
```

- [ ] **Step 3: Rewrite the README**

`README.md`:
```markdown
# Hey Dot

A local-first voice and screen assistant for your Mac. Ask about what is on your screen and get a spoken answer, with models that run on your machine by default.

**Status:** being rebuilt from a hackathon prototype into a full app. The original prototype is preserved at tag [`v0-hackathon`](https://github.com/Shub3am/heyDot/tree/v0-hackathon).

## Planned for the first release

- Push-to-talk and typed questions about your screen, answered in a chat panel and spoken aloud
- Follow-up questions within a conversation
- Local models downloaded on first run, or your own cloud API key, clearly labelled when data leaves your Mac
- "Hey Dot" wake word, tools and MCP, document Q&A in later phases

## Develop

Requirements: macOS, Rust (via rustup; the pinned version installs itself), Node 26, pnpm 12.

    pnpm -C app install
    pnpm -C app tauri dev

See `AGENTS.md` for test and build commands.

## License

Apache-2.0. See `LICENSE`.
```

- [ ] **Step 4: Verify the root doc stays under 60 lines**

Run: `wc -l AGENTS.md`
Expected: a number below 60.

- [ ] **Step 5: Commit**

```bash
git add AGENTS.md app/AGENTS.md README.md
git commit -m "Add routing docs and rewrite README for the rebuild"
```
