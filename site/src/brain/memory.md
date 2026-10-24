# Agent memory and the run journal

## What it is

Before each task, Sentinel selects the notes the agent should read. In learn mode, each run also writes a journal.

## Steps

1. Sentinel picks notes for a task in this order: pinned notes, then notes linked to the task's files, then search hits.
2. The selection stops at a 500 token cap.
3. The stream lists the notes that were loaded for each task.
4. Agents can search and read notes through MCP tools. They can propose a note. They cannot write one.
5. In learn mode, each run writes a run note, a note for each task, and a note for each touched file. Backlinks then show which tasks touched a file.

## See it

<figure class="fig" data-fig="brain-memory-stream" data-ratio="16 / 10" data-title="The stream says which Brain notes were loaded.">
<figcaption>The stream, before the first task.<ol><li>The line that says how many Brain notes were loaded and why.</li></ol></figcaption>
<noscript><p>This view needs JavaScript. Open the <a href="../prototype/sentinel-prototype.html">prototype</a> instead.</p></noscript>
</figure>

## Tip

Live runs use read-only mode. Nothing is written during a live run.

## Related

- [Notes and links](notes.md)
- [Lessons](lessons.md)
- [Settings](../reference/settings.md)
