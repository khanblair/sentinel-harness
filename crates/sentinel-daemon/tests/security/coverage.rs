//! Reads the route source at test time. A `.route(` call with no row in `ROUTES` fails the build,
//! so a new route cannot skip the access matrix.

use std::fs;
use std::path::{Path, PathBuf};

use crate::table::Route;

const METHODS: [&str; 5] = ["get", "post", "put", "delete", "patch"];

/// One `.route(` call: the path it serves and the methods it answers, in lowercase.
#[derive(Debug, PartialEq, Eq)]
pub struct Site {
    pub path: String,
    pub methods: Vec<&'static str>,
}

/// The number of `.route(` calls, and the sites parsed from them.
pub fn read() -> (usize, Vec<Site>) {
    let mut calls = 0;
    let mut sites = Vec::new();
    for file in source_files() {
        let text = fs::read_to_string(&file).unwrap();
        calls += text.matches(".route(").count();
        sites.extend(sites_in(&text));
    }
    (calls, sites)
}

/// The route files, plus `server.rs`, which mounts `/ws`.
fn source_files() -> Vec<PathBuf> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files: Vec<PathBuf> = fs::read_dir(src.join("routes"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    files.push(src.join("server.rs"));
    files
}

fn sites_in(source: &str) -> Vec<Site> {
    source
        .split(".route(")
        .skip(1)
        .filter_map(site_in_call)
        .collect()
}

/// `call` starts just after `.route(`.
fn site_in_call(call: &str) -> Option<Site> {
    let args = call_args(call);
    let open = args.find('"')?;
    let close = open + 1 + args[open + 1..].find('"')?;
    let path = args[open + 1..close].to_owned();
    let after = &args[close + 1..];
    let methods = METHODS
        .into_iter()
        .filter(|method| has_call(after, method))
        .collect();
    Some(Site { path, methods })
}

/// The text inside the call's parentheses, found by counting nesting.
fn call_args(call: &str) -> &str {
    let mut depth = 1_i32;
    for (index, byte) in call.bytes().enumerate() {
        depth += match byte {
            b'(' => 1,
            b')' => -1,
            _ => 0,
        };
        if depth == 0 {
            return &call[..index];
        }
    }
    call
}

/// True when `name(` appears as a whole word, such as `get(handler)` but not `get_state(`.
fn has_call(text: &str, name: &str) -> bool {
    let needle = format!("{name}(");
    text.match_indices(&needle)
        .any(|(index, _)| !text[..index].ends_with(|c: char| c.is_alphanumeric() || c == '_'))
}

/// `METHOD /path` for each source method with no row in `routes`.
pub fn missing_rows(sites: &[Site], routes: &[Route]) -> Vec<String> {
    pairs(sites)
        .filter(|(method, path)| !routes.iter().any(|row| covers(row, method, path)))
        .map(|(method, path)| format!("{} {path}", method.to_uppercase()))
        .collect()
}

/// `METHOD /path` for each row that matches no source method. Such a row is stale.
pub fn stale_rows(sites: &[Site], routes: &[Route]) -> Vec<String> {
    let known: Vec<(&str, &str)> = pairs(sites).collect();
    routes
        .iter()
        .filter(|row| !known.iter().any(|(method, path)| covers(row, method, path)))
        .map(|row| format!("{} {}", row.method, row.path))
        .collect()
}

fn pairs(sites: &[Site]) -> impl Iterator<Item = (&'static str, &str)> {
    sites.iter().flat_map(|site| {
        site.methods
            .iter()
            .map(move |method| (*method, site.path.as_str()))
    })
}

fn covers(row: &Route, method: &str, path: &str) -> bool {
    row.method.as_str().eq_ignore_ascii_case(method) && same_path(path, row.path)
}

/// Compares a route template such as `/api/chats/{id}` with a concrete path such as
/// `/api/chats/nope`. A `{...}` segment matches any segment.
fn same_path(template: &str, concrete: &str) -> bool {
    let wanted: Vec<&str> = template.split('/').collect();
    let given: Vec<&str> = concrete.split('/').collect();
    wanted.len() == given.len()
        && wanted
            .iter()
            .zip(&given)
            .all(|(want, got)| (want.starts_with('{') && want.ends_with('}')) || want == got)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_with_two_methods_reads_both() {
        let sites = sites_in(r#".route("/api/state", get(get_state).put(set_state))"#);
        assert_eq!(
            sites,
            vec![Site {
                path: "/api/state".to_owned(),
                methods: vec!["get", "put"],
            }]
        );
    }

    #[test]
    fn a_handler_named_like_a_method_is_not_a_method() {
        let sites = sites_in(r#".route("/api/state", get(get_state))"#);
        assert_eq!(sites[0].methods, vec!["get"]);
    }

    #[test]
    fn a_call_split_over_lines_still_reads_its_path() {
        let text =
            ".route(\n    \"/api/chats/{id}/attachments/{attachment}\",\n    delete(remove),\n)";
        let sites = sites_in(text);
        assert_eq!(sites[0].path, "/api/chats/{id}/attachments/{attachment}");
        assert_eq!(sites[0].methods, vec!["delete"]);
    }

    #[test]
    fn a_template_matches_a_concrete_id_in_its_place() {
        assert!(same_path("/api/chats/{id}", "/api/chats/nope"));
        assert!(!same_path("/api/chats/{id}", "/api/chats"));
        assert!(!same_path("/api/chats", "/api/tasks"));
    }
}
