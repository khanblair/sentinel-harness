//! Phone pairing (task P7-02).
//!
//! One one-time code at a time. The phone submits the code, the PC approves the device in
//! `sentinel-store`, and the phone polls for its token, which it receives exactly once.
//! The daemon keeps only a hash of the token (`docs/mobile.md`, Connecting and Pairing).
//!
//! Time always arrives as a parameter. The manager keeps the last time it saw, so a poll
//! without a clock still knows whether a pending request ran out of time.

use std::collections::BTreeMap;

use qrcode::QrCode;
use qrcode::render::svg;
use sentinel_core::api::{PairResult, PairStart, PairSubmit, TailscaleStatus};
use sentinel_core::ids::DeviceId;
use sentinel_core::state::TailscaleState;
use sentinel_store::{Device, DeviceKind, Store, StoreError};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::hashing::{hash_token, new_token};

const CODE_LIFETIME_SECONDS: u64 = 5 * 60;
const WRONG_SUBMISSION_LIMIT: u32 = 5;
const WRONG_SUBMISSION_BLOCK_SECONDS: u64 = 60;
const PENDING_LIFETIME_SECONDS: u64 = 5 * 60;

/// Errors pairing can hit.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PairError {
    #[error("that is not the active code")]
    BadCode,
    #[error("the code has expired")]
    Expired,
    #[error("the code was already submitted")]
    AlreadyUsed,
    #[error("there is no such pending pairing")]
    UnknownPending,
    #[error("tailscale is not running")]
    NoTailscale,
    /// The store write for a new device failed. Pairing needs the row, so it surfaces.
    #[error("store error: {0}")]
    Store(String),
}

impl From<StoreError> for PairError {
    fn from(e: StoreError) -> Self {
        Self::Store(e.to_string())
    }
}

/// One pairing a device asked for and the PC has not decided on yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingPair {
    pub id: PendingId,
    pub device_name: String,
    pub requested_at: OffsetDateTime,
}

/// The id of one pending pairing, from `submit` to `approve`, `deny`, or expiry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PendingId(String);

impl PendingId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What the phone sees when it polls a pending pairing. Each end state is offered once,
/// then the id is unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairOutcome {
    Waiting,
    Approved(PairResult),
    Denied,
    Expired,
    Unknown,
}

/// Pairing state.
#[derive(Debug, Default)]
pub struct PairingManager {
    session: Option<CodeSession>,
    requests: BTreeMap<PendingId, PendingRequest>,
    latest_seen_time: Option<OffsetDateTime>,
}

impl PairingManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// A new one-time code for one link. It replaces any open code.
    pub fn start(
        &mut self,
        base_url: &str,
        tailscale: TailscaleStatus,
        now: OffsetDateTime,
    ) -> PairStart {
        self.latest_seen_time = Some(now);
        let mut start = PairStart {
            tailscale,
            url: String::new(),
            code: String::new(),
            qr_svg: String::new(),
            expires_in_seconds: 0,
        };
        if start.tailscale.state != TailscaleState::Running {
            // Without an address the phone cannot reach the daemon at all.
            return start;
        }
        start.url = format!("{}/pair", base_url.trim_end_matches('/'));
        start.expires_in_seconds = u32::try_from(CODE_LIFETIME_SECONDS).unwrap_or(0);
        start.code = display_code(&new_code());
        start.qr_svg = render_qr_svg(&start.url);
        self.session = Some(CodeSession {
            code: normalize_code(&start.code),
            expires_at: now + duration_of(CODE_LIFETIME_SECONDS),
            wrong_submissions: 0,
            blocked_until: None,
            used: false,
        });
        start
    }

    /// The phone sends the code. On success the code is spent and the PC must approve.
    pub fn submit(
        &mut self,
        submit: &PairSubmit,
        now: OffsetDateTime,
    ) -> Result<PendingId, PairError> {
        self.latest_seen_time = Some(now);
        let Some(session) = self.session.as_mut() else {
            return Err(PairError::BadCode);
        };
        if block_rejects(session, now) {
            return Err(PairError::BadCode);
        }
        if session.expires_at < now {
            return Err(PairError::Expired);
        }
        if normalize_code(&submit.code) != session.code {
            count_wrong_submission(session, now);
            return Err(PairError::BadCode);
        }
        if session.used {
            return Err(PairError::AlreadyUsed);
        }
        session.used = true;
        let id = PendingId::new(Uuid::new_v4().to_string());
        self.requests.insert(
            id.clone(),
            PendingRequest {
                device_name: submit.device_name.clone(),
                requested_at: now,
                expires_at: now + duration_of(PENDING_LIFETIME_SECONDS),
                state: RequestState::Waiting,
            },
        );
        Ok(id)
    }

    /// The requests the PC can still answer, oldest first.
    pub fn pending(&self) -> Vec<PendingPair> {
        self.requests
            .iter()
            .filter(|(_, request)| request.state == RequestState::Waiting)
            .map(|(id, request)| PendingPair {
                id: id.clone(),
                device_name: request.device_name.clone(),
                requested_at: request.requested_at,
            })
            .collect()
    }

    /// The PC approves. The device row keeps the token hash, and the token goes out once.
    pub fn approve(
        &mut self,
        id: &PendingId,
        store: &Store,
        now: OffsetDateTime,
    ) -> Result<PairResult, PairError> {
        self.latest_seen_time = Some(now);
        let request = match self.requests.remove(id) {
            Some(request) if request.state == RequestState::Waiting => request,
            // A decision already went out for this pending id.
            Some(request) => {
                self.requests.insert(id.clone(), request);
                return Err(PairError::UnknownPending);
            }
            None => return Err(PairError::UnknownPending),
        };
        if request.expires_at < now {
            return Err(PairError::Expired);
        }
        let token = new_token();
        let device = Device {
            id: DeviceId::new(Uuid::new_v4().to_string()),
            name: request.device_name.clone(),
            kind: DeviceKind::Phone,
            token_hash: hash_token(&token),
            created_at: now,
            revoked_at: None,
        };
        store.insert_device(&device)?;
        let approved = PairResult {
            device: device.id,
            token,
        };
        self.requests.insert(
            id.clone(),
            PendingRequest {
                state: RequestState::Approved(approved.clone()),
                ..request
            },
        );
        Ok(approved)
    }

    /// The PC rejects the request.
    pub fn deny(&mut self, id: &PendingId) -> Result<(), PairError> {
        let request = match self.requests.remove(id) {
            Some(request) if request.state == RequestState::Waiting => request,
            Some(request) => {
                self.requests.insert(id.clone(), request);
                return Err(PairError::UnknownPending);
            }
            None => return Err(PairError::UnknownPending),
        };
        self.requests.insert(
            id.clone(),
            PendingRequest {
                state: RequestState::Denied,
                ..request
            },
        );
        Ok(())
    }

    /// The phone polls this after `submit`. A delivered end state is gone on the next poll.
    pub fn outcome(&mut self, id: &PendingId) -> PairOutcome {
        match self.requests.remove(id) {
            None => PairOutcome::Unknown,
            Some(request) => match request.state {
                RequestState::Waiting => self.wait_or_expire(id, request),
                RequestState::Denied => PairOutcome::Denied,
                RequestState::Approved(approved) => PairOutcome::Approved(approved),
            },
        }
    }

    fn wait_or_expire(&mut self, id: &PendingId, request: PendingRequest) -> PairOutcome {
        let clock_known = self
            .latest_seen_time
            .is_some_and(|seen| request.expires_at < seen);
        if clock_known {
            // Already removed, so the phone reads Expired once and then Unknown.
            return PairOutcome::Expired;
        }
        self.requests.insert(id.clone(), request);
        PairOutcome::Waiting
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingRequest {
    device_name: String,
    requested_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    state: RequestState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RequestState {
    Waiting,
    Approved(PairResult),
    Denied,
}

#[derive(Debug)]
struct CodeSession {
    code: String,
    expires_at: OffsetDateTime,
    wrong_submissions: u32,
    blocked_until: Option<OffsetDateTime>,
    used: bool,
}

fn block_rejects(session: &mut CodeSession, now: OffsetDateTime) -> bool {
    match session.blocked_until {
        Some(blocked_until) if now < blocked_until => true,
        Some(_) => {
            // The block is over: the next submissions get five fresh tries.
            session.blocked_until = None;
            session.wrong_submissions = 0;
            false
        }
        None => false,
    }
}

fn count_wrong_submission(session: &mut CodeSession, now: OffsetDateTime) {
    session.wrong_submissions += 1;
    if session.wrong_submissions >= WRONG_SUBMISSION_LIMIT {
        session.blocked_until = Some(now + duration_of(WRONG_SUBMISSION_BLOCK_SECONDS));
    }
}

/// Codes are uppercase and dash free inside the daemon. `ABcd-ef12` matches `ABCDEF12`.
fn normalize_code(given: &str) -> String {
    given
        .chars()
        .filter(|candidate| *candidate != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// The code the PC shows: `XXXX-XXXX`.
fn display_code(code: &str) -> String {
    format!("{}-{}", &code[..4], &code[4..])
}

fn new_code() -> String {
    // The alphabet has 32 characters, and one masked byte picks one of them evenly.
    // No `rand` crate is allowed here, so UUID bytes are the random source.
    Uuid::new_v4()
        .as_bytes()
        .iter()
        .take(CODE_LENGTH)
        .map(|byte| CODE_ALPHABET[(*byte & 0x1F) as usize] as char)
        .collect()
}

fn render_qr_svg(url: &str) -> String {
    let Ok(code) = QrCode::new(url.as_bytes()) else {
        return String::new();
    };
    code.render()
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .min_dimensions(200, 200)
        .build()
}

fn duration_of(seconds: u64) -> time::Duration {
    time::Duration::seconds(seconds.cast_signed())
}

const CODE_ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const CODE_LENGTH: usize = 8;

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(seconds).unwrap()
    }

    fn running() -> TailscaleStatus {
        TailscaleStatus {
            state: TailscaleState::Running,
            address: Some("100.64.0.9".to_owned()),
            machine_name: Some("work-laptop".to_owned()),
        }
    }

    fn submit_for(code: &str, name: &str) -> PairSubmit {
        PairSubmit {
            code: code.to_owned(),
            device_name: name.to_owned(),
        }
    }

    #[test]
    fn start_shows_a_qr_and_a_dashed_code_when_tailscale_runs() {
        let mut manager = PairingManager::new();
        let start = manager.start("http://100.64.0.9:8080", running(), at(0));
        assert_eq!(start.url, "http://100.64.0.9:8080/pair");
        assert_eq!(start.code.chars().count(), CODE_LENGTH + 1);
        assert_eq!(start.code.chars().nth(4), Some('-'));
        if !start.qr_svg.contains("<svg") {
            eprintln!("QR SVG was: '{}'", start.qr_svg);
        }
        assert!(start.qr_svg.contains("<svg"));
        assert_eq!(start.expires_in_seconds, 300);
    }

    #[test]
    fn start_without_tailscale_shows_install_steps_instead() {
        let signed_out_status = TailscaleStatus {
            state: TailscaleState::SignedOut,
            address: None,
            machine_name: None,
        };
        let start = PairingManager::new().start("http://100.64.0.9", signed_out_status, at(0));
        assert_eq!(start.url, "");
        assert_eq!(start.code, "");
        assert_eq!(start.qr_svg, "");
    }

    #[test]
    fn the_code_never_rides_in_the_link() {
        let start = PairingManager::new().start("http://100.64.0.9", running(), at(0));
        let shown_code = start.code.replace('-', "");
        assert!(!start.url.contains(&shown_code));
        assert!(!start.url.contains("pair="));
    }

    #[test]
    fn start_replaces_the_open_code() {
        let mut manager = PairingManager::new();
        let first = manager.start("http://host", running(), at(0));
        let second = manager.start("http://host", running(), at(0));
        assert_ne!(first.code, second.code);
        let submit = submit_for(&first.code, "phone");
        assert_eq!(manager.submit(&submit, at(10)), Err(PairError::BadCode));
    }

    #[test]
    fn the_happy_path_stores_a_phone_device_and_hands_out_the_token_once() {
        let store = Store::open_in_memory().unwrap();
        let mut manager = PairingManager::new();
        manager.start("http://100.64.0.9", running(), at(0));
        let code = "12345678".to_owned();
        manager.session.as_mut().unwrap().code = code.clone();
        let id = manager
            .submit(&submit_for(&code.to_ascii_lowercase(), "my phone"), at(30))
            .unwrap();
        let approved = manager.approve(&id, &store, at(40)).unwrap();
        let device = store
            .device_by_token_hash(&hash_token(&approved.token))
            .unwrap()
            .unwrap();
        assert_eq!(device.name, "my phone");
        assert_eq!(device.kind, DeviceKind::Phone);
        assert_eq!(manager.outcome(&id), PairOutcome::Approved(approved));
        assert_eq!(manager.outcome(&id), PairOutcome::Unknown);
    }

    #[test]
    fn approve_adds_exactly_one_pending_then_nothing() {
        let store = Store::open_in_memory().unwrap();
        let mut manager = PairingManager::new();
        manager.start("http://host", running(), at(0));
        manager.session.as_mut().unwrap().code = "ABCDEFGH".to_owned();
        let id = manager
            .submit(&submit_for("ABCDEFGH", "phone"), at(10))
            .unwrap();
        assert_eq!(manager.pending().len(), 1);
        manager.approve(&id, &store, at(20)).unwrap();
        assert_eq!(manager.pending(), []);
    }

    #[test]
    fn a_wrong_code_is_rejected_and_the_right_one_still_works() {
        let mut manager = PairingManager::new();
        manager.start("http://host", running(), at(0));
        manager.session.as_mut().unwrap().code = "ABCDEFGH".to_owned();
        assert_eq!(
            manager.submit(&submit_for("XXXXXXXX", "phone"), at(10)),
            Err(PairError::BadCode)
        );
        let right = manager.submit(&submit_for("abcdefgh", "phone"), at(11));
        assert!(right.is_ok());
    }

    #[test]
    fn a_submitted_code_cannot_be_submitted_again() {
        let store = Store::open_in_memory().unwrap();
        let mut manager = PairingManager::new();
        manager.start("http://host", running(), at(0));
        manager.session.as_mut().unwrap().code = "ABCDEFGH".to_owned();
        manager
            .submit(&submit_for("ABCDEFGH", "phone"), at(10))
            .unwrap();
        assert_eq!(
            manager.submit(&submit_for("ABCDEFGH", "phone too"), at(11)),
            Err(PairError::AlreadyUsed)
        );
        // Approval does not hand the spent code out again either.
        let pending = manager.pending().remove(0);
        manager.approve(&pending.id, &store, at(12)).unwrap();
        assert_eq!(
            manager.submit(&submit_for("ABCDEFGH", "phone too"), at(13)),
            Err(PairError::AlreadyUsed)
        );
    }

    #[test]
    fn a_code_stops_working_after_five_minutes() {
        let mut manager = PairingManager::new();
        manager.start("http://host", running(), at(0));
        manager.session.as_mut().unwrap().code = "ABCDEFGH".to_owned();
        let ahead =
            at(0) + time::Duration::seconds(i64::try_from(CODE_LIFETIME_SECONDS).unwrap() + 1);
        assert_eq!(
            manager.submit(&submit_for("ABCDEFGH", "phone"), ahead),
            Err(PairError::Expired)
        );
    }

    #[test]
    fn five_wrong_codes_block_the_sixth_even_when_it_is_right() {
        let mut manager = PairingManager::new();
        manager.start("http://host", running(), at(0));
        manager.session.as_mut().unwrap().code = "ABCDEFGH".to_owned();
        for attempt in 0..WRONG_SUBMISSION_LIMIT {
            let submit = submit_for("XXXXXXXX", "phone");
            assert_eq!(
                manager.submit(&submit, at(10 + i64::from(attempt))),
                Err(PairError::BadCode)
            );
        }
        assert_eq!(
            manager.submit(&submit_for("ABCDEFGH", "phone"), at(50)),
            Err(PairError::BadCode)
        );
        let unblocked = at(14)
            + time::Duration::seconds(i64::try_from(WRONG_SUBMISSION_BLOCK_SECONDS).unwrap() + 1);
        let right = manager.submit(&submit_for("ABCDEFGH", "phone"), unblocked);
        assert!(right.is_ok());
    }

    #[test]
    fn deny_reaches_the_phone_once() {
        let mut manager = PairingManager::new();
        manager.start("http://host", running(), at(0));
        manager.session.as_mut().unwrap().code = "ABCDEFGH".to_owned();
        let id = manager
            .submit(&submit_for("ABCDEFGH", "phone"), at(10))
            .unwrap();
        assert_eq!(manager.deny(&id), Ok(()));
        assert_eq!(manager.outcome(&id), PairOutcome::Denied);
        assert_eq!(manager.outcome(&id), PairOutcome::Unknown);
        assert_eq!(manager.deny(&id), Err(PairError::UnknownPending));
    }

    #[test]
    fn approvals_and_denials_only_answer_known_pendings() {
        let store = Store::open_in_memory().unwrap();
        let mut manager = PairingManager::new();
        assert_eq!(
            manager.approve(&PendingId::new("gone"), &store, at(0)),
            Err(PairError::UnknownPending)
        );
        assert_eq!(
            manager.deny(&PendingId::new("gone")),
            Err(PairError::UnknownPending)
        );
        assert_eq!(
            manager.outcome(&PendingId::new("gone")),
            PairOutcome::Unknown
        );
    }

    #[test]
    fn the_phone_reads_expired_once_the_window_has_passed() {
        let mut manager = PairingManager::new();
        manager.start("http://host", running(), at(0));
        manager.session.as_mut().unwrap().code = "ABCDEFGH".to_owned();
        let _id = manager
            .submit(&submit_for("ABCDEFGH", "phone"), at(10))
            .unwrap();
        // A new `start` is the call that moves the clock past the pending lifetime.
        manager.start(
            "http://host",
            running(),
            at(0) + time::Duration::seconds(600),
        );
        let id = manager
            .pending()
            .pop()
            .expect("the pending request is still listed")
            .id;
        assert_eq!(manager.outcome(&id), PairOutcome::Expired);
        assert_eq!(manager.outcome(&id), PairOutcome::Unknown);
    }
}
