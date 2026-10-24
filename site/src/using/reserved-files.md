# Reserved files and waves

## What it is

Reserved files give each task exclusive write access to the files it needs, so two tasks never write the same file at once.

## Steps

1. Open the Reserved panel. It shows which files each task holds and which tasks wait.
2. Expect tasks with no shared files to run together in one wave.
3. Expect a write to a file the task has not reserved to be blocked and restored.
4. Expect each finished task to become one checkpoint commit.
5. Expect the full suite to run at each barrier, on a clean tree.
6. When a task needs another file, it asks for it. A free file is granted. A busy file makes the task wait or yield.
7. If one task must read the final form of a file that another task writes, the planner makes the first task wait for that checkpoint.

## Screenshot

![Screenshot of reserved-panel](../images/reserved-panel.png)

## Tip

Reserved files are the hard boundary. No permission mode changes that.

## Related

- [Permissions and approvals](permissions.md)
- [The stream](stream.md)
- [Status words](../reference/status-words.md)
