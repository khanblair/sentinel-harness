# Project Structure

## Rules for Keeping It True

1. Every file in the repo appears in a tree below, with a one-line purpose.
2. Adding, moving, or deleting a file updates this doc in the same change.
3. `cargo xtask doc-check` compares this doc to the real tree and fails on drift.
4. Generated files are listed in their own section and are never edited by hand.
5. Planned files carry no marker. If a file is not yet created, `progress-tracker.md` says so.

## Top Level

```
sentinel/
  Cargo.toml              workspace members and shared dependency versions
  Cargo.lock              pinned dependency versions, committed
  rust-toolchain.toml     pinned Rust version and components
  rustfmt.toml            format rules
  clippy.toml             lint thresholds
  .cargo/config.toml      cargo alias for xtask
  deny.toml               license rules and banned crates
  .gitignore              ignores target, .sentinel, dist
  LICENSE                 MIT license text
  docs/                   the 13 project docs
  design/                 interactive prototype and the how-to site wireframe
  site/                   the GitHub Pages how-to site (mdBook project)
  .github/                workflows, including the Pages deploy
  xtask/                  repo automation (checks, build helpers)
  crates/                 all Rust crates
  fixtures/               sample repo and sample backlog for tests and the dry run
  tests/                  end-to-end tests
```

## design/

```
design/
  sentinel-prototype.html  single-file interactive prototype of the whole UI. No build step. Visual and behavior reference for ui-tokens, ui-rules, ui-registry, and mobile
  site-structure.html     wireframe of the how-to site: site map, home page, content page
```

## site/

```
site/
  book.toml               mdBook settings: title, theme folder, search, GitHub link
  src/SUMMARY.md          the page tree (the left menu)
  src/index.md            Home
  src/get-started/        install.md, set-up.md, first-run.md, always-on.md
  src/using/              stream.md, reserved-files.md, permissions.md, agents.md, panels.md, budget.md
  src/brain/              notes.md, memory-and-journal.md, lessons.md, care.md, obsidian.md
  src/remote/             phone.md, telegram.md, what-each-can-do.md
  src/reference/          settings.md, appearance.md, shortcuts.md, statuses.md
  src/guides/             backlog-safely.md, blocked-task.md, hackathon.md, telegram-control.md
  src/try-it.md           embeds the interactive prototype
  src/help/               troubleshooting.md, faq.md, changelog.md
  theme/                  CSS from ui-tokens, fonts, and the light and dark switch
  img/                    screenshots from the prototype, light and dark
```

## .github/

```
.github/
  dependabot.yml                   weekly updates for cargo and for the actions used
  workflows/ci.yml                 cargo xtask ci on Linux, tests on macOS and Windows, UI wasm build and size budget
  workflows/pages.yml              builds the how-to site and deploys it to GitHub Pages
  workflows/release.yml            on a version tag: verify, build four targets, draft release with checksums
  workflows/nightly-agent-smoke.yml  opt-in run of the real agent CLIs on a self-hosted runner
```

## docs/

```
docs/
  README.md               doc map, reading order, upkeep rule
  project-overview.md     one-page summary, locked decisions, writing rules
  product-scope.md        full product definition
  architecture.md         system design, data model, API, security
  project-structure.md    this file
  build-plan.md           phases and tasks
  code-standards.md       code rules, smells, AI slop
  development.md          setup, commands, release
  library-docs.md         approved and banned crates
  ui-tokens.md            design values
  ui-rules.md             UI behavior and copy rules
  ui-registry.md          component list
  mobile.md               phone experience
  progress-tracker.md     live status
```

## xtask/

```
xtask/
  Cargo.toml              xtask crate manifest
  src/main.rs             command dispatch for ci, checks, build
  src/ci.rs               runs the cargo steps and the file checks, reports all failures
  src/finding.rs          Finding and CheckResult shared by every check
  src/files.rs            lists repo files (git ls-files, or a folder walk)
  src/rust_scan.rs        small Rust scanner: function spans, nesting depth
  src/slop.rs             banned words and patterns scanner
  src/smell.rs            function, file, nesting, parameter, data clump, message chain, public field checks
  src/dup.rs              duplicate block check (normalized token windows)
  src/docs.rs             doc-check against project-structure
  src/ban.rs              bans git worktree use and banned crates
  src/contrast.rs         contrast check for design token pairs
  banned-words.txt        the slop word and phrase list
```

## crates/sentinel-core

```
crates/sentinel-core/
  Cargo.toml              manifest
  src/lib.rs              module exports
  src/ids.rs              TaskId, RunId, ReservationId, DeviceId
  src/task.rs             Task, TaskStatus, Tier
  src/scope.rs            Scope and path set math (overlap, contains)
  src/reservation.rs      Reservation, ReservationState
  src/cost.rs             Cost in micro-dollars, estimate flag
  src/event.rs            Event enum shared by daemon and UI
  src/error.rs            error types
  src/permission.rs       PermissionMode, Capability, Approval
  src/tool.rs             ToolInfo and ToolStatus
  src/state.rs            HarnessState, TailscaleState, ConnectionState, Decision
  src/note.rs             NoteType, LessonState, BrainMode, ProposalKind
  src/quality.rs          QualityNote
```

## crates/sentinel-store

```
crates/sentinel-store/
  Cargo.toml              manifest
  src/lib.rs              Store handle
  src/migrations/0001_init.sql   first schema
  src/tasks.rs            task queries
  src/reservations.rs     reservation queries
  src/runs.rs             run and cost queries
  src/events.rs           event append and read
  src/devices.rs          paired device queries
  src/settings.rs         settings queries
  tests/store.rs          store integration tests
```

## crates/sentinel-reserve

```
crates/sentinel-reserve/
  Cargo.toml              manifest
  src/lib.rs              exports
  src/table.rs            in-memory reservation table backed by the store
  src/waves.rs            builds waves of disjoint-scope tasks
  src/scheduler.rs        picks ready tasks, reserves files, handles requests for more files
  src/barrier.rs          barrier state and rules
  tests/waves.rs          wave and overlap tests
```

## crates/sentinel-tree

```
crates/sentinel-tree/
  Cargo.toml              manifest
  src/lib.rs              exports
  src/git.rs              thin wrapper over the git binary (plumbing only)
  src/checkpoint.rs       commit reserved paths with a private index
  src/watcher.rs          filesystem watcher for writes to unreserved files
  src/restore.rs          restore a file from the last checkpoint
  src/tamper.rs           detect edits to existing tests
  src/guard_config.rs     write guard config for agents that support hooks
  tests/checkpoint.rs     checkpoint tests on temp repos
```

## crates/sentinel-workers

```
crates/sentinel-workers/
  Cargo.toml              manifest
  src/lib.rs              exports
  src/worker.rs           Worker trait
  src/supervisor.rs       spawn, timeout, kill, output capture
  src/budget.rs           budget wrapper, per-task ceiling, kill signal
  src/context.rs          retry prompt builder, failure summary, output truncation, loop compaction
  src/detect.rs           PATH scan, version probe, login check
  src/profile.rs          adapter profile loader (TOML)
  src/adapter_cli.rs      generic CLI adapter driven by a profile
  src/adapter_api.rs      OpenRouter worker loop
  src/mcp_files.rs        MCP file server that checks reservations
  src/router.rs           tier selection and escalation
  profiles/claude.toml    profile for Claude Code
  profiles/codex.toml     profile for Codex
  profiles/opencode.toml  profile for OpenCode
  profiles/gemini.toml    profile for Gemini CLI
  profiles/goose.toml     profile for goose
  profiles/pi.toml        profile for pi
  profiles/agy.toml       profile for agy
  profiles/deepseek.toml  profile for deepseek
  profiles/mcode.toml     profile for mcode
  profiles/generic.toml   template for unknown tools
  tests/detect.rs         detection tests with fake binaries
```

## crates/sentinel-plan

```
crates/sentinel-plan/
  Cargo.toml              manifest
  src/lib.rs              exports
  src/backlog.rs          backlog parser
  src/planner.rs          planner model call, scope and dependency output
  src/graph.rs            task graph, cycle check
  src/contradiction.rs    contradiction flags and decision requests
  tests/graph.rs          graph tests
```

## crates/sentinel-verify

```
crates/sentinel-verify/
  Cargo.toml              manifest
  src/lib.rs              exports
  src/gate.rs             run the full suite at a barrier
  src/verifier.rs         second-model diff review
  src/verdict.rs          combine checks into a status
  src/quality.rs          quality notes on agent diffs (warnings only)
  src/report.rs           honest report generator
  tests/verdict.rs        verdict tests
  tests/quality.rs        notes never change a verdict
```

## crates/sentinel-brain

```
crates/sentinel-brain/
  Cargo.toml              manifest
  src/lib.rs              exports
  src/note.rs             Note, NoteType, front matter, wikilink parsing
  src/vault.rs            read and write markdown files, atomic replace, generated-text markers
  src/watcher.rs          watch the vault for outside edits
  src/index.rs            brain.db: search, links, backlinks, graph data
  src/select.rs           agent memory: pick notes within the token cap
  src/journal.rs          run, task, and file notes from run evidence
  src/lesson.rs           lesson notes: typed fields, proof rules, states
  src/mode.rs             off, read-only, learn, and the tighten-only rule
  src/care.rs             care run: refresh, consolidate, improve, prune
  src/schedule.rs         care schedule and missed-run handling
  src/proposal.rs         proposals as files, accept, dismiss, auto-apply rules
  src/strength.rs         strength rise and decay
  tests/vault.rs          round trip, outside edits, generated markers
  tests/select.rs         token cap, pinned first
  tests/lesson.rs         proof rules, no task text in lesson notes
  tests/care.rs           care never changes a user note without an accept
```

## crates/sentinel-telegram

```
crates/sentinel-telegram/
  Cargo.toml              manifest
  src/lib.rs              exports
  src/poller.rs           long polling of the Bot API
  src/router.rs           commands and button presses to API calls
  src/format.rs           message text at three detail levels
  src/outbox.rs           queue, retries, batching, live-message edits
  src/auth.rs             allowlist, pairing code, one-time button tokens
  src/notifier.rs         Notifier interface
  src/drafts.rs           pending /task text and confirm tokens
  tests/format.rs         Summary never holds file contents, keys, or note text
  tests/auth.rs           unknown senders are ignored, expired buttons fail
  tests/router.rs         each level allows only its routes
```

## crates/sentinel-daemon

```
crates/sentinel-daemon/
  Cargo.toml              manifest
  src/lib.rs              exports
  src/server.rs           axum app and routes
  src/ws.rs               WebSocket event stream
  src/auth.rs             tokens, origin check, pairing
  src/tailscale.rs        address lookup, bind rules, serve helper
  src/pairing.rs          QR payload and one-time code
  src/onboarding.rs       onboarding state and steps
  src/permissions.rs      permission mode, per-agent capabilities, extension request routing
  src/approvals.rs        approval cards: pending, allowed, denied, defaulted
  src/connections.rs      connection state and test actions (Tailscale, OpenRouter, MCP, git, keychain)
  src/doctor.rs           install check used by onboarding and `sentinel doctor`
  src/host.rs             headless mode, heartbeat, back-online handling
  src/service.rs          `sentinel service install` for systemd and launchd
  src/folders.rs          native folder dialog, phone folder browser, repo checks
  src/assets.rs           embedded UI assets
  src/run.rs              run loop tying planner, scheduler, workers, verifier
  tests/api.rs            API tests
```

## crates/sentinel-cli

```
crates/sentinel-cli/
  Cargo.toml              manifest
  src/main.rs             entry
  src/commands.rs         run, scan, pair, report, doctor
```

## crates/sentinel-ui

```
crates/sentinel-ui/
  Cargo.toml              manifest
  Trunk.toml              WASM build config
  index.html              shell page and manifest link
  manifest.webmanifest    PWA manifest
  assets/fonts/           bundled font files (see library-docs)
  src/main.rs             mount
  src/app.rs              router and layout
  src/state.rs            client state from snapshot and events
  src/api.rs              typed API client
  src/ws.rs               WebSocket client
  src/theme.css           CSS variables from ui-tokens
  src/components/         one file per component in ui-registry
  src/views/board.rs      board view
  src/views/reserved.rs     reserved files view
  src/views/agents.rs     agent stream view
  src/views/cost.rs       cost view
  src/views/tools.rs      detected tools view
  src/views/report.rs     report view
  src/views/settings.rs   settings and devices
  src/views/brain.rs      Brain panel: notes, search, editor, backlinks, proposals
  src/views/brain_graph.rs graph view
  src/views/telegram.rs   Telegram settings page
  src/views/telegram_chat.rs chat preview panel
  src/views/onboarding.rs install check and five-step onboarding
  src/tour.rs             guided tour steps and anchors (seven stops)
  src/guide.rs            in-app user flow guide (13 steps with try-it actions)
  src/palette.rs          command palette (Ctrl or Cmd + K)
  src/slash.rs            slash command parser for the composer
  src/prefs.rs            appearance preferences and token overrides, stored per device
  src/layout.rs           breakpoints and panel open state
  src/views/stream.rs     stream and composer
  src/views/permissions.rs permission modes and capability matrix
  src/views/connections.rs connections page
  src/views/install.rs    install checklist and install log
  src/views/folder_picker.rs folder choice and repo checks
  src/views/pairing.rs    Tailscale checks, phone steps, link, code
  src/views/dry_run.rs    dry run explanation, stages, and result
```

## fixtures/ and tests/

```
fixtures/
  sample-repo/            tiny repo with tests, used in dry run and tests
  sample-backlog.md       backlog with overlaps, a contradiction, a fake-done task
  fake-cli/               scripts that mimic agent CLIs for tests
  brain-seed/             a small vault with notes, lessons, and a proposal for tests
tests/
  e2e/collision.rs        two agents, one file, no collision
  e2e/resume.rs           kill and resume
  e2e/budget.rs           cap enforcement
  e2e/tamper.rs           edited test is rejected
  e2e/compaction.rs       retry prompt stays under its cap
  e2e/permissions.rs      each permission mode behaves as documented, writes to unreserved files always blocked
  e2e/brain.rs            second run uses notes, live mode stays read-only, journal off in read-only
  e2e/brain_care.rs       care run makes proposals and never changes a user note without an accept
  e2e/telegram.rs         fake Bot API: pair, notify, approve, task, revoke
```

## Generated Files (never edit by hand)

| Path | Source |
|---|---|
| `target/` | Cargo |
| `crates/sentinel-ui/dist/` | Trunk build, embedded into the daemon |
| `<repo>/.sentinel/*` | Runtime state, log, report |
| `<repo>/.sentinel/brain/` | The vault. Written by Sentinel and by you. Not generated wholesale |
| `docs/` tables marked generated | None yet |

## Where Tests Go

| Kind | Location |
|---|---|
| Unit | Bottom of the same file in `#[cfg(test)] mod tests` |
| Crate integration | `crates/<name>/tests/` |
| End to end | `tests/e2e/` |
| Fixtures | `fixtures/` |

## Naming Rules

- Crates: `sentinel-<area>`, lowercase, hyphen.
- Files and modules: `snake_case.rs`, named for the thing, never `utils.rs`, `helpers.rs`, `common.rs`, or `misc.rs`.
- Types: `UpperCamelCase`. Functions: `snake_case`. Constants: `SCREAMING_SNAKE_CASE`.
- UI component file name equals component name in snake case.
- Test names state the behavior: `second_task_waits_for_reserved_file`.
- Docs: lowercase, hyphen.

## Special and Shared Files

| File | Rule |
|---|---|
| `Cargo.toml` (root) | Only place for shared dependency versions |
| `crates/sentinel-core` | The only crate both daemon and UI depend on for shared types |
| `deny.toml` | Only place for the banned crate list |
| `xtask/banned-words.txt` | Only place for the slop word list |
| `profiles/*.toml` | Data, no code. Adding a tool should not need Rust changes |
| `brain.db` | Index only. Rebuildable from the vault. Written only by `sentinel-brain` |
