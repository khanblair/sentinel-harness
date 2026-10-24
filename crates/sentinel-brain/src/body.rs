//! Body text: wikilinks, the Sentinel generated block, and the lesson free-text check.

const START: &str = "<!-- sentinel:start -->";
const END: &str = "<!-- sentinel:end -->";

/// A body cut around the generated block. `before` and `after` are kept byte for byte.
struct Parts<'a> {
    before: &'a str,
    inner: &'a str,
    after: &'a str,
}

fn split_generated(body: &str) -> Option<Parts<'_>> {
    let start = body.find(START)?;
    let inner_start = start + START.len();
    let end = inner_start + body[inner_start..].find(END)?;
    Some(Parts {
        before: &body[..start],
        inner: &body[inner_start..end],
        after: &body[end + END.len()..],
    })
}

/// Text between the sentinel markers, if the markers are present.
pub fn generated_block(body: &str) -> Option<&str> {
    split_generated(body).map(|parts| parts.inner)
}

/// Replace only the text between the markers. Adds the block at the end when missing.
pub fn replace_generated(body: &str, new_text: &str) -> String {
    match split_generated(body) {
        Some(parts) => format!("{}{START}\n{new_text}\n{END}{}", parts.before, parts.after),
        None => append_block(body, new_text),
    }
}

fn append_block(body: &str, new_text: &str) -> String {
    let mut out = body.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(START);
    out.push('\n');
    out.push_str(new_text);
    out.push('\n');
    out.push_str(END);
    out.push('\n');
    out
}

/// True when the body holds anything outside the generated block.
pub fn has_free_text(body: &str) -> bool {
    match split_generated(body) {
        Some(parts) => !parts.before.trim().is_empty() || !parts.after.trim().is_empty(),
        None => !body.trim().is_empty(),
    }
}

/// Link targets in order, deduped. `[[id|label]]` gives `id`. Fenced code is skipped.
pub fn wikilinks(body: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut in_fence = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        for target in line_targets(line) {
            if !found.iter().any(|known| known == target) {
                found.push(target.to_string());
            }
        }
    }
    found
}

fn line_targets(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            break;
        };
        let target = after[..end].split('|').next().unwrap_or_default().trim();
        if !target.is_empty() {
            out.push(target);
        }
        rest = &after[end + 2..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wikilinks_skip_code_fences_and_dedupe() {
        let body = "[[a]] and [[b|label]]\n```\n[[hidden]]\n```\n[[a]]";
        assert_eq!(wikilinks(body), vec!["a", "b"]);
    }

    #[test]
    fn replace_generated_appends_block_when_missing() {
        let text = replace_generated("user", "gen");
        assert_eq!(generated_block(&text).map(str::trim), Some("gen"));
        assert!(text.starts_with("user\n"));
    }
}
