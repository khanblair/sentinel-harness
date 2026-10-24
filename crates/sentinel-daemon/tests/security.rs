//! The route access matrix. Every route answers the right way to a caller with no token, a paired
//! phone, and the desktop. The rows live in `security/table.rs`. A test reads the route source, so
//! a new `.route(` with no row fails the build.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

mod common;
#[path = "security/coverage.rs"]
mod coverage;
#[path = "security/harness.rs"]
mod harness;
#[path = "security/table.rs"]
mod table;

use reqwest::StatusCode;
use serde_json::Value;

use harness::{Reply, Target, Who, send, socket_without_token};
use table::{Access, ROUTES, Route};

const EVIL_ORIGIN: &str = "http://evil.example";
const NEEDS_COMPUTER: &str = "This needs the computer. Do it there.";
const ORIGIN_REFUSAL: &str = "another site";

#[tokio::test]
async fn a_request_without_a_token_is_refused_on_every_guarded_route() {
    let shared = Target::start().await;
    for route in ROUTES {
        let reply = send(&shared, route, Who::Nobody, None).await;
        match route.access {
            Access::Public => assert_not_refused(route, &reply),
            Access::Any | Access::Computer => {
                assert_eq!(
                    reply.status,
                    StatusCode::UNAUTHORIZED,
                    "{}",
                    describe(route, &reply)
                );
                assert_error_only(route, &reply);
            }
            // The socket test below covers `/ws`.
            Access::Socket => {}
        }
    }
}

#[tokio::test]
async fn the_socket_refuses_a_first_frame_with_no_token() {
    let shared = Target::start().await;
    let text = socket_without_token(&shared).await;
    assert!(text.contains("Sign in first."), "{text}");
}

#[tokio::test]
async fn a_paired_phone_is_refused_on_computer_routes_and_passes_any_route() {
    let shared = Target::start().await;
    for route in ROUTES {
        // A changing row gets its own engine, so one row cannot break the next.
        let own;
        let target = if route.is_read() {
            &shared
        } else {
            own = Target::start().await;
            &own
        };
        match route.access {
            Access::Computer => {
                let reply = send(target, route, Who::Phone, None).await;
                assert_eq!(
                    reply.status,
                    StatusCode::FORBIDDEN,
                    "{}",
                    describe(route, &reply)
                );
                assert!(
                    reply.text.contains(NEEDS_COMPUTER),
                    "{}",
                    describe(route, &reply)
                );
                assert_error_only(route, &reply);
            }
            Access::Any => {
                let reply = send(target, route, Who::Phone, None).await;
                assert_not_refused(route, &reply);
            }
            Access::Public | Access::Socket => {}
        }
    }
}

#[tokio::test]
async fn the_desktop_token_passes_the_guard_on_every_route() {
    let shared = Target::start().await;
    for route in ROUTES.iter().filter(|row| row.access != Access::Socket) {
        let own;
        let target = if route.is_read() {
            &shared
        } else {
            own = Target::start().await;
            &own
        };
        let reply = send(target, route, Who::Desktop, None).await;
        assert_not_refused(route, &reply);
    }
}

#[tokio::test]
async fn a_changing_request_from_another_origin_is_refused_before_the_handler() {
    let shared = Target::start().await;
    for route in ROUTES.iter().filter(|row| row.changes()) {
        let reply = send(&shared, route, Who::Desktop, Some(EVIL_ORIGIN)).await;
        assert_eq!(
            reply.status,
            StatusCode::FORBIDDEN,
            "{}",
            describe(route, &reply)
        );
        assert!(
            reply.text.contains(ORIGIN_REFUSAL),
            "{}",
            describe(route, &reply)
        );
    }
}

#[test]
fn every_route_call_in_the_source_has_a_row_in_routes() {
    let (calls, sites) = coverage::read();
    assert!(
        calls > 0,
        "found no .route( calls, so the source path is wrong"
    );
    assert_eq!(sites.len(), calls, "a .route( call has no path literal");
    assert!(
        sites.iter().all(|site| !site.methods.is_empty()),
        "a .route( call names no method: {sites:?}"
    );
    let missing = coverage::missing_rows(&sites, ROUTES);
    assert!(
        missing.is_empty(),
        "these routes have no row in ROUTES:\n{}",
        missing.join("\n")
    );
}

#[test]
fn every_row_in_routes_matches_a_route_call() {
    let (_, sites) = coverage::read();
    let stale = coverage::stale_rows(&sites, ROUTES);
    assert!(
        stale.is_empty(),
        "these rows match no route call:\n{}",
        stale.join("\n")
    );
}

#[tokio::test]
async fn refusals_never_carry_a_token_a_bearer_word_or_a_file_path() {
    let shared = Target::start().await;
    for route in ROUTES.iter().filter(|row| row.access != Access::Socket) {
        let mut replies = vec![send(&shared, route, Who::Nobody, None).await];
        if route.access == Access::Computer {
            replies.push(send(&shared, route, Who::Phone, None).await);
        }
        replies.push(send(&shared, route, Who::Desktop, Some(EVIL_ORIGIN)).await);
        for reply in replies
            .iter()
            .filter(|reply| unauthorized_or_forbidden(reply.status))
        {
            assert_clean(&shared, route, reply);
        }
    }
}

/// The engine also answers 403 for its own refusals, such as `pause` with no run live. Only the
/// guard's 403 means the caller was turned away, so the body decides which kind it is.
fn refused(reply: &Reply) -> bool {
    reply.status == StatusCode::UNAUTHORIZED
        || (reply.status == StatusCode::FORBIDDEN
            && (reply.text.contains(NEEDS_COMPUTER) || reply.text.contains(ORIGIN_REFUSAL)))
}

fn describe(route: &Route, reply: &Reply) -> String {
    format!(
        "{} {} -> {}: {}",
        route.method, route.path, reply.status, reply.text
    )
}

fn unauthorized_or_forbidden(status: StatusCode) -> bool {
    status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN
}

fn assert_not_refused(route: &Route, reply: &Reply) {
    assert!(!refused(reply), "{}", describe(route, reply));
}

/// A refusal body is `{"error": "..."}` and nothing else, so it holds no state.
fn assert_error_only(route: &Route, reply: &Reply) {
    let body: Value = serde_json::from_str(&reply.text).unwrap();
    let keys: Vec<&str> = body
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, vec!["error"], "{}", describe(route, reply));
}

fn assert_clean(target: &Target, route: &Route, reply: &Reply) {
    let home = target.home.display().to_string();
    let forbidden = [
        target.desktop.as_str(),
        target.phone.as_str(),
        home.as_str(),
        "Bearer",
        "/Users/",
        "/var/",
        "/private/",
        "/tmp/",
        "/home/",
    ];
    for secret in forbidden {
        assert!(
            !reply.text.contains(secret),
            "refusal leaks {secret:?}: {}",
            describe(route, reply)
        );
    }
}
