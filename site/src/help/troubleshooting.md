# Troubleshooting

## What it is

This page matches common symptoms to their causes and fixes.

## Steps

1. The daemon refuses to start with a bind error. The cause is that Tailscale has no address. Start Tailscale, then run `sentinel doctor`.
2. The phone cannot connect. The cause is a different tailnet or a block in the access rules. Check both devices in the Tailscale admin panel.
3. An agent shows "needs login". Log in to that tool once, then rescan the tools.
4. The barrier refuses to run. The cause is a dirty working tree. Check the write-blocked events, restore the files, and run again.
5. Cost shows "estimated". This is expected. A tool that reports no dollar figure gets an estimate, and estimates run high.
6. A task stays in waiting. The cause is a stalled agent that holds the files. Kill the holder from the UI.
7. The screen is blank. The interface was not built before the binary. Build the interface, then build the binary again.
8. Agents do not use the notes. The cause is that Brain mode is off, or the notes are weak or unlinked. Run `brain list`, then check the mode and the pinned notes.
9. Edits made in Obsidian are missing. The cause is that the watcher is off, or the file sits in a hidden folder. Run `brain reindex`.
10. The retry prompt looks short. Compaction is working. Open the full output from the log reference.
11. A task was killed at its ceiling. The cause is that its cost passed 2 times its tier estimate. Check the estimate, or let the task escalate.

## Screenshot

![Screenshot of troubleshooting-table](../images/troubleshooting-table.png)

## Tip

Run `sentinel doctor` before you change any setting.

## Related

- [Install](../get-started/install.md)
- [FAQ](faq.md)
- [Handle a blocked task](../guides/blocked-task.md)
