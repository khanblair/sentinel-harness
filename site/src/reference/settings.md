# Settings

## What it is

Settings holds the options Sentinel keeps, grouped into sections.

## Steps

1. Click the cog in the sidebar footer to open Settings.
2. Choose a section:
   - General holds the basic options.
   - Appearance sets the theme, colors, and fonts. See [Appearance](appearance.md).
   - Agents and tools lists the detected tools and the routing for easy, normal, and hard tasks.
   - Connections shows the state of Tailscale, OpenRouter, the MCP file server, git, and the keychain. Each one has a test action.
   - Integrations holds the Telegram setup. See [Telegram](../remote/telegram.md).
   - Permissions sets the permission mode. See [Permissions and approvals](../using/permissions.md).
   - Skills lists your skills and sets Off, Auto, or Always for each. See [Skills](../skills/index.md).
   - Routines holds the Morning brief, the Sundown wrap-up, and your reminders. See [Routines](../routines/index.md).
   - Brain sets the Brain mode and the care schedule. See [Brain care](../brain/care.md).
   - Budget sets the cap.
   - Devices lists the paired phones and the Telegram chat. You can revoke any of them.
   - Shortcuts lists the keyboard shortcuts. See [Shortcuts and commands](shortcuts.md).
   - About holds the details of this install.
3. Change a value. Each setting is also kept in a config file.

## See it

<figure class="fig" data-fig="settings-sections" data-ratio="16 / 10" data-title="The Settings window, with its pages marked.">
<figcaption>The Settings window. Each page holds one kind of setting.<ol><li>Agents and tools: turn each agent on or off.</li><li>Integrations: Telegram and the always-on machine.</li><li>Devices: phones and tablets paired over Tailscale.</li></ol></figcaption>
<noscript><p>This view needs JavaScript. Open the <a href="../prototype/sentinel-prototype.html">prototype</a> instead.</p></noscript>
</figure>

## Tip

The OpenRouter key goes to the OS keychain. Sentinel never prints it.

## Related

- [Appearance](appearance.md)
- [Telegram](../remote/telegram.md)
- [Always-on machine](../get-started/always-on-machine.md)
