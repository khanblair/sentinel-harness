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

## See it

<figure class="fig" data-fig="install-check" data-ratio="16 / 10" data-title="The install check step.">
<figcaption>The install check step.<ol><li>Pick your system. The commands change.</li><li>A missing item says so. Install fills it in.</li><li>Install what is missing. Each step is logged.</li></ol></figcaption>
<noscript><p>This view needs JavaScript. Open the <a href="../prototype/sentinel-prototype.html">prototype</a> instead.</p></noscript>
</figure>

## Tip

Build the interface before the binary. Without it, the screen is blank.

## Related

- [Set up](set-up.md)
- [Always-on machine](always-on-machine.md)
- [Troubleshooting](../help/troubleshooting.md)
