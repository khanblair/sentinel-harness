//! Front matter tokens: the `---` fences, `key: value` lines, and quoting.

/// Ordered `key: value` pairs from the front matter.
pub type Pairs = Vec<(String, String)>;

/// Split a note into front matter and body. The body loses one final newline.
pub fn split(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("---\n")?;
    let (front, after) = match rest.strip_prefix("---\n") {
        Some(after) => ("", after),
        None => rest.split_once("\n---\n")?,
    };
    let body = after.strip_suffix('\n').unwrap_or(after);
    Some((front, body))
}

/// Parse `key: value` lines. Values in double quotes are unescaped.
pub fn pairs(front: &str) -> Result<Pairs, String> {
    let mut out = Pairs::new();
    for line in front.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| format!("invalid line: {line}"))?;
        out.push((key.trim().to_string(), unquote(value.trim())));
    }
    Ok(out)
}

/// Quote a value when a plain value would not read back the same.
pub fn quote_if_needed(value: &str) -> String {
    if needs_quotes(value) {
        format!("\"{}\"", escape(value))
    } else {
        value.to_string()
    }
}

fn needs_quotes(value: &str) -> bool {
    value.is_empty()
        || value.starts_with(['[', ' ', '"'])
        || value.ends_with(' ')
        || value.contains([':', '#', '"', '\\', '\n'])
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
    out
}

fn unquote(raw: &str) -> String {
    let quoted = raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"');
    if !quoted {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw[1..raw.len() - 1].chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_value_round_trips_colon_hash_and_quote() {
        let value = "a: b #c \"d\"";
        let quoted = quote_if_needed(value);
        assert_eq!(unquote(&quoted), value);
    }

    #[test]
    fn split_handles_empty_front_matter() {
        assert_eq!(split("---\n---\nbody\n"), Some(("", "body")));
    }
}
