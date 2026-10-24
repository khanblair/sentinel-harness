# Permissions and approvals

## What it is

One setting decides how much agents may do without asking. Reserved files stay the hard boundary in every mode.

## Steps

1. Open Permissions from the More menu, or from Settings.
2. Choose one of four modes:
   - Plan only adds an approval card before any writing.
   - Ask each wave adds an approval card before every wave.
   - Auto inside reserved files is the default. A request for another file becomes an approval card.
   - Bypass permissions grants a free file at once and logs the grant.
3. Expect a write to a file the task has not reserved to be blocked and restored, in every mode.
4. Choose Bypass permissions only after you read its confirm dialog. The dialog lists what changes and what stays enforced: reserved files, the test-edit guard, and the budget cap.
5. Answer approval cards in the stream, or from a paired phone. Phone answers can be turned off.
6. Set the capability matrix for each agent. It covers read, write reserved files, ask for more files, run tests, run shell commands, and network.

## See it

<figure class="fig" data-fig="permissions-panel" data-ratio="16 / 10" data-title="The Permissions panel.">
<figcaption>The Permissions panel: the modes and the per-agent matrix.<ol><li>Pick a mode. Auto writes only to files a task reserved.</li><li>One row per agent. Turn a capability on or off.</li><li>No mode turns off the file locks.</li></ol></figcaption>
<noscript><p>This view needs JavaScript. Open the <a href="../prototype/sentinel-prototype.html">prototype</a> instead.</p></noscript>
</figure>

## Tip

The Bypass permissions chip uses the warning color, so the setting stands out.

## Related

- [The stream](stream.md)
- [Reserved files and waves](reserved-files.md)
- [What each remote may do](../remote/remote-limits.md)
