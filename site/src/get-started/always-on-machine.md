# Always-on machine

## What it is

An always-on machine runs Sentinel, the repository, and the agents while your own computer sleeps.

## Steps

1. Install Sentinel on the machine that stays on.
2. Run `sentinel service install`. Sentinel then starts at boot and restarts if it stops. On Linux this is a systemd service. On macOS it is a launchd job. Windows support comes later.
3. Sign the machine in to Tailscale.
4. Log in to your agent CLIs on that machine. Clone the repository there.
5. Open the app at the machine's Tailscale address. Run `sentinel doctor` on the machine.
6. Check the host state in Settings. It shows the service, Tailscale, the agent CLIs, the repository, and the last heartbeat.

## See it

<figure class="fig" data-fig="always-on-host-settings" data-ratio="16 / 10" data-title="Settings, Integrations, with the Where Sentinel runs card.">
<figcaption>Settings, Integrations, with the Where Sentinel runs card.<ol><li>Choose where Sentinel runs.</li><li>Each check: the service, Tailscale, the agents, and the repository.</li><li>Check the host now.</li></ol></figcaption>
<noscript><p>This view needs JavaScript. Open the <a href="../prototype/sentinel-prototype.html">prototype</a> instead.</p></noscript>
</figure>

## Tip

A laptop that sleeps is not an always-on machine.

## Related

- [Phone over Tailscale](../remote/phone.md)
- [Telegram](../remote/telegram.md)
- [Settings](../reference/settings.md)
