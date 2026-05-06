# Contributing to Klipo

Thanks for considering a contribution. Klipo is pre-1.0 and we're shipping fast — most surfaces still move week-to-week. The fastest way to land a change is to read this whole page first.

## Ground Rules

- **Discuss large changes first.** Open an issue for anything that touches the storage schema, the sync protocol, the clipboard pipeline, or adds a new permission. Smaller fixes / docs / tests are fine to send straight as a PR.
- **Tests are not optional** for new behavior. If you change a public IPC command, add or update a Vitest / `cargo test` case.
- **Performance budgets are non-negotiable.** See [`docs/perf-budget.md`](./docs/perf-budget.md) for targets and [`docs/perf-runbook.md`](./docs/perf-runbook.md) for the manual measurement playbook. If your change brushes a budget, include the bench numbers in the PR.
- **No clipboard content in logs, ever.** Log targets, sizes, identifiers — never the bytes themselves.

## Setup

```bash
# Toolchain
node 20+
pnpm 9+      (corepack enable && corepack prepare pnpm@latest --activate)
rust 1.83+   (rustup default stable)

# Windows-only: install the WebView2 Evergreen runtime if you're on Win10.

# Bootstrap
git clone https://github.com/aydogandagidir/klipo
cd klipo
pnpm install
```

## Running Locally

```bash
pnpm tauri:dev      # full Tauri runtime (popup + Rust)
pnpm dev            # Vite-only, fastest for pure UI iteration (Tauri IPC unavailable)
```

The popup binds to `Ctrl+Alt+V` by default. Inside the popup, gear icon → Settings.

## Verification Pipeline

Run all of these before opening a PR. CI runs the same set:

```bash
# Frontend
pnpm typecheck
pnpm lint
pnpm format:check
pnpm build

# Backend
cd src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib
```

## Commit and PR Style

- **Commits**: imperative present tense ("Add X", not "Added X"). Reference an issue if there is one.
- **PR titles**: same. Keep them under 72 chars.
- **PR descriptions**: explain *why* the change is needed, not just *what* you did. If the change touches user-visible surface, include a screenshot or a one-line description of the new behavior.
- **Squash merging** is the default. Rebase before merge if there are conflicts.

## Things That Need Help

| Area | Notes |
|---|---|
| macOS port (Faz C) | NSPasteboard polling, vibrancy, Cmd+Option+V |
| Sync server (Faz D) | CF Workers + D1 + R2 schema; see [`docs/sync-protocol.md`](./docs/sync-protocol.md) |
| Theme catalog (E2) | Pre-set themes for the General tab's theme picker |
| Performance benches | More corpora for `bench/` (Turkish, Japanese, code) |
| Plugin API (E4) | JS sandbox + manifest format |

## Code of Conduct

Be civil. Disagreements are fine; personal attacks aren't. Maintainers reserve the right to remove comments and PRs that violate this.

## License

By contributing you agree your code ships under the project's [Apache-2.0 license](./LICENSE).
