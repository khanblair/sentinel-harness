# Brain care

## What it is

Brain care is a scheduled run that refreshes the notes, merges duplicates, and proposes changes for you to accept or dismiss.

## Steps

1. Choose a schedule: off, daily, weekly, monthly, or after every 5 runs.
2. Care runs while Sentinel is on. A missed run starts at the next launch.
3. Care compares changed files with file notes, summarizes old run notes into a digest, looks for patterns, and marks unused notes for review.
4. Pruning weakens notes nobody uses and proposes retirement. It never deletes a note.
5. Accept or dismiss each proposal in the Brain panel. Proposals are also saved as files in `_proposals/`.
6. Each care run has its own budget, by default $0.10. It uses the cheapest ready agent and stops at its budget.

## See it

<figure class="fig" data-fig="brain-care-proposals" data-ratio="16 / 10" data-title="The Brain proposals tab.">
<figcaption>The Brain proposals tab.<ol><li>Proposals wait here. Nothing changes until you accept.</li><li>Each proposal says what changes and why.</li><li>Accept applies it. Dismiss drops it.</li></ol></figcaption>
<noscript><p>This view needs JavaScript. Open the <a href="../prototype/sentinel-prototype.html">prototype</a> instead.</p></noscript>
</figure>

## Tip

A setting can let care apply changes to notes that Sentinel wrote. It never changes your notes.

## Related

- [Notes and links](notes.md)
- [Lessons](lessons.md)
- [Settings](../reference/settings.md)
