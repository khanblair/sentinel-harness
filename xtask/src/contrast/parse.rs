//! Reads the color tables out of `prefs.rs`. The check reads the source text on purpose: the
//! UI crate is not a dependency of `xtask`.

use super::Palette;

/// The five colors of one tint row, in this order.
const TINT_KEYS: [&str; 5] = ["bg", "sidebar", "surface", "surface2", "border"];

pub(super) struct Tables {
    pub dark: Palette,
    pub light: Palette,
    pub dark_tints: Vec<(String, Palette)>,
    pub light_tints: Vec<(String, Palette)>,
}

/// All four tables, or every problem found with them.
pub(super) fn read_tables(text: &str) -> Result<Tables, Vec<String>> {
    let dark = parse_table(text, "DARK");
    let light = parse_table(text, "LIGHT");
    let dark_tints = parse_tints(text, "DARK_TINTS");
    let light_tints = parse_tints(text, "LIGHT_TINTS");
    match (dark, light, dark_tints, light_tints) {
        (Ok(dark), Ok(light), Ok(dark_tints), Ok(light_tints)) => Ok(Tables {
            dark,
            light,
            dark_tints,
            light_tints,
        }),
        (a, b, c, d) => Err([a.err(), b.err(), c.err(), d.err()]
            .into_iter()
            .flatten()
            .collect()),
    }
}

/// The text of the array that follows `const NAME:`, brackets included.
fn array_text<'a>(text: &'a str, name: &str) -> Result<&'a str, String> {
    let start = text
        .find(&format!("const {name}:"))
        .ok_or_else(|| format!("table {name} not found"))?;
    let after = &text[start..];
    let open = after
        .find("= &[")
        .ok_or_else(|| format!("table {name} missing array opening"))?;
    let array = &after[open + 3..];
    let close =
        matching_bracket(array).ok_or_else(|| format!("table {name} missing closing bracket"))?;
    Ok(&array[..=close])
}

fn matching_bracket(text: &str) -> Option<usize> {
    let mut depth = 0_i32;
    for (index, ch) in text.char_indices() {
        depth += match ch {
            '[' => 1,
            ']' => -1,
            _ => 0,
        };
        if ch == ']' && depth == 0 {
            return Some(index);
        }
    }
    None
}

/// Every `#RRGGBB` in a line, in order.
fn hex_colors(line: &str) -> Vec<String> {
    line.match_indices('#')
        .filter_map(|(index, _)| line.get(index..index + 7))
        .filter(|color| color[1..].chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_owned)
        .collect()
}

/// The first quoted string of a line.
fn first_quoted(line: &str) -> Option<&str> {
    let open = line.find('"')? + 1;
    let close = line[open..].find('"')?;
    Some(&line[open..open + close])
}

fn parse_table(text: &str, name: &str) -> Result<Palette, String> {
    let table = array_text(text, name)?;
    let mut palette = Palette::new();
    for line in table.lines().map(str::trim).filter(|l| l.starts_with('(')) {
        if let (Some(key), Some(color)) = (first_quoted(line), hex_colors(line).into_iter().next())
        {
            palette.insert(key.to_owned(), color);
        }
    }
    Ok(palette)
}

fn parse_tints(text: &str, name: &str) -> Result<Vec<(String, Palette)>, String> {
    let table = array_text(text, name)?;
    let mut tints = Vec::new();
    for line in table.lines().map(str::trim).filter(|l| l.starts_with('(')) {
        let colors = hex_colors(line);
        let Some(tint) = first_quoted(line).filter(|_| colors.len() == TINT_KEYS.len()) else {
            continue;
        };
        let palette = TINT_KEYS.iter().map(|key| (*key).to_owned()).zip(colors);
        tints.push((tint.to_owned(), palette.collect()));
    }
    Ok(tints)
}
