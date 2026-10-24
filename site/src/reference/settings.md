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
   - Brain sets the Brain mode and the care schedule. See [Brain care](../brain/care.md).
   - Budget sets the cap.
   - Devices lists the paired phones and the Telegram chat. You can revoke any of them.
   - Shortcuts lists the keyboard shortcuts. See [Shortcuts and commands](shortcuts.md).
   - About holds the details of this install.
3. Change a value. Each setting is also kept in a config file.

## Screenshot

![Screenshot of settings-sections](../images/settings-sections.png)

## Tip

The OpenRouter key goes to the OS keychain. Sentinel never prints it.

## Related

- [Appearance](appearance.md)
- [Telegram](../remote/telegram.md)
- [Always-on machine](../get-started/always-on-machine.md)
