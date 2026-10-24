# Product Scope

## Problem

Running one coding agent is easy. Running several on one codebase breaks:

- Two agents edit the same file.
- One agent builds on work another has not finished.
- An agent reports "done" with nothing checked.
- The usual fix, git worktrees, copies the repo per agent. That eats disk, hides what is happening, and pushes the merge pain to the end.

## Solution

Sentinel keeps **one working tree** and makes agents safe inside it. It grants each task an exclusive reservation on the files it may write, runs together only tasks whose reserved files do not overlap, guards writes, checkpoints each task as its own commit, and runs the full test suite at clean barriers. Only the verifier can call a task done.

## Who It Is For

| User | Need |
|---|---|
| Solo developer with several AI CLIs | One place to run them all on one repo |
| Hackathon entrant | Provable coordination, cost control, honest report |
| Developer away from the desk | Watch and steer the run from a phone |

## Principles

See [project-overview.md](project-overview.md). Short form: tests decide, one tree, visible by default, cheap first, fail loudly, phone is a remote.

## Core Concepts

See the table in [project-overview.md](project-overview.md). Full terms in the glossary below.

## Features

Each feature: what, how, why.

### F1. Single-Tree Coordination

- **What:** Many agents edit one tree with no collisions and no worktrees.
- **How:**
  1. Planner predicts each task's write scope.
  2. Scheduler reserves files. A file has at most one writer.
  3. Tasks with disjoint scopes form a wave and run together.
  4. A write guard blocks or reverts writes to files the task has not reserved.
  5. Each finished task becomes a checkpoint commit made with git plumbing and a private index, so no checkout is needed.
  6. At the barrier between waves, the tree is clean and the full suite runs.
  7. An agent needing more paths asks for a scope extension. It gets it if free, otherwise waits or yields.
  8. If task B must read the final form of a file task A writes, the planner adds a dependency edge. B waits for A's checkpoint.
- **Why:** Zero extra disk, one place to look, and conflicts are prevented, not merged later.

### F2. CLI Auto-Detection

- **What:** On first run Sentinel finds the agent CLIs already on the PC.
- **How:**
  1. Scan `PATH` for known names: `claude`, `codex`, `opencode`, `gemini`, `goose`, `pi`, `agy`, `deepseek`, `mcode`, plus any in a user list.
  2. Probe each with a version command with a short timeout.
  3. Check for signs of login (config directory, environment key, or a harmless status command).
  4. Match to an adapter profile. Unknown tools become a generic adapter with a command template the user confirms.
  5. Show results in onboarding step 2 with a checkbox, version, and "ready / needs login / failed" status.
  6. Re-scan on demand from Settings.
- **Why:** The user should not hand-write config for tools they already installed.

### F3. Cheap-First Routing

- **What:** Each task goes to the cheapest capable worker. Failures escalate.
- **How:** Planner rates difficulty. Router picks a tier. Failed verification or timeout moves the task up a tier, within the budget.
- **Why:** Cost is scored, and most tasks are easy.

### F4. Verified-Only Done

- **What:** A task is VERIFIED only when all hold: full suite passes at the barrier, a new test covers the change, a different model confirms the diff matches the task, and no existing test file was edited.
- **How:** Verifier runs after merge to the checkpoint chain. Status is written from evidence, never from agent text.
- **Quality notes:** Smells and slop found in agent diffs are listed in the report as notes. They never change the verdict.
- **Why:** Honesty of "done" claims is scored and is the core product promise.

### F5. Hard Budget Guard

- **What:** Spend can never exceed the cap.
- **How:** Every model call and every CLI run reports cost through one wrapper. If a tool cannot report cost, Sentinel estimates high from tokens or time. At 70% no new tasks start. At 95% all workers are killed through a shared signal. Each task also has a ceiling (default 2x its tier estimate). A task over its ceiling is killed, then escalated or blocked.
- **Why:** A runaway agent must not drain credits.

### F6. Crash Resume

- **What:** Restart the same command and pick up where it stopped.
- **How:** All task, reservation, cost, and checkpoint state lives in SQLite. On start, Sentinel reconciles the tree against the last checkpoint, restores stray changes, and requeues interrupted tasks.
- **Why:** Required by the hackathon and basic reliability.

### F7. Full-Harness Phone UI

- **What:** The whole harness, not a summary, on the phone over Tailscale.
- **How:** The daemon serves the same Rust UI to desktop and phone, bound to the Tailscale address only. Pairing uses a QR code. See [mobile.md](mobile.md).
- **Why:** Control the run away from the desk with no cloud service.

### F8. Onboarding (5 steps)

1. Choose the repository with your computer's own folder picker. Sentinel checks for git, a backlog.md, and a clean tree.
2. Find agent CLIs on this computer and show each one as found or missing.
3. Set budget cap, permission mode, routing preferences, and Brain mode.
4. Pair the phone over Tailscale. Sentinel first checks that Tailscale is installed and running, then shows the steps for the phone, a link, and a one-time code.
5. Dry run on a built-in sample project. A short rehearsal that never touches your repository. The screen says what it does, which agents it uses, and what it costs, and ticks off each stage live.

Before step 1 an install check confirms Rust, the WASM target, trunk, git, and Tailscale, and offers to install what is missing (see F18). Skippable steps are marked. Everything onboarding sets is also a config file.

### F9. Guided Tour

- **What:** A coach-mark tour in seven stops: the chat list, the agent switcher, the panel icons, the stream, the permission chip, the send-to chip, and settings.
- **How:** Anchored tooltips, one at a time, keyboard and swipe navigable, replayable from Settings, with a sample run so there is something to point at.
- **Why:** First-time users should understand the board in two minutes.

### F10. Honest Report and Run Log

- **What:** One report with a status per task and a receipt per VERIFIED task. One append-only log of everything.
- **How:** Generated from the database. Statuses: VERIFIED, PARTIAL, BLOCKED, CONTRADICTION, SKIPPED.
- **Why:** Process evidence and honesty are scored, and the user needs a trustworthy record.

### F11. Quality Checks

| Check | Catches | Tool |
|---|---|---|
| Format and lint | Style, common bugs | `rustfmt`, `clippy` |
| Code smell limits | Long functions, deep nesting, big files | clippy config and `xtask smell-check` |
| Unused dependencies | Dead crates | `cargo-machete` |
| License and ban list | Banned or risky crates | `cargo-deny` |
| AI slop in docs, copy, comments | Banned words and patterns | `xtask slop-check` |
| Doc drift | Docs not matching files | `xtask doc-check` |
| Worktree ban | Any `git worktree` use | `xtask ban-check` |
| Test tamper | Edited existing tests in agent diffs | Sentinel tamper guard |
| Duplicate code | Copy-pasted blocks | `xtask dup-check` |
| Smells and slop in agent code | Long functions, deep nesting, placeholders, filler comments, tests with no assertion | Verifier quality notes (warnings only) |

### F12. Brain

- **What:** A folder of plain markdown notes, linked to each other like Obsidian notes. Agents read it as memory, every run writes to it, lessons live in it as typed notes, and you browse and edit it in a Brain panel (notes, search, backlinks, graph). The same folder opens in Obsidian.
- **How:**
  1. **Vault.** `<repo>/.sentinel/brain/` holds one markdown file per note, with front matter (type, tags, strength, source) and `[[links]]`. Files are the source of truth. A search index is rebuilt from them, so edits made in Obsidian show up in Sentinel.
  2. **Note types.** Convention, decision, fix, file, task, run, lesson, glossary, how-to.
  3. **Agent memory.** Before each task Sentinel picks notes: pinned ones first, then those linked to the task's files, then search hits, within a 500 token cap. The stream says which notes were loaded. Agents can search and read notes through MCP tools and can propose a note. They cannot write one.
  4. **Run journal.** In learn mode each run writes a run note, a note per task, and a note per touched file. Backlinks then show which tasks touched a file.
  5. **Lessons as notes.** A lesson is a note with typed front matter only: kind, key, value, evidence, helped, hurt, state. A new lesson is a **candidate**. After 2 helped uses and no harm it is **proven**. A harmful use makes it **rejected**. A lesson unused for a long time is **retired**. Lesson notes never hold task titles or backlog text.
  6. **Modes.** `off`, `read-only`, `learn`. Live runs use `read-only`: agents read, nothing is written. A phone can only move the mode toward `off`.
  7. **Brain panel.** Notes list with search and type filter, a note view with backlinks, an editor, a graph view, and a proposals tab.
  8. **Phone.** Read notes, view the graph, review lessons, accept or dismiss proposals. Editing notes is done on the PC, because notes become agent prompts.
- **Why:** Plain files outlast any tool, are readable by you, and give agents project memory that gets better with use. Typed lessons keep the learning safe. Notes that mention task text stay on this computer.

### F13. Context Compaction and Task Ceiling

- **What:** Sentinel controls what it feeds back into a model, and stops a task that burns too much.
- **How:**
  1. A retry prompt holds: task, scope, proven lessons, Brain notes, failure summary, diff stat. The failure summary is failing test names, the first 20 lines of each error, and the last 30 lines of output, capped at 4 KB. Raw logs never go back. Full output stays in the log by reference.
  2. In Sentinel's own OpenRouter loop, each tool output is cut to its first 40 and last 40 lines. When context passes 70% of the model window, older steps are summarized by the cheap model to under 300 tokens. Task, scope, plan, and the last 4 steps are kept.
  3. Compaction cost counts against the cap. A `ContextCompacted` event records tokens before and after.
  4. CLI workers compact their own context. Sentinel limits them with timeouts and the per-task cost ceiling.
- **Why:** Tokens are money, and very long contexts make agents worse. All numbers above are defaults in `config.toml`.

### F14. Stream-First Workspace

- **What:** A layout people already know from Claude, Claude Code, and Codex: a sidebar of chats grouped by project, one stream, and panels that slide in.
- **How:**
  1. The sidebar lists chats by project (a repository). Each chat shows its title, a status icon, a verified count, a dot when it needs you, and a phone icon when it can be steered from a phone. New chat sits at the top. Archived chats sit in a collapsed group at the bottom. Scheduled Brain care runs sit under Scheduled. The sidebar is open by default on tablet and desktop. [F23](#f23-chats-projects-and-attachments) says how chats work.
  2. The stream shows the messages of the open chat and the work Sentinel did for them: the plan, waves, agent steps, file checkpoints, barriers, verdicts, approvals, and the report, in order.
  3. Top-bar icons open panels: Board, Terminal, Changes, Reserved, Brain, Remote. The More menu opens Files, Cost, Report, Agents, and Permissions.
  4. Panels open as tabs on the right and can be expanded or closed. On tablet and phone they slide over the stream.
  5. The composer takes plain instructions and slash commands: `/pause`, `/resume`, `/stop`, `/cap`, `/agent`, `/focus`, `/kill`, `/plan`, `/brain`, `/report`, `/help`. The plus button adds files, folders, a backlog, or a note ([F23](#f23-chats-projects-and-attachments)).
  6. Ctrl or Cmd + K opens a command palette for chats, agents, panels, and settings.
- **Why:** Familiar layout means no manual. The stream is the product, and everything else sits beside it.

### F15. Permissions and Approvals

- **What:** One setting decides how much agents may do without asking. Reserved files stay the hard boundary.
- **How:**
  1. Four modes: **Plan only**, **Ask each wave**, **Auto inside reserved files** (default), **Bypass permissions**.
  2. A write to a file the task has not reserved is always blocked and restored. No mode changes that.
  3. An agent that needs one more file raises a request for more files. In Auto inside reserved files and stricter modes it becomes an approval card. In Bypass permissions a free file is granted and logged.
  4. Plan only adds a card before any writing. Ask each wave adds a card before every wave.
  5. A capability matrix sets, per agent: read, write reserved files, ask for more files, run tests, run shell commands, network.
  6. Approval cards can be answered from a paired phone unless that is turned off.
  7. An unanswered contradiction defaults to skipping both tasks.
  8. Choosing Bypass permissions opens a confirm dialog that says what changes and what stays enforced: reserved files, the test-edit guard, and the budget cap.
- **Why:** The same control model people know from the coding tools, with the collision guarantee underneath.

### F16. Agent Switching

- **What:** See, focus, and steer any agent.
- **How:**
  1. The top bar shows an avatar stack of ready agents, each with a state dot.
  2. Opening it lists each agent with the kind of task it handles, its current task, and cost. Per agent: focus the stream, open its terminal, or kill it.
  3. The send-to chip in the composer sends messages to Sentinel (which routes by difficulty) or straight to one agent.
  4. Task routing is set in Settings: which agent takes easy, normal, and hard tasks. A failed task escalates one step up.
  5. A message never changes which files a task has reserved.
- **Why:** Several agents are only useful if each one can be seen and steered.

### F17. Appearance Settings

- **What:** Theme, color, and type are the user's choice, per device.
- **How:** Settings, Appearance holds theme (system, light, dark), accent (six swatches or custom), surface tint (neutral, warm, cool), interface font, code font, text size 12 to 18 px, stream width, density, corner radius, per-theme color tokens, a live contrast readout, reset, and copy as CSS. Preferences are stored on each device, so the phone can differ from the desktop.
- **Why:** People read this screen for hours. Fit matters, and changing it should not need code.

### F18. Install and Connections

- **What:** A first-run install check and a live view of what Sentinel connects to.
- **How:**
  1. The install check lists Rust, the WASM target, trunk and check tools, git, and Tailscale. It offers to install what is missing and ends with the `sentinel doctor` result. Commands are shown for macOS, Linux, and Windows.
  2. The Connections page shows state and a test action for Tailscale, OpenRouter (the key goes to the OS keychain), the MCP file server, git, and the keychain.
  3. The Agents page lists detected tools as found or missing, with a switch to use each one. A tool that needs a login or a command template has a fix action. Task routing is set in Settings, not on this list.
  4. Folder choice uses the operating system's own folder dialog on the computer that runs Sentinel. Nobody types a path. From a phone the same choice is a folder browser limited to the home folder.
  5. Pairing checks come first: before showing a code, Sentinel checks that Tailscale is installed, running, and signed in. It then shows install steps, a sign-in button, or the phone steps with a link, a machine name, and a code.
- **Why:** Setup problems should be visible and fixable in one place.

### F19. Brain Care (scheduled runs)

- **What:** A scheduled run that looks after the Brain so its memory evolves instead of rotting.
- **How:**
  1. **Schedule.** Off, daily, weekly, monthly, or after every 5 runs. It runs while Sentinel is on. A missed run starts on the next launch. Care runs appear in the sidebar under Scheduled.
  2. **Refresh.** Compare changed files with file notes. Update or mark notes stale.
  3. **Consolidate.** Summarize old run notes into a digest. Find duplicates.
  4. **Improve.** Look for patterns across runs and propose new conventions, updates to existing notes, merges, and test or backlog ideas.
  5. **Prune.** Weaken notes nobody uses (strength falls). Propose retirement. Never delete.
  6. **Proposals.** Every change to a note you wrote is a proposal in the Brain panel (also saved as a file in `_proposals/`). Accept or dismiss each one. A setting can let care apply changes to notes Sentinel wrote, never to yours.
  7. **Budget.** Each care run has its own small budget (default $0.10) and uses the cheapest ready agent. It stops at the budget.
  8. **Strength.** Every note has a strength. It rises when agents use it and when you confirm it, and falls when it goes unused. Weak notes are loaded last.
- **Why:** A notebook that is never tended gets stale and starts misleading agents. Care keeps it small, current, and honest, and you stay in charge of what changes.

### F20. Telegram

- **What:** A Telegram bot that is one more remote for Sentinel. It notifies you, lets you answer approvals with one tap, steers a run, passes a short instruction to one agent, shows a diff summary, adds tasks to the backlog, and starts runs, from anywhere, without opening the app.
- **Why Telegram:** It is already on your phone, it pushes notifications when no app is open, and it works on mobile data with no VPN.
- **Decided by the owner:** all four levels in v1 (Notify, Control, Code, Reports). Code means: message one agent, show a diff summary, add a task (`/task`), start a run (`/run`). Default detail is **Detail**. **One private chat, only you.** It runs on an **always-on machine** (see F21).
- **How it connects:**
  1. The Sentinel daemon calls the Telegram Bot API outward and polls for new messages (long polling). No port is opened and no webhook is needed. The host must be on and online.
  2. You create the bot in Telegram with @BotFather. In Settings, Integrations you enter two things: the **bot token** and **your Telegram user ID** (a number, not your @username). The token goes to the OS keychain.
  3. Sentinel keeps an allowlist of exactly one user ID: yours. You send `/start` to the bot (a bot cannot message you first) and the chat is connected. Messages from any other ID are ignored and logged.
- **Four levels, set for the chat:**

| Level | What it allows |
|---|---|
| Notify | Sentinel sends events to the chat |
| Control | Approve or deny, answer decisions, pause, resume, stop, kill an agent, lower the cap |
| Code | Message one agent directly. Ask for a short diff summary. Add a task to backlog.md. Start a run of one task or the whole backlog |
| Reports | A summary when a run finishes and a daily summary, with cost and Brain proposals |

- **Events you can switch on:** approval needed, decision needed, budget at 70% and 95%, task blocked, wave finished, run finished, Brain care proposals, errors and crashes. Quiet hours hold everything except approvals and budget alerts. Events are grouped, and progress edits one live message instead of sending many. When the host comes back after being down, the bot says so.
- **Detail levels:** Status only (task IDs and states), Summary (titles, file names, cost), **Detail** (adds short diffs and agent snippets, at most 6 lines). Detail is the default. Hackathon mode forces Status only.
- **What is never sent, at any level:** whole files, keys, environment values, and Brain note text. Anything that looks like a key is replaced with `[hidden]` before sending. Proposal titles may appear at Detail. Note text does not.
- **Example messages:**

```
Sentinel, fee-ledger-api
Wave 2 done. 3 of 3 verified.
T-04 Verified: src/ledger/post.ts
T-02 Verified: src/fees/rules.ts
T-06 Verified: src/limits/daily.ts
Cost $0.88 of $2.00
[Pause]  [Diff summary]
```

```
codex asks for another file
T-05 wants to write src/api/errors.ts. Nobody has reserved it.
[Allow]  [Deny]
```

```
You: /task Add a rate limit to transfers
Sentinel: Add this task to backlog.md?  [Add task]  [Cancel]
Sentinel: Added T-13 to backlog.md.
Plan: 2 files (src/limits/rate.ts, src/api/transfer.ts). No conflicts.
[Run T-13]  [Later]
```

- **Coding through Telegram:**
  - `/agent claude <text>` sends one agent a message, applied on its next step with the files it has reserved.
  - `/diff [task]` returns a short diff summary of the latest checkpoint or a named task.
  - `/task <text>` shows a confirm button, then appends the task to `backlog.md`, plans it, and replies with the files and any conflicts.
  - `/run [task]` shows a confirm button, then starts a run of that task or of the whole backlog in the current permission mode.
  - A message never changes which files a task has reserved.
- **Commands:** `/status`, `/pause`, `/resume`, `/stop`, `/cap 1.50`, `/kill <agent>`, `/agent <name> <text>`, `/diff [task]`, `/task <text>`, `/run [task]`, `/report`, `/daily`, `/brain <search>` (titles only), `/help`.
- **Security:**
  - Allowlist of one Telegram user ID. Unknown senders get no reply and are logged.
  - The bot token lives only in the OS keychain. It is never sent to a chat or written to a log.
  - Telegram bots are not end-to-end encrypted, so Telegram's servers can read the messages. Detail is the owner's choice. Redaction and the never-sent list limit the risk. Status only is one tap away.
  - A button carries a one-time token that expires after 10 minutes and cannot be replayed.
  - A run started from Telegram never uses Bypass permissions. In Plan only and Ask each wave it still sends approval cards.
  - Some things need the PC: switching to Bypass permissions, raising the cap, editing notes or tests, and running shell commands. The bot refuses them and logs the attempt.
  - `/task`, `/run`, stop, and kill ask for a confirm tap.
  - Groups and channels are not supported.
  - Every Telegram action is logged.
- **Limits:** Telegram caps a message at 4096 characters and limits how fast a bot may send, so Sentinel batches, edits one live message, and keeps a retry queue. A bot cannot message you first: you start the chat.
- **Hackathon:** In hackathon mode messages show status only, so no task titles or code leave the computer (rule 9). Task text you type in `/task` is not echoed back.
- **Out of scope for v1:** more than one user, groups, channels, voice notes, attachments, WhatsApp or Slack (a `Notifier` interface leaves room), shell commands, editing tests or notes.
- **Phases:** 1 Notify and Reports, 2 Control, 3 Code (`/agent`, `/diff`, `/task`, `/run`), 4 Polish (quiet hours, batching, hackathon mode).
- **In the prototype:** Settings, Integrations has the setup (bot token, user ID, `/start` check, levels, detail, events, quiet hours, test message, activity log) and the always-on host card. The Telegram panel is a simulated chat that receives real events from the demo run, with working Allow, Deny, Pause, and decision buttons, and answers `/task` and `/run`.

### F21. Always-On Host

- **What:** Run Sentinel on a machine that stays on, such as a server or a spare computer, so runs and Telegram keep working while your own computer sleeps.
- **How:**
  1. The daemon runs headless on the host. `sentinel service install` sets it to start at boot and restart if it stops (a systemd service on Linux, a launchd job on macOS. Windows comes later).
  2. The repository, the agent CLIs (installed and logged in), and the OS keychain live on the host. Agents run there.
  3. The host joins Tailscale. Your PC, phone, and tablet open the app at the host's address. Telegram polls from the host.
  4. On a headless host, choosing a folder uses the folder browser, not the native dialog.
  5. Settings shows the host's state: service running, Tailscale, agent CLIs, repository, last heartbeat.
  6. When the host comes back after being down, the bot sends "Sentinel is back online after N minutes" and says which run was paused.
- **Why:** A remote is only useful if the thing it controls is awake.
- **Limits:** If the host loses power or network, nothing can tell you, because Telegram cannot notice silence by itself. A later version can add an outside heartbeat check. A laptop that sleeps is not an always-on host.

### F22. How-To Site

- **What:** A simple static site on GitHub Pages that shows how to install, set up, and use Sentinel, one feature per page, with the interactive prototype embedded.
- **How:** mdBook (written in Rust) builds the site from markdown in `site/`. GitHub Actions builds and publishes it on every push to main. The theme uses the app's colors and fonts, with light and dark. Pages: Home, Get started, Using Sentinel, Brain, Remote, Reference, Guides, Try it, Help. Every content page has the same parts: what it is, numbered steps, a screenshot, a tip, and related pages. The structure is shown in `design/site-structure.html`. Screenshots come from the prototype in both themes. The slop check and a link check run on the site text.
- **Why:** New users need a place to learn that is not the app.
- **When:** Built in the coding phase (Phase 15). Only the structure exists now.

### F23. Chats, Projects, and Attachments

- **What:** Sentinel is a coding tool you talk to. You open a chat in a project, say what you want built, and Sentinel plans it, runs the agents, and verifies the result. It works the way Claude does: a list of chats, a plus button to add things, and chats you can rename, pin, archive, and delete.
- **Words:** A **project** is a repository folder. A **chat** is one conversation with Sentinel in a project. A **run** is one pass of the harness (plan, waves, verify, report) that a message starts. A chat holds many runs, one after another. The word "run" stays for the harness work (Pause run, Stop run, run report). The word "chat" is what you open, name, and keep.
- **How:**
  1. **New chat.** The New chat button at the top of the sidebar, a plus on each project header, the shortcut Ctrl or Cmd + Shift + O, and the palette all open an empty chat in a project. The title is empty until the first message. Then it becomes the first line of that message (cut at 48 characters), or the backlog file name. You can rename it.
  2. **Starting work.** A plain message in an idle chat starts a run. Sentinel's planner turns the message into one or more tasks. A backlog file can be attached instead, and then each section of the file is a task, as before. A message in a chat with a live run goes to the agents as a note (F16). One run is live per project, because there is one working tree. Another chat that tries to start work in the same project is told which chat is busy.
  3. **The plus button.** The composer's plus opens a menu: **Add files** (choose from the repository's files, or upload from this device), **Add a folder** (a folder of the repository, as a hint about where the work belongs), **Use a backlog file** (a file in the repository, or an upload), and **Add a context note** (text that goes into the agents' prompts). Each choice becomes a chip above the text box with an x to remove it. Chips travel with the next message. Files and folders reach the planner and the agents as paths, with the file text included when a file is small. Uploads are saved under `.sentinel/chats/CHAT/attachments/`, never inside the tracked tree. Attached files are hints for the planner. They do not reserve files.
  4. **Chat menu.** A row in the sidebar has a menu (three dots, or a long press on a phone) with Rename, Pin, Archive, and Delete. The title in the top bar opens the same menu plus Export report. Rename edits in place. Pin keeps a chat at the top of its project. Archive hides a chat in the Archived group, and Unarchive brings it back. Delete asks first, removes the chat's messages and uploads, and never touches the repository or its commits. A chat with a live run cannot be archived or deleted until the run is stopped.
  5. **Search.** The search button filters chats by title, across projects.
  6. **Projects.** The sidebar groups chats under their project. One Sentinel daemon serves one project in this version. The model, the API, and the screens already speak in projects, so adding more projects later (a project switcher and an Add project button) needs no redesign.
  7. **History.** A chat keeps its messages and the events of its runs, so reopening an old chat shows what happened, including the honest report. The Board, Reserved, and Cost panels show the project's live run, or the last run when none is live.
  8. **Phone.** A phone may create a chat, send, rename, pin, and archive. Deleting a chat needs the computer ([mobile.md](mobile.md), Security).
- **Why:** People already know this shape from Claude. A coding tool you talk to needs chats you can keep, name, and clean up, and a plus button for the things you want to hand it. A backlog file is one way to say what you want, not the only way.
- **Limits:** A chat is not a Q&A thread with a model. Every message either starts work, steers running work, or is a slash command. Questions about the code go to an agent through the send-to chip.

## User Flow

How to use Sentinel properly. The in-app guide walks through the same steps with a try-it button on each.

1. **Install.** Build Sentinel, install Tailscale, run `sentinel doctor`.
2. **Set up.** Choose the repository, enable at least 3 agents across 2 tools, set the cap and permission mode, pair a phone, finish with the dry run.
3. **Write the backlog.** A `backlog.md` in the repo, one heading per task, with what done means. Do not list files. Sentinel finds the scope.
4. **Read the plan.** Check scopes, hidden dependencies, contradictions, and tasks that claim to be done. Use Plan only until the plan can be trusted.
5. **Answer decisions.** Pick one of two contradicting tasks, or skip both. No answer means both are skipped and recorded as Contradiction.
6. **Pick permissions and Brain mode.** For live runs keep the Brain on read-only.
7. **Watch the stream.** Open panels from the top bar. Use the agent stack to focus, open a terminal, or kill.
8. **Approve requests.** Allow a free, sensible file. Deny to keep the agent to its own files.
9. **Trust the verdicts.** Verified needs a passing suite on a clean tree, a new test, a review by a different agent, and no edited tests. Quality notes are warnings only.
10. **Handle blocked tasks.** Read the reason. Raise the tier, narrow the scope, or fix the cause, then run again.
11. **Watch the budget.** 70% stops new tasks. 95% stops workers. The cap only goes down.
12. **Control it from a phone.** Pair over Tailscale. The phone can pause, kill, answer cards, lower the cap, and review the Brain. It cannot raise the cap or touch files.
13. **Review the Brain.** Read the run note, approve only the lessons that helped, fix notes that are wrong, and accept or dismiss care proposals.

## Integrations

| Integration | Role |
|---|---|
| Git | Checkpoint commits via plumbing |
| Tailscale | Private network path to the phone |
| Agent CLIs | Workers |
| OpenRouter and model APIs | Cheap worker and verifier |
| MCP | Sentinel's own file server given to agents |

## Out of Scope

- Native iOS or Android app.
- Hosted or multi-user service.
- Non-git projects.
- Training or fine-tuning models.
- Merging by worktree or branch-per-agent.
- Editing the user's tests automatically.

## Roadmap

| Stage | Contents |
|---|---|
| v1 | Features F1 to F22 |
| v1.1 | Per-tool cost accuracy, more adapters, richer reserved files map, parser-based smell checks on agent code |
| v2 | Optional native shell, multi-repo runs, shared team tailnet |

## Risks

| Risk | Mitigation |
|---|---|
| CLI agents bypass guards and write anywhere | MCP file tools preferred, hooks where supported, filesystem watcher as backstop that restores the file |
| An agent reads a file mid-edit by another | Read-after-write dependency edges, barrier-clean tests |
| Half-finished edits break the build for others | Waves limit overlap. Failures during a wave are "inconclusive", retried at the barrier |
| Unknown CLI flags differ by version | Adapter profiles are data files, verified at detection |
| CLIs under a subscription report no dollars | Estimate high, show "estimated" label |
| Rust compile and borrow friction | Simple ownership, clone freely, tidy later |
| A bad lesson entrenches | Candidate until proven. Harm rejects it. Human review. Easy retire |
| Lesson notes leak backlog text | Lesson notes hold typed front matter only. A test seeds a task title and fails if it reaches a lesson note. Run, task, and file notes may hold task text, stay on this computer, and are never sent anywhere |
| Persisted notes seen as using the hidden backlog | Live run is read-only: nothing is written. Only notes from the practice backlog are read. Ask the organizers (Q8 in progress-tracker.md) |
| Bypass permissions is chosen without understanding it | A confirm dialog says what stays enforced. Warn color on the chip |
| Compaction drops something the agent needs | Keep task, scope, plan, last steps. Full output stays in the log. Retry once with a larger summary before blocking |

## Open Questions

Open questions live in [progress-tracker.md](progress-tracker.md).

## Benchmarks

| Metric | Target |
|---|---|
| Writes to unreserved files reaching a checkpoint | 0 |
| Extra disk vs a worktree-per-agent baseline | At least 90% less |
| Time agents spend blocked on reserved files | Under 15% of agent time |
| Cost per VERIFIED task | Tracked, lower each release |
| Honest-claims ratio (VERIFIED that pass rerun) | 100% |
| Reserving latency | Under 10 ms |
| Phone event latency | Under 300 ms |
| Scope prediction accuracy (predicted vs written paths) | Rises across runs when the Brain is on |
| Retry success rate | Rises across runs when the Brain is on |
| Prompt tokens spent on retries | Falls once compaction is on |
| Lesson hit rate (proven lessons that helped) | Tracked per lesson kind |
| Notes used by agents | Share of loaded notes an agent actually opened or cited |
| Stale notes | Falls after each care run |

## Glossary

| Term | Definition |
|---|---|
| Project | A repository folder Sentinel works in. Chats are grouped by project |
| Chat | One conversation with Sentinel in a project. It holds messages and many runs. You name, pin, archive, and delete chats |
| Run | One pass of the harness started by a message: plan, waves, verify, report. At most one is live per project |
| Attachment | A file, folder, backlog, or note added with the composer plus button. It goes with the next message |
| Adapter | Code mapping one CLI to the Worker interface |
| Barrier | Clean-tree pause where the full suite runs |
| Checkpoint | Commit of one task's reserved paths |
| Contradiction | Two tasks that cannot both be true |
| Reserved files | The files one task has reserved. No other task can write them until the task is checkpointed |
| Always-on host | A server or spare computer that runs Sentinel, the repository, and the agents all the time |
| Scope | Paths a task is predicted or granted to write |
| Tamper | An agent edit to an existing test |
| Wave | Tasks whose reserved files do not overlap, running together |
| Compaction | Cutting model-bound context to a fixed size |
| Lesson | A note with typed front matter: an evidence-backed fact from past runs |
| Brain | The vault of linked markdown notes for one repository |
| Note | One markdown file in the Brain, with front matter and links |
| Brain care | A scheduled run that refreshes, merges, and prunes notes and proposes improvements |
| Quality note | Smell or slop warning on agent code |
| Task ceiling | Per-task cost limit that kills a runaway task |
| Easy, normal, hard tasks | The words the UI uses for the cheap, mid, and smart tiers |
