//! Every route of the daemon and who may call it. Add a row for each new `.route(` line; the
//! coverage test fails until you do.

use reqwest::Method;

/// Who may call a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// No token needed.
    Public,
    /// Any valid device token, the phone included.
    Any,
    /// The desktop token only. A phone gets 403.
    Computer,
    /// The WebSocket. It checks its token in the first frame, so only the refusal is tested.
    Socket,
}

/// One route, with a fixed fake id where the path has a parameter.
#[derive(Debug, Clone)]
pub struct Route {
    pub method: Method,
    pub path: &'static str,
    pub access: Access,
    /// A minimal JSON body. `{repo}` stands for the temp repository path.
    pub body: Option<&'static str>,
}

impl Route {
    const fn new(method: Method, path: &'static str, access: Access) -> Self {
        Self {
            method,
            path,
            access,
            body: None,
        }
    }

    const fn json(mut self, body: &'static str) -> Self {
        self.body = Some(body);
        self
    }

    pub const fn get(path: &'static str, access: Access) -> Self {
        Self::new(Method::GET, path, access)
    }

    pub const fn post(path: &'static str, access: Access) -> Self {
        Self::new(Method::POST, path, access)
    }

    pub const fn put(path: &'static str, access: Access) -> Self {
        Self::new(Method::PUT, path, access)
    }

    pub const fn delete(path: &'static str, access: Access) -> Self {
        Self::new(Method::DELETE, path, access)
    }

    /// Reads change nothing, so their rows can share one engine.
    pub fn is_read(&self) -> bool {
        self.method == Method::GET
    }

    /// The methods the Origin rule checks.
    pub fn changes(&self) -> bool {
        [Method::POST, Method::PUT, Method::DELETE, Method::PATCH].contains(&self.method)
    }
}

pub const ROUTES: &[Route] = &[
    Route::get("/api/health", Access::Public),
    Route::post("/api/pair", Access::Public)
        .json(r#"{"code":"000000","device_name":"Test phone"}"#),
    Route::get("/api/pair/nope/outcome", Access::Public),
    Route::get("/ws", Access::Socket),
    Route::post("/api/pair/start", Access::Computer).json("{}"),
    Route::get("/api/pair/pending", Access::Computer),
    Route::post("/api/pair/nope/approve", Access::Computer).json("{}"),
    Route::post("/api/pair/nope/deny", Access::Computer).json("{}"),
    Route::get("/api/devices", Access::Any),
    Route::delete("/api/devices/nope", Access::Any),
    Route::get("/api/tailscale/status", Access::Any),
    Route::post("/api/tailscale/signin", Access::Computer).json("{}"),
    Route::get("/api/host", Access::Any),
    Route::get("/api/whoami", Access::Any),
    Route::post("/api/host/check", Access::Computer),
    Route::post("/api/fs/pick", Access::Computer),
    Route::get("/api/fs/list", Access::Any),
    Route::get("/api/telegram", Access::Any),
    Route::put("/api/telegram", Access::Computer)
        .json(r#"{"levels":[],"events":[],"detail":"summary","quiet_hours":null}"#),
    Route::post("/api/telegram/token", Access::Computer).json(r#"{"token":""}"#),
    Route::post("/api/telegram/connect", Access::Computer).json(r#"{"user_id":1}"#),
    Route::post("/api/telegram/test", Access::Computer),
    Route::delete("/api/telegram/chats/nope", Access::Computer),
    Route::get("/api/dry-run", Access::Any),
    Route::post("/api/dry-run", Access::Computer),
    Route::post("/api/run", Access::Any).json("{}"),
    Route::post("/api/pause", Access::Any),
    Route::post("/api/resume", Access::Any),
    Route::post("/api/stop", Access::Any),
    Route::post("/api/tasks/nope/kill", Access::Any),
    Route::post("/api/agents/nope/kill", Access::Any),
    Route::post("/api/cap/lower", Access::Any).json(r#"{"cap":"1.00"}"#),
    Route::post("/api/decisions/nope", Access::Any).json(r#"{"answer":"skip_both"}"#),
    Route::post("/api/approvals/nope", Access::Any).json(r#"{"allow":false}"#),
    Route::post("/api/message", Access::Any).json(r#"{"text":"hello"}"#),
    Route::post("/api/brain/mode", Access::Any).json(r#"{"mode":"off"}"#),
    Route::get("/api/state", Access::Any),
    Route::get("/api/context", Access::Any),
    Route::get("/api/changes", Access::Any),
    Route::get("/api/changes/diff", Access::Any),
    Route::get("/api/backlog/tasks", Access::Any),
    Route::get("/api/events", Access::Any),
    Route::get("/api/tasks/nope/output", Access::Any),
    Route::get("/api/report", Access::Any),
    Route::get("/api/approvals", Access::Any),
    Route::get("/api/permissions", Access::Any),
    Route::put("/api/permissions", Access::Any).json("{}"),
    Route::put("/api/routing", Access::Computer)
        .json(r#"{"routing":{"cheap":null,"mid":null,"smart":null}}"#),
    Route::get("/api/tools", Access::Any),
    Route::post("/api/tools/scan", Access::Computer),
    Route::put("/api/tools/nope", Access::Computer).json(r#"{"enabled":false}"#),
    Route::post("/api/doctor", Access::Computer),
    Route::get("/api/connections", Access::Any),
    Route::post("/api/chats/nope/attachments", Access::Any)
        .json(r#"{"kind":"note","text":"hello"}"#),
    Route::get("/api/chats", Access::Any),
    Route::get("/api/chats/nope/events", Access::Any),
    Route::post("/api/chats", Access::Any).json("{}"),
    Route::delete("/api/chats/nope", Access::Computer),
    Route::put("/api/chats/nope", Access::Any).json("{}"),
    Route::get("/api/chats/nope/attachments", Access::Any),
    Route::delete("/api/chats/nope/attachments/nope", Access::Any),
    // The Brain writes below take `Caller` and refuse a phone inside the handler with
    // `require_computer`. They are Computer here, because that is what a phone gets.
    Route::get("/api/brain/notes", Access::Any),
    Route::post("/api/brain/notes", Access::Computer)
        .json(r#"{"note_type":"fix","title":"Title","body":"Body"}"#),
    Route::get("/api/brain/notes/nope", Access::Any),
    Route::put("/api/brain/notes/nope", Access::Computer).json(r#"{"body":"Body","version":"1"}"#),
    Route::post("/api/brain/notes/nope/pin", Access::Computer).json(r#"{"pinned":true}"#),
    Route::get("/api/brain/graph", Access::Any),
    Route::post("/api/brain/lessons/nope/review", Access::Any).json(r#"{"action":"approve"}"#),
    Route::get("/api/brain/proposals", Access::Any),
    Route::post("/api/brain/proposals/nope", Access::Any).json(r#"{"accept":false}"#),
    Route::get("/api/brain/care", Access::Any),
    Route::put("/api/brain/care", Access::Computer).json(
        r#"{"frequency":{"every":"off"},"budget":"1.00","auto_apply":false,"last_run":null}"#,
    ),
    Route::post("/api/brain/care/run", Access::Computer),
    Route::post("/api/brain/seed", Access::Computer),
    Route::get("/api/skills", Access::Any),
    Route::post("/api/skills", Access::Computer)
        .json(r#"{"name":"house-style","description":"Rules for this repository.","body":"Be short.","scope":"project"}"#),
    Route::get("/api/skills/found", Access::Computer),
    Route::post("/api/skills/import", Access::Computer)
        .json(r#"{"path":"/nowhere","scope":"project"}"#),
    Route::get("/api/skills/nope", Access::Any),
    Route::put("/api/skills/nope", Access::Any).json(r#"{"mode":"off"}"#),
    Route::delete("/api/skills/nope", Access::Computer),
    Route::post("/api/quit", Access::Computer),
    Route::get("/api/routines", Access::Any),
    Route::post("/api/routines", Access::Computer).json(
        r#"{"name":"Standup","kind":"reminder","minute_of_day":540,"days":31,"text":"Plan"}"#,
    ),
    Route::put("/api/routines/nope", Access::Any).json(r#"{"enabled":false}"#),
    Route::delete("/api/routines/nope", Access::Computer),
    Route::post("/api/routines/nope/run", Access::Any),
    Route::get("/api/briefs", Access::Any),
    Route::put("/api/config/cap", Access::Computer).json(r#"{"cap":"5.00"}"#),
    Route::put("/api/secrets/openrouter", Access::Computer).json(r#"{"key":"test-key"}"#),
    Route::post("/api/connections/nope/test", Access::Computer),
    Route::get("/api/files", Access::Any),
    Route::get("/api/repo/tree", Access::Any),
    Route::post("/api/repo/check", Access::Any).json(r#"{"path":"{repo}"}"#),
    Route::post("/api/repo/starter-backlog", Access::Computer).json(r#"{"path":"{repo}"}"#),
    Route::put("/api/repo", Access::Any).json(r#"{"path":"{repo}"}"#),
    Route::get("/api/onboarding", Access::Any),
    Route::post("/api/onboarding/1", Access::Computer),
    Route::post("/api/onboarding/skip/1", Access::Computer),
    Route::post("/api/onboarding/complete", Access::Computer),
    Route::post("/api/onboarding/reset", Access::Computer),
];
