# UI Rules

How the UI looks, reads, and behaves. Values come from [ui-tokens.md](ui-tokens.md). Components come from [ui-registry.md](ui-registry.md).

## Core Rules

1. **Truth first.** Show what is, not what is hoped. A task is Verified only when the verifier says so.
2. **Same UI everywhere.** Phone and desktop show the same views and controls. Layout changes, content does not.
3. **State is never color alone.** Color, shape, and label together.
4. **One primary action per screen.**
5. **Dangerous actions confirm.** Kill, stop, and lower cap ask once, with the effect stated.
6. **Live by default.** Data updates from events. A stale view says so.
7. **No decoration.** Every element carries information or an action.
8. **Tokens only.** No raw colors, sizes, or fonts in components.
9. **Stream first.** The stream is the product. Everything else slides in beside it and never replaces it.
10. **Familiar before clever.** Where Claude Code, Codex, or Antigravity set a convention (sidebar of runs, top-bar icons, slash commands, permission chip, send-to chip), follow it.

## Layout

Three zones.

| Zone | Contents |
|---|---|
| Left sidebar (264 px) | New run, then runs grouped by repository. Each run shows a status icon, a verified count, a dot when it needs you, and a phone icon when it can be steered from a phone. Footer: Relaunch to update (when ready), theme switch, settings |
| Center | Top bar, stream, composer |
| Right panels (440 px) | Tabs: Board, Terminal, Changes, Reserved, Brain, Remote, Files, Cost, Report, Agents, Permissions. Slides in and out. Can expand |

| Breakpoint | Layout |
|---|---|
| phone, under 620 px | One column. Bottom tabs: Stream, Board, Reserved, Changes, More. Sidebar is a left drawer. Panels are bottom sheets |
| tablet, 620 to 1179 px | The sidebar stays docked and can be collapsed. Panels slide over the stream with a scrim |
| desktop, 1180 px and up | Sidebar, stream, and panels side by side |

The sidebar is open by default on tablet and desktop and can be collapsed. On phone it is a drawer. The layout follows the width of the app, not the device.

## Top Bar

Left of the stream header: sidebar toggle (when hidden), back and forward through opened runs, a search toggle for the run list.

| Control | Behavior |
|---|---|
| Run title with caret | Menu: duplicate, export report, rename, archive |
| Agent stack | Avatars of ready agents with a state dot. Opens the agent popover: focus the stream, open the terminal, or kill, per agent |
| Cost chip | Spend, a bar, and the cap. Opens the Cost panel. Turns warn at 70% and danger at 95% |
| Pause and stop | Shown while a run is live |
| Panel icons | Board, Terminal, Changes, Reserved, Brain, Remote. The active panel is highlighted |
| More | Files, Cost, Report, Agents, Permissions, guide, tour, command palette, settings |

On phone the panel icons move to the bottom tab bar and the More tab.


## Views

| View | Where | Contents | Primary action |
|---|---|---|---|
| Stream | Center | Plan, waves, agent steps, file checkpoints, barriers, verdicts, approvals, report | Answer an approval |
| Board | Panel | Tasks grouped by status | Open task |
| Terminal | Panel | Raw output per agent | Switch agent |
| Changes | Panel | Checkpoint commits with diffs, blocked writes, quality notes | Open a diff |
| Reserved | Panel | Reserved files, which task holds them, waiting queue | Open holder |
| Remote | Panel | Tailscale state, pairing QR, paired devices, what a phone can do | Pair or revoke |
| Files | Panel | Repository files with reserved-file markers and a filter | Filter |
| Cost | Panel | Spend, cap, 70% and 95% marks, per-agent cost, lower cap | Lower cap |
| Report | Panel | Honest report, per-task status and receipt, reasons | Export |
| Brain | Panel | Notes with search and type filter, a note view with backlinks and an editor, a graph, and proposals | Open a note |
| Telegram | Settings page and a panel | Settings: token, pairing, levels, detail, events, quiet hours, activity. Panel: a simulated chat with working buttons | Send test message |
| Agents | Panel | Detected CLIs with status and tier | Rescan |
| Permissions | Panel | Mode list and capability matrix | Pick a mode |
| Settings | Modal | General, Appearance, Agents and tools, Connections, Integrations, Permissions, Brain, Budget, Devices, Shortcuts, About | Change a setting |
| Setup | Full screen | Install check, then five onboarding steps | Continue |
| Guide | Modal | 13-step user flow with try-it buttons | Try it |

## Composer

- A text box with a plus menu (use sample backlog, attach a backlog, add a context note), a voice button, and send.
- Enter sends. Shift + Enter adds a line. Typing `/` opens the command list.
- Footer chips, left to right: permission mode, Brain mode, then on the right the send-to chip (Sentinel or one agent) and a spinner while a run is live.
- Commands: `/pause`, `/resume`, `/stop`, `/cap 1.50`, `/agent claude`, `/focus codex`, `/kill codex`, `/plan`, `/brain off`, `/report`, `/help`. `/cap` only accepts a lower value.
- A message to Sentinel or an agent never changes which files a task has reserved. The reply says so when it matters.
- When no run is live, the first message starts a run with that instruction.

## Permissions and Approvals

- The permission chip names the mode. The color is normal except for Bypass permissions, which uses the warn color.
- Choosing Bypass permissions opens a confirm dialog. It says what changes (no approval cards, free files granted) and what stays enforced (locks, the test-edit guard, the budget cap).
- An approval card says who asks, for what, and why. Buttons are verbs: Allow, Deny, Approve wave, Stop run.
- Once answered, the card collapses to one line that says what was decided.
- A card waiting for you puts a dot on its run in the sidebar.
- Writes to files a task has not reserved are never an approval. They are blocked and shown as a blocked-write card.


## Brain

- The Brain panel has three tabs: Notes, Graph, Proposals. A count badge shows open proposals.
- Notes: a search box and a type filter above a list. Each row shows the type, the title, and a strength bar. A lesson row shows its state instead.
- A note opens in the same panel with a back button. It shows who wrote it (you, Sentinel, or Brain care), when it was confirmed, and how often agents used it. Links in the text are underlined buttons in the accent color. A link to a missing note is struck through.
- Below the text: strength, and a list of notes that link here (backlinks).
- Edit opens a text box. Save writes the file. Editing is done on the PC. On a phone the note is read-only.
- Always load for agents pins a note. Delete is offered only for notes you wrote.
- A lesson note shows its typed fields and Approve, Reject, and Retire buttons.
- Graph: one dot per note, colored by type, sized by how many notes link to it. Tapping a dot opens the note. The graph has a text alternative (the notes list).
- Proposals: each shows a kind chip (Add, Update, Retire, Merge), a title, why, and Accept and Dismiss. Nothing changes until Accept.
- The stream says which notes were loaded into agent memory, as links, and says when the Brain was updated after a run.
- Brain care runs sit in the sidebar under Scheduled. Opening one before it has run shows the schedule and a Run care now button. After a run it shows each step and a card that links to the proposals.

## Key Element Anatomy

### Task Card

```
[status shape] Title (max 2 lines)
 owner tool  .  tier  .  attempt n
 scope: 3 paths  [lock icon if waiting]
 cost  $0.04 (est)
```

Quality notes show as a count on the card and as a list in task detail. They never change the status shape.

### Cost Meter

Large figure, cap beside it, bar below with marks at 70% and 95%. Label says "estimated" when any part is estimated.

### Reserved Files Row

Path, lock icon, holder task, state. Waiting tasks listed under the path.

## Interactions

| Action | Behavior |
|---|---|
| Tap or click card | Opens detail. On phone, a sheet |
| Long press (phone) or right click | Task action menu |
| Swipe left or right on phone | Move between top-level tabs |
| Pull down on phone | Refresh snapshot |
| Kill agent | Confirm dialog naming the task and the reserved files it will release |
| Lower cap | Number input that only accepts a lower value. Raising is not offered |
| Decision prompt | Two clear choices plus "skip both" |
| Review lesson | Approve, Reject, or Retire. Each confirms with its effect stated, for example "Retire: this lesson stops being used" |
| Brain mode | Switch offers only equal or stricter values than the current mode on the phone |
| Command palette | Ctrl or Cmd + K: switch agent, focus stream, open panel, start run, theme, settings, preview |
| Sidebar and panels | Ctrl or Cmd + B toggles the sidebar. Ctrl or Cmd + J toggles panels |
| Agent popover | Focus, terminal, and kill are icon buttons with tooltips and accessible names |
| Keyboard | Tab order follows reading order |

## Feedback States

| State | Treatment |
|---|---|
| Loading | Skeleton of the final layout. No spinner over empty space |
| Empty | One sentence on why it is empty and the next action |
| Error | What failed, what it means, what to do. Keep partial data visible |
| Offline | Top banner "Disconnected. Showing last state from HH:MM". Controls disabled |
| Reconnecting | Banner switches to "Reconnecting". Snapshot reloads on success |
| Toast | A short result of an action, about 3 seconds. Uses the surface and text tokens and a plain 1 px border. No colored side bar. The kind (ok, warn, danger, info) is shown by the icon color. Status changes show on the element, not in a toast |
| Context compacted | Small note in the agent panel: "Context compacted: 52k to 9k tokens" |
| Task ceiling hit | Task card shows "Stopped at cost ceiling" with the amount |
| Budget threshold | 70%: warn banner. 95%: danger banner "Workers stopped" |

## Writing Rules

- Plain words, short sentences, present tense.
- Say what happened and what to do. No blame, no cheer.
- Buttons are verbs: "Kill agent", "Pair phone", "Start run".
- Numbers include units. Money shows as `$0.04`. Estimated values add "est".
- Statuses use the exact labels in `ui-tokens.md`.
- No exclamation marks. No emojis. No marketing words.
- The UI says easy, normal, and hard tasks. Docs and code say cheap, mid, and smart.
- Banned words and patterns: see [project-overview.md](project-overview.md).

| Instead of | Write |
|---|---|
| "Oops! Something went wrong" | "Could not reach the daemon. Check Tailscale on this device." |
| "Take your agents to the next level" | "Run agents together." |
| "Effortless pairing" | "Scan the code to pair." |
| "Awesome, you're all set!" | "Setup complete." |

## Accessibility

- Contrast limits in `ui-tokens.md`, checked in CI.
- Touch targets at least 44 px.
- Full keyboard use on desktop. Visible focus.
- Every icon-only control has an accessible name.
- Live regions announce status changes and budget alerts, politely, not for every output line.
- Respect reduced motion and system theme.
- Text scales to 200% without losing controls.
- Charts have a table alternative.

## Performance

| Item | Rule |
|---|---|
| Lists over 100 rows | Virtualize |
| Agent output | Show the last 500 lines, load older on request |
| Event updates | Batch into one render per animation frame |
| Bundle | Within the budget in `architecture.md` |
| Images | None needed. Icons are inline SVG |

## Responsive Layouts

- Phone: one column with a bottom tab bar. Stream, Board, Reserved, Changes, More. More opens every other panel and settings.
- Panels are bottom sheets. The sidebar is a left drawer with a scrim.
- Tablet: the sidebar stays docked and can be collapsed. Panels slide over the stream with a scrim. The panel icons stay in the top bar.
- The composer chips scroll sideways when they do not fit.
- Tables become stacked rows with the key field first.
- Settings and the guide stack their list above the content as a scrolling row.
- Dialogs become full-height sheets.
- Touch targets are at least 44 px.
- Popovers and menus stay inside the app, flip above their button when there is no room below, and scroll when taller than the app.
- Toasts are centered above the composer (above the tab bar on phone) and never leave the app.

## Do and Don't

| Do | Don't |
|---|---|
| Show "estimated" next to guessed cost | Show a guessed number as exact |
| Keep the cost meter in the header | Hide cost in a sub menu |
| Use shape plus label for status | Use a green dot alone |
| Say "Blocked: file held by task 7" | Say "Something went wrong" |
| Disable controls when offline | Let a tap fail silently |
| Offer only lowering the cap | Offer a field that can raise the cap |
| Use a cog for settings and a sun or moon for the theme | Draw settings as a sun |
| Label every control in a swatch row (Custom, Reset to default) | Put a text button inside a round swatch |
| Open the computer's folder dialog | Ask the user to paste a path |
| Show agents as found or missing in setup | Show tier dropdowns or letter icons in setup |
| Check Tailscale first, then show the matching steps | Show a code before Tailscale is known to run |
| Show a lesson as a typed fact with its evidence count | Show a free-text story about a task |
| Say what the Brain loaded, as links | Load notes silently |
| Keep Telegram messages short and at Summary level | Put code or notes in a chat |
| Show quality notes as warnings beside the verdict | Hide a verdict because of a note |

## AI Slop Checklist for Visuals and Copy

Each tell, and the product's rule for it. Reviewers tick every line.

| Tell | Rule |
|---|---|
| Purple to blue gradients | Banned. Flat surfaces only |
| Glass blur cards | Banned. Bordered surfaces |
| Everything rounded and soft | Radius set by `--r` (2 to 16 px, default 8 px) |
| Emoji as icons or bullets | Banned. Use the icon set |
| Generic hero with a tagline | None. The app opens on the board or onboarding |
| Stock illustrations of robots | Banned |
| Glow and neon effects | Banned. Motion only for state change |
| Centered everything | Left-aligned, grid-based |
| Three feature cards in a row | Not used |
| Fake stats or testimonials | None |
| Filler copy such as "Seamlessly manage your agents" | Banned. State the action |
| Over-friendly microcopy | Plain, neutral |
| Placeholder names like "Acme" or "John Doe" | Use the sample repo names |
| Lorem ipsum | Banned |
| Decorative loading animations | Skeletons only |
| Inconsistent labels for one status | One label per status, from tokens |
| Dark mode only with unreadable grays | Contrast check runs on both themes |
