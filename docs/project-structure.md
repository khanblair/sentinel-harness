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
  src/contrast/color.rs                Color math: relative luminance and the WCAG contrast ratio
  src/contrast/mod.rs                  Contrast check: design token pairs meet the contrast rules of `docs/ui-tokens.md`
  src/contrast/parse.rs                Reads the color tables out of `prefs.rs`. The check reads the source text on purpose: the
  src/contrast/rules.rs                The contrast rules: which token goes on which, and the least ratio each pair needs
  src/contrast/tests.rs                Tests for the contrast check
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
  src/chat.rs                          Chats and attachments (`docs/product-scope.md`, F23). A chat is one conversation in a project
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
  src/attachments.rs                   Attachments: what the composer plus button added to a chat
  src/barriers.rs                      Wave barrier records. A barrier starts before a wave of tasks and finishes
  src/capabilities.rs                  The per-agent capability matrix. Rows are stored even when a capability is
  src/chats.rs                         Chats: the conversations of a project. The engine adds the run state to each row
  src/checkpoints.rs                   Checkpoint records: one commit per task per point in time. The `id` column
  src/decisions.rs                     Decision prompt persistence. A decision is the contradiction question from
  src/devices.rs                       paired device queries
  src/events.rs                        event append and read
  src/lib.rs                           Store handle
  src/migrations/0001_init.sql         first schema
  src/migrations/0002_extras.sql       schema
  src/migrations/0003_chats.sql        schema
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
  src/capture.rs                       Reads a worker's stdout and stderr line by line, keeps a bounded tail, and drains the
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
  src/plan_check.rs                    Plan validation: ids, references, scope paths, and dependency cycles
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
  Cargo.toml                           manifest
  src/body.rs                          Body text: wikilinks, the Sentinel generated block, and the lesson free-text check
  src/error.rs                         error types
  src/frontmatter.rs                   Front matter tokens: the `---` fences, `key: value` lines, and quoting
  src/index.rs                         brain.db: search, links, backlinks, graph data
  src/lib.rs                           exports
  src/note.rs                          Note, NoteType, front matter, wikilink parsing
  src/query.rs                         Read side of the index: search, backlinks, the link graph, and pinned notes
  src/vault.rs                         read and write markdown files, atomic replace, generated-text markers
  src/watcher.rs                       watch the vault for outside edits
  tests/index.rs                       file
  tests/vault.rs                       round trip, outside edits, generated markers
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
  build.rs                             The daemon embeds the built UI from `crates/sentinel-ui/dist/`. On a fresh clone that folder
  src/access.rs                        Engine methods for getting at the daemon: Tailscale, pairing, devices, the install check,
  src/agents.rs                        Finding the agent CLIs on this computer, turning them into workers, and asking one of them
  src/ask.rs                           Questions the run loop puts to the user: approval cards and decisions. Each one waits for an
  src/assets.rs                        The web UI, embedded in the daemon. A path with no file gets the page, so the single page app
  src/attach.rs                        Attachments: what the composer plus button adds to a chat. A file, a folder, or a backlog is a
  src/attempt.rs                       One attempt at one task: choose an agent, run it, and either checkpoint its work or put the
  src/auth.rs                          allowlist, pairing code, one-time button tokens
  src/chats.rs                         Chats: create, list, rename, pin, archive, delete. A chat is one conversation in the project
  src/close_out.rs                     Closing a run: every task that has not reached an end gets one, the report is written, and the
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
  src/pumps.rs                         Two small background jobs of a run: the write guard's events become stream events, and the
  src/report_build.rs                  Build the honest report from what the store knows, and write `.sentinel/report.md`
  src/resume.rs                        Crash resume. A run that stopped without finishing keeps its tasks in the store. The next
  src/reviewer.rs                      Who reviews a task's diff. A different agent gives the real review. Without one (fake mode, or
  src/routes/access.rs                 Pairing, devices, Tailscale, and the folder pickers
  src/routes/chats.rs                  Chats and their attachments: the sidebar list, new chat, rename, pin, archive, delete, and the
  src/routes/mod.rs                    The HTTP routes. Each handler checks the caller, calls one Engine method, and returns JSON
  src/routes/run.rs                    Routes that start, steer, and answer a run
  src/routes/setup.rs                  Repository choice, the first-run steps, and the settings only the computer may change
  src/routes/state.rs                  Read routes and the settings that change what the daemon is allowed to use
  src/run.rs                           run loop tying planner, scheduler, workers, verifier
  src/run_ctx.rs                       Everything the tasks of one run share. A run owns one `RunCtx`; every wave and attempt gets an
  src/run_source.rs                    What a run starts from. A message in a chat can bring plain words, a backlog file, and
  src/secrets.rs                       Secrets for the daemon. Values live in the OS keychain, never on disk and never in logs
  src/server.rs                        Binds the daemon to the tailnet address and serves the API, the WebSocket, and the UI
  src/settings.rs                      settings queries
  src/tailscale.rs                     address lookup, bind rules, serve helper
  src/verify_phase.rs                  After the barrier: check each checkpointed task of the wave and give it a status. Only
  src/wave.rs                          One wave: reserve files, run the tasks together, wait at the barrier, run the full suite on a
  src/ws.rs                            The live stream on `GET /ws`. The first frame must authenticate. Then the client subscribes
  tests/api.rs                         The HTTP API, served on a real listener and called with `reqwest`. The WebSocket framing is
  tests/chats.rs                       Chats end to end with the fake agents: create, rename, pin, archive, delete, a run from plain
  tests/common/mod.rs                  Helpers the integration tests share: the repository root, a copy of the sample repository, git
  tests/engine_run.rs                  A whole run with the fake agents on the sample repository. It proves the harness end to end:
  tests/folders.rs                     Folder browser, repository checks, and starter backlog behavior
```

## crates/sentinel-cli

```
crates/sentinel-cli/
  Cargo.toml                           manifest
  src/commands.rs                      run, scan, pair, report, doctor
  src/main.rs                          entry
  src/serve.rs                         The commands that run or reach the daemon: `daemon`, `open`, `pair`, and `keys`
  tests/resume.rs                      Crash resume, end to end: start a run, kill the whole process with SIGKILL after the first
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
  assets/icons/apple-touch-icon.png    file
  assets/icons/icon-192.png            file
  assets/icons/icon-512.png            file
  assets/icons/icon-maskable-512.png   file
  assets/icons/icon.svg                file
  assets/sw.js                         file
  index.html                           shell page and manifest link
  manifest.webmanifest                 PWA manifest
  src/api.rs                           typed API client
  src/app.rs                           router and layout
  src/components/accent_picker.rs      `AccentPicker`: the six accent swatches, a custom color, and Reset to default
  src/components/agent_stack.rs        `AgentStack`: avatars of the ready agents in the top bar, each with a state dot. Opens the
  src/components/agent_step_card.rs    One agent's card for one attempt at one task. The steps are not in the events, so the card
  src/components/appearance_form.rs    The controls of Settings, Appearance, from theme down to corner radius. The density choices
  src/components/approval_card.rs      An approval card: an agent asks for one more file, or the user approves a wave or the plan
  src/components/barrier_card.rs       The barrier card: the full test suite on a clean tree after a wave
  src/components/blocked_write_card.rs A write to a file the task had not reserved. Sentinel blocks it and puts the file back
  src/components/board_column.rs       `BoardColumn`: one status group on the Board, with its heading and cards
  src/components/brain_chip.rs         The Brain chip: shows the Brain mode and opens the Brain menu
  src/components/budget_banner.rs      The budget banner: a warn notice at 70 percent of the cap, a danger notice at 95 percent
  src/components/budget_form.rs        The Agents and tools page, the Budget page, and the routing choices they share. The cap can only
  src/components/command_palette.rs    `CommandPalette`: Ctrl or Cmd + K. It draws its own overlay (`.pal`), not the modal frame,
  src/components/connection_banner.rs  `ConnectionBanner`: says when the page shows the last known state, not the live one
  src/components/connection_card.rs    The Connections page and the Devices page of Settings (`docs/product-scope.md`, F18). Each
  src/components/contrast_readout.rs   `ContrastReadout`: the live contrast check in Settings, Appearance. Each pair shows its ratio and
  src/components/cost_chart.rs         `CostChart`: spend over time as an SVG line, the cap as a dashed line, and a text table as the
  src/components/cost_meter.rs         `CostMeter`: the spend bar with marks at 70 and 95 percent of the cap
  src/components/css_export.rs         `CssExport`: the current tokens as one `:root` block, with a Copy button (`prefs::tokens_css`)
  src/components/decision_card.rs      A decision card: two tasks contradict each other, and only the user can choose which goes on
  src/components/device_list.rs        `DeviceList`: paired devices, each with a revoke action that asks first
  src/components/dry_run_stages.rs     `DryRunStages`: the six stages the dry run goes through, shown as a static preview. The daemon
  src/components/empty_run_state.rs    The empty stream: what the run does, and the three first actions
  src/components/file_change_card.rs   A file checkpoint: one changed path and the short commit that holds it
  src/components/font_select.rs        `FontSelect`: a dropdown of the font choices in `prefs::UI_FONTS` or `prefs::MONO_FONTS`. The
  src/components/guide_modal.rs        `GuideModal`: the thirteen-step guide. `ModalHost` draws the dialog frame. This is its body
  src/components/icon.rs               `Icon`: one inline SVG from the outline set, sized in `em` so it scales with the text size
  src/components/install_checklist.rs  `InstallChecklist`: the install check rows, each found or missing, with the command that
  src/components/install_log.rs        `InstallLog`: the terminal-style box that shows the install commands and what to do with
  src/components/kill_button.rs        `KillButton`: stops one task after a confirm that names the files it releases
  src/components/menus.rs              Menu bodies for the one popover. Each entry calls into `Ui` or the daemon API, then closes
  src/components/message_cards.rs      Plain stream lines: the user's instruction, a Sentinel line, and a muted note with an icon
  src/components/mod.rs                One file per component in `docs/ui-registry.md`
  src/components/modal.rs              `ModalHost`: the overlay frame for settings, the guide, the palette, and the bypass confirm
  src/components/onboarding_stepper.rs `OnboardingStepper`: the six step names across the top of the setup wizard, and the rules
  src/components/output_view.rs        `OutputView`: monospace agent output. It follows the newest line while the reader is at the
  src/components/pair_checklist.rs     `PairChecklist`: Part 1 of pairing. Tailscale must be installed, running, and signed in, with
  src/components/pair_qr.rs            `PairQr`: the pairing link as a QR code. The daemon sends the SVG markup
  src/components/pair_steps.rs         `PairSteps`: Part 2 of pairing. The phone steps, the QR code and the one-time code with its
  src/components/panel_host.rs         `PanelHost`: the right panel. Its tab strip lists the open panels, and its body shows the
  src/components/permission_chip.rs    The permission chip: shows the mode and opens the mode menu. Bypass uses the warn color
  src/components/permission_matrix.rs  `PermissionMatrix`: one row per enabled agent, one switch per capability
  src/components/permission_mode_list.rs `PermissionModeList`: the four permission mode cards (`docs/ui-registry.md`, Permissions)
  src/components/plan_card.rs          The plan card: the plan's counts, and the task table from the snapshot
  src/components/popover.rs            The one popover of the page. It sits under its anchor, flips above when there is no room
  src/components/range_field.rs        `RangeField`: a slider with its value in pixels, used for text size, stream width, and corner
  src/components/receipt_block.rs      `ReceiptBlock`: the commit that proves a VERIFIED task (`docs/ui-registry.md`, Board and Tasks)
  src/components/repo_checks.rs        `RepoChecks`: the rows of a check (the repository's git, backlog.md, and clean tree, or the
  src/components/report_card.rs        The report card: counts by status from the snapshot, and buttons to the Report and the Brain
  src/components/reserved_row.rs       `ReservedRow`: one file path, the task that holds it, and the queue of waiting tasks
  src/components/sidebar.rs            `Sidebar`: the run list grouped by repository, the New run button, search, and the footer
  src/components/slash_menu.rs         The command list under the composer. The arrow keys and Tab are handled by the composer, which
  src/components/status_badge.rs       `StatusBadge`: a shape, a color, and a label together, so state is never color alone
  src/components/tab_bar.rs            `TabBar`: the phone tabs Stream, Board, Reserved, Changes, More, and the swipe between them
  src/components/target_chip.rs        The send-to chip: shows where the next message goes, and opens the target menu
  src/components/task_card.rs          `TaskCard`: one task on the Board. Click opens the detail inline under the card
  src/components/toast.rs              `Toast`: one short result message, about three seconds, centered above the composer
  src/components/toggle.rs             `Toggle` and `Segmented`: the two small choice controls of Settings. Both use the prototype's
  src/components/token_editor.rs       `TokenEditor`: one color input per theme token in Settings, Appearance. Each edit is kept in
  src/components/tool_row.rs           `ToolRow`: one detected worker CLI with its status, path, version, enable switch, and a fix
  src/components/top_bar.rs            `TopBar`: the run title, agent stack, cost chip, run controls, and panel icons
  src/components/tour_coach.rs         `TourCoach`: the highlight ring and the coach mark for the current stop. It draws nothing
  src/components/tour_menu.rs          `TourMenu`: shows how far the tour got, and replays or skips it. Settings uses it
  src/components/verdict_card.rs       A verdict card: the task, its status, the reason, and the quality notes for the task
  src/components/wave_card.rs          The wave card: the tasks that run together, each with the agent it was routed to
  src/fonts.css                        styles
  src/guide.rs                         The guide's thirteen steps and the Try it buttons under each one. The copy is the prototype's
  src/icons.rs                         Icon paths, ported from the prototype's `ICONS` table. One outline set, 1.7 px stroke
  src/layout.rs                        breakpoints and panel open state
  src/main.rs                          mount
  src/palette.rs                       Command palette items and the search over them. The overlay is `components/command_palette.rs`
  src/prefs.rs                         appearance preferences and token overrides, stored per device
  src/slash.rs                         Slash commands. One parser and one list. Each command maps to one API route in `composer.rs`,
  src/state.rs                         client state from snapshot and events
  src/theme.css                        CSS variables from ui-tokens
  src/theme.rs                         Applies appearance prefs to the page root: the `data-theme` attribute and the CSS variables
  src/tour.rs                          The tour's seven stops, their copy, and the math that places a coach mark. Reading the DOM
  src/tour_state.rs                    The tour's state: which stop shows, where its highlight and coach mark go, and the seen flag
  src/views/agents.rs                  The Agents panel: detected worker CLIs, and which agent serves each tier
  src/views/appearance.rs              The Appearance page of Settings. Every control writes to `ui.prefs`. The app saves the prefs
  src/views/board.rs                   The Board panel: tasks grouped by status (`docs/ui-rules.md`, Views)
  src/views/budget_step.rs             The budget step of the setup wizard: the cap, the permission mode, the Brain mode, and which
  src/views/changes.rs                 The Changes panel: checkpoint commits, blocked writes, and quality notes, from the event log
  src/views/composer.rs                The composer: the text box, the slash menu, the footer chips, and what each send does. A plain
  src/views/cost.rs                    The Cost panel: spend against the cap, the 70 and 95 percent marks, per-agent cost, spend over
  src/views/dry_run.rs                 The dry run step of the setup wizard (`docs/product-scope.md`, F8 step 5). It says what the
  src/views/files.rs                   The Files panel: the repository files with reserved-file markers and a filter
  src/views/folder_picker.rs           The repository step: choose the project folder, check it, and fix what the check finds
  src/views/install.rs                 The install check and the agents step of the setup wizard (`docs/product-scope.md`, F8 and
  src/views/mod.rs                     Views that hang off the shell. Panels and modal bodies land here one at a time; until a
  src/views/onboarding.rs              `SetupWizard`: the full-screen setup (prototype `renderWizard`, `wizNav`, `finishSetup`). The
  src/views/pair_page.rs               The page a phone opens from the QR code or the link `/pair`. It asks for the one-time code and
  src/views/pairing.rs                 The phone step of the setup wizard (`docs/product-scope.md`, F8 step 4). It checks Tailscale
  src/views/panel_stubs.rs             Temporary panel bodies. The reviewer replaces each one with its real panel
  src/views/permissions.rs             The Permissions panel: the four modes and the per-agent capability matrix
  src/views/remote.rs                  The Remote panel: Tailscale state, pairing a phone, paired devices, and what a phone can do
  src/views/report.rs                  The Report panel: the honest per-task report, reasons for tasks not done, and an export
  src/views/reserved.rs                The Reserved panel: which task holds each file, and who waits (`docs/ui-rules.md`, Views)
  src/views/settings.rs                The Settings modal (`docs/ui-rules.md`, Views: Settings). The overlay frame is
  src/views/settings_pages.rs          Settings pages: General (repository and wake lock) and Brain. The other pages live in
  src/views/stream.rs                  The stream: the run's items in order, built from the snapshot. The view follows the newest
  src/views/stream_model.rs            Turns the daemon's events into stream items. Pure: no signals, no DOM, so every mapping has a
  src/views/telegram_slot.rs           Empty slots in Settings, Integrations. The Telegram card and the always-on host card are built
  src/views/terminal.rs                The Terminal panel: raw output per agent, with a switcher and a load-earlier action
  src/ws.rs                            The live socket. It keeps the page in step with the daemon: events go into the snapshot, and
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
