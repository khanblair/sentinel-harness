//! The web UI, embedded in the daemon. A path with no file gets the page, so the single page app
//! can route it. An unknown API path gets a JSON 404 instead.

use axum::Json;
use axum::body::Body;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;
use sentinel_core::api::ApiError;

#[derive(RustEmbed)]
#[folder = "../sentinel-ui/dist/"]
struct Dist;

const INDEX: &str = "index.html";
const NO_CACHE: &str = "no-cache";
/// Files whose name carries a content hash never change, so the browser may keep them.
const LONG_CACHE: &str = "public, max-age=31536000, immutable";

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path == "api" || path.starts_with("api/") {
        return api_not_found();
    }
    if !path.is_empty()
        && let Some(file) = Dist::get(path)
    {
        return file_response(path, file.data.into_owned());
    }
    index_response()
}

fn index_response() -> Response {
    match Dist::get(INDEX) {
        Some(file) => file_response(INDEX, file.data.into_owned()),
        None => (StatusCode::NOT_FOUND, "The UI is missing from this build.").into_response(),
    }
}

fn api_not_found() -> Response {
    let body = ApiError {
        error: "No such API route.".to_owned(),
    };
    (StatusCode::NOT_FOUND, Json(body)).into_response()
}

fn file_response(name: &str, bytes: Vec<u8>) -> Response {
    let cache = if name != INDEX && has_content_hash(name) {
        LONG_CACHE
    } else {
        NO_CACHE
    };
    (
        [
            (header::CONTENT_TYPE, content_type(name)),
            (header::CACHE_CONTROL, cache),
        ],
        Body::from(bytes),
    )
        .into_response()
}

/// True when a name has a segment of 8 or more hex digits, such as `app-3f9a0c1d.js`.
pub fn has_content_hash(name: &str) -> bool {
    name.split(['-', '.', '/'])
        .any(|part| part.len() >= 8 && part.bytes().all(|b| b.is_ascii_hexdigit()))
}

pub fn content_type(name: &str) -> &'static str {
    let extension = name.rsplit('.').next().unwrap_or_default();
    match extension {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "wasm" => "application/wasm",
        "json" => "application/json",
        "webmanifest" => "application/manifest+json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hashed_asset_name_is_recognized() {
        assert!(has_content_hash("assets/app-3f9a0c1d.js"));
        assert!(has_content_hash("sentinel-ui-0123abcd45ef.wasm"));
    }

    #[test]
    fn a_plain_name_is_not_taken_for_a_hash() {
        assert!(!has_content_hash("index.html"));
        assert!(!has_content_hash("sentinel-ui.js"));
    }

    #[test]
    fn known_extensions_get_their_media_type() {
        assert_eq!(content_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(content_type("app.wasm"), "application/wasm");
        assert_eq!(content_type("data.bin"), "application/octet-stream");
    }
}
