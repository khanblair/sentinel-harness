# The stream

## What it is

The stream is the ordered feed of the open chat. It shows each run in that chat, one after another, and the composer sends your messages into it.

## Steps

1. Read the stream from top to bottom. For each run it shows the plan, the waves, agent steps, file checkpoints, barriers, verdicts, approvals, and the report, in that order.
2. Type a plain message in the composer. In an idle chat it starts a run, and Sentinel routes the work by difficulty. In a chat with a live run it goes to the agents as a note. See [Start a chat](chats/start-a-chat.md). Use the plus button to attach files, folders, a backlog, or a note. See [Add files, folders, a backlog, or a note](chats/attachments.md).
3. Type a slash command to run a control. The commands are `/pause`, `/resume`, `/stop`, `/cap`, `/agent`, `/focus`, `/kill`, `/plan`, `/brain`, `/report`, and `/help`.
4. Open the send-to chip to send a message to one agent instead of Sentinel.
5. Press Ctrl+K on Windows or Linux, or Cmd+K on macOS, to open the command palette for chats, agents, panels, and settings.

## Screenshot

![Screenshot of stream-composer](../images/stream-composer.png)

## Tip

A message never changes which files a task has reserved.

## Related

- [Chats](chats/index.md)
- [Agents](agents.md)
- [Permissions and approvals](permissions.md)
- [Shortcuts and commands](../reference/shortcuts.md)
