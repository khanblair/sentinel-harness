# Your first run

## What it is

A first run takes one chat from its first message to verified tasks, and you answer each decision on the way.

## Steps

1. Open a new chat in your repository. See [Start a chat](../using/chats/start-a-chat.md).
2. Describe the work in a message, and say what done means. Or attach a `backlog.md` with the plus button. Use one heading per task. Do not list files. Sentinel finds the scope.
3. Start with Plan only. Read the plan. Check the scopes, hidden dependencies, contradictions, and any task that claims to be done.
4. Answer each decision card. Pick one of two contradicting tasks, or skip both. No answer skips both and records a Contradiction.
5. Choose the permission mode. Keep the Brain on read-only for live runs.
6. Watch the stream. Open panels from the top bar when you need them.
7. Answer requests for more files. Allow a free, sensible file. Deny anything outside the agent's own work.
8. Read each verdict. A task is VERIFIED only after the suite passes, a new test covers the change, a different model confirms the diff, and no existing test was edited.

## See it

<figure class="fig" data-fig="first-run-stream" data-ratio="16 / 10" data-title="The stream during a run: plan, wave, and agent.">
<figcaption>The stream during a run: the plan, a wave, and an agent.<ol><li>The plan. It lists the files each task reserves.</li><li>A wave: tasks that run together.</li><li>The agent: each step, as it happens.</li></ol></figcaption>
<noscript><p>This view needs JavaScript. Open the <a href="../prototype/sentinel-prototype.html">prototype</a> instead.</p></noscript>
</figure>

## Tip

Use Plan only until the plan can be trusted.

## Related

- [Chats](../using/chats/index.md)
- [One run at a time](../using/chats/one-run-at-a-time.md)
- [The stream](../using/stream.md)
- [Permissions and approvals](../using/permissions.md)
- [Run a backlog safely](../guides/backlog-safely.md)
- [Status words](../reference/status-words.md)
