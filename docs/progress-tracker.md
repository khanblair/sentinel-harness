# Progress Tracker

The live status of the project. Update at the start and end of every work session.

## How to Update

1. Change a task's status when you start it and when you finish it.
2. Add a line to the Session Log at the end of every session.
3. Log every decision in the Decisions Log with the reason.
4. Log every added, moved, or deleted file in the File Changes Log.
5. Log every doc edit in the Doc Changes Log.
6. Put unresolved questions in Open Questions. Do not guess.
7. Keep this file short. Move old session entries into a summary line once there are more than 20.

## Status Values

| Value | Meaning |
|---|---|
| `todo` | Not started |
| `doing` | In progress, one owner |
| `blocked` | Waiting on a decision or another task. Say which |
| `review` | Built, checks not yet green |
| `done` | Done-when is true and `cargo xtask ci` passes |

## Current Phase

**Phases 0 to 6 and the Brain, Telegram, and host parts are built; Phase 8 and the Telegram and host UI cards are in progress.** A full run with the scripted fake agents on the sample repo gives honest statuses (`cargo nextest run -p sentinel-daemon --test engine_run`). The workspace has about 1,250 passing tests. `cargo xtask ci` is nearly green: the remaining findings are smell fixes in the UI crate (workers U-S1 and U-S2 are on them). Not yet done: the real-phone rehearsal (P7-06), the dry run (P8-06), the MCP file server (P3-08), and a real agent run that finishes the sample backlog (P5-05).

## Phase Status

| Phase | Name | Status |
|---|---|---|
| P0 | Foundations | review |
| P1 | State and Reserved Files | done |
| P2 | Single-Tree Guard | done |
| P3 | Workers and Detection | review |
| P4 | Planner, Verifier, Report | done |
| P5 | Daemon, Log, Resume | review |
| P6 | Web UI Core | review |
| P7 | Phone over Tailscale | doing |
| P8 | Onboarding | doing |
| P9 | Guided Tour | review |
| P10 | Hardening | todo |
| P11 | Brain | review |
| P12 | Brain Care | review |
| P13 | Telegram | doing |
| P14 | Always-On Host | doing |
| P15 | How-To Site | review |
| P16 | Skills | doing |
| P17 | Routines | doing |
| P18 | Desktop Window | doing |

Hackathon cut line: end of P7.

## Tasks

| ID | Task | Owner | Status |
|---|---|---|---|
| P0-01 | Workspace, toolchain, lint config | Claude | done |
| P0-02 | `xtask` with `ci` command | Claude | done |
| P0-03 | `slop-check` | opencode | done |
| P0-04 | `smell-check`, `doc-check`, `ban-check` | opencode | done |
| P0-05 | `deny.toml` | Claude | review |
| P0-06 | Extended `smell-check` and `dup-check` | opencode | review |
| P0-07 | GitHub workflows and dependabot | Claude | review |
| P0-08 | `contrast-check` | opencode, Claude | review |
| P1-01 | Core types | Claude | done |
| P1-02 | Store schema, migrations, extra tables | opencode | done |
| P1-03 | Reservation table | opencode | done |
| P1-04 | Wave builder | opencode | done |
| P1-05 | Scheduler and barrier | opencode | done |
| P2-01 | Git wrapper | opencode | done |
| P2-02 | Checkpoint with private index | opencode | done |
| P2-03 | Filesystem watcher and guard | opencode | done |
| P2-04 | Restore and revert | opencode | done |
| P2-05 | Tamper detection | opencode | done |
| P2-06 | Barrier logic | opencode | done |
| P3-01 | Worker trait and supervisor | opencode | done |
| P3-02 | Budget guard | Claude | done |
| P3-03 | Adapter profiles | opencode | done |
| P3-04 | CLI detection | opencode | done |
| P3-05 | Profiles for claude, codex, opencode, gemini, goose, pi | opencode | review |
| P3-06 | Profiles for agy, deepseek, mcode, generic | opencode | review |
| P3-07 | OpenRouter worker loop (`sentinel-workers`: `openrouter.rs`, `agent_tools.rs`, `agent_schema.rs`, `transcript.rs`; not yet chosen by the daemon) | Haiku (CLI) | review |
| P3-08 | MCP file server (`sentinel mcp`, crate `sentinel-mcp`: file tools that check the scope, Brain tools, skill tools). Built and tested. Not yet handed to the CLI workers (a per-task MCP config for claude is next) | Haiku (CLI) | review |
| P3-09 | Router | opencode | done |
| P3-10 | Context builder | opencode | done |
| P3-11 | Per-task cost ceiling | Claude | done |
| P3-12 | Permission modes and approvals | opencode | review |
| P4-01 | Backlog parser | opencode | done |
| P4-02 | Planner contract | opencode | done |
| P4-03 | Contradictions and already-done | opencode | done |
| P4-04 | Test gate | opencode | done |
| P4-05 | Diff reviewer contract | opencode | done |
| P4-06 | Verdict and honest report | opencode | done |
| P4-07 | Quality notes | Claude | done |
| P5-01 | Run loop | Claude | done |
| P5-02 | Append-only log | Claude | done |
| P5-03 | Crash resume | Claude | done |
| P5-04 | HTTP API and WebSocket | Haiku (Agent tool), Claude | review |
| P5-05 | Real agents on the sample repo | Claude | doing (D84) |
| P6-01 to P6-13 | Web UI core | Haiku (CLI and Agent tool), Claude | review |
| P6-14 | Prototype moves to the chat model | Haiku (CLI), Claude | done |
| P6-15 to P6-18 | Chats: core, store, routes, attachments, runs from a message | Claude | done |
| P6-19 | Sidebar of chats (rows, menu, archive group, rename, delete) | Haiku (CLI), Claude | review |
| P6-20 | Composer plus menu, chips, file picker, empty chat state | Haiku (CLI), Claude | review |
| P7-01 to P7-05 | Phone over Tailscale: status, pairing, layouts, PWA, offline | Haiku, Claude | review |
| P7-06 | Phone rehearsal on a real phone | owner | todo |
| P8-01 to P8-05, P8-07 | Onboarding state, folder, tools, budget, pairing, install check | Haiku, Claude | review |
| P8-06 | Dry run on the sample project (`POST /api/dry-run`) | Haiku (CLI) | doing |
| P11-01 to P11-10 | Brain: vault, index, search, graph, memory selection in prompts, run journal, lessons, modes, daemon routes, panel | Haiku, Claude | review |
| P12-01 to P12-05 | Brain care: schedule, care run, proposals, settings | Haiku, Claude | review |
| P13-01 to P13-08 | Telegram: crate, poller, router, notifier, settings routes (UI card and panel are U-I) | Haiku, Claude | review |
| P16-01 to P16-06 | Skills: files, switches and API, prompts, import, Settings page and chip, MCP tools | Haiku (CLI), Claude | doing |
| P17-01 to P17-06 | Routines: scheduler, brief builders, delivery, reminder and backlog run, Routines page, phone rules | Haiku (CLI), Claude | doing |
| P18-01 to P18-06 | Desktop window: feature flag, window and event loop, menu bar, navigation and downloads, close and quit, release smoke test | Free worker, Claude | doing |
| P14-01 to P14-05 | Always-on host: heartbeat, back online, `sentinel service`, headless (card is U-K) | Haiku, Claude | review |

Add the rows for later phases when a phase starts. Copy IDs from [build-plan.md](build-plan.md).

## Planned Files

Paths listed in [project-structure.md](project-structure.md) that do not exist yet. `cargo xtask doc-check` accepts a listed file that is missing only when its path starts with a prefix below. Remove a line when its files land.

- `crates/sentinel-reserve/`
- `crates/sentinel-tree/`
- `crates/sentinel-workers/`
- `crates/sentinel-plan/`
- `crates/sentinel-verify/`
- `crates/sentinel-brain/`
- `crates/sentinel-telegram/`
- `crates/sentinel-daemon/`
- `crates/sentinel-cli/`
- `crates/sentinel-ui/`
- `site/`
- `.github/`
- `fixtures/`
- `tests/`
- `rust-toolchain.toml`

## Decisions Log

| # | Decision | Reason |
|---|---|---|
| D1 | Language is Rust only, including the UI | Single language, shared types |
| D2 | No git worktrees. One shared working tree with reserved files | Worktrees cost disk and hide what is happening |
| D3 | Name is Sentinel | Chosen by the owner |
| D4 | Phone shows the whole harness over Tailscale as a PWA | Full control away from the desk, no cloud |
| D5 | Daemon auto-detects agent CLIs on the PC (claude, codex, opencode, gemini, goose, pi, agy, deepseek, mcode, others) | No hand-written setup. goose and pi added by D42 |
| D6 | Five-step onboarding and a guided tour are v1 features | First use must be easy |
| D7 | The cap can only be lowered, never raised, including from the phone | Keeps the budget guard hard |
| D8 | Only the verifier can mark a task VERIFIED | Honesty of "done" |
| D9 | Shell out to `git` for plumbing, no libgit2 | Small build |
| D10 | Docs follow the 13-file set in `README.md` | Owner request |
| D11 | Sentinel is the outer loop. Worker CLIs run the inner reason-act-observe loop | Matches how the agent harness loop splits in practice |
| D12 | The Brain is a vault of plain markdown notes in `.sentinel/brain/`. Lessons are typed notes in it. Lesson notes never hold task text. Trusted only after proof | Readable memory that stays safe. Avoids leaking the hidden backlog and entrenching bad habits |
| D13 | The live run uses the Brain in `read-only` mode: agents read, nothing is written. In-run adaptation is not saved unless approved | Rule 9: hidden backlog must not be shared |
| D14 | Smell and slop findings on agent code are warnings. They never change VERIFIED | Hidden tests are 40% of the score. Retries cost money |
| D15 | Retry prompts use a capped failure summary. Each task has a cost ceiling | Controls tokens and runaway CLIs |
| D16 | The phone can tighten the Brain mode but never loosen it. It cannot edit notes | Same pattern as the cap. Notes become agent prompts |
| D17 | UI layout follows Claude Code: run sidebar, one stream, panels slide in, top-bar icons | Familiar to users of Claude Code, Codex, and Antigravity |
| D18 | Visual direction is a neutral graphite base with one amber accent, Inter and JetBrains Mono. Chosen by the assistant after the owner said "choose best" | Quiet surface, color only for meaning |
| D19 | Four permission modes (Plan only, Ask each wave, Auto inside reserved files, Bypass permissions). Reserved files stay the hard boundary in all of them | Same control model as the coding tools, with the collision guarantee |
| D20 | Agent switching from the top-bar avatar stack and the send-to chip. Tier routing in Settings | Several agents must be steerable one by one |
| D21 | Appearance settings are stored per device and applied as CSS variables | Phone and desktop can differ, no daemon state |
| D22 | Breakpoints are phone under 620, tablet 620 to 1179, desktop 1180 and up. Phone uses bottom tabs Stream, Board, Reserved, Changes, More. Sidebar rule updated by D25 | Center column stays at least about 480 px on desktop |
| D23 | Setup is an install check plus five onboarding steps. A guided tour has seven stops. An in-app guide has 13 steps | Owner request |
| D24 | The interactive prototype lives in `design/sentinel-prototype.html` and is kept in sync with the UI docs | Owner request: artifact changes change the docs |
| D25 | The sidebar is open by default on tablet and desktop and can be collapsed. It is a drawer on phone | Owner feedback: the sidebar was hidden at medium widths |
| D26 | Folders are chosen with the computer's own folder dialog. Nobody types a path | Owner feedback |
| D27 | The fourth permission mode is named Bypass permissions, with a confirm dialog | Owner feedback: matches the coding tools |
| D28 | The setup agent list shows found or missing only, with no tier dropdown and no letter icons. Task routing lives in Settings and says easy, normal, hard | Owner feedback |
| D29 | The pairing step checks Tailscale automatically and shows the matching steps, link, machine name, and code | Owner feedback |
| D30 | The dry run explains what it does and uses a sample project, never the user's repository | Owner feedback |
| D31 | Toasts use theme tokens and stay inside the app. The kind is shown by the icon color only (no side bar, see D35) | Owner feedback: toasts were hard to see |
| D32 | Leases are renamed Reserved files everywhere. The panel is Reserved. The mode is Auto inside reserved files | Owner answer: "lease" was a hard word |
| D33 | Brain scope: agent memory, run journal, lessons as notes, and a Brain panel with graph, search, and backlinks. Plus scheduled Brain care so memory evolves, with proposals for anything you wrote | Owner answer: all four, plus scheduled runs and evolving memory |
| D34 | Telegram is built into the prototype and scoped as F20: outbound long polling, allowlist, four levels (Notify, Control, Code, Reports), default detail **Detail** with redaction and a never-sent list, status only in hackathon mode, coding = message one agent plus a diff summary (extended by D37) | Owner answers |
| D35 | Toasts and notifications have no colored or dark left border. Kind is shown by the icon color | Owner feedback |
| D36 | Telegram connects with two things: the bot token and the owner's Telegram user ID. Then `/start`. One allowlisted user, one private chat, no groups | Owner answers and request |
| D37 | `/task` (add a task to backlog.md) and `/run` (start a run) are in v1, with confirm taps. A run from Telegram never uses Bypass permissions | Owner answer |
| D38 | Telegram runs on an always-on host (server or spare computer). Sentinel gets a headless mode, a service install, a heartbeat, and a back-online message | Owner answer |
| D39 | A how-to site on GitHub Pages, built with mdBook in `site/`, structure in `design/site-structure.html`. Built later in coding (Phases 15) | Owner request |
| D40 | Settings is drawn as a cog and the theme switch as a sun or moon, so they never look alike. The accent row has round swatches plus labeled Custom and Reset to default | Owner feedback |
| D41 | The repo root is `sentinel-harness/`. Docs live in `docs/`, the prototype in `design/`. GitHub repo `khanblair/sentinel-harness`, remote over HTTPS (SSH alias failed). Made public by the owner on 2026-10-24 so GitHub Pages works (a secret scan of tracked files found nothing) | Owner answer. Matches project-structure.md |
| D42 | goose and pi join the detected CLI list, each with a profile (P3-05) | Owner answer. Both run headless today |
| D43 | Build method: free CLI workers write code (opencode, goose, pi, local Ornith model for small drafts). Claude reviews, fixes, and merges. One brief per task ID in `.build/briefs/` (git-ignored). Workers get disjoint files per wave, never commit, and are rotated across free models on quota or error. agy joins when its headless mode returns output | Owner request. Saves paid tokens. Briefs point to docs by path |
| D44 | Q1 closed: a stray write to an unreserved file is restored at once and logged. Q2 closed: an extension for a free file becomes an approval card, or is granted and logged in Bypass permissions | Owner answer. Matches architecture and F15 |
| D45 | (autopilot, review) Blair is away. Decisions are made without asking, take the recommended option, and are logged here. No paid models, no real Telegram bot, and nothing secret in git (the repo is public, see D41) | Owner request 2026-10-24 |
| D46 | (autopilot, review) Ship order: P0 to P7 done end to end first, then P8 on. A task is `done` only when its done-when line is true. Anything less is `review` or `todo` | Honest status is the product |
| D47 | (autopilot, review) Workers write code in isolated copies under `.build/ws/<ID>` (git-ignored). Claude merges the owned files after review. Only Claude edits root `Cargo.toml`, `Cargo.lock`, and the lint config | Two workers in one crate break each other's builds. Same idea as reserved files |
| D48 | (autopilot, review) doc-check lists files with `git ls-files` plus untracked and not ignored. A listed file that is missing passes only under a prefix in Planned Files below | Rule 5 of project-structure.md |
| D49 | (autopilot, review) slop-check skips the Writing Style Rules section and fenced code. ban-check scans only `crates/` and `xtask/`, and skips its own pattern line | The lists must name the banned things |
| D50 | (autopilot, review) Loopback dev bind: `--dev-bind` or `SENTINEL_DEV_LOOPBACK=1` allows 127.0.0.1 and ::1 for tests and the browser pane. 0.0.0.0 is always refused. Off by default | The Tailscale-only bind rule blocks local tests |
| D51 | (autopilot, review) `sentinel-ui` is outside `default-members`. It builds on the host, so host clippy and tests include it for pure logic. A separate step checks it on `wasm32-unknown-unknown` | The host build works, so pure UI logic gets normal tests |
| D52 | (autopilot, review) The prototype CSS and icon SVG paths are copied into the UI crate unchanged. Class names and DOM follow the prototype | Stops visual drift |
| D53 | (autopilot, review) Ornith (local, 127.0.0.1:8089, about 3 tokens per second) is an opencode provider for short drafts. Stopped when idle | Unlimited and local, but slow |
| D54 | (autopilot, review) The UI build output folder keeps a committed placeholder `index.html` so a fresh clone builds the daemon without trunk | rust-embed needs the folder at compile time |
| D55 | (autopilot, review) Added `finding.rs`, `files.rs`, `ci.rs`, `rust_scan.rs` to xtask and `.cargo/config.toml` at the root | Shared plumbing for the checks |
| D56 | (autopilot, review) Sentinel keeps a private index at `.sentinel/index` for the whole run and never writes the user's index. After checkpoints the user's index looks stale until `git reset` | library-docs: never touch the user's index |
| D57 | (autopilot, review) GitHub workflows: `ci.yml` (cargo xtask ci on Linux, tests on macOS and Windows, UI wasm build with a 1.5 MB budget), `pages.yml`, `release.yml` (draft, unsigned), `nightly-agent-smoke.yml` (opt-in, self-hosted), `dependabot.yml`. Style follows the marshal repo. Real agents still never run in CI: the smoke job is off by default and runs on your own machine | Owner request |
| D58 | (autopilot, review) `rust-toolchain.toml` is removed during the build. Parallel `rustup` installs started by workers corrupted the toolchain. Pin to the installed stable (1.99.1) at the end | Incident 2026-10-24 |
| D59 | (autopilot, review) Workers may not run `rustup`, `cargo install`, `brew`, `npm`, or `pip` | Same incident |
| D60 | (autopilot, review) The repo is public, so it carries an MIT `LICENSE` (owner: Khan Blair, from git config). Remove or change it if you want other terms | An open repo without a license has unclear terms. The hackathon wants an open repo |
| D61 | (autopilot, review) Approved `sha2` and `hex` (device token hashes) and `console_error_panic_hook` (UI debugging). `reqwest` uses its default rustls build, so `deny.toml` allows the `OpenSSL` license text that `aws-lc-sys` carries | Needed by pairing and the UI. No other way to hash a token safely |
| D62 | (autopilot, review) smell-check: code in `#[cfg(test)]` modules and in `tests/` does not count toward the 400-line file limit, and `#[tokio::test]` counts as a test. Long functions and nesting limits still apply to non-test code | Test modules sit at the bottom of files by rule, so counting them punishes thorough tests |
| D63 | (autopilot, review) A desktop browser gets its token from `sentinel open`, which opens `http://HOST:PORT/#t=TOKEN`. The UI reads the fragment, stores it on the device, and removes it from the address bar. The token is never in a query string | Fragments are not sent to the server |
| D64 | Owner rule (2026-10-24): subagents are Haiku only, never Sonnet or Opus, and at most 2 at a time. All other work goes to the local Ornith model and the CLI tools (opencode, goose, pi, agy) when their free quota is back, or is done by Claude as reviewer and integrator | Owner instruction. Keeps token spend low |
| D65 | (autopilot, review) Fonts: only Inter and JetBrains Mono are bundled (woff2, latin, Open Font License, from the fontsource packages). The other families in Settings fall back to system fonts. Answers Q14 | Keeps the UI bundle small. Licenses are in `crates/sentinel-ui/assets/fonts` |
| D66 | (autopilot, review) The light theme accent is `#B35F09`, not `#B8620A`. `cargo xtask contrast-check` found white text on the old color at 4.4:1, below the 4.5:1 rule. Changed in the token doc, the prototype, and the UI crate together | The contrast check is the authority over the starting hex values |
| D67 | Owner rule (2026-10-24, later): subagents are Haiku only. At most 2 run through the Claude desktop Agent tool and at most 2 through the Claude Code CLI in headless mode (`.build/claude_worker.sh`, `claude -p --model haiku`, one git worktree per worker), so up to 4 work at the same time. This replaces the "at most 2 at a time" part of D64. Sonnet and Opus stay banned | Owner instruction: "use also claude cli ... use haiku, 2 agents" and "claude desktop max 2 subagents, same for cli. meaning 4 agents can run in parallel" |
| D68 | (autopilot, review) A phone may read state and use the controls in `docs/mobile.md` (Security, updated in this change). These need the computer and give a phone `403`: the native folder dialog, setup steps, tool scan and toggles, routing, capability changes, the doctor, Tailscale sign-in, and pairing approval. A phone may browse folders inside the home folder and choose the repository (`docs/architecture.md` line for `/api/fs/list`, `docs/product-scope.md` F8 step 4, `docs/ui-registry.md` FolderPicker all say a phone needs the browser), but not while a run is live. A phone may also start a run and message an agent (`docs/architecture.md` Request and trigger; the old Security table did not list them) | The Security table said "Nothing else" while three other docs gave a phone the folder browser. The stricter table was widened to match the rest. See Q27 |
| D69 | (autopilot, review) Crash resume (P5-03): the store holds a `run.live` mark from run start to close-out. When a run starts and the mark is set, the run resumes: stray changes go back to HEAD, interrupted tasks go back to Ready, commits that were not yet verified are checked first, open cards take their safe default, and spend from the interrupted run counts toward the cap. No second `RunStarted` event is written. A run that stopped while planning starts over. A daemon that opens a store with the mark shows state Crashed | `docs/product-scope.md` F6 and `docs/architecture.md` Crash Resume. E2E test `crates/sentinel-cli/tests/resume.rs` kills a run with SIGKILL |
| D70 | (autopilot, review) `Engine::emit` takes `&Event`. `set_task_status` to the status a task already has is a no-op (a restarted run may repeat a step). The daemon binds only `127.0.0.1` with `--dev-bind` and only the Tailscale address otherwise; `sentinel daemon` writes `.sentinel/daemon.addr` so `open` and `pair` find it | Lint cleanup and resume. D50 and D63 stay as written |
| D71 | (autopilot, review, owner request) Sentinel is a coding tool you talk to, so the sidebar holds **chats**, not runs. Words: a **project** is a repository folder, a **chat** is a conversation in a project, a **run** is one pass of the harness that a message starts. A chat holds many runs. "New run" becomes "New chat". "Run" stays for harness work (Stop run, run report). Docs changed: `product-scope.md` (F14, new F23, glossary), `architecture.md`, `ui-rules.md`, `ui-registry.md`, `mobile.md`, `project-overview.md`, `build-plan.md` (P6-14 to P6-20) | Owner message 2026-10-24: "this tool is more of a coding tool, i.e. new run button can be new chat ... and we also add feature like new chat in a project, renaming, delete, archive, etc like how claude does it" |
| D72 | (autopilot, review) A plain message in an idle chat starts a run. The planner turns the message into tasks. A backlog file is one kind of attachment, and then each section is a task. A message in a chat with a live run is a note to the agents. At most one run is live per project (one working tree), so another chat that tries to start work is told which chat is busy | One tree is the core rule. A chat is not a Q&A thread with a model |
| D73 | (autopilot, review) The composer plus menu has four entries: Add files (repository files or an upload), Add a folder (repository folder), Use a backlog file, Add a context note. Each becomes a removable chip and goes with the next message. Files and folders reach the planner and agents as paths, with small file text included. Uploads (at most 1 MB each, the API body limit) live under `.sentinel/chats/CHAT/attachments/`, never in the tracked tree. Attached files are hints and never reserve files | Same shape as Claude's plus button. Reservations stay the only write rule |
| D74 | (autopilot, review) Chat menu: Rename (in place), Pin, Archive (and Unarchive), Delete with a confirm. Archived chats sit in a collapsed group. A chat with a live run cannot be archived or deleted. Delete removes the chat's messages and uploads, never repository files or commits. A phone may create, rename, pin, and archive. Deleting needs the computer | Matches Claude's chat list. A lost phone cannot wipe history |
| D75 | (autopilot, review) One Sentinel daemon serves one project in this version. The API, the types, and the screens use the word project, so a project switcher and Add project button can come later without a redesign. See Q28 | Keeps this change inside the single-store design (`.sentinel/` per repository) |
| D76 | (autopilot, review) Brain memory reaches the agents. Before each attempt Sentinel picks notes (pinned, proven lessons, notes linked to the task's files, then search hits on the task title) within 500 tokens and puts them under `## Project memory` in the prompt. A retry gets the same text through the notes section. The stream says `Brain loaded N notes: ...`. Mode `off` loads nothing. Code: `brain_run.rs`, `prompt.rs` | F12 step 3 |
| D77 | (autopilot, review) The run journal is written when a run ends, in `learn` mode only: a run note, a note per task, and a note per checkpointed file. A file note keeps the task links it had from earlier runs, so backlinks grow. A write failure is logged and never changes how the run ended | F12 step 4. A lock held across the write once deadlocked the close-out: the guard is now dropped before the journal runs |
| D78 | (autopilot, review) Telegram runs inside the daemon: a long poll loop, a router that turns a command or a button press into one Engine call as a phone caller after the level check, a notifier that sends events through the outbox, and settings routes. A paired chat starts polling at the newest update, so an old `/stop` cannot replay after a restart. An unpaired chat reads from the start, so the `/start` is never missed. The daemon starts Telegram at boot when a chat and token exist | F20. See Q30 and Q31 |
| D79 | (autopilot, review) Owner rule (2026-10-24, latest): the Claude desktop Agent tool is paused because the owner's desktop usage is nearly used up. All workers run through the Claude CLI (`.build/claude_worker.sh`), Haiku only, at most 2 at a time. Reviews, merges, and checks stay in the main session. This replaces the 2 plus 2 pool in D67 until the owner says otherwise | Owner message |
| D80 | (autopilot, review) The OpenRouter worker (P3-07) is a library: `OpenRouterWorker` implements `Worker` with its own tool loop (read, list, write inside scope, allowed cargo commands, finish), per-step cost from `usage.cost`, retries on 429 and 5xx, and the API key never printed. The daemon does not offer it as a tool yet; wiring it needs a tool entry that holds the key from the keychain (Q32) | F13 step 2 |
| D81 | (autopilot, review) Route access matrix test (`crates/sentinel-daemon/tests/security.rs`, 80 rows: public, any device, computer only, socket). A coverage test reads the route source and fails when a `.route(` has no row. It found that `POST /api/repo/starter-backlog` let a phone write `backlog.md`; it now needs the computer (`docs/mobile.md` says a phone may do nothing outside its list). A phone may revoke a device, including another phone: kept, because `docs/mobile.md` says a lost phone is revoked "from another device". The desktop token is not a device row, so no phone can revoke it | P10-03. Brain note edits and care routes refuse a phone inside the handler (`require_computer`), not in the route signature: the test treats them as computer only |
| D82 | (autopilot, review) Telegram quiet hours use this computer's UTC offset, read once from `date +%z` (override: `SENTINEL_UTC_OFFSET_MINUTES`). No UI setting | Q30. The owner is on EAT |
| D83 | (autopilot, review) OpenRouter is offered as a tool (`openrouter`, adapter `openrouter-loop`, tier cheap, last in the scan list so first-scan defaults never pick it over a real CLI). Ready when a key exists. The model comes from `[models] openrouter`, default `openrouter/auto` | P3-07 wired. Free models share one daily quota per account, so a free reviewer can answer 429 |
| D84 | (autopilot, review) Real-agent smoke on the sample backlog, with Claude Haiku as author: it plans scopes, writes code, and the suite passes. No task reaches Verified yet because VERIFIED needs a second, different agent to approve the diff and none was reachable on this computer (Gemini CLI has a broken keychain module, OpenRouter free quota was used up, Ornith was not serving). Found and fixed on the way: the claude profile's JSON output broke the planner (now unwrapped), the per-task ceiling used the task tier while the clock estimate used the tool tier (ceiling now follows the higher), the planner listed existing test files in scopes (rule added and a filter drops them), and a model flag is now passed to claude, codex, and gemini | P5-05 stays open until a run verifies with two tools |
| D85 | Owner rule (2026-10-25): ship working with every section implemented. Test the essential parts only (core behavior, security guards, crash paths). Test depth comes later, as features land | Owner message |
| D86 | (autopilot, review, owner request) **Skills** (F24): a skill is a folder with `SKILL.md` (front matter `name` and `description`, then markdown), the same format Claude, Codex, and Antigravity read. Project skills live in `.sentinel/skills/`, personal ones in `~/.sentinel/skills/`. Sentinel lists, and can import by copy, skills from `.claude/skills`, `.agents/skills`, `.agent/skills`, `.codex/skills` (repo and home). Each skill is Off, Auto (default), or Always. Chosen skills go into the task prompt under `## Skills`, within 1,500 tokens, and the stream names them. `/skill NAME` adds one by hand. A skill is text: Sentinel never runs its files and a skill never changes reservations, the cap, or the mode. Phone: read and move a switch toward Off | Owner message 2026-10-25. Docs: F24, architecture (Skills), P16 |
| D87 | (autopilot, review, owner request) **Routines** (F25): jobs at a set local time with plain words. Built in: **Morning brief** (default 08:00 weekdays) and **Sundown wrap-up** (default 18:00 daily), both off until turned on. Also **Reminder** and **Backlog run** (refused in Bypass, skipped while a run is live). Briefs are built from the store with no agent call and no cost, hold no code or note text, and show counts only in hackathon mode. They are kept in the app (last 14) and sent to Telegram when the chat has the Reports level, and quiet hours do not hold them. The scheduler ticks every 30 seconds. A routine missed while the daemon was off fires at start only if the miss is under 2 hours old. Telegram: `/brief` and `/wrapup` (`/daily` stays the short day summary). Phone: read, Send now, turn off. Named Routines (not cron) and Sundown wrap-up (not just "evening") for plain meaning | Owner message 2026-10-25 ("morning briefs, evening sundown ... crons ... sent to telegram"). Docs: F25, architecture (Routines), P17 |
| D88 | (autopilot, review, owner request) The site shows **live views of the prototype instead of screenshots**. The prototype has a figure mode, `?fig=ID`: it renders one screen at a fixed size, can crop to a part, dims the rest, and draws numbered outlines and arrow labels on the parts a page names. `site/theme/figures.js` turns a `<figure class="fig" data-fig="ID">` into that view and keeps it in step with the site theme. No PNG files exist or are needed. A page gets a view only where a picture helps. Overview, table, and FAQ pages drop the section | Owner message 2026-10-25. Docs: F22, P15-03, `site/README.md` |
| D89 | Owner rule (2026-10-25): **CI and releases run on GitHub, not on the owner's Mac.** Locally run only the tests of the crate you changed. The build output had grown to about 66 GB (`target` 36, a second target dir 25, worktrees 4.7). Fixes: one target dir (workers share it), `incremental = false` in `.cargo/config.toml`, `debug = "line-tables-only"` and no debug info for dependencies in `Cargo.toml`, merged worktrees removed after use. `docs/development.md` says so | Owner messages |
| D90 | (autopilot, review, owner request) **macOS app and signed disk image.** The `release` workflow builds `Sentinel-VERSION.dmg` on a GitHub Mac: a universal binary (arm64 and x86_64) with the UI embedded, in `Sentinel.app`. With the five `APPLE_*` secrets it is signed with a Developer ID certificate (hardened runtime) and notarized and stapled. Without them it is signed ad hoc, and macOS asks to confirm the first open. Linux and Windows are left out for now. The app runs the new `sentinel app`: it takes the project from `--repo`, the last one (`~/.sentinel/last_repo`), or a folder dialog, serves on the Tailscale address when Tailscale is up and on 127.0.0.1 otherwise, and opens the UI. Opening it again only opens the browser. Quit with Settings, General, `POST /api/quit`, or `sentinel stop` | Owner message. Files: `packaging/macos/`, `.github/workflows/release.yml` |
| D91 | (autopilot, review) The UI no longer guesses "phone" from the window width for rights checks: `GET /api/whoami` says whether the caller is the computer, and `ui.on_phone()` uses it (U-WHO). `GET /api/skills/found` is computer only because it names paths on the computer. The Revoke button is hidden on the computer's own device row | Found while checking the UI in a real browser: a narrow desktop window had controls disabled |
| D92 | (autopilot, review) Fixed in the UI after a real-browser check at 1280, 820, and 390 pixels: phone overlay rules targeted a `.frame` element that the real app does not have (Settings kept its side column and squeezed its content), the plus menu measured itself with `max-height` 0 and opened off screen, and the dialog footer rule was missing from the stylesheet. A check script drives headless Chrome (`.build/cdp.mjs`, not in the repo) | P6 polish |
| D93 | (owner request, 2026-10-25) **Desktop window for the macOS app.** Sentinel.app opens its own window (title "Sentinel", 1280 by 820, menus Sentinel, Edit, View, Window) instead of a browser tab. It loads the daemon's UI at `http://ADDR/#t=TOKEN`. macOS only. Approved crates `tao`, `wry`, and `muda` sit behind the `window` feature and are gated to macOS, so Linux, Windows, the always-on host, and `sentinel daemon` never build them. Closing the window hides it and the daemon keeps running. Cmd+Q, the Sentinel menu, Settings, General, Quit, or `sentinel stop` quits. Links to other sites open in the default browser. A custom protocol is rejected: the daemon's origin check and the phone already use the http address. Tauri is rejected: a full app framework for the same visible result. Supersedes the browser step in D90. Docs: F26, architecture (Desktop Window), P18 | Owner request. Library decision in `library-docs.md` |
| D94 | (owner request, 2026-10-25, away: recommended choices taken) **New chat reuses an empty chat.** `POST /api/chats` with no title returns the newest empty chat instead of making another. A chat is empty when it is not named, pinned, archived, or attached to, and its only events are its own `ChatChanged` events. At start the engine deletes all empty chats but the newest. A pinned chat is never empty because a pin is a choice. The UI shows "This chat is already empty." when the owner is already on it |
| D95 | (owner request, recommended) **Add a folder browses.** `GET /api/repo/tree?path=` lists the folders of one level of the project (counts, parent, no hidden or build folders, no symlink out of the repo). The attach dialog shows a breadcrumb and the folders of the current level, with "Use this folder". The project root cannot be attached, because every agent already works in the whole project. The filter box and the flat list are gone. Choosing the project itself still uses the native Choose folder button in Settings, General |
| D96 | (owner request, recommended) **No pretend agents in the product.** `--fake-agents` is removed from `run` and `daemon`. Sentinel runs only the agent CLIs found on this computer. A scan deletes tool rows and drops config and routing entries for tools it did not find, so an old `fake-a` config is cleaned on the next start. The scripted agent stays only as `EngineOptions.fake_agents`, a test double the Rust tests use, and no command line reaches it. `sentinel starter` and a first-run dialog make a small Python starter project (README, `greet.py`, a unittest, a three-task backlog, one commit, test command `python3 -m unittest`) in place of dummy data. The dry run still uses the real agents on its embedded sample |
| D97 | (owner request, recommended) **The Brain fills itself.** `seed_brain` reads the repository and writes linked notes: a project note, an area note per top-level folder, README, conventions, and agents. It never overwrites a note and runs once at first open when the vault is empty, in Learn mode only (Read-only never writes). `POST /api/brain/seed` runs it again (computer only). New projects default to Learn. Care runs make real proposals: link an orphan note, add an area note for a folder with none, merge notes with the same title. The Brain panel builds the notes on a button, switching to Learn first, draws the graph on rings with no random numbers, and shows loading and error text in every tab |
| D98 | (owner request, recommended) **Live Changes, Tasks, and Reserved.** The Changes panel shows the working tree from `GET /api/changes` and a diff per file from `GET /api/changes/diff`. The Tasks panel shows the backlog file's tasks as Planned before a run, from `GET /api/backlog/tasks`. The Reserved panel refreshes on events and every 5 seconds, shows the holder and how long it held, and says why it is empty. All three routes are any-caller reads |
| D99 | (owner request, recommended) **Context panel.** `GET /api/context?chat=` estimates what fills the agent's context window (about 4 characters a token): messages, instructions, Brain notes, skills, attachments, backlog, free space. The window comes from a table by agent (Claude and Codex 200k, Gemini 1M, others 128k) because the CLI may use a larger one. The panel is a minimal version of the owner's picture: percent used, a segmented bar, colored rows, and lists of the notes, skills, and attachments |
| D100 | (owner request, recommended) **Resizable sidebar and panel.** Drag handles between the sidebar and the stream and between the stream and the panel on the desktop layout. Arrow keys move a handle by 16 pixels, double click resets it, and the sizes are kept on this device. Tablet and phone keep their overlays |
| D101 | (owner request, recommended) **Pairing is in Settings, Devices.** One `PairPhoneCard` (also used by the Remote panel) shows the Tailscale state, the QR code, the link, the one-time code with a countdown, the real phone steps, and the pending request with Approve and Deny. The pairing link uses the Tailscale address and the daemon port, never the browser's own host (127.0.0.1 inside the app window). Connections shows a loading line and the real error text |
| D102 | (reviewer, 2026-10-25) **Found by the one browser check and fixed.** (1) Two first requests opened the Brain twice and the second open rebuilt the index file over the seed's writes, so the graph stayed empty until a restart. The Brain now opens once, with the slot locked during the open. (2) The Brain header reads the counts and links again 2.5 and 5 seconds after opening, because the first-open seed runs in the background. (3) The pairing link by name wrote `Kolaborate's MacBook Air`, which no phone can open. It now writes the name the way MagicDNS does (`kolaborates-macbook-air`). (4) The width signals pushed `Ui` past the 512-byte pass-by-value limit, so `clippy.toml` raises it to 1024. (5) A pinned chat is never counted as empty. Version is 0.1.2 |
| D103 | (reviewer, 2026-10-25, owner away) **UI bundle budget raised from 1.5 MB to 2 MB compressed.** CI measured 1,612,659 bytes after the Brain graph, Context panel, folder browser, pairing card, resize handles, and live panels landed. The release workflow has no budget check, so the release was never blocked. Shrinking options for later: a `wasm-release` profile with `lto` and `codegen-units = 1`, and trimming the icon set |
| D104 | (reviewer, 2026-10-25, owner away) **CI saves its build cache even when a job fails or times out, and the two test jobs may run 75 minutes.** Every earlier `main` run was cancelled or failed, so none saved a cache and each run built the whole workspace cold: the Linux test job ran past its 40 minutes. `Swatinem/rust-cache` now has `cache-on-failure: true`. The job log of a cancelled job is not kept by GitHub, so the cause was read from the timing, not from a test name. If the job still does not finish once the cache exists, look for a hanging test with `cargo nextest run --workspace --no-fail-fast` and its slow-test output |
| D105 | (owner request, 2026-10-25) **Releases publish themselves.** The `release` workflow no longer makes a draft. Its last step publishes the release and marks it latest, so a failed build, notarization, or launch check stops the run first and publishes nothing. The install script and the Releases page serve the new version at once. Before, D57 and D90 made drafts so the owner could look first. The owner published the two old drafts the same day: v0.1.1 (tag on 9b41dab) and v0.1.2 (tag on ec15dd1, marked latest). The release step sets `target_commitish` to the built commit. The owner chose to skip Apple signing for now, so releases publish ad hoc signed and macOS asks for Open Anyway on a downloaded disk image. Open: publish only when signed and notarized (needs the five Apple secrets) |
| D106 | (owner request, 2026-10-25) **The Mac app finds the agent CLIs when opened from Launchpad.** An app started from Finder, Spotlight, or the Dock gets only the system PATH, so the 0.1.2 scan found no agent and then dropped the enabled ones. At the start of `sentinel app` and `sentinel daemon` (the service job runs `daemon`), the process builds a repaired PATH: the current one, then the login shell's (`$SHELL -ilc`, 5 second limit, fish and nu skipped), then the usual install folders that exist (`/opt/homebrew/bin`, `/usr/local/bin`, `~/.local/bin`, `~/.cargo/bin`, `~/.bun/bin`, `~/.npm-global/bin`, `~/go/bin`, `~/.deno/bin`). It then starts itself again with that PATH (`exec`, same process id), guarded by `SENTINEL_PATH_FIXED`, because `set_var` is unsafe and the repository forbids unsafe. Agents started later get the same PATH. Checked by running the daemon under `env -i PATH=/usr/bin:/bin`: five agents found. Released as v0.1.3 and published by the workflow |
| D107 | (owner request, 2026-10-25) **Many projects in one app** (F27). One daemon now serves many folders. Each project keeps its own engine and its own `.sentinel` state (chats, Brain, skills, settings, runs). The first project is the primary and holds the paired phones, the saved keys, Telegram, the routines, the host loop, and the setup wizard. A request names its project with the header `X-Sentinel-Project` (the WebSocket uses `?project=`). Global paths always go to the primary. A project engine takes its sign-in from the primary (`Engine::use_devices_of`), so one token works everywhere. The list of extra projects is kept in `~/.sentinel/projects.json`. This replaces the old rule (D75, Q28) that one daemon serves one project. Only the primary runs Telegram and the routines. Runs in different projects can go at once |
| D108 | (owner request) **Sidebar by project.** Each project is a group with its chats and a plus button. A chat of another project switches the project and reloads the page, which gives every panel a clean start. A New project button opens the Mac folder dialog, and a folder browser on a phone or an always-on host. A phone sees projects but cannot add, remove, or clear. A project folder must be a git repository. Projects with the same folder name show the parent folder too: `Sentinel Starter (a)` |
| D109 | (owner request) **Reset in Settings** (F28). Ten kinds of data, each cleared alone or all together: this project (chats, Brain, run history, settings) and the app (paired phones, saved keys, Telegram, routines, extra projects, the setup wizard). `Clear everything` needs the word RESET and sends the app back to the full setup wizard. Project files are never touched. A Settings clear keeps the folder path of the project. The setup wizard state is read from the primary project, so a reset shows it from any project |
| D110 | (owner request) **API keys, bring your own** (F29). Four providers: OpenRouter, Claude, Gemini, DeepSeek. Keys live in the system keychain only. The list shows the last four characters. Each provider has Save, Test, and Remove. The test makes one read call and answers in a plain sentence, with the key in a header and never in a URL or a log. Only the computer manages keys. An agent of a kind gets its key as an environment variable when the key is saved and the variable is not set, so it bills that key and not its own login. This does not pin a model: pinning Haiku on the owner's own key is done in `~/.claude/anthropic.env` with model aliases, outside the repository |
| D111 | (owner request, found in a check) **The Mac folder dialog and the app PATH.** New project uses the system choose-folder dialog through `osascript`, because the `rfd` call from the daemon's thread did not reliably show a dialog in the window app. Cancel does nothing. The in-app folder list opens only when the host has no screen, and it reads `~`. The Replay tour button closes Settings first. Separately (D106), the app repairs its PATH at start so the agent CLIs are found when it is opened from Launchpad |
| D112 | (owner report, 2026-10-25) **The starter backlog button works.** In the setup wizard, "Create a starter backlog" posted an empty body to a route that needs the folder path, so the daemon answered 422 "Unprocessable Entity". The button now sends the chosen folder. That route answers a bad body with a JSON `error` sentence, through a small `ApiJson` extractor. Every other call from the page that carries a JSON body was checked against its route, and none was wrong. Released as v0.1.5 |
| D113 | (owner report, 2026-10-25) **Agent Test and a fast scan.** Each agent in Settings and in the setup wizard has a Test button. `POST /api/tools/{id}/test` sends one tiny prompt through the real worker path, stops after 90 seconds, and masks secrets in what it shows. The result line stays when the owner moves between the wizard and Settings, because the page keeps it in a map on `Ui`. The Scan again button was made faster (D-SCANFAST). Released as v0.1.6 |
| D114 | (owner report, 2026-10-25) **The dry run is honest.** It uses a built-in plan (`fixtures/sample-plan.json`) so it no longer waits for a planner that may answer in prose. Before the run it checks the agents with the Test probe and says in `DryRunStatus` which ones it skipped and why. The screen text says what the run really does. Released as v0.1.6 |
| D115 | (owner report, 2026-10-25) **Agent health.** An agent that fails with a login, limit, or quota error is paused for 30 minutes (`agent_health.rs`). Routing skips a paused agent. Known gap: the planner call can still pick a paused agent. Released as v0.1.6 |
| D116 | (owner answer, 2026-10-25) **Cost shows the real value.** Free models still show their spend, not a fake zero: the price is known from the model name the agent reports. A time guess alone can no longer stop a task, because a task ceiling applies only when the spend source is real (`SpendSource`, `ceiling_applies`). The token price table is still empty and no agent output is parsed for tokens, so cost from tokens is not used yet. Released as v0.1.6 |
| D117 | (owner report, 2026-10-25) **Message types.** A message is `chat`, `ask`, `research`, `plan`, or `change` (`sentinel_core::Intent`). A heuristic types it first (it looks past a leading greeting or please), the router agent second, and the default is never `change`. Chat, ask, and research get an answer and start no run, so "hi" no longer starts a run. The stream shows `Type: ask. REASON` with a chip. The chip menu and slash commands send the message again with another type. The type is recorded as `Event::IntentChosen`. Released as v0.1.6 |
| D118 | (owner report, 2026-10-25) **New projects inherit the agent choices.** A new project copies which agents the owner turned on in the setup, so an agent the owner never chose (Gemini) does not appear again. `hub_inherit.rs` does the copy, with the default agent too. Released as v0.1.6 |
| D119 | (owner answer, 2026-10-25) **A chat has its own agent.** Each chat remembers one agent, or Auto. A project default sets it for new chats (`PUT /api/config/agent`, Computer only), and `PUT /api/chats/{id}` changes it while the chat has no live run. When a chat has an agent, that agent types the message, answers, plans, and does every task. If it fails, the run stops with `Event::AgentStopped` and asks the owner. No other agent takes over. A failure of the task logic still retries on the same agent. The reviewer is always a different agent. Released as v0.1.6 |
| D120 | (owner request, 2026-10-25) **The page reloads on a new build.** `/api/health` returns `started`. When the page reconnects and `started` changed, it reloads, so a restart of the local app needs no manual refresh. Released as v0.1.6 |
| D121 | (owner report, 2026-10-25) **The DeepSeek agent is Codewhale.** The DeepSeek row said Missing although the key test passed: the key test only checks the key over HTTP, and the agent row looks for a program on the PATH. The old `deepseek` command no longer exists. The project (formerly DeepSeek-TUI, now `codewhale-hq/CodeWhale`) ships the command `codewhale` and the alias `codew`. The profile now runs `codewhale exec --auto --sandbox workspace-write PROMPT`, and the saved DeepSeek key reaches it as `DEEPSEEK_API_KEY`. Installed v0.10.2 into `~/.local/bin` from the release assets after a SHA256 check against the published list. The agent Test answered in 4.1 seconds. The profile stays `verified = false` until a real task has run on it. Released as v0.1.6 |
| D122 | (owner report, 2026-10-25) **The QR code in the setup wizard no longer covers the text.** The earlier fix changed the pairing card in Settings, Devices, but the setup wizard has its own phone step (`pair_steps.rs`) that drew the QR at its full 440 pixel size inside a 9 em box. The wizard now uses the same scaled `PairQr` and the same layout as the card: the QR beside the text on a wide screen, above it on a phone, and the link by machine name under the link. Found while checking: every snapshot update set the wizard step again, which rebuilt the phone step, fetched the Tailscale state again (233 requests in 10 seconds after a restart) and asked for a new one-time code each time. The step view now follows a memo, so it is built once. Checked in the browser at desktop and phone width. Released as v0.1.7 |
| D123 | (owner request, 2026-10-27) **Hackathon fit check.** Collision Course asks for a harness that runs at least 3 agents across 2 tools or models on a hidden 12 task backlog, with a hard $5 cap, honest "done" claims, logs and a one page write-up. Scoring: hidden tests 40, collision efficiency 20, cost 15, honesty 15, process 10. The practice repo (`Kolaborate-Platforms/collision-course-ledger`) uses `backlog/backlog.json`, a plain Node project, and a lock file `tests.lock.json`. Gaps found: no JSON backlog, no Node test command, no way to name the agents of a run, protected files not covered, no proof run. Fixed in D124 to D131. Not yet released
| D124 | **JSON backlog, test command, build without the UI.** `parse_backlog` reads `{version, tasks:[{id,title,body}]}` and a bare array, and keeps ids such as `T01`. `sentinel run --test-cmd` sets the suite command. Without it, the saved setting wins, then detection (`package.json` test script run through `sh -c`, so npm is not needed, then Cargo, pytest, go, make). A fresh clone builds with no UI: `build.rs` writes a placeholder `dist/`. Not yet released
| D125 | **The agents of a run.** `sentinel run --agents "claude=haiku,openrouter=MODEL,openrouter=MODEL"`, cheapest first. The same tool with two models is two agents (`openrouter~2`). The first `n` tasks of a run take the agents in turn, so every agent does real work, every later task starts with the strongest agent, and a failed try moves to the next stronger agent. The run prints each agent and where its spend comes from (reported, free, or a time guess). Not yet released
| D126 | **Protected files.** The paths in `tests.lock.json` (with their hashes) and `Config.protected_paths` can never change. A checkpoint that touches one is reverted and the task ends Blocked with the word tamper in its reason. At each barrier a changed protected file is restored from git. Not yet released
| D127 | **`sentinel selftest`.** Five scripted bad agents (a collider, a tamperer, a liar, an overspender, a crasher) run on a throwaway repo through the real engine, and a PASS or FAIL table prints. It costs nothing. It found two real bugs: a tampered task was moved from checkpointed to blocked, which the state machine refuses, and the write guard could undo a revert by restoring files from the old HEAD. The revert now holds the paths open for 400 ms. Not yet released
| D128 | **The four traps.** Shared files: every task gets a scope (words matched to path parts, never a test file, else the main source folder). Tasks that share a file never share a wave and are taken in id order. No hard link is added for that: the first rehearsal showed that 44 such links skipped the whole backlog after one failure. Hidden dependencies: stronger planner rules and an ordering post-pass. Contradictions: with `park_contradictions` (set by `sentinel run`) both tasks are skipped at once with the reason, and the planner may add a note. A ticket that says it is finished is no longer skipped: the agent must check the code and add a test, and a task that changes nothing says the claim is not confirmed. Not yet released
| D129 | **Proof of new tests.** With `prove_new_tests` (set by `sentinel run`), a task that would be Verified must have new test files that fail on the code from before the task. The tree is rebuilt from git in a temp folder, with no worktree. Tests that also pass without the change make the task Partial. A check that cannot run leaves the task Verified and says so. Not yet released
| D130 | **Attempts per task are a setting.** `max_attempts` in the saved settings, default 3 (it was fixed at 3 in the router). After the last failed try the task ends Blocked. Each failed try moves to a stronger tier or agent. Other limits: 900 s per task (`task_timeout_seconds`), 15 min for the suite, 30 steps per OpenRouter task, 3 HTTP retries for a 429 or 5xx, a run cap that stops new tasks at 70 percent and kills workers at 95. Not yet released
| D131 | **Known limit of the write guard.** The guard watches the shared tree. It blocks and restores a write to a path no running task holds. It cannot tell which agent wrote, so a CLI agent that writes into a file held by another running task is not caught. The OpenRouter loop checks its scope in its own file tools. A checkpoint stages only the paths of its own task. Not yet released
| D132 | **First rehearsal on the practice repo (2026-10-27).** Result: 0 of 12 verified. Causes found and fixed: the OpenRouter account has $0 credit, so paid models answer HTTP 402 and the agent was not paused; the planner asked only the cheap and mid tier agents and never reached Claude, so the offline planner ran; the offline scope matched an existing test file, and Claude edited it, which the guard correctly blocked as tampering; hard ordering links for shared files skipped every later task after one failure. Now: a payment error pauses the agent and gives the try back to the task (up to 3 times), with a roster the strongest agent plans first and the others follow, tests are never a scope, and shared files add no links. Open: the account needs credit before the live run. Not yet released
| D133 | **Rehearsals 2 to 4 (2026-10-27).** What they found, and what was fixed. (1) When the suite failed at a barrier, every task of the wave was reverted. Now each task is checked alone on the wave's starting tree, and only the task that fails alone goes back, then the suite runs again for the rest. (2) A task whose agent wrote a file outside its scope lost that work to the guard and kept failing. Now a failed try adds the blocked paths to the scope (not protected files, not tests), and the planner is told to list every file a task changes, because a shared file only makes tasks run one after the other. (3) The planner answer was one long line cut at 4096 characters, so a 12 task plan could not be read. The tail now keeps lines up to 262144 characters and the stream still shows 4096. (4) The planner prompt let the model say that no request came through; the backlog prompt now has no way out of a plan. (5) A model that copied the example id `T-01` for a backlog that writes `T01` now matches the backlog, and the example uses a real id. (6) A plan inside another object, or a bad answer, is now said in the stream with the reason and how it began, and the same agent is asked once more with that reason. (7) Every task gets a new test file in its scope, in the folder and style the repository uses, because a task with no test file can never be Verified. Not yet released
| D134 | **`SENTINEL_NO_KEYCHAIN` and a clean state folder.** With `SENTINEL_NO_KEYCHAIN` set, `sentinel run` reads keys from the environment only and never asks the macOS Keychain, which blocks on a permission dialog for an unsigned build. `sentinel run` also adds `/.sentinel/` to `.git/info/exclude`, so its own state does not make the tree dirty. A run on a dirty tree stops with a clear message. Not yet released
| D135 | **Locked repositories and lenient dependencies (2026-10-27).** The practice repo locks `test/` and `acceptance/`: `scripts/test-lock.js` reports any new file there as `added`, and the repo's `AGENTS.md` forbids it, so a reviewer rejected a task that added `test/t05-kyc-tiers.test.js`. When the repo has `tests.lock.json`, new tests now go to `sentinel-tests/<id>.test.js`, and the detected Node suite command also runs them (`node --test sentinel-tests/*.test.js`). With `try_after_failed_dependency` (set by `sentinel run`), a task whose prerequisite ended without a checkpoint is still tried, because the first rehearsals skipped five tasks that way, and the practice grader showed that 7 of 12 tasks passed in a run that Sentinel itself called 2 Verified. `sentinel report --page` writes the run as one HTML page. Not yet released
| D136 | **Rehearsal 7 and three honesty fixes (2026-10-27).** Rehearsal 7 (OpenCode glm and deepseek, Claude Haiku, cap $5): Sentinel called 6 of 12 Verified (T01, T02, T03, T05, T08, T10), 3 Partial, 1 Blocked (T12), 2 Skipped (the T07 and T11 contradiction); the practice grader passed 9 of 12 on the final tree. The three Partial tasks failed only because a free model reviewer printed a banner or a diff line (fixed in D135 follow-up commits: the parser finds the YES or NO line, and up to three agents are asked). Three fixes from reading the run: (1) Checkpoints move HEAD through the private index, so the user's real index went stale and `git status` showed staged changes and untracked `sentinel-tests/`. `Git::sync_real_index` now runs `git reset --quiet` on the real index after every wave and at the end of the run; the work tree is not touched. (2) A CLI that prints its cost only at the end (Claude in json mode) was counted by a time guess of $5.40 an hour while it ran, and the guess stayed in the total after the real cost arrived, so the run showed $2.37 est. The budget now keeps reported spend and guessed spend apart, and a reported cost takes back the guess of that run; a run that never reports keeps its guess. (3) The report column `Receipt` showed the reviewer as `(OpenCode)`, which read like the writer; it now reads `(reviewed by OpenCode)`. (4) T12 stayed Blocked after 3 tries on Haiku. Its cash-out duty made the own tests of T02 fail (`sentinel-tests/t02.test.js`, a file Sentinel made, not a locked test), and the guard would not let T12 touch that file. Now a suite failure that names files in `sentinel-tests/` other than the task's own opens them to the task for its next try, the failure summary says they belong to earlier tasks and may be updated to match the ticket, and edits to those files are not counted as tampering. Not yet released |
| D137 | **Rehearsal 8 (2026-10-27).** Roster: OpenCode glm, OpenCode deepseek, `claude=haiku`, `claude=claude-haiku-5-5`; cap $5. Result: 9 of 12 Verified, 1 Partial (T05, the reviewer named an unrelated edit in `src/errors.js`), 2 Skipped (the T07 and T11 contradiction). The practice grader passed 10 of 12, which is the most possible while T07 and T11 contradict. T12 failed its first barrier on the own tests of T02, was allowed to update them (D136), and was Verified on the second try. Spend $0.25 of $5.00, all reported by Claude (no estimate). `git status` was clean after the run. The run took about 18 minutes of the 2 hour window. Weak points seen: no wave held more than one task (shared files such as `docs/api.md` and `src/api/routes.js` in the scopes make the tasks run one after another), and the reviewer of a Claude task is the other Claude agent, which is a different agent entry but the same model family. Not yet released |

## Open Questions

| # | Question | Needed by |
|---|---|---|
| Q1 | Watcher backstop: restore immediately or hold for review? Answered: restore at once and log it (D44) | Done |
| Q2 | Scope extension policy: always ask the scheduler, or allow small free-path edits? Answered: approval card, or grant and log in Bypass permissions (D44) | Done |
| Q3 | Exact headless flags for `agy`, `mcode`, `deepseek` | P3-06 |
| Q4 | HTTPS via `tailscale serve` by default? | P7-04 |
| Q5 | Can the phone start a run, or only steer one? | P7-06 |
| Q6 | Exact crate versions to pin | Each first use |
| Q7 | Announced budget cap and hidden-backlog format | Kickoff material |
| Q8 | Are notes and lessons from the practice backlog allowed to be read in the live run? Ask the organizers | Before P11-06 |
| Q9 | Trust threshold: 2 helped uses for every lesson kind? | P11-03 |
| Q10 | Token counting method for compaction | P3-10 |
| Q11 | Duplicate detection: own tool or external | P0-06 |
| Q12 | Smell checks on agent code when the target language is unknown | P4-07 |
| Q13 | Add an Antigravity-style Agent Manager home across all runs? | After v1 |
| Q14 | Which fonts to bundle, and license check. Answered: Inter and JetBrains Mono (D65) | Done |
| Q15 | Phone opens on Stream or Board? | P7-03 |
| Q16 | Obsidian-style Brain. Answered: all four uses plus scheduled care | Done |
| Q17 | A plainer name for leases. Answered: Reserved files | Done |
| Q18 | Telegram: which levels first? Answered: Notify, Control, Code, and Reports, all in v1 | Done |
| Q19 | Telegram: default detail? Answered: Detail (short diffs and agent snippets). Redaction and a never-sent list apply | Done |
| Q20 | Telegram: who talks to the bot? Answered: only you, one private chat | Done |
| Q21 | Telegram: what should coding from chat do? Answered: message one agent directly, and show a short diff summary | Done |
| Q22 | Telegram: must it work when the PC is asleep? Answered: yes, on an always-on machine | Done |
| Q23 | Telegram: `/task` and `/run` in v1? Answered: yes, both | Done |
| Q24 | How-to site: mdBook (Rust, proposed) or hand-written HTML and CSS? | Before P15-01 |
| Q25 | Always-on host on Windows: a service, or Task Scheduler? Not needed for v1 | Later |
| Q26 | The free worker quota (opencode Zen free models) ran out on 2026-10-24 around 21:00 EAT. If every free model fails, switch the opencode, goose, or pi account (or add credit) to resume free workers. Until then the build uses the OpenRouter free models, the local Ornith model, and Haiku subagents | When quota resets |
| Q27 | Phone rights: D68 lets a phone start a run, message an agent, and choose the repository (not during a run). `docs/mobile.md` used to say "Nothing else". Keep this, or narrow it to the old list? | Before P7 sign-off |
| Q28 | When do we add more than one project to one daemon (a project switcher and an Add project button, one store per repository)? D75 keeps it out of this version | After P7 |
| Q29 | Should an idle chat with a plain message ask "Plan only first?" before starting, or always follow the permission mode? D72 follows the permission mode | Before P6-18 |
| Q30 | Telegram quiet hours time zone. Answered: the daemon reads this computer's offset (`date +%z`, or `SENTINEL_UTC_OFFSET_MINUTES`) once, so quiet hours follow the owner's clock (D82) | Done |
| Q31 | Telegram `/task`, `/diff`, `/daily`. Answered: built (T-3). `/task` confirms, then appends a section to `backlog.md` | Done |
| Q32 | OpenRouter as a tool. Answered: built (D83). The key comes from the keychain or `OPENROUTER_API_KEY` | Done |

## Session Log

| Date | Summary |
|---|---|
| (undated) | Docs set drafted: README, overview, scope, architecture, structure, plan, standards, development, libraries, three UI docs, mobile, tracker |
| 2026-10-24 | Checked docs against the prototype. Prototype matches tokens, breakpoints, panels, modes, 7-stop tour, 13-step guide, 11 settings pages. Found and fixed doc drift (see Doc Changes Log). Created the repo and pushed. Smoke-tested workers: opencode, pi, goose, and local Ornith pass; agy headless returns nothing. No code yet |
| 2026-10-24 (late) | Merged the Brain panel (U-H), chat visuals (U-G2), duplicate cleanup (R-3), the OpenRouter worker (O-1), and Telegram in the daemon (T-2). Wrote the Brain memory and run journal (B-3b) with tests. Checked the chat UI in the browser at desktop and phone width: row menu, rename, plus menu, file picker, chips. Workspace tests: 1,243 pass before T-2. Queued U-I, U-K, and D-RUN briefs. The Agent tool is paused (D79) |
| 2026-10-25 | Merged Telegram `/task`, `/diff`, `/daily` (T-3), the Telegram UI card and panel (U-I), the host card (U-K), the dry run daemon and UI (D-RUN, U-J), the MCP server (M-1), host checks in `doctor` and `report --writeup` (H-1), the route access matrix (S-1), the OpenRouter tool (W-OR), and Routines backend (RT-1). Added Skills (F24) and Routines (F25) to the docs, core types, store tables, prototype, and site. The site now shows live prototype views with highlights and arrows (36 views) instead of screenshots. Real-agent smoke: Claude Haiku plans and writes; nothing reaches Verified yet because no second agent could review (D84). README added. CI: Linux build fixed (rfd without wayland), nextest timeouts |
| 2026-10-25 (later) | Owner asked for eight fixes while away. Haiku workers built them through the Claude CLI and the reviewer merged each: new chat reuse (D94), folder browser (D95), no pretend agents and a starter project (D96), a self-filling Brain (D97), live Changes, Tasks, and Reserved (D98), the Context panel (D99), resizable sidebar and panel (D100), and pairing in Settings with the right link (D101). One visual check at the end. Release v0.1.2 as a draft |

## File Changes Log

| Date | File | Change |
|---|---|---|
| (undated) | `docs/*` | Created the 13 doc files, README included |
| 2026-10-24 | `docs/`, `design/` | Moved from `sentinel-docs/` to the repo root. Added `.gitignore` |
| 2026-10-25 | `crates/*`, `docs/*`, `README.md` | Eight owner fixes (D94 to D101) and the review fixes (D102). New files: `brain_seed.rs`, `brain_survey.rs`, `agents_config.rs`, `starter.rs`, `context_usage.rs`, `changes.rs`, `routes/changes.rs`, `folder_browser.rs`, `pair_phone_card.rs`, `resize_handle.rs`, `diff_view.rs`, `working_tree.rs`, `live_fetch.rs`, `views/context.rs`. `--fake-agents` removed from the CLI |

## Doc Changes Log

| Date | Doc | Change |
|---|---|---|
| (add) | all | First draft |
| (add) | overview, scope, architecture, structure, plan, standards, development, libraries, ui-tokens, ui-rules, ui-registry, mobile, tracker | Added Lessons (F12), compaction and task ceiling (F13), agent loop mapping, extended smell rules, quality notes on agent diffs, Phase 11 |
| (add) | scope, architecture, structure, plan, libraries, ui-tokens, ui-rules, ui-registry, mobile, tracker | Owner feedback round: folder picker, found or missing agent list, Bypass permissions, Tailscale pairing checks, richer dry run, sidebar defaults, themed toasts, responsive fixes |
| (add) | README, scope, architecture, structure, plan, development, libraries, ui-rules, ui-registry, ui-tokens, mobile, tracker | Telegram: token plus user ID, one chat, `/task` and `/run`. Always-on host (F21). How-to site (F22) with Phases 14 and 15. Cog icon and swatch row fixes |
| (add) | scope, architecture, structure, plan, standards, development, libraries, ui-rules, ui-registry, ui-tokens, mobile, tracker | Leases renamed Reserved files. Lessons folded into the Brain (F12) with scheduled care (F19). Telegram scoped (F20) with Phase 13 |
| (add) | README, overview, scope, architecture, structure, plan, standards, development, libraries, ui-tokens (rewritten), ui-rules, ui-registry, mobile, tracker | Synced with the interactive prototype: stream-first workspace (F14), permissions (F15), agent switching (F16), appearance settings (F17), install and connections (F18), user flow, new tokens and breakpoints |

| 2026-10-24 | overview, scope, architecture, structure, development, build-plan, ui-rules, ui-registry, tracker | Drift fixes against the prototype and decisions: feature list numbers match F1 to F22, Telegram default detail is Detail, Telegram setup has no pairing code, structure doc fence and heading, `--r` radius, eleven settings pages, open questions live only here. Added goose and pi (D42). Closed Q1 and Q2 (D44) |
| 2026-10-25 | scope, architecture, build-plan, development, tracker, README, site install, always-on machine, troubleshooting, panels | Desktop window docs: F26, Desktop Window section, Phase 18, D93. `project-overview.md` still says F1 to F25 and needs F26 |
| 2026-10-24 | `docs/*.md and README.md` | Chat wording pass (D71) |

## Notes for Later

- Measure real disk and cost against a worktree baseline. These numbers are the strongest proof for the write-up.
- Idea: a "replay" view that plays back a finished run from the log.
- Idea: let the planner learn which files tend to be touched together to predict scope better.
- Idea: share proven lessons across machines on a team tailnet. Typed data only.
- Prototype: https://claude.ai/artifact/JjSCEQ47a586qrr554tzy4 (source file to be saved as `design/sentinel-prototype.html`).
- Idea: after the Brain proves out, let care runs propose richer skills, with the same proof rule.
- Idea: a shared team Brain over the tailnet, with typed lessons only.
- Check how each CLI behaves when its edits are blocked. Some may loop. Add a retry limit per task.
