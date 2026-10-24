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
  Cargo.toml                           xtask crate manifest
  banned-words.txt                     the slop word and phrase list
  src/ban.rs                           bans git worktree use and banned crates
  src/ci.rs                            runs the cargo steps and the file checks, reports all failures
  src/contrast.rs                      contrast check for design token pairs
  src/docs.rs                          doc-check against project-structure
  src/dup.rs                           duplicate block check (normalized token windows)
  src/files.rs                         lists repo files (git ls-files, or a folder walk)
  src/finding.rs                       Finding and CheckResult shared by every check
  src/main.rs                          command dispatch for ci, checks, build
  src/rust_scan.rs                     small Rust scanner: function spans, nesting depth
  src/slop.rs                          banned words and patterns scanner
  src/smell.rs                         function, file, nesting, parameter, data clump, message chain, public field checks
```

## crates/sentinel-core

```
crates/sentinel-core/
  Cargo.toml                           manifest
  src/api.rs                           typed API client
  src/cost.rs                          Cost in micro-dollars, estimate flag
  src/error.rs                         error types
  src/event.rs                         Event enum shared by daemon and UI
  src/ids.rs                           TaskId, RunId, ReservationId, DeviceId
  src/lib.rs                           module exports
  src/note.rs                          NoteType, LessonState, BrainMode, ProposalKind
  src/permission.rs                    PermissionMode, Capability, Approval
  src/quality.rs                       QualityNote
  src/reservation.rs                   Reservation, ReservationState
  src/scope.rs                         Scope and path set math (overlap, contains)
  src/snapshot.rs                      The full state the daemon sends to a device on connect (`GET /api/state`). The UI builds every
  src/state.rs                         HarnessState, TailscaleState, ConnectionState, Decision
  src/task.rs                          Task, TaskStatus, Tier
  src/tool.rs                          ToolInfo and ToolStatus
```

## crates/sentinel-store

```
crates/sentinel-store/
  Cargo.toml                           manifest
  src/approvals.rs                     approval cards: pending, allowed, denied, defaulted
  src/barriers.rs                      Wave barrier records. A barrier starts before a wave of tasks and finishes
  src/capabilities.rs                  The per-agent capability matrix. Rows are stored even when a capability is
  src/checkpoints.rs                   Checkpoint records: one commit per task per point in time. The `id` column
  src/decisions.rs                     Decision prompt persistence. A decision is the contradiction question from
  src/devices.rs                       paired device queries
  src/events.rs                        event append and read
  src/lib.rs                           Store handle
  src/migrations/0001_init.sql         first schema
  src/migrations/0002_extras.sql       schema
  src/quality.rs                       QualityNote
  src/reservations.rs                  reservation queries
  src/reset.rs                         Clear what belongs to one run, so the next run starts from an empty board. Events, runs, cost
  src/runs.rs                          run and cost queries
  src/settings.rs                      settings queries
  src/tasks.rs                         task queries
  src/tools.rs                         Detected worker CLIs. A rescan rewrites what it found but never changes the
  src/verifications.rs                 Verification records: the four checks and the verdict for one task
  tests/store.rs                       store integration tests
  tests/store2.rs                      file
```

## crates/sentinel-reserve

```
crates/sentinel-reserve/
  Cargo.toml                           manifest
  src/barrier.rs                       barrier state and rules
  src/claims.rs                        claim and conflict helpers for the table
  src/holdings.rs                      read-only queries on held reservations
  src/lib.rs                           exports
  src/scheduler.rs                     picks ready tasks, reserves files, handles requests for more files
  src/table.rs                         in-memory reservation table backed by the store
  src/waves.rs                         builds waves of disjoint-scope tasks
  tests/scheduler.rs                   Scheduler behaviour against a real in-memory store
  tests/waves.rs                       wave and overlap tests
```

## crates/sentinel-tree

```
crates/sentinel-tree/
  Cargo.toml                           manifest
  src/checkpoint.rs                    commit reserved paths with a private index
  src/git.rs                           thin wrapper over the git binary (plumbing only)
  src/guard_config.rs                  write guard config for agents that support hooks
  src/lib.rs                           exports
  src/restore.rs                       restore a file from the last checkpoint
  src/revert.rs                        Undo a checkpoint whose task failed verification, so its code does not break the next wave
  src/tamper.rs                        detect edits to existing tests
  src/watcher.rs                       filesystem watcher for writes to unreserved files
  tests/checkpoint.rs                  checkpoint tests on temp repos
  tests/guard.rs                       Integration tests for the write guard watcher and the guard files
  tests/revert.rs                      A reverted checkpoint puts files back and keeps history append only
```

## crates/sentinel-workers

```
crates/sentinel-workers/
  Cargo.toml                           manifest
  profiles/agy.toml                    profile for agy
  profiles/claude.toml                 profile for Claude Code
  profiles/codex.toml                  profile for Codex
  profiles/deepseek.toml               profile for deepseek
  profiles/gemini.toml                 profile for Gemini CLI
  profiles/generic.toml                template for unknown tools
  profiles/goose.toml                  profile for goose
  profiles/mcode.toml                  profile for mcode
  profiles/opencode.toml               profile for OpenCode
  profiles/pi.toml                     profile for pi
  src/adapter_cli.rs                   generic CLI adapter driven by a profile
  src/budget.rs                        budget wrapper, per-task ceiling, kill signal
  src/context.rs                       retry prompt builder, failure summary, output truncation, loop compaction
  src/detect.rs                        PATH scan, version probe, login check
  src/lib.rs                           exports
  src/profile.rs                       adapter profile loader (TOML)
  src/router.rs                        tier selection and escalation
  src/supervisor.rs                    spawn, timeout, kill, output capture
  src/worker.rs                        Worker trait
  tests/detect.rs                      detection tests with fake binaries
```

## crates/sentinel-plan

```
crates/sentinel-plan/
  Cargo.toml                           manifest
  src/backlog.rs                       backlog parser
  src/contradiction.rs                 contradiction flags and decision requests
  src/graph.rs                         task graph, cycle check
  src/lib.rs                           exports
  src/planner.rs                       planner model call, scope and dependency output
  tests/graph.rs                       graph tests
```

## crates/sentinel-verify

```
crates/sentinel-verify/
  Cargo.toml                           manifest
  src/gate.rs                          run the full suite at a barrier
  src/lib.rs                           exports
  src/quality.rs                       quality notes on agent diffs (warnings only)
  src/report.rs                        honest report generator
  src/verdict.rs                       combine checks into a status
  src/verifier.rs                      second-model diff review
  tests/quality.rs                     notes never change a verdict
  tests/verdict.rs                     verdict tests
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
  Cargo.toml                           manifest
  src/access.rs                        Engine methods for getting at the daemon: Tailscale, pairing, devices, the install check,
  src/agents.rs                        Finding the agent CLIs on this computer, turning them into workers, and asking one of them
  src/ask.rs                           Questions the run loop puts to the user: approval cards and decisions. Each one waits for an
  src/attempt.rs                       One attempt at one task: choose an agent, run it, and either checkpoint its work or put the
  src/cmd.rs                           Run external programs behind one trait
  src/config.rs                        `<repo>/.sentinel/config.toml`: settings and routing that onboarding writes
  src/connections.rs                   connection state and test actions (Tailscale, OpenRouter, MCP, git, keychain)
  src/controls.rs                      Engine methods that change settings and answer questions. Each one says who did it, because a
  src/doctor.rs                        install check used by onboarding and `sentinel doctor`
  src/engine.rs                        The engine: one handle that owns the state of a Sentinel daemon. The HTTP layer, the CLI, and the
  src/folders.rs                       native folder dialog, phone folder browser, repo checks
  src/hashing.rs                       Device tokens: generation, hashing, and comparison
  src/lib.rs                           exports
  src/live.rs                          What the engine keeps while a run is live: control channels, the budget, and the questions the
  src/onboarding.rs                    onboarding state and steps
  src/pairing.rs                       QR payload and one-time code
  src/plan_phase.rs                    The first phase of a run: read the backlog, plan it, settle contradictions, and mark which
  src/prompt.rs                        The text each worker gets. Rules go in the prompt as guidance; the guard enforces the real
  src/report_build.rs                  Build the honest report from what the store knows, and write `.sentinel/report.md`
  src/reviewer.rs                      Who reviews a task's diff. A different agent gives the real review. Without one (fake mode, or
  src/run.rs                           run loop tying planner, scheduler, workers, verifier
  src/run_ctx.rs                       Everything the tasks of one run share. A run owns one `RunCtx`; every wave and attempt gets an
  src/secrets.rs                       Secrets for the daemon. Values live in the OS keychain, never on disk and never in logs
  src/tailscale.rs                     address lookup, bind rules, serve helper
  src/verify_phase.rs                  After the barrier: check each checkpointed task of the wave and give it a status. Only
  src/wave.rs                          One wave: reserve files, run the tasks together, wait at the barrier, run the full suite on a
  tests/engine_run.rs                  A whole run with the fake agents on the sample repository. It proves the harness end to end:
  tests/folders.rs                     Folder browser, repository checks, and starter backlog behavior
```

## crates/sentinel-cli

```
crates/sentinel-cli/
  Cargo.toml                           manifest
  src/commands.rs                      run, scan, pair, report, doctor
  src/main.rs                          entry
```

## crates/sentinel-ui

```
crates/sentinel-ui/
  Cargo.toml                           manifest
  Trunk.toml                           WASM build config
  assets/fonts/LICENSE-inter.txt       text list
  assets/fonts/LICENSE-jetbrains-mono.txt text list
  assets/fonts/inter-latin-400-normal.woff2 bundled font
  assets/fonts/inter-latin-500-normal.woff2 bundled font
  assets/fonts/inter-latin-600-normal.woff2 bundled font
  assets/fonts/jetbrains-mono-latin-400-normal.woff2 bundled font
  assets/fonts/jetbrains-mono-latin-500-normal.woff2 bundled font
  index.html                           shell page and manifest link
  manifest.webmanifest                 PWA manifest
  src/api.rs                           typed API client
  src/app.rs                           router and layout
  src/components/icon.rs               `Icon`: one inline SVG from the outline set, sized in `em` so it scales with the text size
  src/components/mod.rs                One file per component in `docs/ui-registry.md`
  src/components/status_badge.rs       `StatusBadge`: a shape, a color, and a label together, so state is never color alone
  src/fonts.css                        styles
  src/icons.rs                         Icon paths, ported from the prototype's `ICONS` table. One outline set, 1.7 px stroke
  src/layout.rs                        breakpoints and panel open state
  src/main.rs                          mount
  src/prefs.rs                         appearance preferences and token overrides, stored per device
  src/state.rs                         client state from snapshot and events
  src/theme.css                        CSS variables from ui-tokens
```

## crates/sentinel-gate

```
crates/sentinel-gate/
  Cargo.toml                           workspace members and shared dependency versions
  src/book.rs                          In-memory approval cards. The caller persists them and emits the events
  src/lib.rs                           module exports
  src/policy.rs                        What a permission mode asks for, straight from F15 and the disposition
  tests/policy.rs                      file
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
