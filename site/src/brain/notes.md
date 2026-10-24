# Notes and links

## What it is

A note is one markdown file in the Brain, with a short header and links to other notes.

## Steps

1. Find the vault in the repository at `.sentinel/brain/`. Each note is one markdown file.
2. Read the front matter at the top of a note. It holds the type, tags, strength, and source.
3. Follow the `[[links]]` between notes. Backlinks show which other notes link to the one you open.
4. Pick a type for each note: convention, decision, fix, file, task, run, lesson, glossary, or how-to.
5. Open the Brain panel. It has a notes list with search and a type filter, a note view with backlinks, an editor, a graph view, and a proposals tab.

## See it

<figure class="fig" data-fig="brain-notes-list" data-ratio="16 / 10" data-title="The notes list, with search and the type filter.">
<figcaption>The notes list, with search and the type filter.<ol><li>Search by title, text, or tag.</li><li>Show one type of note, such as Lesson.</li><li>Each note: its type, its name, and a strength bar.</li></ol></figcaption>
<noscript><p>This view needs JavaScript. Open the <a href="../prototype/sentinel-prototype.html">prototype</a> instead.</p></noscript>
</figure>

## Tip

The files are the source of truth. The search index is rebuilt from them.

## Related

- [Agent memory and the run journal](memory.md)
- [Using the vault in Obsidian](obsidian.md)
- [Brain care](care.md)
