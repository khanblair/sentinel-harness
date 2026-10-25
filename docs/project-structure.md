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
  README.md               what Sentinel is and how to try it
  install.sh              one-line installer for the latest macOS release
  Cargo.lock              pinned dependency versions, committed
  rustfmt.toml            format rules
  clippy.toml             lint thresholds
  .cargo/config.toml      cargo alias for xtask
  .config/nextest.toml    test runner timeouts
  deny.toml               license rules and banned crates
  .gitignore              ignores target, .sentinel, dist
  LICENSE                 MIT license text
  docs/                   the 13 project docs
  design/                 interactive prototype and the how-to site wireframe
  site/                   the GitHub Pages how-to site (mdBook project)
  .github/                workflows, including the Pages deploy
  xtask/                  repo automation (checks, build helpers)
  packaging/              the macOS app bundle and disk image for releases
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
  img/                    images the site needs that the prototype cannot show
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
  src/api_projects.rs                  Project, data-reset, and key bodies of the HTTP API. `api.rs` re-exports them, so each one keeps
  src/brain_api.rs                     Request and response bodies of the Brain routes (`docs/architecture.md`, API). The daemon
  src/chat.rs                          Chats and attachments (`docs/product-scope.md`, F23). A chat is one conversation in a project
  src/context.rs                       retry prompt builder, failure summary, output truncation, loop compaction
  src/cost.rs                          Cost in micro-dollars, estimate flag
  src/dry_run.rs                       The dry run: a short rehearsal on a sample project that ships with Sentinel
  src/error.rs                         error types
  src/event.rs                         Event enum shared by daemon and UI
  src/ids.rs                           TaskId, RunId, ReservationId, DeviceId
  src/lib.rs                           module exports
  src/note.rs                          NoteType, LessonState, BrainMode, ProposalKind
  src/permission.rs                    PermissionMode, Capability, Approval
  src/quality.rs                       QualityNote
  src/reservation.rs                   Reservation, ReservationState
  src/routines.rs                      Routines: jobs that run at a set time, such as the Morning brief (`docs/product-scope.md`, F25)
  src/scope.rs                         Scope and path set math (overlap, contains)
  src/skills.rs                        Skills: folders with a `SKILL.md` that teach agents one thing (`docs/product-scope.md`, F24)
  src/snapshot.rs                      The full state the daemon sends to a device on connect (`GET /api/state`). The UI builds every
  src/state.rs                         HarnessState, TailscaleState, ConnectionState, Decision
  src/task.rs                          Task, TaskStatus, Tier
  src/telegram_api.rs                  Telegram settings as the API and the UI see them. The bot token is never in these types
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
  src/clear.rs                         Clear the rows of one data scope. Each function runs in one transaction, so a failure leaves the
  src/decisions.rs                     Decision prompt persistence. A decision is the contradiction question from
  src/devices.rs                       paired device queries
  src/events.rs                        event append and read
  src/lib.rs                           Store handle
  src/migrations/0001_init.sql         first schema
  src/migrations/0002_extras.sql       schema
  src/migrations/0003_chats.sql        schema
  src/migrations/0004_skills_routines.sql schema
  src/quality.rs                       QualityNote
  src/reservations.rs                  reservation queries
  src/reset.rs                         Clear what belongs to one run, so the next run starts from an empty board. Events, runs, cost
  src/routines.rs                      Routines: jobs that run at a set time, such as the Morning brief (`docs/product-scope.md`, F25)
  src/runs.rs                          run and cost queries
  src/settings.rs                      settings queries
  src/skills.rs                        Skills: folders with a `SKILL.md` that teach agents one thing (`docs/product-scope.md`, F24)
  src/tasks.rs                         task queries
  src/telegram.rs                      The Telegram chat row. Sentinel keeps one private chat. The four level switches are four integer
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
  src/agent_schema.rs                  The function definitions the `OpenRouter` worker sends with every request
  src/agent_tools.rs                   The tools the `OpenRouter` worker offers its model, and the executor that runs them
  src/budget.rs                        budget wrapper, per-task ceiling, kill signal
  src/capture.rs                       Reads a worker's stdout and stderr line by line, keeps a bounded tail, and drains the
  src/context.rs                       retry prompt builder, failure summary, output truncation, loop compaction
  src/detect.rs                        PATH scan, version probe, login check
  src/lib.rs                           exports
  src/openrouter.rs                    The `OpenRouter` worker: Sentinel's own agent loop for models reached through `OpenRouter`
  src/openrouter_config.rs             Settings and errors of the `OpenRouter` worker
  src/profile.rs                       adapter profile loader (TOML)
  src/router.rs                        tier selection and escalation
  src/supervisor.rs                    spawn, timeout, kill, output capture
  src/transcript.rs                    The messages of the `OpenRouter` loop, and their compaction. Summaries are built here from the
  src/worker.rs                        Worker trait
  tests/detect.rs                      detection tests with fake binaries
  tests/openrouter.rs                  The `OpenRouter` worker against a fake API (`wiremock`), and the tool executor on its own
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
  src/journal.rs                       Run journal: run, task, and file notes. Pure builders. The daemon writes the notes
  src/lesson.rs                        Lessons: typed notes that record what helped or hurt an agent. A lesson holds no free text
  src/lib.rs                           exports
  src/mode.rs                          Brain modes: what the Brain may read and write, and who may move it
  src/note.rs                          Note, NoteType, front matter, wikilink parsing
  src/proposal.rs                      Proposals: changes to notes, saved as files in `_proposals/`. A change to a note you wrote needs an accept
  src/query.rs                         Read side of the index: search, backlinks, the link graph, and pinned notes
  src/schedule.rs                      Care schedule: when a care run is due. The caller passes today's date, so tests use a fake clock
  src/select.rs                        Agent memory: pick the Brain notes for one task within a token cap
  src/strength.rs                      Note strength: how much a note is used and trusted. Weak notes load last
  src/vault.rs                         read and write markdown files, atomic replace, generated-text markers
  src/watcher.rs                       watch the vault for outside edits
  tests/brain_logic.rs                 Brain logic on a real vault and index: selection, lessons, journal links, and proposals
  tests/index.rs                       file
  tests/vault.rs                       round trip, outside edits, generated markers
```

## crates/sentinel-telegram

```
crates/sentinel-telegram/
  Cargo.toml                           manifest
  src/api.rs                           typed API client
  src/buttons.rs                       One-time button tokens. A button carries only a token. The token maps to one action, one chat,
  src/client.rs                        Bot API calls over HTTPS. Each method is one request and returns one typed result
  src/command.rs                       Slash commands from the chat. A command maps to one route. Parsing never runs anything
  src/error.rs                         error types
  src/format.rs                        message text at three detail levels
  src/format/wave.rs                   The wave summary. `WaveTracker` groups the verdicts of one wave into one message
  src/levels.rs                        Chat levels and the routes each level may call. A route that no level allows is refused
  src/lib.rs                           exports
  src/outbox.rs                        queue, retries, batching, live-message edits
  src/quiet.rs                         Quiet hours hold ordinary messages. Approvals and budget alerts always pass
  src/redact.rs                        Replaces key-like text with `[hidden]` before any text goes to Telegram
  tests/buttons.rs                     file
  tests/client.rs                      file
  tests/command.rs                     file
  tests/format.rs                      Summary never holds file contents, keys, or note text
  tests/outbox.rs                      file
```

## crates/sentinel-daemon

```
crates/sentinel-daemon/
  Cargo.toml                           manifest
  build.rs                             The daemon embeds the built UI from `crates/sentinel-ui/dist/`. On a fresh clone that folder
  src/access.rs                        Engine methods for getting at the daemon: Tailscale, pairing, devices, the install check,
  src/agent_test.rs                    Test one agent CLI for real: one tiny headless prompt through the same worker a task uses, in an
  src/agents.rs                        Finding the agent CLIs on this computer, turning them into workers, and asking one of them
  src/agents_config.rs                 Pure helpers that pick the tools and the routing from one scan. They take no engine, so the
  src/ask.rs                           Questions the run loop puts to the user: approval cards and decisions. Each one waits for an
  src/assets.rs                        The web UI, embedded in the daemon. A path with no file gets the page, so the single page app
  src/attach.rs                        Attachments: what the composer plus button adds to a chat. A file, a folder, or a backlog is a
  src/attempt.rs                       One attempt at one task: choose an agent, run it, and either checkpoint its work or put the
  src/auth.rs                          allowlist, pairing code, one-time button tokens
  src/brain.rs                         The Brain in the daemon. The vault and the index open on first use. A watcher keeps the index in
  src/brain_care.rs                    The care schedule and the care run. The run calls no agent. It lowers the strength of unused
  src/brain_edit.rs                    Changes a person makes in the Brain panel. These ignore the Brain mode, which limits only what
  src/brain_notes.rs                   Reading the Brain: the notes list, one note with its links, and the graph
  src/brain_proposals.rs               Proposals from care runs and their answers. A proposal file in `_proposals/` waits until you
  src/brain_run.rs                     The Brain during a run: the notes an agent reads before a task, and the journal a run writes
  src/brain_seed.rs                    The seed: notes that describe the project on disk. It reads the repository root and runs no
  src/brain_survey.rs                  The survey: a walk of the repository root that lists its files, its area folders, and its
  src/briefs.rs                        The facts a brief reports, and the plain words it is written in. The text builders take counts,
  src/changes.rs                       Live project state for the Changes and Board panels: the git working tree, the diff of one
  src/changes/diff.rs                  The diff of one file against HEAD, for the Changes panel. An untracked file shows every line as
  src/chats.rs                         Chats: create, list, rename, pin, archive, delete. A chat is one conversation in the project
  src/close_out.rs                     Closing a run: every task that has not reached an end gets one, the report is written, and the
  src/cmd.rs                           Run external programs behind one trait
  src/config.rs                        `<repo>/.sentinel/config.toml`: settings and routing that onboarding writes
  src/connections.rs                   connection state and test actions (Tailscale, OpenRouter, MCP, git, keychain)
  src/context_usage.rs                 The Context panel's numbers: what the next task would put in the window of the agent routed for
  src/controls.rs                      Engine methods that change settings and answer questions. Each one says who did it, because a
  src/data_clear.rs                    Clear the data of one scope, so the app can start again from the setup wizard. Every call
  src/doctor.rs                        install check used by onboarding and `sentinel doctor`
  src/dry_run.rs                       The dry run: a rehearsal on a sample project that ships with Sentinel. A second engine runs the
  src/dry_run_book.rs                  What the dry run shows: each stage's state, the counts at the end, and the notes the wizard
  src/dry_run_preflight.rs             The agent check before a dry run. Each agent the dry run may use is tested once, with the same
  src/engine.rs                        The engine: one handle that owns the state of a Sentinel daemon. The HTTP layer, the CLI, and the
  src/folders.rs                       native folder dialog, phone folder browser, repo checks
  src/folders_git.rs                   The git half of the repository check: the clean working tree row, and the commit that takes
  src/folders_tree.rs                  The repository tree that the folder browser lists. `folders.rs` re-exports `repo_tree` and
  src/hashing.rs                       Device tokens: generation, hashing, and comparison
  src/host.rs                          The always-on host (`docs/product-scope.md`, F21). The daemon writes a heartbeat every few
  src/hub.rs                           Many projects in one app. Each project is a folder with its own engine and its own `.sentinel`
  src/hub_error.rs                     The errors of the project list. `hub.rs` re-exports `HubError`, so `hub::HubError` keeps working
  src/keys.rs                          Keys for the model providers: the provider table, the rows the UI shows, the live check of a
  src/lib.rs                           exports
  src/live.rs                          What the engine keeps while a run is live: control channels, the budget, and the questions the
  src/onboarding.rs                    onboarding state and steps
  src/openrouter_tool.rs               The `OpenRouter` worker offered as a tool: its entry in the tool list and the worker built for
  src/pairing.rs                       QR payload and one-time code
  src/plan_phase.rs                    The first phase of a run: read the backlog, plan it, settle contradictions, and mark which
  src/prompt.rs                        The text each worker gets. Rules go in the prompt as guidance; the guard enforces the real
  src/pumps.rs                         Two small background jobs of a run: the write guard's events become stream events, and the
  src/quit.rs                          A way to stop the daemon from the UI. The app has no terminal, so Ctrl+C is not there
  src/report_build.rs                  Build the honest report from what the store knows, and write `.sentinel/report.md`
  src/resume.rs                        Crash resume. A run that stopped without finishing keeps its tasks in the store. The next
  src/reviewer.rs                      Who reviews a task's diff. A different agent gives the real review. Without one (fake mode, or
  src/routes/access.rs                 Pairing, devices, Tailscale, and the folder pickers
  src/routes/brain.rs                  The Brain routes under `/api/brain`. A phone may read notes and the graph, review lessons, and
  src/routes/changes.rs                Live project state: the working tree, one file's diff, and the backlog tasks. Any device
  src/routes/chats.rs                  Chats and their attachments: the sidebar list, new chat, rename, pin, archive, delete, and the
  src/routes/clearing.rs               Clear data by scope. The project route clears the project the request names. The app and
  src/routes/dry_run.rs                The dry run routes. Only the computer starts one. Any device may read the last one, so a page
  src/routes/keys.rs                   The model provider keys: one row per provider, a save, a delete, and a test of the saved key
  src/routes/mod.rs                    The HTTP routes. Each handler checks the caller, calls one Engine method, and returns JSON
  src/routes/routines.rs               Routines and briefs. The computer makes and deletes routines. A phone may turn one off and press
  src/routes/run.rs                    Routes that start, steer, and answer a run
  src/routes/setup.rs                  Repository choice, the first-run steps, and the settings only the computer may change
  src/routes/skills.rs                 The Skills routes under `/api/skills`. A phone may list and read skills, see the found ones,
  src/routes/state.rs                  Read routes and the settings that change what the daemon is allowed to use
  src/routes/telegram.rs               The Telegram routes under `/api/telegram`. A phone may read the state. Saving the token, the
  src/routines.rs                      Routines: jobs that run at a set time, such as the Morning brief (`docs/product-scope.md`, F25)
  src/routines_tick.rs                 The scheduler. Every 30 seconds it finds the routines due in local time, marks each one fired,
  src/run.rs                           run loop tying planner, scheduler, workers, verifier
  src/run_ctx.rs                       Everything the tasks of one run share. A run owns one `RunCtx`; every wave and attempt gets an
  src/run_source.rs                    What a run starts from. A message in a chat can bring plain words, a backlog file, and
  src/secrets.rs                       Secrets for the daemon. Values live in the OS keychain, never on disk and never in logs
  src/server.rs                        Binds the daemon to the tailnet address and serves the API, the WebSocket, and the UI
  src/settings.rs                      settings queries
  src/skills.rs                        Skills: folders with a `SKILL.md` that teach agents one thing (`docs/product-scope.md`, F24)
  src/skills_found.rs                  Skills that other tools keep in their own folders, offered for import (`docs/product-scope.md`,
  src/skills_pick.rs                   Which skills a task gets, and the text they add to its prompt. Always skills come first, then
  src/starter.rs                       A small starter project to try Sentinel on: a Python greeting with a unittest test and a backlog
  src/tailscale.rs                     address lookup, bind rules, serve helper
  src/telegram.rs                      Telegram inside the daemon. The `sentinel-telegram` crate holds the Bot API client, the parser,
  src/telegram_code.rs                 The phone commands that read or write the repository: `/task` adds a section to `backlog.md`,
  src/telegram_notify.rs               The notifier turns bus events into queued messages. The sender drains the queue to the Bot API
  src/telegram_poll.rs                 The long-poll loop. It fetches updates, drops the ones from anyone but the owner, and hands the
  src/telegram_router.rs               The router turns one chat command or one button press into one Engine call, made as the phone
  src/telegram_store.rs                The Telegram chat row and the paired flag, read and written by the daemon. The store keeps
  src/verify_phase.rs                  After the barrier: check each checkpointed task of the wave and give it a status. Only
  src/wave.rs                          One wave: reserve files, run the tasks together, wait at the barrier, run the full suite on a
  src/ws.rs                            The live stream on `GET /ws`. The first frame must authenticate. Then the client subscribes
  tests/agent_test.rs                  The agent test route over HTTP. The answers that need no agent program are checked here: an
  tests/api.rs                         The HTTP API, served on a real listener and called with `reqwest`. The WebSocket framing is
  tests/brain_api.rs                   The Brain API, served on a real listener and called with `reqwest`. Notes are written through the
  tests/brain_api/support.rs           Helpers for the Brain API tests: an engine on a temporary repository, a listener, requests with a
  tests/brain_run.rs                   The Brain during a run: pinned notes reach the stream as "loaded", and the journal is written
  tests/chats.rs                       Chats end to end with the fake agents: create, rename, pin, archive, delete, a run from plain
  tests/common/mod.rs                  Helpers the integration tests share: the repository root, a copy of the sample repository, git
  tests/data_clear.rs                  Clearing data by scope through the hub: a phone is refused, the desktop clears chats and the
  tests/dry_run.rs                     The dry run on the built-in sample project, over the real HTTP routes. The fake agents run the
  tests/engine_run.rs                  A whole run with the fake agents on the sample repository. It proves the harness end to end:
  tests/folders.rs                     Folder browser, repository checks, and starter backlog behavior
  tests/host.rs                        The always-on host: the heartbeat, the back-online gap, and the host card
  tests/hub.rs                         The hub: many projects behind one server. The first project keeps the paired phones, the keys,
  tests/keys.rs                        The provider key routes over HTTP: the rows, a save, a delete, the refusals, and a test with no
  tests/openrouter_tool.rs             The `OpenRouter` worker offered as a tool in a real-mode engine: its entry after a scan, and a
  tests/routines.rs                    Routines end to end: the two built-in routines, who may change what, the input rules, and the
  tests/security.rs                    The route access matrix. Every route answers the right way to a caller with no token, a paired
  tests/security/coverage.rs           Reads the route source at test time. A `.route(` call with no row in `ROUTES` fails the build,
  tests/security/harness.rs            Starts the daemon for one test and sends it requests. A test gets its own engine, so a row
  tests/security/table.rs              Every route of the daemon and who may call it. Add a row for each new `.route(` line; the
  tests/skills.rs                      Skills on disk, end to end through the engine: create, list, read, switches, and the phone
  tests/telegram.rs                    Telegram inside the daemon, against a `wiremock` Bot API. The tests drive the real routes with a
  tests/telegram_code.rs               The phone commands `/task`, `/diff`, and `/daily`, against a `wiremock` Bot API. The rig is a
```

## crates/sentinel-cli

```
crates/sentinel-cli/
  Cargo.toml                           manifest
  src/app.rs                           router and layout
  src/commands.rs                      run, scan, pair, report, doctor
  src/doctor_host.rs                   The host checks of `sentinel doctor` (P14-06, `docs/product-scope.md` F21). Each row says
  src/login_path.rs                    Finds the agent CLIs when the Mac app starts from Finder, Spotlight, or the Dock. Those
  src/main.rs                          entry
  src/projects.rs                      The projects the app serves besides the first one. The first project starts the host loop, the
  src/serve.rs                         The commands that run or reach the daemon: `daemon`, `open`, `pair`, and `keys`
  src/service.rs                       `sentinel service`: run the daemon at boot and restart it when it stops. A launchd job on
  src/window.rs                        The macOS app window: the same local UI, served by the same daemon, inside a native window
  src/window_menu.rs                   The app menu bar. The web view needs it for the standard clipboard shortcuts to work
  src/writeup.rs                       The one-page write-up of the last run (P10-04, `sentinel report --writeup`). It reads the
  tests/resume.rs                      Crash resume, end to end: start a run, kill the whole process with SIGKILL after the first
  tests/writeup.rs                     The write-up draft, built from a fixed report and a fixed run log
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
  src/brain.rs                         Brain panel logic: link parsing, strength classes, the phone rule, the graph layout, and the care
  src/brain_text.rs                    Brain panel copy: labels, button words, and the lines the panel shows after an action. Plain
  src/chats.rs                         Chats: the conversations of a project. The engine adds the run state to each row
  src/chats/menu.rs                    The lines of a chat menu: what each line does, and which lines are disabled and why. The menu
  src/chats/tests.rs                   file
  src/components/accent_picker.rs      `AccentPicker`: the six accent swatches, a custom color, and Reset to default
  src/components/add_project.rs        `AddProjectButton` and the project dialogs: the New project flow, the folder browser for a host
  src/components/agent_stack.rs        `AgentStack`: avatars of the ready agents in the top bar, each with a state dot. Opens the
  src/components/agent_step_card.rs    One agent's card for one attempt at one task. The steps are not in the events, so the card
  src/components/agent_test_button.rs  The Test button of an agent row and the line that shows its answer. A test sends one tiny
  src/components/appearance_form.rs    The controls of Settings, Appearance, from theme down to corner radius. The density choices
  src/components/approval_card.rs      An approval card: an agent asks for one more file, or the user approves a wave or the plan
  src/components/attach_menu.rs        The entries of the composer's plus menu. Each entry opens one add dialog
  src/components/attachment_chip.rs    `AttachmentChip`: one pending attachment of the open chat, and the `Chip` markup it uses
  src/components/barrier_card.rs       The barrier card: the full test suite on a clean tree after a wave
  src/components/blocked_write_card.rs A write to a file the task had not reserved. Sentinel blocks it and puts the file back
  src/components/board_column.rs       `BoardColumn`: one status group on the Board, with its heading and cards
  src/components/brain_chip.rs         The Brain chip: shows the Brain mode and opens the Brain menu
  src/components/brief_card.rs         `BriefCard`: one brief in "Latest briefs" on the Routines page. The body keeps its line breaks
  src/components/budget_banner.rs      The budget banner: a warn notice at 70 percent of the cap, a danger notice at 95 percent
  src/components/budget_form.rs        The Agents and tools page, the Budget page, and the routing choices they share. The cap can only
  src/components/care_settings.rs      `CareSettings`: the care schedule, the budget per run, the switch that lets care apply changes
  src/components/chat_menu.rs          `ChatMenu`: the menu of one chat, used by the chat row and the top bar title
  src/components/chat_row.rs           `ChatRow`: one chat in the sidebar (`docs/ui-rules.md`, Layout). Its more button and a long
  src/components/command_palette.rs    `CommandPalette`: Ctrl or Cmd + K. It draws its own overlay (`.pal`), not the modal frame,
  src/components/confirm_clear.rs      `ConfirmClear`: asks before a data reset runs. The reset that clears everything needs the owner
  src/components/confirm_delete.rs     `ConfirmDelete`: asks before a chat, its messages and its uploads are removed. The repository
  src/components/connection_banner.rs  `ConnectionBanner`: says when the page shows the last known state, not the live one
  src/components/connection_card.rs    The Connections page and the Devices page of Settings (`docs/product-scope.md`, F18). Each
  src/components/contrast_readout.rs   `ContrastReadout`: the live contrast check in Settings, Appearance. Each pair shows its ratio and
  src/components/cost_chart.rs         `CostChart`: spend over time as an SVG line, the cap as a dashed line, and a text table as the
  src/components/cost_meter.rs         `CostMeter`: the spend bar with marks at 70 and 95 percent of the cap
  src/components/css_export.rs         `CssExport`: the current tokens as one `:root` block, with a Copy button (`prefs::tokens_css`)
  src/components/decision_card.rs      A decision card: two tasks contradict each other, and only the user can choose which goes on
  src/components/device_file.rs        Files chosen on this device. Each file is read with `FileReader` and sent as base64. A file over
  src/components/device_list.rs        `DeviceList`: paired devices, each with a revoke action that asks first
  src/components/diff_view.rs          `DiffView`: a unified diff in a `<pre>`. Added lines show in the ok color, removed lines in the
  src/components/dry_run_stages.rs     `DryRunStages`: the six stages the dry run goes through, shown as a static preview. The daemon
  src/components/empty_run_state.rs    The empty stream: what the run does, and the three first actions
  src/components/file_change_card.rs   A file checkpoint: one changed path and the short commit that holds it
  src/components/folder_browser.rs     `FolderBrowser`: the body of "Add a folder". It walks the project one folder at a time with
  src/components/font_select.rs        `FontSelect`: a dropdown of the font choices in `prefs::UI_FONTS` or `prefs::MONO_FONTS`. The
  src/components/graph_view.rs         `GraphView`: one dot per note, grouped by type, with a line for each link. Tapping a dot opens
  src/components/guide_modal.rs        `GuideModal`: the thirteen-step guide. `ModalHost` draws the dialog frame. This is its body
  src/components/host_card.rs          The "Where Sentinel runs" card in Settings, Integrations. It shows what the daemon reports about
  src/components/icon.rs               `Icon`: one inline SVG from the outline set, sized in `em` so it scales with the text size
  src/components/install_checklist.rs  `InstallChecklist`: the install check rows, each found or missing, with the command that
  src/components/install_log.rs        `InstallLog`: the terminal-style box that shows the install commands and what to do with
  src/components/key_card.rs           `KeyCard`: one provider's row in the API keys section. The typed text lives only in the
  src/components/keys_section.rs       `KeysSection`: the bring-your-own-key block on the Connections page. The daemon keeps each key
  src/components/kill_button.rs        `KillButton`: stops one task after a confirm that names the files it releases
  src/components/lesson_card.rs        `LessonCard`: the typed fields of a lesson with Approve, Reject, and Retire. A phone may review
  src/components/menus.rs              Menu bodies for the one popover. Each entry calls into `Ui` or the daemon API, then closes
  src/components/message_cards.rs      Plain stream lines: the user's instruction, a Sentinel line, and a muted note with an icon
  src/components/mod.rs                One file per component in `docs/ui-registry.md`
  src/components/modal.rs              `ModalHost`: the overlay frame for settings, the guide, the palette, and the bypass confirm
  src/components/note_dialog.rs        `NoteDialog`: a context note that goes into every agent prompt of the chat
  src/components/note_editor.rs        `NoteEditor`: the text box that replaces a note's text while you edit it. Save sends the text
  src/components/note_row.rs           `NoteRow`: one line of the notes list. The type, the title, a pin chip, and either the strength
  src/components/note_view.rs          `NoteView`: one note. Its text with links, the lesson card for a lesson, Edit and pin on the
  src/components/onboarding_stepper.rs `OnboardingStepper`: the six step names across the top of the setup wizard, and the rules
  src/components/output_view.rs        `OutputView`: monospace agent output. It follows the newest line while the reader is at the
  src/components/pair_checklist.rs     `PairChecklist`: Part 1 of pairing. Tailscale must be installed, running, and signed in, with
  src/components/pair_phone_card.rs    `PairPhoneCard`: everything about pairing a phone. Settings, Devices, and the Remote panel all
  src/components/pair_qr.rs            `PairQr`: the pairing link as a QR code. The daemon sends the SVG markup
  src/components/pair_steps.rs         `PairSteps`: Part 2 of pairing. The phone steps, the QR code and the one-time code with its
  src/components/panel_host.rs         `PanelHost`: the right panel. Its tab strip lists the open panels, and its body shows the
  src/components/permission_chip.rs    The permission chip: shows the mode and opens the mode menu. Bypass uses the warn color
  src/components/permission_matrix.rs  `PermissionMatrix`: one row per enabled agent, one switch per capability
  src/components/permission_mode_list.rs `PermissionModeList`: the four permission mode cards (`docs/ui-registry.md`, Permissions)
  src/components/plan_card.rs          The plan card: the plan's counts, and the task table from the snapshot
  src/components/popover.rs            The one popover of the page. It sits under its anchor, flips above when there is no room
  src/components/project_group.rs      `ProjectGroup`: one project in the sidebar, with its header, its chats, and its archived chats
  src/components/proposal_list.rs      `ProposalList`: one card per proposal from a care run, with Accept and Dismiss. Nothing changes
  src/components/range_field.rs        `RangeField`: a slider with its value in pixels, used for text size, stream width, and corner
  src/components/receipt_block.rs      `ReceiptBlock`: the commit that proves a VERIFIED task (`docs/ui-registry.md`, Board and Tasks)
  src/components/repo_checks.rs        `RepoChecks`: the rows of a check (the repository's git, backlog.md, and clean tree, or the
  src/components/repo_file_picker.rs   `PickerDialog`: the add dialogs for files, a folder, and a backlog file. The "This project" tab
  src/components/report_card.rs        The report card: counts by status from the snapshot, and buttons to the Report and the Brain
  src/components/reserved_row.rs       `ReservedRow`: one file path, the task that holds it, and the queue of waiting tasks
  src/components/reset_section.rs      `ResetSection`: the Reset block at the end of General. Each row clears one kind of data, and the
  src/components/resize_handle.rs      Drag handles on the desktop layout: one on the sidebar edge, one on the panel edge. The widths
  src/components/routine_editor.rs     `RoutineEditor`: the dialog body that adds a reminder or edits one (`docs/product-scope.md`,
  src/components/routine_row.rs        `RoutineRow`: one routine on the Routines page (`docs/product-scope.md`, F25). On a phone a
  src/components/sidebar.rs            `Sidebar`: the run list grouped by repository, the New run button, search, and the footer
  src/components/skill_editor.rs       `SkillEditor`: the New skill and Edit form (`docs/ui-registry.md`, `SkillEditor`). It sits on the
  src/components/skill_import_list.rs  `SkillImportList`: the skills found in other tools' folders that Sentinel does not have yet
  src/components/skill_picker.rs       `SkillPicker`: the Use a skill dialog from the plus menu. It lists the skills Sentinel has, takes
  src/components/skill_row.rs          `SkillRow`: one skill on the Skills page (`docs/ui-registry.md`, `SkillRow`). Its name opens the
  src/components/slash_menu.rs         The command list under the composer. The arrow keys and Tab are handled by the composer, which
  src/components/status_badge.rs       `StatusBadge`: a shape, a color, and a label together, so state is never color alone
  src/components/strength_bar.rs       `StrengthBar`: how strong a note is, as a thin bar with the prototype's `sbar` classes
  src/components/tab_bar.rs            `TabBar`: the phone tabs Stream, Board, Reserved, Changes, More, and the swipe between them
  src/components/target_chip.rs        The send-to chip: shows where the next message goes, and opens the target menu
  src/components/task_card.rs          `TaskCard`: one task on the Board. Click opens the detail inline under the card
  src/components/telegram_card.rs      The Telegram card of Settings, Integrations. It saves the bot, waits for `/start`, and once the
  src/components/telegram_levels.rs    The Telegram rows that choose what the chat gets: levels, detail, events, and quiet hours
  src/components/telegram_log.rs       The Activity list of the Telegram panel: one line per Telegram event in the snapshot, newest
  src/components/toast.rs              `Toast`: one short result message, about three seconds, centered above the composer
  src/components/toggle.rs             `Toggle` and `Segmented`: the two small choice controls of Settings. Both use the prototype's
  src/components/token_editor.rs       `TokenEditor`: one color input per theme token in Settings, Appearance. Each edit is kept in
  src/components/tool_row.rs           `ToolRow`: one detected worker CLI with its status, path, version, enable switch, and a fix
  src/components/top_bar.rs            `TopBar`: the run title, agent stack, cost chip, run controls, and panel icons
  src/components/tour_coach.rs         `TourCoach`: the highlight ring and the coach mark for the current stop. It draws nothing
  src/components/tour_menu.rs          `TourMenu`: shows how far the tour got, and replays or skips it. Settings uses it
  src/components/verdict_card.rs       A verdict card: the task, its status, the reason, and the quality notes for the task
  src/components/wave_card.rs          The wave card: the tasks that run together, each with the agent it was routed to
  src/dry_run.rs                       Pure logic for the dry run step: stage titles, how a stage looks, how stage events fold into
  src/fonts.css                        styles
  src/guide.rs                         The guide's thirteen steps and the Try it buttons under each one. The copy is the prototype's
  src/host.rs                          The always-on host card: what the daemon reports about the host, as check rows and short text,
  src/icons.rs                         Icon paths, ported from the prototype's `ICONS` table. One outline set, 1.7 px stroke
  src/layout.rs                        breakpoints and panel open state
  src/main.rs                          mount
  src/palette.rs                       Command palette items and the search over them. The overlay is `components/command_palette.rs`
  src/prefs.rs                         appearance preferences and token overrides, stored per device
  src/routines.rs                      Routines: jobs that run at a set time, such as the Morning brief (`docs/product-scope.md`, F25)
  src/skills.rs                        Skills: folders with a `SKILL.md` that teach agents one thing (`docs/product-scope.md`, F24)
  src/slash.rs                         Slash commands. One parser and one list. Each command maps to one API route in `composer.rs`,
  src/state.rs                         client state from snapshot and events
  src/telegram.rs                      Telegram settings and activity: the pure rules the card and the panel share, and the calls to
  src/theme.css                        CSS variables from ui-tokens
  src/theme.rs                         Applies appearance prefs to the page root: the `data-theme` attribute and the CSS variables
  src/tour.rs                          The tour's seven stops, their copy, and the math that places a coach mark. Reading the DOM
  src/tour_state.rs                    The tour's state: which stop shows, where its highlight and coach mark go, and the seen flag
  src/views/agents.rs                  The Agents panel: detected worker CLIs, and which agent serves each tier
  src/views/appearance.rs              The Appearance page of Settings. Every control writes to `ui.prefs`. The app saves the prefs
  src/views/board.rs                   The Board panel: tasks grouped by status (`docs/ui-rules.md`, Views)
  src/views/brain.rs                   The Brain panel: a header with the mode, the note count, and the vault path, then three tabs
  src/views/brain_graph.rs             The Graph tab: the note graph from `GET /api/brain/graph`, a legend of the types, and one
  src/views/brain_notes.rs             The Notes tab: a search box and a type filter over the list, a New note form on the computer,
  src/views/brain_proposals.rs         The Proposals tab: the open proposals from care runs, or the empty card with Run care now
  src/views/brain_settings.rs          The Brain settings page: the mode, the vault folder, the run journal, and the care schedule
  src/views/budget_step.rs             The budget step of the setup wizard: the cap, the permission mode, the Brain mode, and which
  src/views/changes.rs                 The Changes panel: checkpoint commits, blocked writes, and quality notes, from the event log
  src/views/composer.rs                The composer: the text box, the slash menu, the footer chips, and what each send does. A plain
  src/views/composer_chips.rs          The pending attachments of the open chat: the chips row above the footer, and the reload that
  src/views/composer_send.rs           The composer's send path: a plain message starts a run or goes to the live run, a slash
  src/views/context.rs                 The Context panel: what fills the agent's context window for the open chat. Counts are
  src/views/cost.rs                    The Cost panel: spend against the cap, the 70 and 95 percent marks, per-agent cost, spend over
  src/views/dry_run.rs                 The dry run step of the setup wizard (`docs/product-scope.md`, F8 step 5). It says what the
  src/views/files.rs                   The Files panel: the repository files with reserved-file markers and a filter
  src/views/folder_picker.rs           The repository step: choose the project folder, check it, and fix what the check finds
  src/views/install.rs                 The install check and the agents step of the setup wizard (`docs/product-scope.md`, F8 and
  src/views/live_fetch.rs              Keeps a panel's data fresh while the panel is open. It loads on mount, again when the event id
  src/views/mod.rs                     Views that hang off the shell. Panels and modal bodies land here one at a time; until a
  src/views/onboarding.rs              `SetupWizard`: the full-screen setup (prototype `renderWizard`, `wizNav`, `finishSetup`). The
  src/views/pair_page.rs               The page a phone opens from the QR code or the link `/pair`. It asks for the one-time code and
  src/views/pairing.rs                 The phone step of the setup wizard (`docs/product-scope.md`, F8 step 4). It checks Tailscale
  src/views/panel_stubs.rs             Temporary panel bodies. The reviewer replaces each one with its real panel
  src/views/permissions.rs             The Permissions panel: the four modes and the per-agent capability matrix
  src/views/remote.rs                  The Remote panel: Tailscale state, pairing a phone, paired devices, and what a phone can do
  src/views/remote_tailscale.rs        The Tailscale half of the Remote panel: the three checks on this computer, the setup steps
  src/views/report.rs                  The Report panel: the honest per-task report, reasons for tasks not done, and an export
  src/views/reserved.rs                The Reserved panel: which task holds each file, and who waits (`docs/ui-rules.md`, Views)
  src/views/routines_page.rs           The Routines page of Settings (`docs/product-scope.md`, F25): the routines, the add button, the
  src/views/settings.rs                The Settings modal (`docs/ui-rules.md`, Views: Settings). The overlay frame is
  src/views/settings_pages.rs          Settings pages: General (repository and wake lock) and Brain. The other pages live in
  src/views/skills_page.rs             `SkillsPage`: the Skills page of Settings (`docs/ui-rules.md`, Settings; `docs/ui-registry.md`,
  src/views/stream.rs                  The stream: the run's items in order, built from the snapshot. The view follows the newest
  src/views/stream_model.rs            Turns the daemon's events into stream items. Pure: no signals, no DOM, so every mapping has a
  src/views/telegram_panel.rs          The Telegram panel. Once the chat is paired it shows the bot, the detail, the levels that are
  src/views/telegram_slot.rs           Empty slots in Settings, Integrations. The Telegram card and the always-on host card are built
  src/views/terminal.rs                The Terminal panel: raw output per agent, with a switcher and a load-earlier action
  src/views/working_tree.rs            The "Working tree" section at the top of the Changes panel: the files git reports right now,
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

## crates/sentinel-mcp

```
crates/sentinel-mcp/
  Cargo.toml                           workspace members and shared dependency versions
  src/brain_tools.rs                   The Brain tools. Agents search and read notes, and propose new ones. Nothing here writes a note:
  src/files.rs                         lists repo files (git ls-files, or a folder walk)
  src/lib.rs                           module exports
  src/protocol.rs                      The JSON-RPC 2.0 messages MCP needs, and the tool list. Replies are built as `serde_json`
  src/server.rs                        Binds the daemon to the tailnet address and serves the API, the WebSocket, and the UI
  src/skill_tools.rs                   The skill tools. Agents list the skills of the project and of the home folder, and read one
  tests/mcp/brain.rs                   file
  tests/mcp/files.rs                   file
  tests/mcp/main.rs                    Integration tests for the MCP server: the protocol, the file tools, and the Brain tools
  tests/mcp/protocol.rs                file
  tests/mcp/support.rs                 Helpers shared by the test modules: run the server over lines and read the replies
```

## packaging/

```
packaging/
  macos/Info.plist.in     the app's Info.plist, with the version and build number filled in
  macos/build-app.sh      builds Sentinel.app from the release binary and signs it
  macos/make-dmg.sh       puts the app in Sentinel.dmg with an Applications shortcut and signs it
  macos/notarize.sh       notarizes and staples an app or image, and checks Gatekeeper accepts it
  macos/set-signing-secrets.sh  stores the five Apple signing secrets on the repository from your terminal
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
