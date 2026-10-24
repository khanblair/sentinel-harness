# Budget

## What it is

The budget cap sets the most a run may spend, and Sentinel stops work as spend reaches each limit.

## Steps

1. Set the cap before you start a run. You can lower the cap during a run. You cannot raise it.
2. At 70% of the cap, no new tasks start.
3. At 95% of the cap, all workers are stopped through one shared signal.
4. Each task has a ceiling, by default 2 times its tier estimate. A task over its ceiling is killed, then escalated or blocked.
5. Open the Cost panel to see spend. Tools that report no dollar figure show an "est" label. Estimates run high.

## See it

<figure class="fig" data-fig="cost-panel" data-ratio="16 / 10" data-title="The Cost panel.">
<figcaption>The Cost panel, with the cap and the meter.<ol><li>Amber at 70%: no new tasks start. Red at 95%: workers stop.</li><li>Lower the cap. It can only go down during a run.</li></ol></figcaption>
<noscript><p>This view needs JavaScript. Open the <a href="../prototype/sentinel-prototype.html">prototype</a> instead.</p></noscript>
</figure>

## Tip

Lower the cap when you want fewer new tasks to start.

## Related

- [Status words](../reference/status-words.md)
- [Troubleshooting](../help/troubleshooting.md)
- [Control a run from Telegram](../guides/telegram-control.md)
