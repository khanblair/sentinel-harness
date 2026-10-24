# Sentinel

Sentinel runs many coding agents in one shared working tree without collisions. It verifies every "done" claim, and you can steer it from a phone over Tailscale.

Open the how-to site: <https://khanblair.github.io/sentinel-harness/>

## What it does

- **Chats and projects.** You talk to Sentinel in a chat inside a project (a repository). A message starts a run. Attach files, folders, a backlog, or a note with the plus button.
- **Reserved files.** Each task reserves the files it may write. Two agents never write the same file. A write outside the reservation is blocked and restored.
- **Honest status.** A task is Verified only when the tests pass, no existing test was edited, and a second, different agent approved the diff. Anything less is Partial, Blocked, or Skipped, and the report says why.
- **Budget guard.** One cap for the run, a ceiling per task, and a cap that can only be lowered once the run starts.
- **Many agents.** Claude, Codex, Gemini, OpenCode, Goose, Pi, and others through their CLIs, plus a built-in OpenRouter loop.
- **Brain.** A folder of markdown notes (Obsidian compatible) that agents read as memory. Each run can write a journal and lesson candidates.
- **Phone, Telegram, always-on host.** Pair a phone over Tailscale, get approvals and alerts in Telegram, and run the daemon as a service.

## Try it

You need Rust (stable), the `wasm32-unknown-unknown` target, and `trunk`.

```bash
rustup target add wasm32-unknown-unknown
(cd crates/sentinel-ui && trunk build)
cargo run -- daemon --repo fixtures/sample-repo --dev-bind --fake-agents
```

In another terminal, run `cargo run -- open`. It opens the UI signed in as this computer. `--fake-agents` swaps real CLIs for scripted pretend agents, so a run costs nothing.

For a real run, drop `--fake-agents`, run `cargo run -- scan` to see which agent CLIs it found, and use `cargo run -- doctor` to check your setup.

## Checks

```bash
cargo xtask ci
```

It runs format, clippy, tests, cargo-deny, cargo-machete, and the repository checks (smell, duplicates, slop, docs, banned items, contrast).

## Docs

Start with [docs/README.md](docs/README.md). The interactive prototype is `design/sentinel-prototype.html`. Project status is in [docs/progress-tracker.md](docs/progress-tracker.md).

## License

MIT. See [LICENSE](LICENSE).
