//! Who may call the API. Every protected route needs a device token in the `Authorization`
//! header. Changing requests must come from the page's own origin. No cookie is read or set, so
//! a page on another site cannot ride on a login it never had.

use std::future::Future;

use axum::extract::{FromRequestParts, Request, State};
use axum::http::{HeaderMap, Method, header, request::Parts};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use sentinel_core::DeviceKind;

use crate::controls::Caller;
use crate::engine::Engine;
use crate::routes::ApiFailure;

/// Rejects a request with no device token, or an unknown or revoked one. On success the
/// device is stored in the request for handlers to read as a [`Caller`].
pub async fn require_device(
    State(engine): State<Engine>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(token) = bearer_token(request.headers()).map(str::to_owned) else {
        return ApiFailure::unauthorized().into_response();
    };
    match caller_for_token(&engine, token).await {
        Some(caller) => {
            request.extensions_mut().insert(caller);
            next.run(request).await
        }
        None => ApiFailure::unauthorized().into_response(),
    }
}

/// The caller behind a token, when the token belongs to an unrevoked device.
pub async fn caller_for_token(engine: &Engine, token: String) -> Option<Caller> {
    let engine = engine.clone();
    // A store lookup is a blocking call, so it runs off the async runtime.
    let device = tokio::task::spawn_blocking(move || engine.authenticate(&token))
        .await
        .ok()??;
    Some(caller_for(device.kind == DeviceKind::Desktop, device.name))
}

/// A desktop keeps full control. Phones and Telegram chats get the limited caller.
pub fn caller_for(from_desktop: bool, name: String) -> Caller {
    if from_desktop {
        Caller::pc(name)
    } else {
        Caller::phone(name)
    }
}

/// Rejects a changing request whose `Origin` names another host than its `Host`. A request with
/// no `Origin` (a phone app, the CLI) is let through.
pub async fn check_origin(request: Request, next: Next) -> Response {
    if is_changing(request.method()) && !origin_matches_host(request.headers()) {
        let refusal = ApiFailure::new(
            axum::http::StatusCode::FORBIDDEN,
            "This request came from another site and was refused.",
        );
        return refusal.into_response();
    }
    next.run(request).await
}

fn is_changing(method: &Method) -> bool {
    *method == Method::POST || *method == Method::PUT || *method == Method::DELETE
}

pub fn origin_matches_host(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    let origin_host = origin.to_str().ok().and_then(origin_host);
    match (origin_host, host) {
        (Some(from), Some(to)) => from.eq_ignore_ascii_case(to),
        _ => false,
    }
}

/// The `host[:port]` part of an origin such as `http://100.64.0.5:7878`.
pub fn origin_host(origin: &str) -> Option<&str> {
    origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
        .map(|rest| rest.trim_end_matches('/'))
        .filter(|rest| !rest.is_empty())
}

pub fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

impl<S: Send + Sync> FromRequestParts<S> for Caller {
    type Rejection = ApiFailure;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let found = parts
            .extensions
            .get::<Caller>()
            .cloned()
            .ok_or_else(ApiFailure::unauthorized);
        std::future::ready(found)
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            if let Ok(value) = HeaderValue::from_str(value) {
                map.insert(*name, value);
            }
        }
        map
    }

    #[test]
    fn bearer_token_is_read_from_the_authorization_header() {
        let map = headers(&[("authorization", "Bearer abc123")]);
        assert_eq!(bearer_token(&map), Some("abc123"));
    }

    #[test]
    fn a_missing_or_empty_bearer_token_is_no_token() {
        assert_eq!(bearer_token(&HeaderMap::new()), None);
        let map = headers(&[("authorization", "Bearer ")]);
        assert_eq!(bearer_token(&map), None);
        let map = headers(&[("authorization", "Basic abc")]);
        assert_eq!(bearer_token(&map), None);
    }

    #[test]
    fn origin_with_the_same_host_passes() {
        let map = headers(&[
            ("origin", "http://100.64.0.5:7878"),
            ("host", "100.64.0.5:7878"),
        ]);
        assert!(origin_matches_host(&map));
    }

    #[test]
    fn origin_with_another_host_fails() {
        let map = headers(&[
            ("origin", "http://evil.example"),
            ("host", "100.64.0.5:7878"),
        ]);
        assert!(!origin_matches_host(&map));
    }

    #[test]
    fn no_origin_header_passes() {
        let map = headers(&[("host", "100.64.0.5:7878")]);
        assert!(origin_matches_host(&map));
    }

    #[test]
    fn origin_host_drops_the_scheme_and_slash() {
        assert_eq!(origin_host("http://a.example:80/"), Some("a.example:80"));
        assert_eq!(origin_host("https://a.example"), Some("a.example"));
        assert_eq!(origin_host("null"), None);
        assert_eq!(origin_host("http://"), None);
    }
}
