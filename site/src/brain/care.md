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

## Screenshot

![Screenshot of brain-care-proposals](../images/brain-care-proposals.png)

## Tip

A setting can let care apply changes to notes that Sentinel wrote. It never changes your notes.

## Related

- [Notes and links](notes.md)
- [Lessons](lessons.md)
- [Settings](../reference/settings.md)
