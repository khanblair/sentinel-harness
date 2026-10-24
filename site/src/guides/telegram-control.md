# Control a run from Telegram

## What it is

This guide steers a run from Telegram: alerts, approvals, short commands, and small instructions to one agent.

## Steps

1. Choose the level for the chat: Notify, Control, Code, or Reports. See the level table in [Telegram](../remote/telegram.md).
2. Let the alerts arrive. Choose the detail level that fits the place you are in.
3. Answer an approval with one tap. Each button expires after 10 minutes and works only once.
4. Use `/pause`, `/resume`, `/stop`, or `/kill <agent>` to control the run. Stop and kill ask for a confirm tap, and so do `/task` and `/run`.
5. Use `/agent <name> <text>` to send one agent a message. Use `/diff [task]` for a short summary. Use `/task <text>` to add a task to `backlog.md`. Use `/run [task]` to start a run.
6. Expect a request for a PC-only action to be refused and logged. Those actions are switching to Bypass permissions, raising the cap, editing notes or tests, and running shell commands.

## Screenshot

![Screenshot of telegram-control](../images/telegram-control.png)

## Tip

Telegram is not end-to-end encrypted, so its servers can read the messages. Choose Status only to send less.

## Related

- [Telegram](../remote/telegram.md)
- [What each remote may do](../remote/remote-limits.md)
- [Budget](../using/budget.md)
