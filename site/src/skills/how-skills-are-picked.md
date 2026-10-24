# How Sentinel picks skills

## What it is

For each task, Sentinel chooses which skills go into the prompt. The choice is plain and the stream shows it.

## Steps

1. Every skill set to Always goes in first.
2. Skills you added by hand for this message go next.
3. Then Sentinel looks at the skills set to Auto. It compares the words in a skill's name and description with the words in the task's title, text, and file names. A skill needs at least one shared word. At most 3 Auto skills are used per task.
4. All chosen skills have to fit in 1,500 tokens. A skill that does not fit is left out whole. It is never cut in half.
5. The stream shows a line such as "Skills loaded: house-style, release-checklist."

## See it

<figure class="fig" data-fig="skills-loaded" data-ratio="16 / 10" data-title="The Skills loaded line in the stream.">
<figcaption>The stream, before the first task.<ol><li>The skills that went into the task prompts.</li></ol></figcaption>
<noscript><p>This view needs JavaScript. Open the <a href="../prototype/sentinel-prototype.html">prototype</a> instead.</p></noscript>
</figure>

## Tip

If a skill is never picked, make its description say when to use it, using the words you use in your tasks.

## Related

- [Skills](index.md)
- [Use a skill by hand](use-a-skill.md)
- [Agent memory and the run journal](../brain/memory.md)
