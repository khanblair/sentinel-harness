# Telegram

## What it is

Telegram is a bot in one private chat. It sends alerts and takes short commands, and it works without the app open.

## Steps

1. In Telegram, message @BotFather, send `/newbot`, and choose a name and a username. Copy the token it sends you.
2. In Sentinel, open Settings, then Integrations, then Telegram. Paste the bot token and your numeric Telegram user ID. Use the number, not your @username. The token goes to the OS keychain.
3. Open your bot in Telegram and send `/start`. Sentinel connects the chat. A bot cannot message you first.
4. Choose a level, the events you want, and a detail level. Detail is the default. It adds short diffs, at most 6 lines each.
5. Press Send test message to check the connection.

Levels, set for the chat:

| Level | What it allows |
|---|---|
| Notify | Sentinel sends events to the chat |
| Control | Approve or deny, answer decisions, pause, resume, stop, kill an agent, lower the cap |
| Code | Message one agent, ask for a short diff summary, add a task to `backlog.md`, start a run |
| Reports | A summary when a run finishes, and a daily summary with cost and Brain proposals |

Commands: `/status`, `/pause`, `/resume`, `/stop`, `/cap`, `/kill`, `/agent`, `/diff`, `/task`, `/run`, `/report`, `/daily`, `/brief`, `/wrapup`, `/brain`, `/help`.

## See it

<figure class="fig" data-fig="telegram-setup" data-ratio="16 / 10" data-title="Settings, Integrations, before the chat is paired.">
<figcaption>Settings, Integrations, before the chat is paired.<ol><li>Paste the bot token from BotFather.</li><li>Your numeric user ID, not your @username.</li><li>Save checks both values.</li></ol></figcaption>
<noscript><p>This view needs JavaScript. Open the <a href="../prototype/sentinel-prototype.html">prototype</a> instead.</p></noscript>
</figure>

## Tip

Choose Status only detail if you want the fewest words in each message.

## Related

- [Control a run from Telegram](../guides/telegram-control.md)
- [What each remote may do](remote-limits.md)
- [Settings](../reference/settings.md)
