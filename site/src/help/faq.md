# FAQ

## What it is

The FAQ answers common questions about how Sentinel works.

## Steps

1. **Does Sentinel use git worktrees?** No. It keeps one working tree and reserves files instead.
2. **Does Sentinel need a cloud service?** No. Remote access runs over Tailscale only. The daemon binds to the Tailscale address only.
3. **Where are my keys stored?** In the OS keychain. Sentinel never prints them.
4. **Can a phone or Telegram raise the cap?** No. Every remote can only lower the cap.
5. **Which agent CLIs does Sentinel find?** It scans for claude, codex, opencode, gemini, goose, pi, agy, deepseek, and mcode, and you can add others.
6. **Does a task that an agent calls done count as done?** No. Only the verifier marks a task VERIFIED. It needs a passing suite and a check by a different model.
7. **Can I edit notes from the phone?** No. Edit notes on the PC.

## Screenshot

![Screenshot of faq-verdicts](../images/faq-verdicts.png)

## Tip

When a verdict looks wrong, read the status words page first.

## Related

- [Status words](../reference/status-words.md)
- [What each remote may do](../remote/remote-limits.md)
- [Install](../get-started/install.md)
