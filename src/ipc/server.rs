//! TCP server for receiving IPC commands from the MCP server
//!
//! This server runs in a background thread and communicates with the GUI
//! via channels, allowing the main eframe event loop to process commands.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use super::commands::{IpcCommand, IpcResponse};
use super::DEFAULT_IPC_PORT;

/// IPC Server that listens for commands from the MCP server
pub struct IpcServer {
    /// Receiver for incoming commands (polled by the GUI)
    command_rx: Receiver<(IpcCommand, Sender<IpcResponse>)>,
    /// Port the server is listening on
    port: u16,
    /// Whether the server is running
    is_running: bool,
}

impl IpcServer {
    /// Start a new IPC server on the default port
    pub fn start() -> Result<Self, String> {
        Self::start_on_port(DEFAULT_IPC_PORT)
    }

    /// Start a new IPC server on a specific port
    pub fn start_on_port(port: u16) -> Result<Self, String> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
            .map_err(|e| format!("Failed to bind to port {}: {}", port, e))?;

        // Set non-blocking so we can check for shutdown
        listener
            .set_nonblocking(true)
            .map_err(|e| format!("Failed to set non-blocking: {}", e))?;

        let (command_tx, command_rx) = mpsc::channel();

        // Spawn the listener thread
        thread::spawn(move || {
            Self::listener_loop(listener, command_tx);
        });

        tracing::info!("IPC server started on port {}", port);

        Ok(Self {
            command_rx,
            port,
            is_running: true,
        })
    }

    /// Get the port the server is listening on
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Check if there's a pending command and return it
    pub fn poll_command(&self) -> Option<(IpcCommand, Sender<IpcResponse>)> {
        self.command_rx.try_recv().ok()
    }

    /// Check if the server is running
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// Main listener loop (runs in background thread)
    fn listener_loop(listener: TcpListener, command_tx: Sender<(IpcCommand, Sender<IpcResponse>)>) {
        loop {
            match listener.accept() {
                Ok((stream, addr)) => {
                    tracing::info!("MCP client connected from {}", addr);
                    let tx = command_tx.clone();
                    thread::spawn(move || {
                        Self::handle_connection(stream, tx);
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No connection available, sleep briefly
                    thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(e) => {
                    tracing::error!("Error accepting connection: {}", e);
                    thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }
    }

    /// Handle a single client connection
    fn handle_connection(
        mut stream: TcpStream,
        command_tx: Sender<(IpcCommand, Sender<IpcResponse>)>,
    ) {
        let peer_addr = stream.peer_addr().ok();

        // Set timeouts
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
        let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(10)));

        let reader = BufReader::new(stream.try_clone().expect("Failed to clone stream"));

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    tracing::debug!("Connection closed: {}", e);
                    break;
                }
            };

            if line.trim().is_empty() {
                continue;
            }

            // Parse the command
            let command: IpcCommand = match serde_json::from_str(&line) {
                Ok(cmd) => cmd,
                Err(e) => {
                    let response = IpcResponse::error(format!("Invalid command JSON: {}", e));
                    let _ = Self::send_response(&mut stream, &response);
                    continue;
                }
            };

            tracing::debug!("Received command: {:?}", command);

            // Create a channel for the response
            let (response_tx, response_rx) = mpsc::channel();

            // Send the command to the GUI thread
            if command_tx.send((command, response_tx)).is_err() {
                let response = IpcResponse::error("GUI is not responding");
                let _ = Self::send_response(&mut stream, &response);
                break;
            }

            // Wait for the response from the GUI
            let response = match response_rx.recv_timeout(std::time::Duration::from_secs(30)) {
                Ok(resp) => resp,
                Err(_) => IpcResponse::error("Timeout waiting for GUI response"),
            };

            if Self::send_response(&mut stream, &response).is_err() {
                break;
            }
        }

        if let Some(addr) = peer_addr {
            tracing::info!("MCP client disconnected: {}", addr);
        }
    }

    /// Send a response to the client
    fn send_response(stream: &mut TcpStream, response: &IpcResponse) -> Result<(), std::io::Error> {
        let json = serde_json::to_string(response).unwrap_or_else(|_| {
            r#"{"status":"Error","data":{"message":"Failed to serialize response"}}"#.to_string()
        });
        writeln!(stream, "{}", json)?;
        stream.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::commands::ResponseData;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    /// Find an available port for testing
    fn find_available_port() -> u16 {
        // Bind to port 0 to get an available port from the OS
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    }

    #[test]
    fn test_ipc_server_starts_and_accepts_connections() {
        let port = find_available_port();
        let server = IpcServer::start_on_port(port).expect("Failed to start server");
        assert!(server.is_running());
        assert_eq!(server.port(), port);

        // Try to connect
        let stream = TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", port).parse().unwrap(),
            Duration::from_secs(2),
        );
        assert!(stream.is_ok(), "Should be able to connect to server");
    }

    #[test]
    fn test_ipc_server_receives_command_via_channel() {
        let port = find_available_port();
        let server = IpcServer::start_on_port(port).expect("Failed to start server");

        // Connect and send a command
        let mut stream = TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", port).parse().unwrap(),
            Duration::from_secs(2),
        )
        .expect("Failed to connect");
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

        // Send a Ping command
        let cmd = IpcCommand::Ping;
        let json = serde_json::to_string(&cmd).unwrap();
        writeln!(stream, "{}", json).expect("Failed to write");
        stream.flush().expect("Failed to flush");

        // Poll for the command (give the server thread time to process)
        let mut received = None;
        for _ in 0..50 {
            if let Some(cmd) = server.poll_command() {
                received = Some(cmd);
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(received.is_some(), "Should receive command via channel");
        let (command, response_tx) = received.unwrap();
        assert!(matches!(command, IpcCommand::Ping));

        // Send response back
        response_tx
            .send(IpcResponse::ok_with_data(ResponseData::Pong))
            .expect("Failed to send response");

        // Read response from stream
        let mut reader = BufReader::new(&stream);
        let mut response_line = String::new();
        reader
            .read_line(&mut response_line)
            .expect("Failed to read");

        let response: IpcResponse = serde_json::from_str(&response_line).expect("Failed to parse");
        assert!(matches!(
            response,
            IpcResponse::Ok(Some(ResponseData::Pong))
        ));
    }

    #[test]
    fn test_ipc_server_handles_sequential_connections() {
        // This test mirrors the real usage: one command per connection
        // (as documented in client.rs: "Each command creates a new TCP connection")
        let port = find_available_port();
        let server = IpcServer::start_on_port(port).expect("Failed to start server");

        let commands = vec![IpcCommand::Ping, IpcCommand::GetState, IpcCommand::Ping];

        for cmd in commands {
            // New connection for each command (matches real MCP client behavior)
            let mut stream = TcpStream::connect_timeout(
                &format!("127.0.0.1:{}", port).parse().unwrap(),
                Duration::from_secs(2),
            )
            .expect("Failed to connect");
            stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
            stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

            let json = serde_json::to_string(&cmd).unwrap();
            writeln!(stream, "{}", json).expect("Failed to write");
            stream.flush().expect("Failed to flush");

            // Poll for command
            let mut received = None;
            for _ in 0..50 {
                if let Some(c) = server.poll_command() {
                    received = Some(c);
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }

            assert!(received.is_some(), "Should receive command");
            let (_command, response_tx) = received.unwrap();

            // Send response
            response_tx
                .send(IpcResponse::ok())
                .expect("Failed to send response");

            // Read response
            let mut reader = BufReader::new(&stream);
            let mut response_line = String::new();
            reader
                .read_line(&mut response_line)
                .expect("Failed to read");
            let response: IpcResponse =
                serde_json::from_str(&response_line).expect("Failed to parse");
            assert!(matches!(response, IpcResponse::Ok(_)));
        }
    }

    #[test]
    fn test_ipc_server_handles_invalid_json() {
        let port = find_available_port();
        let _server = IpcServer::start_on_port(port).expect("Failed to start server");

        let mut stream = TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", port).parse().unwrap(),
            Duration::from_secs(2),
        )
        .expect("Failed to connect");
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

        // Send invalid JSON
        writeln!(stream, "{{not valid json}}").expect("Failed to write");
        stream.flush().expect("Failed to flush");

        // Read error response
        let mut reader = BufReader::new(&stream);
        let mut response_line = String::new();
        reader
            .read_line(&mut response_line)
            .expect("Failed to read");

        let response: IpcResponse = serde_json::from_str(&response_line).expect("Failed to parse");
        assert!(matches!(response, IpcResponse::Error { .. }));
    }

    #[test]
    fn test_ipc_server_poll_returns_none_when_empty() {
        let port = find_available_port();
        let server = IpcServer::start_on_port(port).expect("Failed to start server");

        // Poll without any connections - should return None immediately
        assert!(server.poll_command().is_none());
    }
}
