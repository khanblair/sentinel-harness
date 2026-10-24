# Install

## What it is

On a Mac, one command installs the app. On other systems, or to work on Sentinel itself, you build it from its repository. An install check confirms the tools it needs.

## Steps

**On a Mac, the app:**

1. Open Terminal and run:
   ```
   curl -fsSL https://raw.githubusercontent.com/khanblair/sentinel-harness/main/install.sh | sh
   ```
   It downloads the latest release, checks its SHA-256, and puts Sentinel in Applications. macOS opens it with no warning.
2. Open Sentinel from Launchpad or Spotlight. It asks for a project folder the first time.
3. Prefer the disk image? Download `Sentinel-VERSION.dmg` from the [releases page](https://github.com/khanblair/sentinel-harness/releases/latest). A downloaded image is blocked once until the build is signed with a Developer ID certificate: click Done, open System Settings, Privacy and Security, and choose Open Anyway.

**From source, on any system:**

1. Install Rust (the stable channel).
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
