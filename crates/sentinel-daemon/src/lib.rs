//! The Sentinel daemon: the engine that runs a backlog, and the services around it.

pub mod access;
pub mod cmd;
pub mod config;
pub mod connections;
pub mod controls;
pub mod doctor;
pub mod engine;
pub mod folders;
pub mod hashing;
pub mod live;
pub mod onboarding;
pub mod pairing;
pub mod secrets;
pub mod tailscale;
