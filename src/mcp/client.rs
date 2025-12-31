//! TCP client for communicating with the UltraLog GUI's IPC server

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

use crate::ipc::commands::{IpcCommand, IpcResponse};
use crate::ipc::DEFAULT_IPC_PORT;

/// Client for communicating with the UltraLog GUI
///
/// Each command creates a new TCP connection to avoid stale connection issues.
/// The IPC server handles one command per connection.
pub struct GuiClient {
    port: u16,
}

impl GuiClient {
    /// Create a new GUI client
    pub fn new() -> Self {
        Self {
            port: DEFAULT_IPC_PORT,
        }
    }

    /// Create a new GUI client with a specific port
    pub fn with_port(port: u16) -> Self {
        Self { port }
    }

    /// Connect to the GUI
    fn connect(&self) -> Result<TcpStream, String> {
        let addr = format!("127.0.0.1:{}", self.port);
        tracing::debug!("MCP client connecting to IPC server at {}", addr);

        // Use connect_timeout to avoid blocking indefinitely
        let socket_addr: std::net::SocketAddr = addr
            .parse()
            .map_err(|e| format!("Invalid address: {}", e))?;
        let stream = TcpStream::connect_timeout(&socket_addr, Duration::from_secs(5))
            .map_err(|e| format!("Failed to connect to UltraLog GUI at {}: {}", addr, e))?;

        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| format!("Failed to set read timeout: {}", e))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| format!("Failed to set write timeout: {}", e))?;

        tracing::debug!("MCP client connected to IPC server");
        Ok(stream)
    }

    /// Send a command to the GUI and get a response
    pub fn send_command(&self, command: IpcCommand) -> Result<IpcResponse, String> {
        tracing::debug!("MCP client sending command: {:?}", command);

        // Create a new connection for each command
        let mut stream = self.connect()?;

        // Serialize and send the command
        let json = serde_json::to_string(&command)
            .map_err(|e| format!("Failed to serialize command: {}", e))?;

        tracing::debug!("MCP client writing to IPC: {}", json);
        writeln!(stream, "{}", json)
            .map_err(|e| format!("Failed to send command: {}", e))?;
        stream.flush()
            .map_err(|e| format!("Failed to flush: {}", e))?;

        tracing::debug!("MCP client waiting for response...");

        // Read the response
        let mut reader = BufReader::new(&stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line)
            .map_err(|e| format!("Failed to read response: {}", e))?;

        tracing::debug!("MCP client received response: {}", response_line.trim());

        // Connection will be closed when stream is dropped

        // Parse the response
        serde_json::from_str(&response_line)
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Check if the GUI is running and responsive
    pub fn ping(&self) -> bool {
        matches!(self.send_command(IpcCommand::Ping), Ok(IpcResponse::Ok(_)))
    }
}

impl Default for GuiClient {
    fn default() -> Self {
        Self::new()
    }
}
