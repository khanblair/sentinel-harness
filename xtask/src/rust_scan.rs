//! A tiny scanner for Rust source: masked line views, function spans, nesting depth.

use std::path::PathBuf;

/// One `.rs` file held as lines, ready to scan.
pub struct RustFile {
    pub path: PathBuf,
    pub lines: Vec<String>,
}

/// The span of one `fn` item. Lines are 1-based and inclusive.
pub struct FnSpan {
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
    pub is_test: bool,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Lex {
    #[default]
    Code,
    Block(usize),
    Quoted(char),
}

impl RustFile {
    pub fn new(path: impl Into<PathBuf>, text: &str) -> Self {
        Self {
            path: path.into(),
            lines: text.lines().map(str::to_string).collect(),
        }
    }

    /// Lines with string and comment text blanked, so keywords and braces stay readable.
    pub fn code_lines(&self) -> Vec<String> {
        self.mask_all().0
    }

    pub fn line_comments(&self) -> Vec<Option<String>> {
        self.mask_all().1
    }

    pub fn functions(&self) -> Vec<FnSpan> {
        self.scan().0
    }

    /// One flag per line: true inside a `#[test]` function or a `#[cfg(test)] mod`.
    pub fn test_lines(&self) -> Vec<bool> {
        let mut flags = vec![false; self.lines.len()];
        let (spans, mods) = self.scan();
        for span in spans.iter().filter(|span| span.is_test) {
            mark(&mut flags, span.start_line, span.end_line);
        }
        for (start, end) in mods {
            mark(&mut flags, start, end);
        }
        flags
    }

    /// The deepest brace depth inside the function body. The body itself is 0.
    pub fn max_nesting(&self, span: &FnSpan) -> usize {
        let code = self.code_lines();
        let Some((open_line, open_col)) = body_open(&code, span.start_line - 1) else {
            return 0;
        };
        let mut depth = 0usize;
        let mut peak = 0usize;
        for (idx, line) in code.iter().enumerate().skip(open_line) {
            let start = if idx == open_line { open_col + 1 } else { 0 };
            for ch in line.chars().skip(start) {
                if ch == '{' {
                    depth += 1;
                    peak = peak.max(depth);
                } else if ch == '}' && depth == 0 {
                    return peak;
                } else if ch == '}' {
                    depth -= 1;
                }
            }
        }
        peak
    }

    fn scan(&self) -> (Vec<FnSpan>, Vec<(usize, usize)>) {
        let code = self.code_lines();
        let test_mods = test_mod_ranges(&code);
        let spans: Vec<FnSpan> = code
            .iter()
            .enumerate()
            .filter_map(|(idx, line)| span_at(&code, &test_mods, idx, line))
            .collect();
        (spans, test_mods)
    }

    fn mask_all(&self) -> (Vec<String>, Vec<Option<String>>) {
        let mut lex = Lex::Code;
        let mut code = Vec::with_capacity(self.lines.len());
        let mut comments = Vec::with_capacity(self.lines.len());
        for line in &self.lines {
            let (masked, comment) = mask_line(line, &mut lex);
            code.push(masked);
            comments.push(comment);
        }
        (code, comments)
    }
}

fn span_at(
    code: &[String],
    test_mods: &[(usize, usize)],
    idx: usize,
    line: &str,
) -> Option<FnSpan> {
    let name = fn_name(line.trim_start())?;
    let (open_line, open_col) = body_open(code, idx)?;
    let above = idx > 0 && code[idx - 1].contains("#[test]");
    let cfg = idx > 0 && code[idx - 1].contains("#[cfg(test)]");
    let start_line = idx + 1;
    Some(FnSpan {
        name: name.to_string(),
        start_line,
        end_line: match_brace(code, open_line, open_col),
        is_test: above || cfg || in_range(test_mods, start_line),
    })
}

/// Line ranges of `#[cfg(test)] mod` blocks, from the attribute line to the closing brace.
fn test_mod_ranges(code: &[String]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut idx = 0;
    while idx < code.len() {
        if code[idx].contains("#[cfg(test)]")
            && let Some(mod_line) = next_mod_line(code, idx)
            && let Some((open_line, open_col)) = body_open(code, mod_line)
        {
            ranges.push((idx + 1, match_brace(code, open_line, open_col)));
            idx = mod_line;
        }
        idx += 1;
    }
    ranges
}

// The `mod` belongs to the attribute only when it is on the same line or directly below.
fn next_mod_line(code: &[String], idx: usize) -> Option<usize> {
    let same = &code[idx];
    if same.contains("mod ") && same.contains('{') {
        return Some(idx);
    }
    let next = code.get(idx + 1)?;
    next.trim_start().starts_with("mod ").then_some(idx + 1)
}

fn in_range(ranges: &[(usize, usize)], line: usize) -> bool {
    ranges
        .iter()
        .any(|(start, end)| line > *start && line <= *end)
}

/// Finds the start of a function body: the first `{` outside brackets, or `None` on `;`.
fn body_open(code: &[String], from: usize) -> Option<(usize, usize)> {
    let mut brackets = 0usize;
    for (idx, line) in code.iter().enumerate().skip(from) {
        for (col, ch) in line.char_indices() {
            if ch == '[' {
                brackets += 1;
            } else if ch == ']' {
                brackets = brackets.saturating_sub(1);
            } else if ch == '{' && brackets == 0 {
                return Some((idx, col));
            } else if ch == ';' && brackets == 0 {
                return None;
            }
        }
    }
    None
}

fn match_brace(code: &[String], open_line: usize, open_col: usize) -> usize {
    let mut depth = 1usize;
    for (idx, line) in code.iter().enumerate().skip(open_line) {
        for (col, ch) in line.char_indices() {
            if idx == open_line && col <= open_col {
                continue;
            }
            if ch == '{' {
                depth += 1;
            } else if ch == '}' && depth == 1 {
                return idx + 1;
            } else if ch == '}' {
                depth -= 1;
            }
        }
    }
    code.len()
}

fn fn_name(line: &str) -> Option<&str> {
    let rest = strip_qualifiers(line);
    let rest = rest.strip_prefix("fn")?;
    if !(rest.starts_with(char::is_whitespace) || rest.starts_with(['(', '<'])) {
        return None;
    }
    let name = leading_ident(rest.trim_start());
    (!name.is_empty()).then_some(name)
}

fn strip_qualifiers(line: &str) -> &str {
    let mut rest = line.trim_start();
    loop {
        if let Some(after) = rest.strip_prefix("pub") {
            rest = skip_visibility(after.trim_start());
            continue;
        }
        if let Some(after) = qualifier_tail(rest) {
            rest = after;
            continue;
        }
        return rest;
    }
}

fn skip_visibility(after: &str) -> &str {
    let Some(inner) = after.strip_prefix('(') else {
        return after;
    };
    let end = inner.find(')').unwrap_or(inner.len());
    inner[end + 1..].trim_start()
}

fn qualifier_tail(rest: &str) -> Option<&str> {
    ["async", "const", "unsafe"].iter().find_map(|kw| {
        let after = rest.strip_prefix(kw)?;
        let spaced = after.starts_with(char::is_whitespace) || after.starts_with('(');
        spaced.then(|| after.trim_start())
    })
}

pub fn leading_ident(word: &str) -> &str {
    let end = word
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(word.len());
    &word[..end]
}

fn mark(flags: &mut [bool], start: usize, end: usize) {
    for flag in flags.iter_mut().take(end).skip(start.saturating_sub(1)) {
        *flag = true;
    }
}

fn mask_line(line: &str, lex: &mut Lex) -> (String, Option<String>) {
    let src: Vec<char> = line.chars().collect();
    let mut masker = Masker {
        src: &src,
        out: String::with_capacity(line.len()),
        lex: *lex,
        comment: None,
    };
    let mut at = 0;
    while at < masker.src.len() {
        at = masker.step(at);
    }
    *lex = masker.lex;
    (masker.out, masker.comment)
}

struct Masker<'a> {
    src: &'a [char],
    out: String,
    lex: Lex,
    comment: Option<String>,
}

impl Masker<'_> {
    fn step(&mut self, at: usize) -> usize {
        match self.lex {
            Lex::Code => self.step_code(at),
            Lex::Block(depth) => self.step_block(at, depth),
            Lex::Quoted(close) => self.step_quoted(at, close),
        }
    }

    fn step_code(&mut self, at: usize) -> usize {
        let ch = self.src[at];
        let next = self.src.get(at + 1).copied();
        if ch == '/' && next == Some('/') {
            self.comment = Some(self.src[at + 2..].iter().collect());
            self.out.push_str(&" ".repeat(self.src.len() - at));
            return self.src.len();
        }
        if ch == '/' && next == Some('*') {
            self.lex = Lex::Block(1);
            self.out.push_str("  ");
            return at + 2;
        }
        if ch == '"' {
            self.lex = Lex::Quoted('"');
            self.out.push(' ');
            return at + 1;
        }
        if ch == '\'' && is_char_lit(self.src, at) {
            self.lex = Lex::Quoted('\'');
            self.out.push(' ');
            return at + 1;
        }
        self.out.push(ch);
        at + 1
    }

    fn step_block(&mut self, at: usize, depth: usize) -> usize {
        let nested = self.src[at] == '/' && self.src.get(at + 1) == Some(&'*');
        let close = self.src[at] == '*' && self.src.get(at + 1) == Some(&'/');
        if !(nested || close) {
            self.out.push(' ');
            return at + 1;
        }
        let depth = if nested {
            depth + 1
        } else {
            depth.saturating_sub(1)
        };
        self.lex = if depth == 0 {
            Lex::Code
        } else {
            Lex::Block(depth)
        };
        self.out.push_str("  ");
        at + 2
    }

    fn step_quoted(&mut self, at: usize, close: char) -> usize {
        if self.src[at] == '\\' {
            self.out.push_str("  ");
            return (at + 2).min(self.src.len());
        }
        self.out.push(' ');
        self.lex = if self.src[at] == close {
            Lex::Code
        } else {
            Lex::Quoted(close)
        };
        at + 1
    }
}

fn is_char_lit(src: &[char], start: usize) -> bool {
    matches!(
        (src.get(start + 1), src.get(start + 2)),
        (Some(_), Some('\'')) | (Some('\\'), _)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(text: &str) -> RustFile {
        RustFile::new("probe.rs", text)
    }

    #[test]
    fn spans_ignore_quoted_braces_and_skip_declarations() {
        let text = "fn quoted() {\n    let brace = \"{\";\n    let raw = r#\" } \"#;\n    // keep { this } open\n    let ch = '{';\n}\nfn after() {}\n";
        let spans = scan(text).functions();
        assert_eq!((spans.len(), spans[0].name.as_str()), (2, "quoted"));
        assert_eq!((spans[0].start_line, spans[0].end_line), (1, 6));
        assert_eq!(spans[1].name, "after");
        let text =
            "fn declared();\npub(crate) async fn worker() {}\npub const unsafe fn other() {}\n";
        let file = scan(text);
        let spans = file.functions();
        assert_eq!((spans.len(), spans[1].name.as_str()), (2, "other"));
        assert_eq!(spans[0].name, "worker");
        assert_eq!(file.max_nesting(&spans[0]), 0);
    }

    #[test]
    fn nested_closures_stay_inside_the_enclosing_function_and_add_depth() {
        let text = "fn outer() {\n    let add = |a: u8| {\n        let inner = |b: u8| { b + a };\n        inner(1)\n    };\n    add(2);\n}\nfn next() {}\n";
        let file = scan(text);
        let spans = file.functions();
        assert_eq!((spans.len(), spans[0].name.as_str()), (2, "outer"));
        assert_eq!((spans[0].start_line, spans[0].end_line), (1, 7));
        assert_eq!(file.max_nesting(&spans[0]), 2);
    }

    #[test]
    fn test_attributes_and_cfg_test_mods_mark_their_functions() {
        let text = "fn build() {}\n#[test]\nfn checks() {}\n#[cfg(test)]\nmod tests {\n    fn helper() {}\n}\n";
        let spans = scan(text).functions();
        assert_eq!(spans.len(), 3);
        assert!(!spans[0].is_test && spans[1].is_test && spans[2].is_test);
    }
}
