# UI Registry

Every UI component. If it is not here, it does not exist yet.

## How to Read an Entry

| Column | Meaning |
|---|---|
| Name | Component name. The file is the same name in snake case |
| Purpose | One line |
| Props | Main inputs. Typed in code |
| States | Visual and behavioral states the component must handle |
| Phase | Build-plan phase that delivers it |
| Status | `planned`, `in progress`, `done` |

## Where Components Live

- Files: `crates/sentinel-ui/src/components/<name>.rs`
- Views using them: `crates/sentinel-ui/src/views/`
- Tokens: `crates/sentinel-ui/src/theme.css`, defined in [ui-tokens.md](ui-tokens.md)
- Icons: one inline SVG icon file in the UI crate. Names listed below

## Rules for Changing Entries

1. Add or change the entry in the same change as the code.
2. Props change means the usage in views is checked.
3. A new state needs a matching rule in `ui-rules.md` if it affects behavior.
4. Removing a component removes its row and its file from `project-structure.md`.
5. Status changes are logged in `progress-tracker.md`.

## Layout and Navigation

| Name | Purpose | Props | States | Phase | Status |
|---|---|---|---|---|---|
| AppShell | Page frame with header and navigation | children, theme | phone, tablet, desktop, wide | P6 | planned |
| TopBar | Title menu, agent stack, cost chip, run controls, panel icons | run, cost, layout | running, paused, offline, phone | P6 | planned |
| Sidebar | Chat list grouped by project, New chat button, archived group, update button, profile row | chats, active, query | expanded, collapsed, searching | P6 | planned |
| ProjectHeader | Project name, collapse toggle, plus for a new chat in the project | project, closed | open, closed | P6 | planned |
| ProjectGroupView | One project in the sidebar: its header, its chats, and its archived chats | group, folds, closed, archived_open | open, closed, searching | P19 | done |
| PlainChatRow | A chat of another project: no menu, a click switches to the project and opens the chat | row, project | idle, active | P19 | done |
| AddProjectButton | The sidebar's New project button: opens the folder dialog, or the folder browser on a phone | none | idle, choosing, phone note | P19 | done |
| ProjectDialogHost | Overlay for the New project flow and the Remove dialog | none | open, closed | P19 | done |
| ChatRow | One chat: title, status icon, count, attention dot, phone icon, menu button | chat, active | idle, running, paused, done, blocked, pinned, archived, renaming | P6 | planned |
| ChatMenu | Rename, Pin or Unpin, Archive or Unarchive, Delete. The top bar title opens the same menu plus Export report and Stop run | chat, device | each item, delete disabled on phone and during a run | P6 | planned |
| ConfirmDelete | Asks before a chat and its uploads are deleted | chat | open, working | P6 | planned |
| BottomTabBar | Phone tabs: Stream, Board, Reserved, Changes, More | items, active | default, active, badge | P7 | planned |
| ConnectionBanner | Offline and reconnecting notice | state, since | offline, reconnecting | P7 | planned |

## Stream and Composer

| Name | Purpose | Props | States | Phase | Status |
|---|---|---|---|---|---|
| Stream | Ordered feed of the open chat: messages and the events of its runs | events, focus | live, paused, empty, filtered | P6 | planned |
| EmptyChatState | Empty chat: describe the work, use the backlog, plan only, take the tour | chat | idle | P6 | planned |
| UserMessage | Instruction from the user, optionally to one agent | text, to | default | P6 | planned |
| SystemMessage | Sentinel narrative text | html | default | P6 | planned |
| NoteLine | One muted line with an icon | text, icon, spinner | default, working | P6 | planned |
| PlanCard | Plan summary, flags, task table | plan | collapsed, expanded | P6 | planned |
| WaveCard | Tasks in a wave with agents | wave | default | P6 | planned |
| AgentStepCard | One agent's steps on one task | agent, task, steps | running, done, failed, killed | P6 | planned |
| FileChangeCard | File checkpoint with add and delete counts and Revert | path, add, del | default, reverted | P6 | planned |
| ApprovalCard | Allow or deny an agent request | approval | pending, allowed, denied, defaulted | P6 | planned |
| DecisionCard | Choose between contradicting tasks | decision | pending, answered, defaulted | P6 | planned |
| BlockedWriteCard | Shows a blocked write or rejected test edit | event | default | P6 | planned |
| BarrierCard | Full suite result on a clean tree | wave, result | running, pass, fail | P6 | planned |
| VerdictCard | Status, checks, receipt, quality note | verdict | verified, blocked | P6 | planned |
| BudgetBanner | 70% and 95% notices | level | warn, danger | P6 | planned |
| ReportCard | Counts and links to the report and the Brain | report | default | P6 | planned |
| Composer | Text box, plus menu, attachment chips, voice, send | value, attachments | idle, typing, slash open, disabled | P6 | planned |
| AttachMenu | Add files, Add a folder, Use a backlog file, Use a skill, Add a context note | chat | open, closed | P6 | planned |
| AttachmentChip | One attachment above the text box, with an x | attachment | file, folder, backlog, note, upload, error | P6 | planned |
| RepoFilePicker | A list of repository files with a filter box, for the plus menu | files, kind | files, folders | P6 | planned |
| SlashMenu | Command list under the composer | commands, query | hidden, open | P6 | planned |
| PermissionChip | Shows and changes the permission mode | mode | each mode, free (warn) | P6 | planned |
| BrainChip | Shows and changes the Brain mode | mode, device | each mode, locked on phone | P11 | planned |
| AgentChip | Shows the agent of the open chat (Auto or one tool) and opens the menu that changes it. Off during a live run | chat, tools | auto, agent, disabled | P6 | done |
| IntentChip | The type line of a message (`Type: ask. REASON`). The chip opens the five types and sends the last message again with the pick | intent, reason | chat, ask, research, plan, change | P6 | done |
| AgentStoppedCard | The chat's agent stopped. Names the agent and the reason, with Retry and Switch agent | tool, reason, last message | with buttons, without buttons | P6 | done |
| CommandPalette | Search commands | items, query | open, empty | P6 | planned |

## Agents and Panels

| Name | Purpose | Props | States | Phase | Status |
|---|---|---|---|---|---|
| AgentStack | Avatars of ready agents with a state dot | agents | ready, running, waiting, failed | P6 | planned |
| AgentPopover | Per-agent focus, terminal, kill | agents | open, closed | P6 | planned |
| PanelTabs | Tab strip for open panels | tabs, active | default, overflow | P6 | planned |
| PanelHost | Right panel container and slide behavior | open, wide, layout | open, closed, expanded, sheet | P6 | planned |
| PermissionModeList | Four mode cards | mode | each mode | P6 | planned |
| PermissionMatrix | Per-agent capability switches | agents, caps | default | P6 | planned |

## Settings and Setup

| Name | Purpose | Props | States | Phase | Status |
|---|---|---|---|---|---|
| SettingsModal | Thirteen settings pages | page | open, closed | P6 | planned |
| AppearanceForm | Theme, accent, tint, fonts, size, density, radius, width | prefs | default, changed | P6 | planned |
| AccentPicker | Six round swatches, a labeled Custom color box, and a Reset to default button | accent | selected, custom | P6 | planned |
| FontSelect | Choose interface or code font | role, value | default | P6 | planned |
| RangeField | Slider with value label | min, max, value | default, dragging | P6 | planned |
| TokenEditor | Edit color tokens per theme | tokens | default, edited | P6 | planned |
| ContrastReadout | Pass or fail for key color pairs | tokens | pass, fail | P6 | planned |
| CssExport | Show and copy tokens as CSS | tokens | collapsed, open | P6 | planned |
| ConnectionCard | State and test action for one connection | connection | connected, needs login, missing, testing | P6 | planned |
| KeysSection | The API keys block at the top of Connections: one card per provider, a phone note, and the load line or error | none | loading, ready, failed, phone note | P20 | done |
| KeyCard | One provider's key: a password input, the last four characters, Save, Test, and Remove with a confirm step. One action at a time, and the last result stays until the next action | info, on_change | no key, saved, saving, testing, removing, confirming removal | P20 | done |
| ToolRow | Switch, name, path, and found or missing status. No tier dropdown and no letter icon | tool | found, needs login, needs command, missing | P6 | planned |
| InstallChecklist | Install check rows | checks | found, missing, installing | P8 | planned |
| InstallLog | Streaming install and doctor output | lines | idle, running, done | P8 | planned |
| FolderPicker | Button that opens the OS folder dialog, with a folder browser for phones | path | idle, choosing, chosen | P8 | planned |
| RepoChecks | Git, backlog.md, and clean-tree rows with fix buttons | folder | ok, needs action, invalid | P8 | planned |
| PairChecklist | Tailscale installed, running, and has an address | tailscale | checking, ok, needs action, missing | P7 | planned |
| PairSteps | Numbered phone steps with link, name, code, QR | tailscale, code | waiting, paired | P7 | planned |
| DryRunStages | What the dry run does, its stages, tasks, and result | run | idle, running, done | P8 | planned |
| BypassConfirm | Confirm dialog for Bypass permissions | mode | open, closed | P6 | planned |
| GuideModal | 13-step user flow with try-it buttons | step | open, closed | P9 | planned |
| UpdateButton | Relaunch to update | version | ready, restarting | P6 | planned |
| ProfileRow | Local daemon name, theme switch, settings | daemon | default | P6 | planned |
| ResetSection | The Reset block at the end of Settings, General: one row per data kind, and one row for everything | none | default, phone note | P19 | done |
| ConfirmClear | Asks before a data reset runs. Only the everything reset needs the word RESET typed | title, body, action | open, typing, working | P19 | done |

## Board and Tasks

| Name | Purpose | Props | States | Phase | Status |
|---|---|---|---|---|---|
| Board | Columns of task cards | tasks, filter | loading, empty, populated | P6 | planned |
| BoardColumn | One status column | status, tasks | empty, populated, snapped (phone) | P6 | planned |
| TaskCard | Summary of one task | task | each status, waiting, over budget | P6 | planned |
| TaskDetail | Full task view with history | task_id | loading, populated, error | P6 | planned |
| StatusBadge | Shape plus label for a status | status | all statuses | P6 | planned |
| ReceiptBlock | Commit and test proof for VERIFIED | receipt | present, missing | P6 | planned |

## Reserved Files and Tree

| Name | Purpose | Props | States | Phase | Status |
|---|---|---|---|---|---|
| ReservedMap | File tree with reserved-file overlay | tree, reservations | collapsed, expanded, filtered | P6 | planned |
| ReservedRow | One file with the task that reserved it and its queue | reservation | held, waiting, violated | P6 | planned |
| ViolationNote | Shows a blocked or reverted write | event | new, acknowledged | P6 | planned |

## Agents and Output

| Name | Purpose | Props | States | Phase | Status |
|---|---|---|---|---|---|
| AgentPanel | Streaming output of one agent | run_id | streaming, finished, killed, error | P6 | planned |
| AgentSwitcher | Pick which agent to view on phone | agents, selected | default, active | P7 | planned |
| OutputView | Virtualized monospace output | lines | live, paused, truncated | P6 | planned |
| KillButton | Kill one agent with confirm | task_id | idle, confirming, done | P6 | planned |

## Cost

| Name | Purpose | Props | States | Phase | Status |
|---|---|---|---|---|---|
| CostMeter | Spend, cap, threshold marks | cost, cap | normal, 70 percent, 95 percent, estimated | P6 | planned |
| CostChart | Spend over time | series | loading, populated, table view | P6 | planned |
| LowerCapDialog | Lower the cap only | current_cap | idle, invalid, confirming | P6 | planned |

## Tools

| Name | Purpose | Props | States | Phase | Status |
|---|---|---|---|---|---|
| ToolList | Detected CLIs | tools | scanning, empty, populated | P6 | planned |
| ToolCard | One CLI with version, status, tier | tool | ready, needs login, failed, disabled | P6 | planned |
| ToolTemplateForm | Command template for an unknown CLI | profile | editing, testing, saved, error | P8 | planned |

## Decisions and Report

| Name | Purpose | Props | States | Phase | Status |
|---|---|---|---|---|---|
| DecisionPrompt | Contradiction choice | decision | pending, answered, defaulted | P6 | planned |
| ReportTable | Honest report | rows | loading, populated | P6 | planned |
| LogFeed | Event feed with filters | events, filter | live, paused, empty | P6 | planned |

## Brain and Quality

| Name | Purpose | Props | States | Phase | Status |
|---|---|---|---|---|---|
| BrainPanel | Notes, Graph, and Proposals tabs with the mode summary | brain | notes, graph, proposals | P11 | planned |
| NoteList | Search, type filter, and rows | notes, query, type | loading, empty, populated | P11 | planned |
| NoteRow | Type, title, and strength or lesson state | note | default, pinned | P11 | planned |
| NoteView | One note with links, backlinks, and actions | note | read, editing, read-only on phone | P11 | planned |
| NoteEditor | Text box with link hint and Save | note | idle, dirty, saved | P11 | planned |
| WikiLink | Underlined link to another note | id | live, dead | P11 | planned |
| BacklinkList | Notes that link to this one | id | empty, populated | P11 | planned |
| StrengthBar | Strength from 0 to 1 | value | low, medium, high | P11 | planned |
| LessonCard | Typed fields, evidence, and Approve, Reject, Retire | note | candidate, proven, rejected, retired | P11 | planned |
| GraphView | Dots and links, colored by note type | notes, links | default, selected, empty | P11 | planned |
| ProposalList | Care proposals with Accept and Dismiss | proposals | empty, populated | P12 | planned |
| CareRunView | Schedule summary, run steps, and result card | care | idle, running, done | P12 | planned |
| CareSettings | Mode, vault, journal, schedule, budget, auto-apply | care | default, changed | P12 | planned |
| BrainLoadedNote | Stream line listing the notes loaded for agents | notes | default | P11 | planned |
| QualityNoteList | Quality notes for a task | notes | empty, populated | P6 | planned |
| CompactionNote | Inline note in the agent panel | before, after | default | P6 | planned |
| CeilingNote | Task stopped at its cost ceiling | cost, ceiling | default | P6 | planned |

## Skills and Routines

| Component | Purpose | Props | States | Phase | Status |
|---|---|---|---|---|---|
| SkillsPage | Settings page: the skills Sentinel has, the ones found on this computer, and New skill | skills | empty, listed | P16 | planned |
| SkillRow | One skill: name, description, scope, and the Off, Auto, Always switch | skill | off, auto, always, locked on phone | P16 | planned |
| SkillEditor | Dialog to add or edit a skill: name, description, body | skill | new, editing, error | P16 | planned |
| SkillImportList | Skills found in `.claude`, `.agents`, `.agent`, and `.codex` folders, each with an Import button | found | empty, listed, importing | P16 | planned |
| SkillChip | A skill added to the next message | name | default | P16 | planned |
| RoutinesPage | Settings page: the routines, their times, and the last briefs | routines | empty, listed | P17 | planned |
| RoutineRow | One routine: name, kind, time, days, switch, destination, last run, Send now | routine | on, off, locked on phone | P17 | planned |
| RoutineEditor | Dialog to add or edit a routine: kind, time, days, text or backlog, destination | routine | new, editing, error | P17 | planned |
| RoutinesRow | The Routines row under Scheduled in the sidebar, with the count of routines that are on | count | default | P17 | planned |
| BriefCard | One brief as text, with its kind and time | brief | default, sent to Telegram | P17 | planned |

## Telegram

| Name | Purpose | Props | States | Phase | Status |
|---|---|---|---|---|---|
| TelegramSettings | Bot token, user ID, levels, events, detail, quiet hours, test | telegram | not connected, waiting for /start, connected | P13 | planned |
| HostCard | Where Sentinel runs: this computer or an always-on machine, with checks | host | this computer, always-on, checking | P14 | planned |
| TelegramConnect | Two fields (bot token, user ID) with format checks and the /start step | token, user_id | empty, invalid, saved, waiting for /start, connected | P13 | planned |
| EventToggles | Choose which events are sent | events | default, changed | P13 | planned |
| DetailLevelSelect | Status only, Summary, or Detail, with an example | level | each level, forced by hackathon mode | P13 | planned |
| ChatList | Paired chats with level and revoke | chats | empty, populated | P13 | planned |
| TelegramChat | Simulated chat with bubbles, inline buttons, and an input | messages | empty, live, answered | P13 | planned |
| ActivityLog | Last events sent, buttons used, and refused or ignored messages | log | empty, populated | P13 | planned |

## Phone, Onboarding, Tour

| Name | Purpose | Props | States | Phase | Status |
|---|---|---|---|---|---|
| PairQr | QR and one-time code | payload | showing, expired, paired | P7 | planned |
| DeviceList | Paired devices with revoke | devices | populated, revoking | P7 | planned |
| OnboardingStepper | Five-step flow frame | step, steps | active, complete, skipped | P8 | planned |
| RepoPicker | Choose and validate the repo | path | empty, valid, invalid | P8 | planned |
| BudgetForm | Cap, routing, and Brain mode | config | editing, invalid, saved | P8 | planned |
| DryRunPanel | Sample run progress | run | running, done, failed | P8 | planned |
| TourCoach | Anchored tour step | step, anchor | showing, hidden | P9 | planned |
| TourMenu | Replay or skip the tour | progress | default | P9 | planned |

## Basics

| Name | Purpose | Props | States | Phase | Status |
|---|---|---|---|---|---|
| Button | Action | label, kind, disabled | default, hover, focus, disabled, busy | P6 | planned |
| IconButton | Action with icon only | icon, name | default, focus, disabled | P6 | planned |
| Toggle | On or off setting | value | on, off, disabled | P6 | planned |
| TextField | Single line input | value, label, error | default, focus, error, disabled | P6 | planned |
| Dialog | Modal on desktop | title, children | open, closed | P6 | planned |
| Sheet | Bottom sheet on phone | title, children | open, closed, dragging | P7 | planned |
| Toast | Short result of an action, themed with tokens | message, kind | ok, warn, danger, info | P6 | planned |
| EmptyState | One sentence and a next action | text, action | default | P6 | planned |
| Skeleton | Loading shape | shape | pulsing off by default | P6 | planned |
| Icon | Inline SVG icon | name, size | default | P6 | planned |

## Icon Names

`board`, `reserved`, `agent`, `cost`, `tool`, `report`, `log`, `settings`, `lock`, `kill`, `pause`, `play`, `stop`, `scan`, `pair`, `device`, `check`, `warning`, `error`, `info`, `chevron`, `close`, `refresh`, `export`, `lesson`, `note`, `desktop`, `tablet`, `palette`, `keyboard`, `sliders`, `book`, `key`, `net`, `undo`, `expand`, `eye`, `list`, `sidebar`, `panel`, `code`, `search`, `play2`.
