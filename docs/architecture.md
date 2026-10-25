# Architecture

## System Diagram

```
 Phone / Desktop browser (Leptos UI, PWA)
            |  HTTPS or HTTP over Tailscale, WebSocket
            v
 +---------------------- sentinel daemon (one process) ----------------------+
 |  API (axum)  <->  Event bus (tokio broadcast)                              |
 |      |                 |                                                   |
 |  Planner   Scheduler + Reservation table   Budget guard   Verifier + Report      |
 |  Brain (markdown vault + index): read by planner, router, prompts          |
 |      |            |                        |             |                |
 |      +------------+---- Store (SQLite) ----+-------------+                |
 |                   |                                                        |
 |        Worker supervisor --> Adapters --> child CLI processes              |
 |                   |                                                        |
 |        Tree guard (checkpoint, watcher, restore)  +  MCP file server       |
 +----------------------------------|-----------------------------------------+
                                    v
                 ONE git working tree (the user's repo)
```

## Processes

| Process | Role |
|---|---|
| `sentinel` daemon | Everything above, single process, many tokio tasks |
| Agent child processes | One per running task, spawned and killed by the supervisor |
| `git` | Short-lived plumbing calls |
| Test runner | Short-lived, launched by the verifier |

The CLI binary `sentinel` is the same executable. It starts the daemon or talks to a running one.

## Agent Loop Mapping

Sentinel is the **outer loop**. Worker CLIs run the inner reason-act-observe loop. Sentinel owns an inner loop only for its OpenRouter worker.

| Step | Where it happens | Sentinel part |
|---|---|---|
| 1. Request and trigger | Backlog, CLI, phone | Planner, run loop |
| 2. Context and skill loading | Sentinel builds the prompt | Task, scope, Brain notes and proven lessons, retry summary (`context.rs`) |
| 3. Model reasoning | Worker (CLI or API) | Router picks worker and tier |
| 4. Policy and permission check | Sentinel | Reserved files, MCP file tools, hooks, watcher, tamper guard, budget guard |
| 5. Execute and observe | Worker runs tools, Sentinel captures | Supervisor, output capture |
| 6. Context management | CLI workers do their own. Sentinel does retries and its own loop | `context.rs`, task ceiling |
| 7. Verify and loop decision | Sentinel | Barrier, verifier, retry, escalate, block |

Limit: for CLI workers, step 4 is complete only where the CLI supports hooks. Elsewhere the MCP tools and the watcher restore stray writes after the fact.

### Context Rules

| Item | Rule (defaults in `config.toml`) |
|---|---|
| Retry prompt parts | Task, scope, Brain notes, proven lessons, failure summary, diff stat |
| Failure summary | Failing test names, first 20 lines of each error, last 30 lines of output, 4 KB cap |
| Raw logs | Never fed back. Kept in the log by reference |
| Brain notes in a prompt | Pinned first, then linked to the task's files, then search hits. 500 token cap shared with lessons |
| Own loop tool output | First 40 and last 40 lines |
| Own loop compaction | Above 70% of the model window, summarize old steps to under 300 tokens. Keep task, scope, plan, last 4 steps |
| Compaction cost | Counts against the cap. `ContextCompacted` event logs before and after |
| Task ceiling | 2x the tier estimate. Over it: kill, then escalate or block |

## Modules and Ownership

| Module (crate) | Owns |
|---|---|
| `sentinel-core` | Shared types, ids, errors, event definitions |
| `sentinel-store` | SQLite schema, migrations, queries |
| `sentinel-reserve` | Reservation table, scope math, wave builder, scheduler |
| `sentinel-tree` | Checkpoints, filesystem watcher, restore, tamper detection |
| `sentinel-workers` | Worker trait, adapters, CLI detection, budget wrapper and task ceiling, context builder and compaction, MCP file server |
| `sentinel-plan` | Backlog parsing, planner calls, task graph |
| `sentinel-brain` | Vault files, links and backlinks, search index, agent memory selection, run journal, lesson rules, care runs and proposals |
| `sentinel-telegram` | Telegram bot: long polling, command router, message formatter, outbox, allowlist and pairing. Implements the `Notifier` interface |
| `sentinel-verify` | Test gate, tamper guard, verifier, quality notes, report |
| `sentinel-daemon` | HTTP API, WebSocket, auth, Tailscale, pairing, permissions, approvals, connections |
| `sentinel-cli` | Binary entry, commands |
| `sentinel-ui` | Leptos front end |

## Internal Communication

- **Event bus:** a `tokio::sync::broadcast` channel carries `Event` values. The store persists each event. The WebSocket forwards them.
- **Commands:** `mpsc` channels from the API to the scheduler and supervisor.
- **Kill signal:** one `watch<bool>` channel the budget guard flips. Every worker selects on it.
- **State of record:** SQLite. Memory is a cache. Anything needed after a crash is written first.

## Key Flows

### Run

0. A message in an idle chat starts the run. The message, with its attachments, is the request. A backlog attachment gives one task per section. Plain text gives the planner the whole message to turn into tasks. The run belongs to the chat that started it, and every event of the run carries the chat id.
1. Load backlog and repo. Planner builds the task graph (scopes, dependencies, contradictions, already-done flags).
2. Scheduler builds Wave 1: ready tasks with disjoint scopes, capped by worker count and budget.
3. For each task: reserve files, choose a worker, spawn it with the task prompt, the MCP file server, and a guard config.
4. Tree guard watches the tree. Writes to files the task has not reserved are blocked or reverted and logged.
5. A finished worker is checkpointed: stage only reserved paths with a private index, commit.
6. When the wave drains, the barrier runs: tree must be clean, then the full suite.
7. Verifier checks each task in the wave and writes the status. Quality notes are attached and never change the status.
8. The Brain journal is written from the wave (learn mode only).
9. Next wave. Repeat until nothing is ready or the budget stops the run.
10. Report is generated from the store.

### Escalation

Verification fails or the worker times out. Task returns to Ready with the next tier. Failure output is added to the prompt. After the retry limit it becomes BLOCKED.

### Brain Journal and Learning

1. A barrier ends. In `learn` mode the journal writer reads evidence from the store: tasks, verdicts, reserved files, tier outcomes, failure classes, real vs estimated cost.
2. It writes a run note, one note per task, and a note per touched file, linked to each other. Existing notes get a backlink line, never a rewrite of text a user wrote.
3. It writes or updates **candidate** lesson notes. A lesson note has typed front matter only: kind, key, value, evidence, helped, hurt, state. No task text.
4. Before a task, the memory selector picks notes: pinned, then linked to the task's files, then search hits, then strongest first, within the token cap. The stream lists the notes loaded.
5. After the task, each lesson used is marked helped, neutral, or hurt, and each note used gains a use and a little strength.
6. Trust rules: 2 helped and 0 hurt makes a candidate **proven**. Any hurt makes it **rejected**. Long unused makes it **retired**.
7. In `read-only` mode steps 1 to 3 and 5 are skipped. In-run adaptation (a retry prompt using this run's failure) uses run memory that is dropped at the end.

### Scope Extension

Agent calls `request_files(path)` via MCP. Free path: granted and logged. Held path: the task waits up to a limit, then yields and returns to Ready after the holder checkpoints.

### Contradiction

Planner flags two tasks. Sentinel sends a decision prompt to the phone. No answer within the timeout: default is skip both and record CONTRADICTION.

### Crash Resume

1. Open the store, read the last state.
2. Compare tree to the last checkpoint. Restore any uncommitted file changes.
3. Mark interrupted tasks Ready. Release their reserved files.
4. Continue the run.

### Pairing

1. Daemon checks Tailscale with `tailscale status --json`. Result is one of: not installed, installed but signed out, running. The UI shows install steps, a sign-in button, or goes on.
2. With Tailscale running, the daemon reads this computer's address and name.
3. It shows a QR code, a link, the machine name (if MagicDNS is on), and a one-time code.
4. The phone opens the link and submits the code. The PC asks the user to approve the new device.
5. The device gets a token, stored on the device and, as a hash, in the store. It is revocable.

## State Machines

### Task

```
Planned -> Ready -> Reserved -> Running -> Checkpointed -> Verifying -> VERIFIED
                      ^          |             |               |
                      |          v             v               v
                      +------- Failed <---------+-------------- Failed
Failed -> Ready (retry, higher tier) | BLOCKED (limit reached)
Planned -> CONTRADICTION | SKIPPED | PARTIAL (some checks pass)
```

### Reservation

```
Requested -> Granted -> Released
                |-> Revoked (crash, kill, violation)
Requested -> Waiting -> Granted | Abandoned
```

### Harness

```
Idle -> Onboarding -> Ready -> Running <-> Paused -> Stopping -> Done
Running -> Crashed -> Resuming -> Running
```

## Data Model (SQLite)

| Table | Key columns |
|---|---|
| `chats` | id, project (repository name), title, pinned, archived_at, created_at, updated_at |
| `attachments` | id, chat_id, kind (file, folder, backlog, note, upload), name, path_or_text, size, created_at, used_by_message (the event id that carried it, null until sent) |
| `tasks` | id, title, body, status, tier, attempts, scope_json, flags |
| `task_deps` | task_id, depends_on, kind (order, read-after-write) |
| `reservations` | id, task_id, path, state, reserved_at, released_at |
| `runs` | id, task_id, tool_id, model, started_at, ended_at, exit, cost_micro_usd, cost_estimated |
| `checkpoints` | id, task_id, commit_sha, paths_json, created_at |
| `barriers` | id, wave, started_at, suite_result, output_ref |
| `verifications` | task_id, tests_pass, new_test, reviewer_ok, tamper_clear, verdict |
| `tools` | id, name, path, version, adapter, status, tier |
| `devices` | id, name, token_hash, created_at, revoked_at |
| `decisions` | id, kind, prompt, answer, actor (human or default), at |
| `events` | id, ts, kind, chat_id (null for events that belong to no chat), task_id, payload_json |
| `settings` | key, value |
| `telegram_chats` | id, telegram_user_id (one row), level flags (notify, control, code, reports), detail (status, summary, detail), events_json, quiet_hours, connected_at, revoked_at |
| `telegram_outbox` | id, chat_id, kind, payload_hash, state (queued, sent, failed), tries, created_at |
| `quality_notes` | task_id, kind, location, detail_code, created_at |
| `skills` | name, scope (project, personal), mode (off, auto, always). The skill text is in the files, not here |
| `routines` | id, name, kind (morning, sundown, reminder, backlog_run), minute_of_day, days_mask, enabled, to_telegram, text, backlog, last_fired_on, created_at |
| `briefs` | id, routine_id, kind, title, body, created_at, sent_to_telegram |
| `permissions` | agent_id, capability (read, write, ext, tests, shell, net), allowed |
| `approvals` | id, kind (extension, wave, plan, decision), task_id, agent_id, path, state (pending, allowed, denied, defaulted), decided_by (device or default), decided_at |

The Brain vault is a folder of markdown files in `<repo>/.sentinel/brain/`. Files are the source of truth. `brain.db` (SQLite, in `.sentinel/`) is an index that can be rebuilt from the files at any time:

| Table | Key columns |
|---|---|
| `notes` | id, type, title, path, tags, by (you, run, care), strength, confirmed_at, uses, pinned, state (lessons only) |
| `links` | from_id, to_id |
| `fts` | full-text search over title, body, tags |
| `proposals` | id, kind (add, update, retire, merge), target_id, why, state (open, accepted, dismissed), created_at |
| `care_runs` | id, started_at, finished_at, cost_micro_usd, summary |

Money is stored as integer micro-dollars.

## API

### Routes (JSON over HTTP)

| Route | Purpose |
|---|---|
| `GET /api/state` | Full snapshot |
| `GET /api/tasks`, `GET /api/tasks/:id` | Task list and detail |
| `GET /api/tasks/:id/stream` | Agent output (SSE or WS topic) |
| `GET /api/reserved` | Reserved files map |
| `GET /api/cost` | Spend, cap, estimate flags |
| `GET /api/tools`, `POST /api/tools/scan` | Detected CLIs, rescan |
| `GET /api/chats?archived=` | The chats of the project, newest first, pinned first. Each row has title, pinned, archived, run state, verified and total counts, and whether it needs you |
| `POST /api/chats` | Create an empty chat. Body: optional title. Returns the chat |
| `PUT /api/chats/:id` | Rename, pin or unpin, archive or unarchive. Body: any of `title`, `pinned`, `archived`. Archive is refused while the chat has a live run |
| `DELETE /api/chats/:id` | Delete a chat, its events, and its uploads. Refused while the chat has a live run. The computer only |
| `GET /api/chats/:id/events?after=&limit=` | The stored events of one chat, for opening an old chat |
| `POST /api/chats/:id/attachments` | Add an attachment. Body: `kind` and `path` for a repository file, folder, or backlog; `text` for a note; `name` and `content_base64` for an upload (at most 1 MB each). Returns the attachment |
| `DELETE /api/chats/:id/attachments/:attachment` | Remove an attachment that was not sent yet |
| `POST /api/run`, `/pause`, `/resume`, `/stop` | Harness control. `POST /api/run` takes `chat`, `text`, and `attachments` (ids). With `text` the planner makes the tasks. With a backlog attachment each section is a task. A project with a live run refuses a second run and names the busy chat |
| `POST /api/tasks/:id/kill` | Kill one agent |
| `POST /api/cap/lower` | Lower the cap. No raise route exists |
| `POST /api/decisions/:id` | Answer a contradiction prompt |
| `GET /api/report` | Honest report |
| `POST /api/pair`, `GET/DELETE /api/devices` | Pairing and revoke |
| `GET /api/onboarding`, `POST /api/onboarding/:step` | Onboarding state |
| `GET /api/brain/notes?q=&type=` | Search and list notes (`NoteList` in `sentinel-core/src/brain_api.rs`, which holds every Brain body) |
| `POST /api/brain/notes` | Write a new note (`NewNote`). The computer only |
| `POST /api/brain/notes/:id/pin` | Pin or unpin a note (`PinNote`): a pinned note is always loaded for agents. The computer only |
| `GET /api/brain/notes/:id`, `PUT /api/brain/notes/:id` | Read a note. Edit a note (PC only, not from a phone) |
| `GET /api/brain/graph` | Nodes and links for the graph |
| `POST /api/brain/lessons/:id/review` | Approve, reject, or retire a lesson note |
| `POST /api/brain/mode` | Move the Brain mode to a stricter value (`learn` to `read-only` to `off`) |
| `GET /api/brain/proposals`, `POST /api/brain/proposals/:id` | List proposals. Accept or dismiss one |
| `GET /api/brain/care`, `PUT /api/brain/care`, `POST /api/brain/care/run` | Care schedule, settings, and run now |
| `GET /api/tasks/:id/quality` | Quality notes for a task |
| `GET /api/permissions`, `PUT /api/permissions` | Permission mode and the per-agent capability matrix |
| `GET /api/approvals`, `POST /api/approvals/:id` | Pending approvals, and allow or deny |
| `POST /api/agents/:id/kill` | Kill the task an agent is running |
| `PUT /api/routing` | Set which agent serves each tier |
| `GET /api/connections`, `POST /api/connections/:name/test` | Connection state and a test action |
| `POST /api/tools/:id/fix` | Log in or save a command template for a tool |
| `POST /api/doctor` | Run the install check and return the result |
| `GET /api/host`, `POST /api/host/check` | Host state, last heartbeat, and a check now |
| `GET /api/whoami` | Who is calling: `from_pc` and the device name. The UI reads it to tell a phone from the computer |
| `GET /api/skills`, `POST /api/skills` | List skills with their switch, and create one. Create needs the computer |
| `GET /api/skills/found`, `POST /api/skills/import` | Skills in other tools' folders, and copy one into Sentinel. Computer only: the answer names paths on this computer |
| `GET /api/skills/:name`, `PUT /api/skills/:name`, `DELETE /api/skills/:name` | Read a skill. Edit text or move its switch. Delete. A phone may read and move a switch toward Off |
| `GET /api/routines`, `POST /api/routines` | List routines, and create one. Create needs the computer |
| `PUT /api/routines/:id`, `DELETE /api/routines/:id` | Edit or delete a routine. A phone may only turn one off |
| `POST /api/routines/:id/run` | Send now: build the brief or start the run at once |
| `GET /api/briefs?limit=` | The last briefs, newest first |
| `GET /api/telegram`, `PUT /api/telegram` | Bot state, levels, events, detail, quiet hours |
| `POST /api/telegram/token` | Save a bot token to the keychain and check it |
| `POST /api/telegram/connect` | Save the token and user ID, check the bot, and wait for `/start` |
| `POST /api/telegram/test` | Send a test message |
| `DELETE /api/telegram/chats/:id` | Revoke a chat |
| `GET /api/dry-run`, `POST /api/dry-run` | Last dry run (stages, result), and start one on the built-in sample project. Computer only. 409 while a run or another dry run is live |
| `GET /api/tailscale/status` | Not installed, signed out, or running, with address and name |
| `POST /api/tailscale/signin` | Run `tailscale up` and report the result |
| `POST /api/fs/pick` | Open the native folder dialog on the PC and return the chosen path |
| `GET /api/fs/list?path=` | Folder browser for a phone, limited to the home folder. Allowed from a phone |
| `POST /api/repo/check` | Check git, backlog.md, and a clean tree for a folder |
| `POST /api/repo/starter-backlog` | Create a starter backlog.md with example tasks |

### WebSocket `/ws`

Server sends `Event` frames. Client sends `Subscribe{topics}` and `Ping`. No control commands travel over the socket. Control uses the routes above.

### Events

`ChatChanged` (created, renamed, pinned, archived, deleted), `RunStarted`, `PlanReady`, `WaveStarted`, `Verdict`, `RunFinished`, `ReportReady`, `Message`, `TaskStatus`, `FilesReserved`, `FilesReleased`, `WriteBlocked`, `Checkpoint`, `BarrierStart`, `BarrierEnd`, `CostUpdate`, `BudgetThreshold`, `ToolDetected`, `AgentOutput`, `DecisionRequested`, `HarnessState`, `DeviceAction`, `NoteWritten`, `LessonStateChanged`, `ProposalCreated`, `CareFinished`, `QualityNote`, `ContextCompacted`, `TaskCeilingHit`, `ApprovalRequested`, `ApprovalResolved`, `PermissionChanged`, `ToolStatusChanged`, `ConnectionChanged`, `TailscaleChanged`, `TelegramPaired`, `TelegramSent`, `TelegramReceived`, `HostHeartbeat`, `HostBackOnline`, `DryRunStage`, `DryRunFinished`, `RoutineFired`, `RoutineSkipped`, `BriefReady`.

### Shared Types

Defined once in `sentinel-core` and used by both daemon and UI (same Rust types, no schema drift): `ChatId`, `ChatInfo`, `Attachment`, `AttachmentKind`, `TaskId`, `TaskStatus`, `Reservation`, `Scope`, `Event`, `Cost`, `ToolInfo`, `Decision`, `Note`, `NoteType`, `Lesson`, `LessonState`, `Proposal`, `QualityNote`, `PermissionMode`, `Capability`, `Approval`.

## Files on Disk

| Path | Content |
|---|---|
| `<repo>/.sentinel/state.db` | SQLite store |
| `<repo>/.sentinel/log.jsonl` | Append-only run log |
| `<repo>/.sentinel/report.md` | Latest honest report |
| `<repo>/.sentinel/config.toml` | Settings and routing |
| `<repo>/.sentinel/adapters/*.toml` | Adapter profiles |
| `<repo>/.sentinel/guard/` | Generated guard config for agents |
| `<repo>/.sentinel/chats/<chat>/attachments/` | Files uploaded into a chat. Removed when the chat is deleted |
| `<repo>/.sentinel/brain/` | The Brain vault: markdown notes, `_proposals/` for open proposals. Open it in Obsidian |
| `<repo>/.sentinel/brain.db` | Search and graph index. Rebuildable from the vault |
| `<repo>/.sentinel/skills/NAME/SKILL.md` | A project skill, with any files beside it |
| `~/.sentinel/skills/NAME/SKILL.md` | A personal skill, shared by every project |
| Browser storage on each device | Appearance preferences and color token overrides. Not stored on the daemon |

`.sentinel/` is git-ignored. No other folder is created in or next to the repo.

## Security Model

| Threat | Control |
|---|---|
| Public exposure | Bind to the Tailscale address only. Refuse `0.0.0.0` |
| Stolen link | One-time pairing code. Per-device token, revocable |
| Cross-site requests | Origin check, token in header, no cookie auth |
| Phone abuse | Fixed route list. No shell, no file edit, no test edit. Cap can only go down. Approvals only if the user allows it |
| Secret leakage | API keys in the OS keychain. Never sent to the UI. Redacted in logs |
| Agent writes to unreserved files | MCP tools, hooks, watcher restore |
| Test tampering | Tamper guard rejects diffs touching existing tests |
| Hidden backlog leak | Logs and reports exclude task text outside the run folder |
| Lesson notes leak task text | Lesson notes hold typed front matter only. A test seeds a task title and fails if it reaches a lesson note. Run, task, and file notes may hold task text, stay on this computer, and are never sent anywhere (Telegram included) |
| A bad lesson entrenches | Candidate until proven. Harm rejects it. Review and retire from the phone |
| Phone loosens learning | Phone can only move the mode toward `off`. Moving toward `learn` needs the PC |
| Runaway cost | Hard budget guard |
| Folder browsing | Browser limited to the home folder. Paths outside it are rejected. The native dialog is used on the PC |
| Telegram reads messages | Telegram can read bot messages. Default detail is Detail, with redaction and the never-sent list. Contents of files, keys, env values, and Brain notes are never sent. Hackathon mode sends status only |
| Stolen Telegram account | Allowlist plus level. Control and Code can be turned off. Risky actions need the PC. The cap only goes down. Revoke from the PC. Messages never hold whole files, keys, or note text |
| Telegram button replay | One-time tokens, 10 minute expiry, bound to chat and action |

## Brain

| Part | Detail |
|---|---|
| Vault | `<repo>/.sentinel/brain/`. One `.md` file per note. Front matter: `id`, `type`, `tags`, `by`, `strength`, `confirmed`, and for lessons `kind`, `key`, `value`, `evidence`, `helped`, `hurt`, `state` |
| Links | `[[note-id]]` in the body. Backlinks are computed. A link to a missing note is shown as dead |
| Watcher | `notify` watches the vault. Edits from Obsidian or an editor reindex within a second. Sentinel writes use atomic replace and skip a file changed since it was read |
| Generated text | Journal text sits between `<!-- sentinel:start -->` and `<!-- sentinel:end -->` markers. Text outside the markers is yours and is never rewritten |
| MCP tools for agents | `brain_search`, `brain_read`, `brain_propose`. No write tool. A proposal becomes a file in `_proposals/` |
| Memory selection | Pinned, linked to the task's files, search hits, strongest first. Token cap shared with lessons |
| Journal | Run, task, and file notes. Written in `learn` mode only |
| Care scheduler | A timer in the daemon. Frequency off, daily, weekly, monthly, or after N runs. A missed run starts on the next launch. One care run at a time |
| Care run | Refresh, consolidate, improve, prune. Cheapest ready agent. Own budget. Output is proposals, a digest note, and strength changes |
| Proposals | Add, update, retire, merge. A change to a note a user wrote always waits for an accept. Optional auto-apply covers only notes Sentinel wrote |
| Strength | 0 to 1. Rises on use and confirmation. Falls when unused. Used to order selection and to propose retirement. Nothing is deleted automatically |

### Care Run Flow

1. The scheduler starts a care run (or the user presses Run care now).
2. Refresh: compare changed files with file notes. Update or mark stale.
3. Consolidate: write a digest note for old run notes. Flag duplicates.
4. Improve: read recent run, task, and fix notes. Create proposals.
5. Prune: lower strength of unused Sentinel-written notes. Propose retirement.
6. Write proposals as files in `_proposals/` and rows in `proposals`. Emit `ProposalCreated` and `CareFinished`. The sidebar shows the care run under Scheduled.
## Skills

| Part | Detail |
|---|---|
| Format | A folder with `SKILL.md`. Front matter holds `name` and `description`. The body is markdown. Extra keys are kept and ignored. The same file works for Claude, Codex, and Antigravity |
| Sources | Sentinel's own two folders (project and personal), read at start and when a file changes. Other tools' folders are only listed, and import copies the folder |
| Limits | Name: lowercase letters, digits, dashes, at most 48. Description: at most 300 characters. `SKILL.md` at most 20 KB. Folder at most 200 KB. A name that exists in both places: the project skill wins |
| Switch | Off, Auto, Always per skill, in the `skills` table. A new skill is Auto |
| Selection | Per task: every Always skill, then the Auto skills ranked by word overlap between name plus description and the task's title, body, and file names. At most 3 Auto skills. All inside 1,500 tokens. A skill that does not fit is skipped whole |
| Prompt | A `## Skills` section between the task block and the rules. The stream gets `Skills loaded: ...` |
| Hand pick | `/skill NAME` adds a chip, stored as an attachment of kind `skill`. It goes with the next message |
| MCP | `skill_list` and `skill_read`, read only |
| Never | Sentinel never runs a file from a skill folder. A skill never changes reservations, the cap, or the mode |

## Routines

| Part | Detail |
|---|---|
| Scheduler | One task in the daemon wakes every 30 seconds, converts now to local time (the computer's offset, as for Telegram quiet hours), and fires each enabled routine whose time has passed on an allowed day and whose `last_fired_on` is not today |
| Missed runs | At start, a routine whose time passed less than 2 hours ago fires once. Older ones are skipped with `RoutineSkipped` |
| Kinds | `morning` and `sundown` build a brief. `reminder` sends its text. `backlog_run` starts a run through the same path as a chat message, as a phone caller, and is refused in Bypass permissions or when a run is live |
| Brief builder | A pure function of the store: tasks, the last run, approvals and decisions pending, Brain proposals open, cost this week and today, tools ready, host status, and the hackathon flag. No model call |
| Privacy | A brief never holds code, files, keys, environment values, or note text. With hackathon mode on it holds counts only |
| Delivery | A `briefs` row, then the `BriefReady` event. The Telegram notifier sends it when the chat has the Reports level, with redaction and the length cut, and quiet hours do not hold it |
| Telegram | `/brief` and `/wrapup`. `/daily` stays the short day summary (verified today, spend today) |

## Desktop Window

The macOS app shows Sentinel in its own window. The window is built only with the `window` cargo feature, which is gated to macOS.

| Part | Detail |
|---|---|
| Threads | The main thread owns the window and the event loop. A tokio runtime on other threads runs the daemon, the same code as `sentinel daemon` |
| Load | The window loads `http://ADDR/#t=TOKEN`. `ADDR` is the daemon's own address, and the token signs the window in as this computer |
| Close and quit | Closing the window hides it. The daemon keeps running, so the phone and Telegram keep working. A Dock click shows the window again. Cmd+Q, the Sentinel menu, Settings, General, Quit, `POST /api/quit`, or `sentinel stop` quits the app and stops the daemon |
| Navigation | Only the daemon's own address loads in the window. Any other link opens in the default browser |
| Downloads | Export report saves to the Downloads folder |
| Menu bar | Sentinel, Edit, View, and Window menus. Edit holds copy, paste, and select all, so text fields work |
| File drop | Files dragged into the window reach the UI |
| Developer tools | Off |
| No custom protocol | The daemon's origin check and the phone already use the same http address. A custom protocol would need a second origin and a second rule |

## Always-On Host

| Part | Detail |
|---|---|
| Mode | `sentinel daemon --headless`. No window, no native dialogs. UI served to your devices over Tailscale |
| Service | `sentinel service install` writes a systemd unit (Linux) or a launchd job (macOS) that starts at boot and restarts on failure. `sentinel service uninstall` removes it. Windows later |
| What lives on the host | The repository, the agent CLIs and their logins, the keychain entries, the vault, the Telegram poller, the care scheduler |
| Heartbeat | The daemon writes a heartbeat every few seconds. The app shows the last one. After a gap of more than a minute the next start sends a back-online message |
| Back online | On start after a gap, the daemon reconciles the tree with the last checkpoint, pauses any run that was in flight, and tells Telegram and the app |
| Folder choice | Folder browser limited to the home folder. No native dialog |
| Checks | `sentinel doctor` on the host checks the service, Tailscale, agent CLIs, repository, and sleep settings where the OS reports them |
| Limit | Nothing outside the host can tell you it is down. An outside heartbeat check is a later option |


## Telegram

| Part | Detail |
|---|---|
| Transport | Outbound HTTPS to the Telegram Bot API. Long polling for updates. No inbound port, no webhook |
| Bot token | In the OS keychain. Checked with a `getMe` call when saved |
| Auth | Allowlist of exactly one numeric Telegram user ID, entered with the bot token. The user then sends `/start`. Other senders are ignored and logged |
| Levels | Notify, Control, Code, Reports, set per chat. Each level maps to a fixed list of API routes, the same routes the UI and the phone use |
| Router | A command or button press becomes one API call. It never does more than its route allows |
| Buttons | Inline keyboard. Each button carries a one-time token, valid 10 minutes, bound to the chat and the action |
| Formatter | Three detail levels: Status only, Summary, Detail (the default). Detail adds short diffs and agent snippets, at most 6 lines. A redaction pass replaces key-like text with `[hidden]`. Hackathon mode forces Status only. A test checks that no message contains whole files, keys, environment values, or note text |
| Outbox | Queue with retry and backoff, de-duplication, batching, and one live message that is edited as a run progresses. Quiet hours hold all but approvals and budget alerts |
| Limits | Messages are split or shortened under 4096 characters. Send rate is limited |
| Notifier | A small interface. Telegram implements it now. Other channels can later |
| Devices | A paired chat appears in the devices list with kind `telegram`. It can be revoked like a phone |

### Telegram Flows

1. **Connect:** the user enters the bot token and their user ID. The daemon calls `getMe`, stores the token in the keychain, and waits for `/start` from that ID. On `/start` the chat is connected.
2. **Notify:** an event arrives on the bus. The notifier checks the chat's event list, quiet hours, and detail level, formats the text, and queues it.
3. **Approve:** an approval card is created. The notifier sends it with Allow and Deny buttons. A press arrives as an update. The router checks the token and the chat, then calls the approval route.
4. **Task from chat:** `/task <text>` shows a confirm button. On confirm the daemon appends the task to `backlog.md`, plans it, and replies with files and conflicts.
5. **Run from chat:** `/run [task]` shows a confirm button. On confirm it starts a run in the current permission mode. It is refused in Bypass permissions. In Plan only or Ask each wave it sends approval cards.
6. **Agent message:**  `/agent claude <text>` becomes a message to that agent. It is applied on the agent's next step. It never changes reserved files.
7. **Diff summary:** `/diff [task]` reads the latest checkpoint (or the named task) and formats a short summary at the chat's detail level.

## Permission Modes and Approvals

| Mode | Before writing | Extension request | Wave start |
|---|---|---|---|
| Plan only | One approval card after the plan | Approval card | No card after the plan is approved |
| Ask each wave | None | Approval card | Approval card per wave |
| Auto inside reserved files (default) | None | Approval card | None |
| Bypass permissions | None | Free path granted and logged | None |

- The reservation guard is the same in every mode. A write to a file the task has not reserved is blocked and restored.
- After a Plan only approval the run switches to Auto inside reserved files and says so in the stream.
- Per-agent capabilities are checked before a mode applies. A capability that is off wins over the mode.
- An approval card has a state machine: pending, then allowed, denied, or defaulted. A pending card marks its chat with an attention dot and a phone alert.
- Every decision is logged with who made it (a device name or the default).

## Performance Budgets

| Item | Budget |
|---|---|
| Reserving files | Under 10 ms |
| Event to phone | Under 300 ms |
| Phone first paint | Under 2 s on a mid phone over Tailscale |
| UI bundle (WASM, compressed) | Under 1.5 MB |
| Daemon idle memory | Under 100 MB |
| Extra disk per run | Under 50 MB |
| Watcher overhead | Under 5% of one core |
| Note selection per task | Under 20 ms |
| Brain notes in a prompt | Under 500 tokens |
| Failure summary in a retry | Under 4 KB |

## Key Decisions and Trade-offs

| Decision | Gain | Cost |
|---|---|---|
| One tree, reserved files | No disk copies, clear view | A guard is needed. Reads may see half-written files |
| Waves and barriers | Clean, trustworthy test runs | Less parallelism at barrier edges |
| Plumbing commits with private index | Per-task history without checkout | More git detail to get right |
| Shell out to `git` | Small build, fewer bugs | Process spawn cost |
| SQLite as truth | Crash safety, simple | Single-writer limits |
| Leptos UI in Rust | One language, shared types | WASM size, newer ecosystem |
| Phone is a PWA | No store, same code as desktop | No deep native features |
| MCP file server plus watcher | Strong guard across tools | Two guard paths to keep aligned |
| Markdown vault as the source of truth | Readable by you, opens in Obsidian, survives tool changes | The index must be rebuilt from files, and outside edits can conflict with Sentinel writes |
| Trust after proof | Stops bad lessons spreading | Learning is slower |
| Smells as warnings on agent code | No wasted retries, hidden tests stay the priority | Smelly code can be VERIFIED |
| Task ceiling | Stops one runaway CLI from eating the cap | A hard task may be killed too early. Escalation path softens it |
| Reserved files as the only hard boundary, modes decide who answers | One simple guarantee, familiar mode names | "Bypass permissions" can grant more than a user expects, so it is shown in the warn color |
| Appearance stored per device | Phone and desktop can differ, no daemon state | Settings do not follow the user between devices |
| Native folder dialog through the daemon | No typed paths, full path kept | The dialog opens on the PC, so a phone needs a folder browser |
| Telegram by long polling | Nothing exposed, works behind any network | Needs the PC on and online. Latency of a second or two |
| wry window around the local web UI (chosen) versus Tauri or Electron | Small and ours. The UI and the daemon stay as they are, and the window is a thin shell over them | Only macOS for now. The window uses the macOS web view, so its behavior follows the system version. Tauri or Electron would bring a second framework, build, and runtime, which the project does not need |

## Automated Quality Checks

`cargo xtask ci` runs in this order:

1. `cargo fmt --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo nextest run`
4. `cargo deny check`
5. `cargo machete`
6. `xtask smell-check` (limits in code-standards)
7. `xtask dup-check` (copied blocks)
8. `xtask slop-check` (banned words and patterns in docs, UI copy, comments)
9. `xtask doc-check` (structure matches project-structure)
10. `xtask ban-check` (no `git worktree`, no banned crates)
11. `xtask contrast-check` (token pairs meet contrast rules)

Any failure blocks merge of Sentinel's own code.

### Quality on Agent Code

The verifier runs quality checks on each agent diff at the barrier.

| Check | Looks for |
|---|---|
| Size | Very long added functions, very long files, deep nesting (language-neutral heuristics) |
| Duplication | Added blocks copied from elsewhere in the diff or repo |
| Slop | Comments restating code, placeholder code, TODO with no ID, tests with no assertion |
| Tamper | Already a hard gate. Not a note |

Results are `quality_notes`. They appear in the report and task detail. They never change VERIFIED. Worker prompts include the size limits as guidance. Language-aware parsing is a v1.1 item.
