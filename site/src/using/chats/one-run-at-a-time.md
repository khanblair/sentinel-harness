# One run at a time

## What it is

A project has one working tree, so it has one live run at a time. A chat holds many runs, one after another. Only one run in the project can be live.

## Steps

1. Start work in one chat. Its run becomes the live run of the project.
2. In that chat, send a message while the run is live. The message goes to the agents as a note. It does not start a second run, and it never changes which files a task has reserved.
3. In another chat of the same project, send a message. The message is not sent. A note names the busy chat and offers to open it.
4. Open the busy chat. Watch its stream, or send it a note.
5. To free the project, wait for the run to finish, or stop it. Click the stop control in the top bar, or type `/stop` in the composer. Stop asks once. Then send your message again in the other chat.

## Screenshot

![Screenshot of busy-chat-note](../../images/busy-chat-note.png)

## Tip

This version serves one project. Send a note to the live chat to steer the run. Use another chat only after the project is free.

## Related

- [Start a chat](start-a-chat.md)
- [Rename, pin, archive, and delete](manage-chats.md)
- [Permissions and approvals](../permissions.md)
