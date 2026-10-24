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
| P3-08 | MCP file server | unassigned | todo |
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

## File Changes Log

| Date | File | Change |
|---|---|---|
| (undated) | `docs/*` | Created the 13 doc files, README included |
| 2026-10-24 | `docs/`, `design/` | Moved from `sentinel-docs/` to the repo root. Added `.gitignore` |

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
