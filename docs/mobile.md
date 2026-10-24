# Mobile

## What the App Is

The phone experience is the **same Leptos web UI** the daemon serves, installed as a PWA and reached over Tailscale. It shows the whole running harness: board, reserved files, agent streams, cost, tools, report, log, settings, and decisions. It is a remote for the daemon, not a second engine.

## Why This Approach

| Option | Verdict |
|---|---|
| PWA from the daemon | Chosen. One codebase, shared Rust types, no store, updates with the daemon |
| Native Android or iOS app | Out of scope for v1. Revisit in v2 |
| Cloud relay | Rejected. Adds a service and a data leak path |

## How It Differs from Desktop

| Area | Desktop | Phone |
|---|---|---|
| Navigation | Sidebar | Bottom tab bar |
| Detail | Right panel | Full-height sheet |
| Board | All columns visible | Snap scroll by column, status chips |
| Agents | Several panels | One at a time with a switcher |
| Log | Bottom panel on wide screens | Own tab |
| Shortcuts | Keyboard | Gestures |
| Top-bar panel icons | Visible | In the bottom tabs and the More tab |
| Approvals | Cards in the stream | Same cards with larger buttons |

Content and controls are identical. Layout follows width: under 620 px is phone (sidebar is a drawer), 620 to 1179 px is tablet (sidebar docked, panels slide over), 1180 px and up is desktop.

## Connecting and Pairing

1. The PC runs the daemon. It binds to the PC's Tailscale address only.
2. Sentinel checks the PC first: Tailscale installed, running and signed in, with a private address. If not, it shows install steps or a sign-in button.
3. The phone needs Tailscale installed and signed in to the same account. The PC screen lists these steps.
4. The PC shows a QR code, a link, the machine name (if MagicDNS is on), and a one-time code. This appears in onboarding step 4, in the Remote panel, or with `sentinel pair`.
5. The phone scans the code or opens the link and enters the one-time code. The PC asks you to approve the device. The phone receives a device token.
6. The token is stored on the phone. The daemon stores only its hash.
7. Devices can be listed and revoked from Settings on any paired device or from the CLI.

Pairing codes expire after a short window and work once.

**HTTPS:** `tailscale serve` can front the daemon with HTTPS inside the tailnet. This is needed for install prompts on some phones and for web push. The app works over plain HTTP on the tailnet without it, with fewer PWA features. Decision pending (Open Questions).

## Telegram as a Second Remote

Telegram works without Tailscale and without the app being open. It is a smaller remote: notifications, buttons for approvals and decisions, and short commands. It cannot show the full UI. The Tailscale phone app stays the way to see everything.

| Need | Use |
|---|---|
| See the whole harness, read notes, watch agent output | Tailscale phone app |
| Get told when something needs you, and answer with one tap | Telegram |
| Message an agent, ask for a diff summary, add a task, or start a run while away | Telegram, Code level |

Telegram rules are in `product-scope.md` (F20) and `architecture.md` (Telegram). A paired Telegram chat appears in Settings, Devices, and can be revoked there.

## Offline Behavior

| Situation | Behavior |
|---|---|
| Connection lost | Banner "Disconnected. Showing last state from HH:MM". View is read-only |
| Controls while offline | Disabled. Nothing is queued, so no stale command runs later |
| Reconnect | Banner "Reconnecting", then a fresh snapshot, then live events |
| Daemon down, phone up | Same as connection lost |
| Phone asleep | On wake, snapshot reload before any control is enabled |

The last snapshot is cached in browser storage for read-only viewing.

## Native Features

| Feature | Use | Notes |
|---|---|---|
| Installable PWA | Home screen icon, full screen | Needs the manifest and a service worker |
| Camera | Scan the pairing QR | Via the phone's camera app or the browser |
| Wake lock | Keep the screen on during a live run, optional | Setting |
| Vibration | Alert on budget threshold or decision needed | Where supported |
| Clipboard | Copy a task id or log line | |

## Notifications

| Event | Alert |
|---|---|
| Decision needed | In-app banner, vibration, optional web push |
| 70% of cap | Warn banner |
| 95% of cap, workers stopped | Danger banner, vibration |
| Run finished | Banner with report link |
| Harness crashed and resumed | Banner |
| New lesson candidates and Brain proposals | Badge on the More tab |
| Task stopped at its cost ceiling | Banner with the task name |

Web push needs HTTPS. Without it, alerts show only while the app is open.

## Screens and Gestures

| Tab | Screen |
|---|---|
| Stream | The chat stream and composer, with approval cards |
| Board | Tasks grouped by status |
| Reserved | Reserved files and queue |
| Changes | Checkpoints, diffs, blocked writes, quality notes |
| More | Terminal, Brain, Remote, Files, Cost, Report, Agents, Permissions, guide, settings |

| Gesture | Action |
|---|---|
| Tap | Open |
| Long press on a task | Task action menu |
| Swipe left or right | Switch tab |
| Pull down | Refresh snapshot |
| Drag sheet down | Close sheet |

## Design Rules

- Follow [ui-tokens.md](ui-tokens.md) and [ui-rules.md](ui-rules.md).
- Touch targets at least 44 px.
- Thumb zone: primary actions in the lower half. Destructive actions need a confirm.
- Text at least 14 px for table content, 16 px for body.
- Dark theme by default, follows the system.
- No hover-only actions.

## Security

| Concern | Control |
|---|---|
| Network | Tailscale only. No public endpoint |
| Identity | Per-device token. Revocable |
| Lost phone | Revoke the device from another device or the CLI |
| Allowed actions | Pause, resume, stop, kill agent, answer decisions and approvals, lower cap, create and rename a chat, pin or archive a chat, start a run, send a message to an agent, browse folders inside the home folder and choose the repository (not while a run is live), review lessons (approve, reject, retire), accept or dismiss Brain proposals, tighten Brain mode, read notes and the graph. Nothing else. Deleting a chat needs the computer |
| Forbidden from the phone | Raising the cap, loosening the Brain mode, editing or deleting notes, editing tests, running shell commands, editing files |
| Secrets | Never sent to the phone |
| Logging | Every phone action is recorded with the device name |

## Budgets

| Item | Budget |
|---|---|
| First paint over tailnet | Under 2 s |
| Event to screen | Under 300 ms |
| Snapshot size | Under 200 KB for a 12-task run |
| UI bundle | Under 1.5 MB compressed |
| Battery | No polling. WebSocket only. Pause rendering when the tab is hidden |

## Code Structure

- UI crate: `crates/sentinel-ui`, shared with desktop.
- PWA files: `manifest.webmanifest`, service worker built by the UI crate.
- Pairing and auth: `crates/sentinel-daemon/src/pairing.rs`, `auth.rs`.
- Tailscale helpers: `crates/sentinel-daemon/src/tailscale.rs`.
- Phone layout rules: CSS breakpoints from `ui-tokens.md`, no separate phone codebase.

## Development

1. Run the daemon with fake agents.
2. Pair the phone.
3. Run `trunk serve` proxied to the daemon, or build the UI and reload.
4. Use browser device emulation first, then the real phone.
5. Check each view at phone width, with offline mode, and with the screen locked and unlocked.

## Release

No store release. The phone gets the UI from the daemon, so shipping the binary ships the phone app. After an update, the service worker refreshes on next load and shows "Updated" once.

## Tasks

Phase 7 in [build-plan.md](build-plan.md): P7-01 to P7-06. Related: P8-05 (pairing step in onboarding) and P9-03 (tour on phone).

## Open Questions

1. Use `tailscale serve` for HTTPS by default, or stay on plain tailnet HTTP?
2. Is web push worth the HTTPS requirement for v1?
3. Should the phone be able to start a new run, or only steer a run started on the PC? Answered by D68: it may start a run in a chat.
4. Wake lock on by default during a live run?
5. Should the phone open on the Stream tab or the Board tab?
