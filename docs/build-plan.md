# Build Plan

## How to Use This Plan

1. Work phases in order. A phase starts when its dependencies are done.
2. Take one task at a time. Task IDs are stable. Never renumber.
3. A task is done only when its "done when" line is true and `cargo xtask ci` passes.
4. Update `progress-tracker.md` when a task starts and when it ends.
5. If a task needs a decision, add it to Open Questions in the tracker and stop.

No calendar dates here. Order and dependencies matter, not days.

## Milestones

| Milestone | Reached when |
|---|---|
| M1 Engine | Two fake agents edit one tree with zero collisions |
| M2 Honest | Verified-only status and the report work end to end |
| M3 Real Agents | Detected real CLIs complete tasks on the sample repo |
| M4 Phone | The whole harness is usable from a phone over Tailscale |
| M5 Guided | Onboarding and tour work on a clean machine |
| M6 Ready | Rehearsal passes. Write-up done |
| M7 Brain | A second run uses Brain notes and proven lessons, and the result is measured |
| M8 Telegram | Connect with a bot token and user ID, get notified, approve, add a task, and start a run from Telegram |
| M9 Always-on | Sentinel runs headless as a service on another machine and survives a restart |
| M10 Site | The how-to site is live on GitHub Pages |

**Hackathon cut line:** end of M4. Everything before it is required for the live run. M5 improves first use but the harness must also run from a config file without it. The Brain (P11, P12) and Telegram (P13) come after the cut line. The live run uses `read-only` Brain mode and Telegram off or status only.

## Phase 0: Foundations

Goal: a repo that checks itself.

| ID | Task | Done when |
|---|---|---|
| P0-01 | Create workspace, toolchain pin, lint and format config | `cargo build` and `cargo clippy` pass on empty crates |
| P0-02 | Create `xtask` with `ci` command | `cargo xtask ci` runs and passes |
| P0-03 | Add `slop-check` with banned list | Fails on a seeded banned word, passes on docs |
| P0-04 | Add `smell-check`, `doc-check`, `ban-check` | Each fails on a seeded violation |
| P0-05 | Add `deny.toml` with banned crates | `cargo deny check` passes |
| P0-06 | Extend `smell-check` (data clumps, message chains, public fields) and add `dup-check` | Each fails on a seeded violation and passes on clean code |
| P0-07 | GitHub workflows (`ci`, `pages`, `release`, `nightly-agent-smoke`) and `dependabot.yml` | Each file parses as YAML. `ci.yml` runs `cargo xtask ci` on a push to main |

Dependencies: none. Tests: each check has a failing fixture and a passing fixture.

## Phase 1: State and Reserved Files

Goal: tasks, reserved files, waves in memory and in SQLite.

| ID | Task | Done when |
|---|---|---|
| P1-01 | Core types: ids, task, scope, reservation, cost, event | Types compile, unit tests for scope overlap |
| P1-02 | Store schema and migrations | Open, migrate, round-trip every table |
| P1-03 | Reservation table with grant, wait, release, revoke | Two tasks cannot hold one path. Test proves it |
| P1-04 | Wave builder | Overlapping tasks never share a wave. Property test |
| P1-05 | Scheduler with dependency order | Dependent task starts only after its dependency checkpoints |

Dependencies: P0. Testing: unit plus `proptest` on scope overlap and waves.

## Phase 2: Single-Tree Guard

Goal: safe shared tree.

| ID | Task | Done when |
|---|---|---|
| P2-01 | Git wrapper for plumbing calls | Wrapper works on a temp repo |
| P2-02 | Checkpoint with private index | Commit contains only reserved paths. Working tree unchanged for other paths |
| P2-03 | Filesystem watcher | A write to an unreserved file is detected within 500 ms |
| P2-04 | Restore from last checkpoint | Stray write is reverted and logged |
| P2-05 | Tamper detection | Edit to an existing test file is flagged |
| P2-06 | Barrier logic | Barrier refuses to run on a dirty tree |

Dependencies: P1. Testing: temp-repo integration tests. E2E `collision.rs` passes with fake agents. **M1.**

## Phase 3: Workers and Detection

Goal: run real and fake agents safely.

| ID | Task | Done when |
|---|---|---|
| P3-01 | Worker trait and supervisor (spawn, timeout, kill, capture) | Fake CLI runs, times out, and is killed on signal |
| P3-02 | Budget wrapper and kill channel | E2E `budget.rs`: run stops at 95% |
| P3-03 | Adapter profile format and loader | Profile TOML produces a command line for a task |
| P3-04 | CLI detection | Fake binaries on a temp `PATH` are found with version and status |
| P3-05 | Profiles for claude, codex, opencode, gemini, goose, pi | Each profile verified against the real tool on the dev PC |
| P3-06 | Profiles for agy, deepseek, mcode, plus generic template | Each confirmed or marked "needs user template" |
| P3-07 | OpenRouter worker loop | Cheap model completes a task on the sample repo |
| P3-08 | MCP file server that checks reservations | Write to a reserved path works. Others are refused |
| P3-09 | Router with cheap-first escalation | Failed task retries on a higher tier |
| P3-10 | Context builder: failure summary, truncation, own-loop compaction | E2E `compaction.rs`: retry prompt stays under 4 KB. Own loop compacts above 70% of the window and logs `ContextCompacted` |
| P3-11 | Per-task cost ceiling | Runaway fake CLI is killed at its ceiling, then escalated or blocked |
| P3-12 | Permission modes and approval requests | E2E `permissions.rs`: each mode behaves as documented. A write to a file the task has not reserved is blocked in every mode |

Dependencies: P1, P2. Testing: fake CLIs in `fixtures/fake-cli`.

## Phase 4: Planner, Verifier, Report

Goal: honest done.

| ID | Task | Done when |
|---|---|---|
| P4-01 | Backlog parser | Sample backlog parses into tasks |
| P4-02 | Planner call producing scopes, dependencies, flags | Sample backlog yields expected graph |
| P4-03 | Contradiction and already-done handling | Both traps in the sample are flagged |
| P4-04 | Test gate at barrier | Suite result stored with output reference |
| P4-05 | Second-model diff verifier | Mismatched diff is rejected |
| P4-06 | Verdict and honest report | Every status has a reason. VERIFIED has a receipt. **M2** |
| P4-07 | Quality notes on agent diffs | Seeded long function and placeholder code produce notes. Verdict is unchanged |

Dependencies: P1 to P3.

## Phase 5: Daemon, Log, Resume

Goal: one command runs everything and survives a crash.

| ID | Task | Done when |
|---|---|---|
| P5-01 | Run loop tying all parts | `sentinel run` completes the sample backlog |
| P5-02 | Append-only JSONL log | Every event appears once, in order |
| P5-03 | Crash resume | E2E `resume.rs`: kill mid-run, restart, finish |
| P5-04 | HTTP API and WebSocket | API tests cover every route |
| P5-05 | Real agents on the sample repo | At least 3 agents on at least 2 tools finish tasks. **M3** |

Dependencies: P1 to P4.

## Phase 6: Web UI Core

Goal: the whole harness visible in a browser.

| ID | Task | Done when |
|---|---|---|
| P6-01 | Trunk build and embedded assets | Daemon serves the UI |
| P6-02 | Tokens and theme CSS | Matches `ui-tokens.md`. `contrast-check` passes |
| P6-03 | Board, reserved files map, cost meter | Live updates from events |
| P6-04 | Agent stream and task detail | Output streams for a running task |
| P6-05 | Tools view, report view, settings | All data from the API |
| P6-06 | Controls: pause, resume, kill, lower cap, decisions | Each control works and is logged |
| P6-07 | Workspace shell: chat sidebar, top bar with panel icons, stream, composer, slide-in panels | Matches `design/sentinel-prototype.html` at desktop, tablet, and phone widths. Sidebar is open by default on tablet and desktop |
| P6-08 | Slash commands and command palette | `/help` lists commands. Ctrl or Cmd + K switches agents and opens panels |
| P6-09 | Permission modes, approval cards, capability matrix | Mode changes who answers extension requests. Writes to files a task has not reserved stay blocked in every mode |
| P6-10 | Agent stack, agent popover, send-to chip | Focus, terminal, and kill work for each agent. Messages never change which files a task has reserved |
| P6-11 | Appearance settings and token editor | Theme, accent, tint, fonts, size, density, radius, and stream width change live and persist per device. Contrast readout works |
| P6-12 | Connections page and agents page | Each connection shows real state and a test action. Tools needing a login or template have a fix action |
| P6-13 | Popovers and toasts stay inside the app at every width and use tokens | Checked at phone, tablet, desktop. Toast text passes contrast in both themes |
| P6-14 | Prototype moves to the chat model: New chat, project headers, chat menu, archived group, plus menu with chips, empty chat state, shortcuts | `design/sentinel-prototype.html` shows every chat feature of F23. The Leptos port copies it |
| P6-15 | Chats in core and store: `ChatId`, `ChatInfo`, `chats` table, `events.chat_id`, `ChatChanged` event | Round-trip tests. Old stores migrate. Events of a run carry the chat |
| P6-16 | Chat routes: list, create, rename, pin, archive, delete, events of one chat. A chat with a live run cannot be archived or deleted | API tests, including phone rights (phone cannot delete) |
| P6-17 | Attachments: routes, storage under `.sentinel/chats/`, size limits, prompt context for planner and agents | A file, a folder, a note, a backlog, and an upload each reach the planner prompt. Uploads stay out of the tracked tree |
| P6-18 | Runs from a message: plain text and attachments start a run, one live run per project, busy chat named | E2E with fake agents: a message with no backlog produces tasks and a report |
| P6-19 | Sidebar of chats: rows, project header plus, row menu, pin, archive group, search, rename in place, delete confirm | Matches the prototype at phone, tablet, desktop |
| P6-20 | Composer plus menu, attachment chips, repository file picker, empty chat state, New chat shortcut | Matches the prototype. Chips go with the next message |

Dependencies: P5.

## Phase 7: Phone over Tailscale

Goal: everything on the phone.

| ID | Task | Done when |
|---|---|---|
| P7-01 | Tailscale status check (not installed, signed out, running), address lookup, bind rule | Daemon refuses non-Tailscale bind. Each state shows the right guidance |
| P7-02 | Pairing: QR, one-time code, device token | Phone pairs. Revoke works |
| P7-03 | Responsive layouts and bottom tabs (Stream, Board, Reserved, Changes, More) | All views usable at phone width. Tap targets at least 44 px |
| P7-04 | PWA manifest and installability | Phone can install it |
| P7-05 | Offline read-only snapshot and reconnect | Banner shows. State resumes on reconnect |
| P7-06 | Phone rehearsal | Full sample run controlled from a real phone. **M4** |

Dependencies: P6. See [mobile.md](mobile.md).

## Phase 8: Onboarding

| ID | Task | Done when |
|---|---|---|
| P8-01 | Onboarding state and API | Resumable across restarts |
| P8-07 | Install check (Rust, WASM target, trunk, git, Tailscale) with install-missing and doctor output | Missing items are found, installed, and the doctor result is shown |
| P8-02 | Step 1: choose the repository with the OS folder dialog | Git repo with a clean tree accepted. Others rejected with a reason and a fix button. A phone gets the folder browser |
| P8-03 | Step 2: detect tools | Each tool shown as found or missing, with a switch. No tier dropdowns |
| P8-04 | Step 3: budget, routing, and Brain mode | Saved to config |
| P8-05 | Step 4: pair phone, with Tailscale checks and phone steps | Not installed, signed out, and running each show the right steps. Link, code, and QR shown. Step completes on pairing |
| P8-06 | Step 5: dry run on the sample project, with an explanation of what it does, uses, and costs | Stages tick off live. Run completes and shows the report. The user's repository is untouched |

Dependencies: P3, P6, P7.

## Phase 9: Guided Tour

| ID | Task | Done when |
|---|---|---|
| P9-01 | Tour engine with anchors | Steps highlight real elements |
| P9-02 | Tour content for seven stops: chat list, agent switcher, panel icons, stream, permission chip, send-to chip, settings | Reads at plain language level, passes slop check |
| P9-03 | Replay from Settings, skip, resume | Works on desktop and phone. **M5** |
| P9-04 | In-app guide with the 13-step user flow, each step with a try-it action | Every action opens the right screen. Text passes slop check |

Dependencies: P6, P8.

## Phase 10: Hardening

| ID | Task | Done when |
|---|---|---|
| P10-01 | Rehearsal on a shuffled backlog with a forced crash | Run completes, report honest |
| P10-02 | Benchmarks recorded | Numbers from `product-scope.md` measured and stored |
| P10-03 | Security review of routes and bind rules | Checklist in code-standards passes |
| P10-04 | One-page write-up generator | Draft produced from the report |
| P10-05 | Cleanup of smells and slop | `cargo xtask ci` clean. **M6** |

## Phase 11: Brain

Goal: a readable, linked memory that agents use and Sentinel keeps.

| ID | Task | Done when |
|---|---|---|
| P11-01 | Vault format and note model: front matter, wikilinks, generated-text markers | Round trip keeps user text. Obsidian opens the folder. Outside edits are picked up |
| P11-02 | Index and search in `brain.db`, rebuilt from files | Search and backlinks match the files. A full rebuild gives the same result |
| P11-03 | Agent memory: select within the token cap, pinned notes, MCP tools `brain_search`, `brain_read`, `brain_propose` | Prompt stays under 500 tokens. An agent cannot write a note directly |
| P11-04 | Run journal: run, task, and file notes in learn mode | After the sample run the notes exist and link correctly. Nothing is written in read-only |
| P11-05 | Lessons as typed notes with proof rules | Candidate becomes proven after 2 helped uses. Harm rejects. A seeded task title never reaches a lesson note |
| P11-06 | Brain modes off, read-only, learn. Phone can only tighten | Live run defaults to read-only. Phone cannot loosen the mode |
| P11-07 | Brain panel: notes, search, editor, backlinks | Edit saves to the file. Links resolve. Phone can read but not edit |
| P11-08 | Graph view | Nodes and links match the vault. Tapping a node opens the note |
| P11-09 | Second-run benchmark | Practice backlog run twice. Scope accuracy or retry success improves, or the result is recorded as no gain. **M7** |

Dependencies: P5 for data, P6 and P7 for the panel and phone review. Testing: unit tests on proof rules and selection, e2e `brain.rs`.

## Phase 12: Brain Care

Goal: the Brain evolves on a schedule, and you stay in control.

| ID | Task | Done when |
|---|---|---|
| P12-01 | Schedule: off, daily, weekly, monthly, after N runs. Missed runs start on the next launch | Schedule fires in a test with a fake clock. A missed run starts once |
| P12-02 | Care run: refresh, consolidate, improve, prune, with its own budget | The run stops at its budget. Uses the cheapest ready agent |
| P12-03 | Proposals as files and rows. Accept, dismiss, optional auto-apply to Sentinel-written notes only | E2E `brain_care.rs`: a note you wrote never changes without an accept |
| P12-04 | Strength: rise on use and confirm, decay when unused, propose retirement | Unit tests on decay and floor. Nothing is deleted automatically |
| P12-05 | Care runs in the sidebar under Scheduled, with a run view and a settings page | Matches `design/sentinel-prototype.html` |

Dependencies: P11.

## Phase 13: Telegram

Goal: notifications and control from Telegram. Details are in `product-scope.md`, F20.

| ID | Task | Done when |
|---|---|---|
| P13-01 | Bot connection: token in the keychain, `getMe` check, long polling, `Notifier` interface | Fake Bot API test passes. Token never appears in logs |
| P13-02 | Connection: bot token and user ID, one allowed user, `/start` check | Only that user ID is answered. Others are ignored and logged. The token never appears in logs |
| P13-03 | Notify and Reports: formatter, outbox, batching, live message, run and daily summaries | Events arrive once, in order, under 4096 characters. Summaries hold cost and Brain proposals |
| P13-04 | Control: commands and inline buttons with one-time tokens | Approve, decide, pause, kill, lower cap work. An expired or reused button fails |
| P13-05 | Code: `/agent`, `/diff`, `/task`, `/run` with confirm taps | A task added from Telegram appears in `backlog.md` and is planned. A run starts in the current mode and is refused in Bypass permissions |
| P13-06 | Detail levels, redaction, and hackathon status-only mode | Detail adds at most 6 lines. Key-like text is hidden. Status only holds no titles |
| P13-07 | Telegram settings page and test message | Matches the prototype. Revoke works |
| P13-08 | Allowed and forbidden actions test | Each level allows only its routes. Bypass permissions, raising the cap, and editing notes are refused. **M8** |

Dependencies: P5, P6, P11 for `/brain`, and P14 for the always-on host.

## Phase 14: Always-On Host

Goal: Sentinel keeps working on a machine that stays on.

| ID | Task | Done when |
|---|---|---|
| P14-01 | Headless daemon mode | Starts with no window and serves the UI over Tailscale |
| P14-02 | `sentinel service install` and `uninstall` for systemd and launchd | Starts at boot and restarts after a kill |
| P14-03 | Heartbeat and host state in Settings | The app shows service, Tailscale, agent CLIs, repository, and last heartbeat |
| P14-04 | Back-online handling | After a forced stop, the daemon reconciles the tree, pauses the run in flight, and sends the Telegram message |
| P14-05 | Folder browser on a headless host | A folder outside the home folder is refused |
| P14-06 | Host checks in `sentinel doctor` | Reports each check and what to fix. **M9** |

## Phase 15: How-To Site

Goal: a simple GitHub Pages site that shows all of this.

| ID | Task | Done when |
|---|---|---|
| P15-01 | mdBook project with the theme from `ui-tokens.md` | Light and dark work. Fonts load locally |
| P15-02 | Pages from the site map in `design/site-structure.html` | Every page has the same parts: what it is, steps, a live view where a picture helps, tip, related |
| P15-03 | Live views from the prototype (`?fig=ID`, in both themes) with numbered highlights and arrows, and the embedded prototype page | Each view loads and points at the right part. The embeds work on phone width |
| P15-04 | GitHub Actions workflow to build, check, and deploy | A push to main updates the site |
| P15-05 | Slop check and link check on the site | Both pass in CI |
| P15-06 | Publish and link from the README. **M10** | The public URL works |

## Phase 16: Skills

Goal: add skills once, and agents use them.

| ID | Task | Done when |
|---|---|---|
| P16-01 | Skill files: parse `SKILL.md`, list project and personal skills, enforce the limits | A bad name, a long description, and a big folder are refused. The project skill wins on a name clash |
| P16-02 | Switches and API: list, create, read, edit, delete, import, found | A phone may read and move a switch toward Off. The rest needs the computer |
| P16-03 | Skills in task prompts: Always, then Auto by word match, 1,500 token cap, stream line | The prompt holds the chosen skills and stays under the cap. The stream says which were loaded |
| P16-04 | Import from `.claude`, `.agents`, `.agent`, and `.codex` folders | A skill from each folder is listed, and import copies it without touching the original |
| P16-05 | Skills page in Settings, the `/skill` command, and the skill chip | Matches the prototype. A chip adds the skill to the next message |
| P16-06 | MCP tools `skill_list` and `skill_read` | An MCP client lists and reads a skill. It cannot write one |

Dependencies: P3, P6. See F24 in [product-scope.md](product-scope.md).

## Phase 17: Routines

Goal: timed jobs and briefs, in the app and in Telegram.

| ID | Task | Done when |
|---|---|---|
| P17-01 | Routine store and the scheduler: 30 second tick, local time, days, last fired, missed-run rule | A fake clock fires a routine once a day. A miss under 2 hours fires at start. An older miss is skipped and logged |
| P17-02 | Brief builders for the Morning brief and the Sundown wrap-up | The text is built from fixture state, holds no code or note text, and shows counts only in hackathon mode |
| P17-03 | Delivery: `briefs` rows, `BriefReady`, Telegram send, `/brief`, `/wrapup`, `/daily` | A brief reaches the fake Bot API when the chat has the Reports level and not otherwise. Quiet hours do not hold it |
| P17-04 | Reminder and backlog run kinds | A reminder is sent at its time. A backlog run starts in the current mode, is refused in Bypass, and is skipped while a run is live |
| P17-05 | Routines page in Settings, the sidebar row, and the brief card | Matches the prototype. Send now works |
| P17-06 | Phone and security rules, route access rows | A phone may read, send now, and turn off. Everything else needs the computer. The access matrix test covers every new route |

Dependencies: P5, P13. See F25 in [product-scope.md](product-scope.md).

## Testing Approach per Phase

| Phase | Method |
|---|---|
| 0 | Failing and passing fixtures per check |
| 1 | Unit and property tests |
| 2 | Temp git repos, fake agents |
| 3 | Fake CLIs, then one real run per tool |
| 4 | Fixture backlog with known traps |
| 5 | E2E: collision, resume, budget, tamper |
| 6 to 9 | Component tests, manual phone checks, tour dry run |
| 10 | Full rehearsal, forced failures |
| 11 | Vault round trip, selection under the cap, proof rules, no task text in lesson notes, two-run benchmark |
| 12 | Fake clock for the schedule, care never edits a user note, budget stop |
| 13 | Fake Bot API server: connect, notify, buttons, replay, level limits, `/task` and `/run` |
| 14 | Kill and restart the service. Fake clock for the heartbeat gap |
| 15 | Build the book. Check slop and links |
| 16 | Fixture skills: limits, selection under the cap, import leaves the original alone |
| 17 | Fake clock: one fire per day, missed-run rule, brief text from fixture state, no private text in a brief |

## Quality and Slop Tooling Tasks

Covered by P0-03, P0-04, P0-06, P4-07, P6-02, P9-02, and P10-05. Each new phase must keep `cargo xtask ci` green.
