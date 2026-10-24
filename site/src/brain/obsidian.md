# Using the vault in Obsidian

## What it is

The Brain folder is a plain Obsidian vault, so you can edit notes in Obsidian and see the changes in Sentinel.

## Steps

1. Open Obsidian and open the folder `.sentinel/brain/` in your repository as a vault.
2. Edit, link, and tag notes the way you would in any Obsidian vault.
3. Sentinel rebuilds its search index from the files, so edits made in Obsidian show up in Sentinel.
4. If an edit is missing in Sentinel, run `brain reindex` from the command line.
5. Make edits on the PC only. Notes become agent prompts, and the phone cannot edit notes.

## Screenshot

![Screenshot of brain-obsidian-vault](../images/brain-obsidian-vault.png)

## Tip

The notes are plain files, so any text editor can read them.

## Related

- [Notes and links](notes.md)
- [Brain care](care.md)
- [What each remote may do](../remote/remote-limits.md)
