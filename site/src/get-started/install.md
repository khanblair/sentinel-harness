# Install

## What it is

Sentinel is built from its repository, and an install check confirms the tools it needs.

## Steps

1. Install Rust. Use the version in `rust-toolchain.toml`.
2. Add the WASM target with `rustup target add wasm32-unknown-unknown`, and install `trunk`.
3. Install git and Tailscale. Sign in to Tailscale.
4. From the repository root, build the interface: `cd crates/sentinel-ui && trunk build --release`.
5. From the repository root, build the binary: `cargo build --release -p sentinel-cli`. The file is `target/release/sentinel`.
6. Run `sentinel doctor`. It reports the agent CLIs found, the Tailscale state, and the git version.

## Screenshot

![Screenshot of install-check](../images/install-check.png)

## Tip

Build the interface before the binary. Without it, the screen is blank.

## Related

- [Set up](set-up.md)
- [Always-on machine](always-on-machine.md)
- [Troubleshooting](../help/troubleshooting.md)
