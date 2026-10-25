# Project Overview

## What It Is

Sentinel is a Rust harness that coordinates several coding agents (Claude Code, Codex, OpenCode, Gemini CLI, and others it finds on your PC) inside **one shared working tree**. It stops agents from colliding, tracks what is truly finished, enforces a hard budget, and shows the whole run live on your phone through Tailscale.

Built first for the hackathon *Collision Course: Build Your Own Agent Harness*. Designed to outlive it.

Sentinel is the **outer loop** around agents. Each worker CLI runs its own inner reason-act-observe loop. Sentinel decides what each agent may touch, what goes back into its context, and whether its work counts. See `architecture.md`, Agent Loop Mapping.

## Goals and Success Criteria

| Goal | Measure |
|---|---|
| No collisions in one tree | 0 writes to unreserved files survive into a checkpoint |
| No extra disk for isolation | 0 extra checkouts. Disk overhead under 50 MB of state |
| Honest "done" | 100% of VERIFIED tasks pass hidden tests on rerun |
| Cost control | Spend never exceeds cap. Hard stop at 95% |
| Real tool coverage | At least 3 agents across at least 2 tools or models in a run |
| Phone parity | Every harness view and control works on the phone |
| Setup speed | New user reaches first run in under 5 minutes |
| Familiarity | A user who knows Claude Code, Codex, or Antigravity finds the stream, panels, and permissions without a manual |
| Crash safety | Restart resumes with no lost task state |
| Learning | On a second run of the practice backlog, scope accuracy or retry success improves, or the result is recorded as no gain |

## Audience

- Solo developers running several AI CLIs on one repo.
- Hackathon entrants and judges who need proof of coordination.
- Anyone who wants to watch and steer agents from a phone.

## Principles

1. **Tests decide, agents do not.** An agent claim counts for nothing without a passing check.
2. **One tree.** No worktrees, no copies. Isolation comes from reserved files, not folders.
3. **Visible by default.** Every action is logged and shown.
4. **Cheap first.** Escalate to costly models only after failure.
5. **Fail loudly.** A blocked or wrong task says so.
6. **The phone is a remote, not the engine.** The daemon keeps running if the phone vanishes.
7. **Learn from evidence, and keep it readable.** Memory is plain markdown you can open and edit. Lessons are typed facts, proven before they are trusted. Notes you wrote are never changed without your OK.

## Core Concepts

| Term | Meaning |
|---|---|
| Task | One unit of backlog work |
| Reserved files | The files one task has reserved. No other task can write them until the task is checkpointed |
| Scope | The paths a task is predicted or granted to write |
| Wave | A group of tasks with disjoint scopes that run together |
| Barrier | Pause between waves where the full suite runs on a clean tree |
| Checkpoint | A git commit of one task's reserved paths |
| Adapter | Code that runs one kind of CLI agent |
| Verifier | The gate that alone may mark a task VERIFIED |
| Lesson | A note with typed front matter: an evidence-backed fact from past runs. Candidate, proven, rejected, or retired |
| Brain | A folder of plain markdown notes linked like Obsidian notes. Agent memory, run journal, and lessons live there |
| Brain care | A scheduled run that keeps the Brain fresh and proposes improvements |
| Stream | The ordered feed of one open chat: its messages, then the plan, waves, agent steps, checkpoints, barriers, verdicts, and approvals of its runs |
| Panel | A tab that slides in beside the stream: Board, Terminal, Changes, Reserved, Brain, Remote, Files, Cost, Report, Agents, Permissions |
| Permission mode | One of four settings for how much agents may do without asking. Reserved files stay the hard boundary |
| Approval card | A card in the stream asking the user to allow or deny an agent request |
| Compaction | Cutting what is fed back into a model to a fixed size |
| Quality note | A smell or slop warning on agent code. Never changes a verdict |

## Short Feature List

- F1. Single-Tree Coordination
- F2. CLI Auto-Detection
- F3. Cheap-First Routing
- F4. Verified-Only Done
- F5. Hard Budget Guard
- F6. Crash Resume
- F7. Full-Harness Phone UI
- F8. Onboarding (5 steps)
- F9. Guided Tour
- F10. Honest Report and Run Log
- F11. Quality Checks
- F12. Brain
- F13. Context Compaction and Task Ceiling
- F14. Stream-First Workspace
- F15. Permissions and Approvals
- F16. Agent Switching
- F17. Appearance Settings
- F18. Install and Connections
- F19. Brain Care (scheduled runs)
- F20. Telegram
- F21. Always-On Host
- F22. How-To Site
- F23. Chats, Projects, and Attachments
- F24. Skills
- F25. Routines (timed jobs and briefs)
- F26. Desktop Window (macOS)
- F27. Projects
- F28. Reset

## v1 Scope

In: features F1 to F28 in product-scope.md, Linux/macOS/Windows daemon, web UI served as an installable PWA.

Out: native mobile app, cloud hosting, multi-user teams, non-git repos, fine-tuning, any use of git worktrees.

## Locked Technical Decisions

| Decision | Choice |
|---|---|
| Language | Rust everywhere, including the UI (Leptos, compiled to WASM) |
| Isolation | One tree, reserved files, no `git worktree` |
| State | SQLite via `rusqlite` (bundled) |
| Server | `axum` on `tokio` |
| Remote access | Tailscale only. No public bind |
| Agent invocation | Child processes in headless mode, plus an MCP file server |
| Git use | Shell out to `git` for plumbing |
| Logs | Append-only JSONL plus SQLite events |
| Brain | Plain markdown vault in `.sentinel/brain/`, Obsidian compatible, files are the source of truth. Lessons are typed notes, trusted only after proof. Lesson notes never hold task text |
| UI layout | Claude style: chats grouped by project in a sidebar, one stream, a plus button on the composer, panels slide in. Phone uses bottom tabs |
| Visual direction | Neutral graphite with one amber accent. Inter and JetBrains Mono by default. Everything adjustable in Settings |
| Smells on agent code | Warnings in the report. Never block or change VERIFIED |

## Doc Map

See [README.md](README.md). Facts live in one doc each: scope in product-scope, system in architecture, files in project-structure, order in build-plan, rules in code-standards, commands in development, crates in library-docs, visuals in the three ui docs, phone in mobile, status in progress-tracker.

## Reading Order for AI Agents

1. This file.
2. code-standards.md
3. architecture.md
4. project-structure.md
5. library-docs.md
6. build-plan.md, then your task
7. UI work: ui-tokens, ui-rules, ui-registry, mobile
8. progress-tracker.md before starting and before ending a session

## Writing Style Rules

Short sentences. Plain words. Tables for lists of facts. One idea per line. Active voice. Name things exactly once and reuse the name.

**Banned words and phrases (AI slop):** delve, leverage, seamless, robust, cutting-edge, game-changer, unlock, elevate, streamline, empower, holistic, ecosystem (as filler), tapestry, testament, "it's worth noting", "in today's fast-paced world", "navigate the landscape", "at its core", "journey", "supercharge", "next-level".

**Banned patterns:**
- "Not just X, but Y" framing.
- Three-item lists used for rhythm rather than content.
- Em dashes used as filler. Prefer a period.
- Emojis in headings or status text.
- Marketing tone in technical docs.
- Restating a heading in the first sentence under it.
- Hedging stacks ("may potentially help to").

`cargo xtask slop-check` scans docs, UI copy, and comments for the banned list.
