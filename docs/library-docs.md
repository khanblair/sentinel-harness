# Library Docs

## Rules for Adding a Library

1. Check this file first. If the crate is listed, use it.
2. If not listed, add a proposal to Open Library Decisions below. Do not add it to `Cargo.toml` until approved.
3. A proposal states: need, alternatives tried, size cost, maintenance state, license, and why standard library code is not enough.
4. Add shared versions only in the root `Cargo.toml`. Member crates inherit them.
5. `cargo deny check` must pass. `cargo machete` must show no unused crate.
6. Versions: the **Version** column reads "pin on add". When a crate is first added, record the exact version here in the same change. Versions were not verified when this doc was drafted.

## Approved Libraries

### Runtime and Server

| Crate | Version | Use | Reason |
|---|---|---|---|
| `tokio` | 1.54.1 | Async runtime, processes, channels, timers | Standard, needed for many concurrent agents |
| `tokio-util` | 0.7.20 | Cancellation helpers, codecs | Small, pairs with tokio |
| `axum` | 0.8.9 | HTTP API, WebSocket | Fits tokio, small |
| `tower-http` | 0.7.1 | Static files, tracing, limits | Standard axum companion |
| `futures` | 0.3.34 | Stream helpers | Needed for event streams |

### Data and Types

| Crate | Version | Use | Reason |
|---|---|---|---|
| `rusqlite` (feature `bundled`) | 0.40.0 (the version `rusqlite_migration` 2.6.0 needs) | State store | No system SQLite needed |
| `rusqlite_migration` | 2.6.0 | Schema migrations | Small and simple |
| `serde`, `serde_json` | 1.0.229, 1.0.154 | Types, API, JSONL log | Standard |
| `toml` | 1.1.8 | Config and adapter profiles | Human-editable config |
| `uuid` | 1.29.0 | Ids | Standard |
| `time` | 0.3.55 | Timestamps in UTC | Small, clear |
| `petgraph` | 0.8.3 | Task graph, cycle check | Mature graph code |

### Agents and System

| Crate | Version | Use | Reason |
|---|---|---|---|
| `reqwest` (default rustls build, feature `json`) | 0.13.5 | OpenRouter and model APIs | No OpenSSL |
| `sha2`, `hex` | 0.11.1, 0.4.3 | Hash device tokens (SHA-256) and write hex | Small RustCrypto crate. Added by autopilot decision D61 |
| `which` | 8.0.6 | Find CLIs on `PATH` | Cross-platform detection |
| `notify` | 8.2.0 | Filesystem watcher | Write guard backstop |
| `keyring` | 4.2.0 | OS keychain for API keys | Keeps secrets off disk |
| `rmcp` (official Rust MCP SDK) | 3.7.0 (confirm at P3-08) | MCP file server | Allowed by rules, avoids hand-written protocol |
| `qrcode` | 0.14.1 | Pairing QR as SVG | Small, no image stack |
| `rfd` | 0.17.2 | Native folder dialog on the PC | Cross-platform OS dialog, so nobody types a path |
| `rust-embed` | 8.13.0 | Embed UI assets in the binary | One-file release |
| `globset` | 0.4.20 | Scope path patterns | Fast, reliable matching |

### CLI and Errors

| Crate | Version | Use | Reason |
|---|---|---|---|
| `clap` | 4.6.7 | Commands | Standard |
| `thiserror` | 2.0.21 | Library errors | Clear error types |
| `anyhow` | 1.0.104 | Binary errors only | Context in the top layer |
| `tracing`, `tracing-subscriber` | 0.1.44, 0.3.23 | Logs | Structured, async friendly |

### UI

| Crate | Version | Use | Reason |
|---|---|---|---|
| `leptos` (client side rendering) | pin on add | UI | Rust UI with shared types |
| `leptos_router` | pin on add | Routing | Pairs with leptos |
| `web-sys`, `wasm-bindgen` | pin on add | Browser APIs | Needed by WASM |
| `gloo` family (only what is needed) | pin on add | Storage, timers, events | Thin browser helpers |

### Fonts (bundled files, not crates)

| Role | Families |
|---|---|
| Interface | Inter (default), IBM Plex Sans, DM Sans, Source Serif 4, Atkinson Hyperlegible, plus the system font |
| Code | JetBrains Mono (default), IBM Plex Mono, Fira Code, Source Code Pro, plus the system monospace font |

Files live in `crates/sentinel-ui/assets/fonts/`. Each family needs its license checked and included before release. The design prototype loads the same families from Google Fonts.

### Build Tools (not crate dependencies)

| Tool | Use |
|---|---|
| `mdbook` | Builds the how-to site from markdown in `site/`. Written in Rust |
| `mdbook-linkcheck` or an equivalent | Checks links in the site |

### Testing and Quality

| Crate or tool | Use |
|---|---|
| `cargo-nextest` | Test runner |
| `proptest` | Property tests for scope and waves |
| `tempfile` | Temp repos |
| `assert_cmd` | CLI tests |
| `wiremock` | Fake model API |
| `insta` | Snapshots, used sparingly |
| `cargo-deny`, `cargo-machete`, `cargo-llvm-cov` | Checks and coverage |

## Usage Notes and Gotchas

- **rusqlite:** calls block. Wrap in `spawn_blocking` or use one dedicated store thread.
- **notify:** events can arrive in bursts and out of order. Debounce briefly and check the reservation table, not event order.
- **tokio::process:** set `kill_on_drop`. Capture stdout and stderr with bounded buffers.
- **git plumbing:** set `GIT_INDEX_FILE` to a private temp index for checkpoints. Never touch the user's index.
- **axum WebSocket:** drop slow clients. Send a fresh snapshot on reconnect.
- **leptos CSR:** keep the bundle small. Avoid pulling large crates into the UI crate.
- **keyring:** some Linux setups lack a keychain service. Fall back to an environment variable and warn.
- **reqwest:** set timeouts on every call.
- **brain.db:** uses `rusqlite` only. It is an index and can be rebuilt from the vault. Only `sentinel-brain` opens it.
- **Vault watching:** `notify` watches the vault. Debounce, then reindex. Skip files that Sentinel itself just wrote.
- **Telegram:** long polling needs a request timeout longer than the poll window. Handle 429 responses by waiting the time the API asks for.
- **rfd:** the dialog opens in the desktop session of the user who runs the daemon. On a headless machine, or from a phone, use the folder browser route instead.

## Open Library Decisions

| Question | Options | Status |
|---|---|---|
| Official MCP crate name and maturity | `rmcp` or minimal hand-written server | Confirm at P3-08 |
| Frontend crate | Leptos (chosen) or Dioxus | Revisit only if WASM size blocks phone budgets |
| Git access | Shell out (chosen) or `gix` | Revisit if spawn cost shows in benchmarks |
| Fake clock helper | Hand-written or a crate | Decide at P1 |
| Token counting for compaction | Characters divided by 4 estimate, or a tokenizer crate | Decide at P3-10. Estimate is enough for budgets |
| Smell checks on agent code in any language | Line heuristics now, `tree-sitter` later | Decide at P4-07. Parser is a v1.1 item |
| Duplicate detection | Own `dup-check` on token windows, or an external tool | Decide at P0-06. Own tool preferred, no new crate |
| Which fonts to bundle | All eleven, or a smaller set to cut bundle size | Decide at P6-02 with license check |
| Telegram client | `teloxide` (a maintained Rust bot framework), or calls to the Bot API with `reqwest` | Decide at P13-01. It is not an agent orchestration framework, so it is not banned |
| Markdown front matter | A maintained YAML crate, or a small parser for the fixed fields | Decide at P11-01 |

## Banned Libraries

| Banned | Reason | Use instead |
|---|---|---|
| Agent HQ code, binaries, or MCP gateway | Hackathon rule | Own coordination code |
| Any prebuilt multi-agent orchestration framework (including Rust agent frameworks that provide supervisors or crews) | Hackathon rule | Own scheduler and supervisor |
| `git2` / libgit2 | Heavy native build, no need | Shell out to `git` |
| `openssl`, `native-tls` | Build and platform pain | `rustls` |
| `lazy_static` | Superseded | `std::sync::LazyLock` |
| Any crate that wraps `git worktree` | Violates the one-tree design | Reserved files and checkpoints |
| Crates with no license or an incompatible license | Release risk | Find an alternative |
| Unmaintained crates flagged by `cargo deny` advisories | Security | Replace |

Plain model clients and SDKs are allowed. A crate is banned if it supplies agent orchestration, not if it only calls a model API.
