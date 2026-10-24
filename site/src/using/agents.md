# Agents

## What it is

Agent switching lets you see each agent, focus its output, and send it a message.

## Steps

1. Click the avatar stack in the top bar. It shows the ready agents, each with a state dot.
2. Read each row. It shows the kind of task the agent handles, its current task, and its cost.
3. Choose focus to open the agent's output in the stream. Choose terminal to open its terminal. Choose kill to stop it.
4. Open Settings to set routing. Choose which agent takes easy, normal, and hard tasks. A failed task moves one step up.
5. Open Agents from the More menu to see the detected tools. Each one shows as found or missing, with a switch to use it.

## See it

<figure class="fig" data-fig="agent-stack" data-ratio="16 / 10" data-title="The agent stack in the top bar, with its menu open.">
<figcaption>The agent stack in the top bar, with its menu open.<ol><li>One letter for each agent ready in this run.</li><li>Focus: show only this agent in the stream.</li><li>Kill stops this agent now.</li></ol></figcaption>
<noscript><p>This view needs JavaScript. Open the <a href="../prototype/sentinel-prototype.html">prototype</a> instead.</p></noscript>
</figure>

## Tip

A message to an agent never changes the files it has reserved.

## Related

- [The stream](stream.md)
- [Settings](../reference/settings.md)
- [Reserved files and waves](reserved-files.md)
