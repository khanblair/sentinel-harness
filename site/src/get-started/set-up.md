# Set up

## What it is

Onboarding has five steps, and each step sets one part of Sentinel. Every setting is also kept in a config file.

## Steps

1. Choose the repository in your computer's own folder picker. Sentinel checks for git, a `backlog.md`, and a clean tree.
2. Check the agent CLIs Sentinel found. Each one shows as found or missing. Enable at least 3 agents across 2 tools.
3. Set the budget cap, the permission mode, the routing preferences, and the Brain mode.
4. Pair your phone over Tailscale. This step can be skipped. Sentinel first checks that Tailscale is running.
5. Finish with the dry run. It uses a built-in sample project and never touches your repository. The screen ticks off each stage as it runs.

## Screenshot

![Screenshot of set-up-wizard](../images/set-up-wizard.png)

## Tip

Keep the Brain on read-only for live runs.

## Related

- [Install](install.md)
- [Your first run](first-run.md)
- [Permissions and approvals](../using/permissions.md)
- [Phone over Tailscale](../remote/phone.md)
