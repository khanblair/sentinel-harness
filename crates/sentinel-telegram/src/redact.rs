//! Replaces key-like text with `[hidden]` before any text goes to Telegram.
//! The text is split into runs of token characters. Each run is checked on its own.

const HIDDEN: &str = "[hidden]";
const SECRET_WORDS: [&str; 4] = ["KEY", "TOKEN", "SECRET", "PASSWORD"];
const MIN_KEY_BODY: usize = 16;
const MIN_LONG_RUN: usize = 32;
const MIN_BOT_SECRET: usize = 30;

pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    for ch in text.chars() {
        if is_token_char(ch) {
            run.push(ch);
            continue;
        }
        push_run(&mut run, &mut out);
        out.push(ch);
    }
    push_run(&mut run, &mut out);
    out
}

fn push_run(run: &mut String, out: &mut String) {
    if looks_secret(run) {
        out.push_str(HIDDEN);
    } else {
        out.push_str(run);
    }
    run.clear();
}

fn is_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '+' | '/' | '=' | ':')
}

fn looks_secret(run: &str) -> bool {
    run.split(['=', ':']).any(has_key_prefix)
        || is_bot_token(run)
        || is_named_secret(run)
        || has_long_run(run)
}

/// Checks the part after `=` or `:`, so `token:sk-...` is caught too.
fn has_key_prefix(part: &str) -> bool {
    let key_body = |rest: &str| {
        rest.len() >= MIN_KEY_BODY
            && rest
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
    };
    if let Some(rest) = part.strip_prefix("sk-") {
        return key_body(rest);
    }
    if let Some(rest) = part.strip_prefix("ghp_") {
        return key_body(rest);
    }
    if let Some(rest) = part.strip_prefix("AKIA") {
        return rest.len() >= MIN_KEY_BODY
            && rest
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    }
    is_slack_token(part)
}

fn is_slack_token(part: &str) -> bool {
    let Some(rest) = ["xoxb-", "xoxa-", "xoxp-", "xoxr-", "xoxs-"]
        .iter()
        .find_map(|prefix| part.strip_prefix(prefix))
    else {
        return false;
    };
    rest.len() >= 10 && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// The Telegram bot token shape: a numeric bot id, a colon, then a secret of letters and digits.
fn is_bot_token(run: &str) -> bool {
    let Some((bot_id, secret)) = run.split_once(':') else {
        return false;
    };
    bot_id.len() >= 8
        && bot_id.bytes().all(|b| b.is_ascii_digit())
        && secret.len() >= MIN_BOT_SECRET
        && secret
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
}

/// `NAME=value` where the name says it holds a secret, such as `API_KEY=...`.
fn is_named_secret(run: &str) -> bool {
    let Some((name, value)) = run.split_once('=') else {
        return false;
    };
    let name = name.to_ascii_uppercase();
    !value.is_empty() && SECRET_WORDS.iter().any(|word| name.contains(word))
}

/// A run of 32 or more hex or base64 characters that looks random. Paths and long names have no
/// digits or signs, so they pass. Without this, a deep file path would be hidden.
fn has_long_run(run: &str) -> bool {
    run.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=')))
        .any(looks_random)
}

fn looks_random(part: &str) -> bool {
    if part.len() < MIN_LONG_RUN {
        return false;
    }
    if part.bytes().all(|b| b.is_ascii_hexdigit()) {
        return true;
    }
    let has_digit = part.bytes().any(|b| b.is_ascii_digit());
    let mixed_case = part.bytes().any(|b| b.is_ascii_lowercase())
        && part.bytes().any(|b| b.is_ascii_uppercase());
    has_digit && (mixed_case || part.contains(['+', '=']))
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn a_secret_key_with_sk_prefix_is_hidden() {
        let text = "key sk-abcdefghijklmnopqrstu done";
        assert_eq!(redact(text), "key [hidden] done");
    }

    #[test]
    fn a_github_token_is_hidden() {
        assert_eq!(redact("ghp_abcdefghijklmnop1234"), "[hidden]");
    }

    #[test]
    fn a_slack_token_is_hidden() {
        assert_eq!(redact("xoxb-1234-5678-abcdefgh"), "[hidden]");
    }

    #[test]
    fn an_aws_access_key_is_hidden() {
        assert_eq!(redact("AKIAABCDEFGHIJKLMNOP"), "[hidden]");
    }

    #[test]
    fn a_telegram_bot_token_is_hidden() {
        let text = "bot 123456789:AAFakeTokenValueForTestsOnly_abcdef ok";
        assert_eq!(redact(text), "bot [hidden] ok");
    }

    #[test]
    fn a_long_hex_or_base64_run_is_hidden() {
        let hex = "0123456789abcdef0123456789abcdef";
        let b64 = "abcDEF123+/abcDEF123+/abcDEF123+/ab==";
        assert_eq!(redact(hex), "[hidden]");
        assert_eq!(redact(b64), "[hidden]");
    }

    #[test]
    fn a_deep_source_path_stays_unchanged() {
        let path = "src/components/settings/integrations/telegram.ts";
        assert_eq!(redact(path), path);
    }

    #[test]
    fn a_long_camel_case_name_stays_unchanged() {
        let name = "renderTelegramMessageWithButtonsAndQuietHours";
        assert_eq!(redact(name), name);
    }

    #[test]
    fn a_long_mixed_case_string_with_digits_is_hidden() {
        assert_eq!(redact("Qz9xLm2VbN7wKp4RsT8yHj3DfG6cXa1B"), "[hidden]");
    }

    #[test]
    fn a_named_secret_assignment_is_hidden() {
        assert_eq!(redact("OPENAI_API_KEY=abc123"), "[hidden]");
        assert_eq!(redact("db_password=hunter2"), "[hidden]");
        assert_eq!(redact("GITHUB_TOKEN=x SECRET_VALUE=y"), "[hidden] [hidden]");
    }

    #[test]
    fn a_plain_assignment_without_a_secret_word_stays() {
        assert_eq!(redact("MODE=fast"), "MODE=fast");
    }

    #[test]
    fn plain_status_text_stays_unchanged() {
        let text = "Wave 2 done. 3 of 3 verified. T-04 Verified: src/ledger/post.ts";
        assert_eq!(redact(text), text);
    }

    #[test]
    fn money_and_clock_text_stays_unchanged() {
        let text = "Cost $0.88 of $2.00 at 12:30 to 13:00";
        assert_eq!(redact(text), text);
    }
}
