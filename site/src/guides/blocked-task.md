# Handle a blocked task

## What it is

A blocked task has stopped. This guide finds the cause and gets the task running again.

## Steps

1. Read the reason in the stream or on the Board panel. A blocked task always states its reason.
2. If the task hit its cost ceiling, check the cost estimate. The default ceiling is 2 times the tier estimate.
3. If an agent holds the files the task needs and has stalled, kill that agent from the agent stack.
4. Raise the tier in the routing settings, narrow the scope in `backlog.md`, or fix the cause in the code.
5. Run the task again.

## See it

<figure class="fig" data-fig="blocked-task-card" data-ratio="16 / 10" data-title="A blocked task in the stream.">
<figcaption>A blocked task in the stream.<ol><li>Blocked, not done. Nothing is marked done that was not.</li><li>The check that failed.</li></ol></figcaption>
<noscript><p>This view needs JavaScript. Open the <a href="../prototype/sentinel-prototype.html">prototype</a> instead.</p></noscript>
</figure>

## Tip

Read the reason before you change anything.

## Related

- [Agents](../using/agents.md)
- [Budget](../using/budget.md)
- [Troubleshooting](../help/troubleshooting.md)
