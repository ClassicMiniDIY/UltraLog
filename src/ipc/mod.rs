//! Inter-process communication module for UltraLog MCP integration
//!
//! This module defines the protocol for communication between the UltraLog GUI
//! and the MCP server, allowing Claude to control the running application.

pub mod commands;
pub mod handler;
pub mod server;

pub use commands::{ChannelInfo, ChannelStats, FileInfo, IpcCommand, IpcResponse};
pub use server::IpcServer;

/// Default port for the IPC server
pub const DEFAULT_IPC_PORT: u16 = 52384;
