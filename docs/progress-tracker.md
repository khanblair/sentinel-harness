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

**Phases 0 to 5 mostly built, UI in progress.** A full run with the scripted fake agents on the sample repo gives honest statuses (`cargo nextest run -p sentinel-daemon --test engine_run`). `cargo xtask ci` is red until the cleanup pass (smell and duplicate findings). The UI is being ported from the prototype.

## Phase Status

| Phase | Name | Status |
|---|---|---|
| P0 | Foundations | todo |
| P1 | State and Reserved Files | todo |
| P2 | Single-Tree Guard | todo |
| P3 | Workers and Detection | todo |
| P4 | Planner, Verifier, Report | todo |
| P5 | Daemon, Log, Resume | todo |
| P6 | Web UI Core | todo |
| P7 | Phone over Tailscale | todo |
| P8 | Onboarding | todo |
| P9 | Guided Tour | todo |
| P10 | Hardening | todo |
| P11 | Brain | todo |
| P12 | Brain Care | todo |
| P13 | Telegram | todo |
| P14 | Always-On Host | todo |
| P15 | How-To Site | todo |

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
| P3-07 | OpenRouter worker loop | unassigned | todo |
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
| P5-05 | Real agents on the sample repo | unassigned | todo |
| P6-01 to P6-13 | Web UI core | opencode | doing |
| P7-01 to P7-06 | Phone over Tailscale | unassigned | todo |

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

## Session Log

| Date | Summary |
|---|---|
| (undated) | Docs set drafted: README, overview, scope, architecture, structure, plan, standards, development, libraries, three UI docs, mobile, tracker |
| 2026-10-24 | Checked docs against the prototype. Prototype matches tokens, breakpoints, panels, modes, 7-stop tour, 13-step guide, 11 settings pages. Found and fixed doc drift (see Doc Changes Log). Created the repo and pushed. Smoke-tested workers: opencode, pi, goose, and local Ornith pass; agy headless returns nothing. No code yet |

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

## Notes for Later

- Measure real disk and cost against a worktree baseline. These numbers are the strongest proof for the write-up.
- Idea: a "replay" view that plays back a finished run from the log.
- Idea: let the planner learn which files tend to be touched together to predict scope better.
- Idea: share proven lessons across machines on a team tailnet. Typed data only.
- Prototype: https://claude.ai/artifact/JjSCEQ47a586qrr554tzy4 (source file to be saved as `design/sentinel-prototype.html`).
- Idea: after the Brain proves out, let care runs propose richer skills, with the same proof rule.
- Idea: a shared team Brain over the tailnet, with typed lessons only.
- Check how each CLI behaves when its edits are blocked. Some may loop. Add a retry limit per task.
