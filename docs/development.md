# Development

## Required Tools

| Tool | Why |
|---|---|
| Rust, stable channel, edition 2024 (no pin file yet, see D58 in the tracker) | Everything |
| `wasm32-unknown-unknown` target | UI build |
| `trunk` | UI bundling |
| `git` (recent) | Plumbing commands |
| Tailscale, logged in, on PC and phone | Phone access |
| `cargo-nextest`, `cargo-deny`, `cargo-machete`, `cargo-llvm-cov` | Checks |
| At least 2 agent CLIs, or API keys for model providers | Real runs |

## First-Time Setup

1. Clone the repo.
2. Install the stable toolchain: `rustup toolchain install stable`.
3. Add the WASM target: `rustup target add wasm32-unknown-unknown`.
4. Install tools: `cargo install trunk cargo-nextest cargo-deny cargo-machete cargo-llvm-cov`.
5. Run `cargo xtask ci` once. It must pass on a clean clone.
6. Run `cargo run -- doctor`. It reports detected CLIs, Tailscale state, and git version.

## Workspace Layout

See [project-structure.md](project-structure.md).

## Dev Mode

| Goal | Command |
|---|---|
| Daemon with auto-restart | `cargo watch -x "run -- run fixtures/sample-backlog.md --repo fixtures/sample-repo --cap 1.00"` |
| UI with live reload | `cd crates/sentinel-ui && trunk serve` |
| Both against fake agents | `cargo run -- run ... --fake-agents` |

`--fake-agents` swaps real CLIs for scripts in `fixtures/fake-cli`. Use it for all non-agent work.

## Environment Variables

| Variable | Purpose | Default |
|---|---|---|
| `OPENROUTER_API_KEY` | OpenRouter key, used when the keychain has none | read from the OS keychain first |
| `SENTINEL_TELEGRAM_TOKEN` | Bot token, used only when the keychain has none | none |
| `SENTINEL_UTC_OFFSET_MINUTES` | Overrides the computer's time zone for Telegram quiet hours | read from `date +%z` |

The repository, port, and bind are options of the commands: `sentinel daemon --repo PATH --port 7878`. The default port is 7878. The daemon binds only the Tailscale address, or `127.0.0.1` with `--dev-bind` for development. The budget cap, the Brain mode, and the routing live in `.sentinel/config.toml` and in Settings.

Keys are never printed. Prefer the keychain, set through `sentinel keys set`.

## Real Services in Dev

- Real CLIs: run the dev-PC check tasks (P3-05, P3-06) by hand, never in CI.
- Real models: use a tiny cap such as 0.50 USD while developing.
- Tailscale: use the real tailnet. Keep the phone paired for UI checks.

## Everyday Commands

| Command | Purpose |
|---|---|
| `cargo xtask ci` | All checks |
| `cargo nextest run` | Tests |
| `cargo llvm-cov nextest` | Coverage |
| `cargo run -- scan` | Detect agent CLIs |
| `cargo run -- pair` | Show the QR code |
| `cargo run -- report` | Print the last honest report |
| `cargo run -- doctor` | Environment check |
| `cargo run -- daemon --repo PATH --dev-bind` | Serve the UI and API on 127.0.0.1 |
| `cargo run -- open` | Open the UI, signed in as this computer |
| `cargo run -- brain mode read-only` | Set the Brain mode (`off`, `read-only`, `learn`) |
| `cargo run -- brain care` | Run Brain care now |
| `cargo run -- brain reindex` | Rebuild `brain.db` from the vault |

## Testing Notes

- Git tests use temp repos from `tempfile`.
- No test sleeps. Use channels and fake clocks.
- The e2e suite uses fake CLIs only.
- A failing e2e test leaves its temp folder path in the output.

## Production Builds

| Step | Command |
|---|---|
| Build UI | `cd crates/sentinel-ui && trunk build --release` |
| Build binary (embeds the UI) | `cargo build --release -p sentinel-cli` |
| Output | `target/release/sentinel` |

One binary. The UI is embedded.

## Targets

Linux x86_64, macOS (Apple silicon and Intel), Windows x86_64. Hackathon build targets the dev PC first.

## Signing, Versions, Release

- Versions follow semver, set once in the root `Cargo.toml`.
- Release builds are signed where the platform requires it (macOS notarization, Windows signing). Not needed for the hackathon.
- Tag the commit `vX.Y.Z` after `cargo xtask ci` passes.

## Release Steps

1. Update `progress-tracker.md` and the changelog section there.
2. `cargo xtask ci`.
3. Build UI, then the binary.
4. Run the e2e suite on the built binary.
5. Tag and publish the repo. For the hackathon, the repo must be open, list dependencies, and run with one command.

## Telegram Setup

1. In Telegram, message @BotFather and send `/newbot`. Choose a name and a username.
2. Copy the token BotFather sends.
3. In Sentinel, open Settings, Integrations, Telegram. Paste the token and your numeric Telegram user ID (not your @username). Sentinel checks the token, shows the bot's name, and stores the token in the OS keychain.
4. Open your bot in Telegram and send `/start` from that account. Sentinel connects the chat. There is no pairing code.
5. Choose a level, the events you want, and the detail level. Press Send test message.

## Always-On Host

1. Install Sentinel on the machine that stays on.
2. Run `sentinel service install`. It starts at boot and restarts if it stops.
3. Sign the machine in to Tailscale.
4. Log in to your agent CLIs on that machine. Clone the repository there.
5. Open the app at the machine's Tailscale address. Run `sentinel doctor` on the host to check it.

## How-To Site

- Install the tool once: `cargo install mdbook`.
- Preview: `cd site && mdbook serve`.
- Build: `cd site && mdbook build`. Output goes to `site/book/`.
- Deploy: a push to `main` runs the Pages workflow. The structure is in `design/site-structure.html`.

## Design Prototype

- File: `design/sentinel-prototype.html`. No build step. Open it in a browser.
- All data is simulated. Nothing talks to a daemon.
- The toolbar above the app switches desktop, tablet, and phone previews, sets demo speed, opens the guide and the tour, and reopens install and setup. Reset clears saved settings.
- Change the prototype and the three UI docs together (see the README rule).

## Hackathon Run Checklist

1. `cargo xtask ci` passes.
2. Tailscale up on PC and phone. Phone paired.
3. Cap set to at or below the announced cap.
4. Detected tools show at least 3 agents across 2 tools or models.
5. Brain mode is `read-only`, so nothing is written. Telegram is off or set to status only.
6. Logs folder empty or archived.
7. Start the run with one command. Do not change code during the run.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Daemon refuses to start, bind error | No Tailscale address | Start Tailscale, run `doctor` |
| Phone cannot connect | Different tailnet or ACL block | Check both devices in the admin panel |
| A CLI shows "needs login" | Not signed in | Log in with that tool once, rescan |
| Barrier refuses to run | Tree dirty | Check `WriteBlocked` events, restore, rerun |
| Cost shows "estimated" | Tool gives no dollar figure | Expected. Estimates run high |
| Task stuck waiting | Files reserved by a stalled agent | Kill the holder from the UI |
| UI is blank | UI not built before the binary | Build the UI first |
| Notes not used by agents | Brain mode is `off`, or the notes are weak or unlinked | the Brain panel, check the mode and pinned notes |
| Edits in Obsidian missing | The watcher is off or the file is in a hidden folder rule | `brain reindex` |
| Retry prompt looks short | Compaction is working | Open the full output from the log reference |
| Task killed at its ceiling | Task cost passed 2x the tier estimate | Check the cost estimate, or let it escalate |
