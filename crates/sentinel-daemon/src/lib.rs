//! The Sentinel daemon: the engine that runs a backlog, and the services around it.

pub mod access;
mod agents;
mod ask;
mod attempt;
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
mod plan_phase;
mod prompt;
mod report_build;
mod reviewer;
mod run;
mod run_ctx;
pub mod secrets;
pub mod tailscale;
mod verify_phase;
mod wave;
