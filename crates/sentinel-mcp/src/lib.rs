//! Sentinel's MCP server over stdio: file tools that check reservations, and Brain tools.

pub mod brain_tools;
pub mod files;
pub mod protocol;
pub mod server;
pub mod skill_tools;

pub use server::{ServerConfig, serve};
