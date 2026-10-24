# Skills

## What it is

A skill is a folder with a `SKILL.md` file that teaches agents one thing the way you want it done: a house style, a release checklist, how to write a migration. It is the same format Claude, Codex, and Antigravity use, so a skill written for one of them works in Sentinel.

## Steps

1. Open Settings, then Skills.
2. Press New skill. Give it a short name (lowercase letters, digits, and dashes), one sentence that says what it does and when to use it, and the body.
3. Or look under "Found on this computer". Sentinel lists skills from `.claude/skills`, `.agents/skills`, `.agent/skills`, and `.codex/skills`, in the repository and in your home folder. Press Import to copy one into Sentinel. The original is never changed.
4. Pick a switch for each skill: Off, Auto, or Always.
5. Start a run. The stream says which skills were loaded for each task.

## See it

<figure class="fig" data-fig="skills-page" data-ratio="16 / 10" data-title="The Skills page in Settings.">
<figcaption>The Skills page in Settings.<ol><li>Off, Auto, or Always for each skill.</li><li>New skill opens the editor.</li><li>Import copies a skill from another tool. The original stays as it was.</li></ol></figcaption>
<noscript><p>This view needs JavaScript. Open the <a href="../prototype/sentinel-prototype.html">prototype</a> instead.</p></noscript>
</figure>

## Tip

Keep a skill short and specific. Always puts it in every task, so use Always only for rules that apply to all your work, such as a house style.

## Related

- [Use a skill by hand](use-a-skill.md)
- [How Sentinel picks skills](how-skills-are-picked.md)
- [Add files, folders, a backlog, or a note](../using/chats/attachments.md)
