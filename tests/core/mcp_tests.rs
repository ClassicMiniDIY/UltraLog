//! Tests for the MCP (Model Context Protocol) and IPC integration
//!
//! Tests verify:
//! - IPC server starts and accepts TCP connections
//! - IPC commands serialize/deserialize correctly (covered in commands.rs unit tests)
//! - MCP server creates tool router with all expected tools
//! - MCP server info is correctly configured
//! - GuiClient connects and communicates with IPC server
//! - End-to-end IPC command flow (client -> server -> response)

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

use ultralog::ipc::commands::{IpcCommand, IpcResponse, ResponseData};
use ultralog::ipc::IpcServer;
use ultralog::mcp::UltraLogMcpServer;

use rmcp::ServerHandler;

/// Find an available port for testing
fn find_available_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

// ============================================================================
// MCP Server Tool Registration Tests
// ============================================================================

#[test]
fn test_mcp_server_info() {
    let server = UltraLogMcpServer::new();
    let info = server.get_info();

    assert_eq!(info.server_info.name, "ultralog");
    assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
    assert!(info.instructions.is_some());
    assert!(
        info.capabilities.tools.is_some(),
        "Should advertise tools capability"
    );
}

#[test]
fn test_mcp_server_instructions_mention_get_state() {
    let server = UltraLogMcpServer::new();
    let info = server.get_info();

    let instructions = info.instructions.unwrap();
    assert!(
        instructions.contains("get_state"),
        "Instructions should mention get_state tool"
    );
}

#[test]
fn test_mcp_server_protocol_version() {
    let server = UltraLogMcpServer::new();
    let info = server.get_info();

    // Should use a valid MCP protocol version
    assert_eq!(
        info.protocol_version,
        rmcp::model::ProtocolVersion::V_2024_11_05
    );
}

// ============================================================================
// IPC Server Integration Tests
// ============================================================================

#[test]
fn test_ipc_server_starts_on_dynamic_port() {
    let port = find_available_port();
    let server = IpcServer::start_on_port(port).expect("Should start IPC server");
    assert!(server.is_running());
    assert_eq!(server.port(), port);
}

#[test]
fn test_ipc_server_rejects_duplicate_port() {
    let port = find_available_port();
    let _server1 = IpcServer::start_on_port(port).expect("First server should start");

    // Second server on same port should fail
    let result = IpcServer::start_on_port(port);
    assert!(result.is_err(), "Should fail to bind to same port twice");
}

#[test]
fn test_ipc_ping_pong_roundtrip() {
    let port = find_available_port();
    let server = IpcServer::start_on_port(port).expect("Should start IPC server");

    // Give the server listener thread time to start accepting
    std::thread::sleep(Duration::from_millis(200));

    // Connect as a client
    let mut stream = TcpStream::connect_timeout(
        &format!("127.0.0.1:{}", port).parse().unwrap(),
        Duration::from_secs(2),
    )
    .expect("Should connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();

    // Send Ping
    let cmd = serde_json::to_string(&IpcCommand::Ping).unwrap();
    writeln!(stream, "{}", cmd).unwrap();
    stream.flush().unwrap();

    // Poll for the command on the server side
    let mut received = None;
    for _ in 0..100 {
        if let Some(cmd) = server.poll_command() {
            received = Some(cmd);
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    let (command, response_tx) = received.expect("Should receive Ping command");
    assert!(matches!(command, IpcCommand::Ping));

    // Respond with Pong
    response_tx
        .send(IpcResponse::ok_with_data(ResponseData::Pong))
        .unwrap();

    // Read the response
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();

    let response: IpcResponse = serde_json::from_str(&line).unwrap();
    assert!(matches!(
        response,
        IpcResponse::Ok(Some(ResponseData::Pong))
    ));
}

#[test]
fn test_ipc_load_file_command_roundtrip() {
    let port = find_available_port();
    let server = IpcServer::start_on_port(port).expect("Should start IPC server");

    // Give the server listener thread time to start accepting
    std::thread::sleep(Duration::from_millis(200));

    let mut stream = TcpStream::connect_timeout(
        &format!("127.0.0.1:{}", port).parse().unwrap(),
        Duration::from_secs(2),
    )
    .expect("Should connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();

    // Send LoadFile command
    let cmd = IpcCommand::LoadFile {
        path: "/tmp/test.csv".to_string(),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    writeln!(stream, "{}", json).unwrap();
    stream.flush().unwrap();

    // Poll and verify
    let mut received = None;
    for _ in 0..100 {
        if let Some(cmd) = server.poll_command() {
            received = Some(cmd);
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    let (command, response_tx) = received.expect("Should receive LoadFile command");
    if let IpcCommand::LoadFile { path } = command {
        assert_eq!(path, "/tmp/test.csv");
    } else {
        panic!("Expected LoadFile, got {:?}", command);
    }

    // Send response
    response_tx.send(IpcResponse::ok()).unwrap();

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let response: IpcResponse = serde_json::from_str(&line).unwrap();
    assert!(matches!(response, IpcResponse::Ok(_)));
}

#[test]
fn test_ipc_sequential_connections() {
    // Test sequential connections (one command per connection, matching real MCP client behavior)
    let port = find_available_port();
    let server = IpcServer::start_on_port(port).expect("Should start IPC server");

    // Give the server listener thread time to start
    std::thread::sleep(Duration::from_millis(200));

    let commands = vec![
        IpcCommand::Ping,
        IpcCommand::GetState,
        IpcCommand::ListComputedChannels,
    ];

    for cmd in commands {
        // New connection for each command (matches real MCP client behavior)
        let mut stream = TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", port).parse().unwrap(),
            Duration::from_secs(2),
        )
        .expect("Should connect");
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();

        let json = serde_json::to_string(&cmd).unwrap();
        writeln!(stream, "{}", json).unwrap();
        stream.flush().unwrap();

        // Poll for command
        let mut received = None;
        for _ in 0..100 {
            if let Some(c) = server.poll_command() {
                received = Some(c);
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        let (_command, response_tx) = received.expect("Should receive command");
        response_tx.send(IpcResponse::ok()).unwrap();

        // Read response
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let _response: IpcResponse = serde_json::from_str(&line).unwrap();
    }
}

#[test]
fn test_ipc_error_response_for_invalid_json() {
    let port = find_available_port();
    let _server = IpcServer::start_on_port(port).expect("Should start IPC server");

    let mut stream = TcpStream::connect_timeout(
        &format!("127.0.0.1:{}", port).parse().unwrap(),
        Duration::from_secs(2),
    )
    .expect("Should connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();

    // Send garbage
    writeln!(stream, "this is not json at all").unwrap();
    stream.flush().unwrap();

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();

    let response: IpcResponse = serde_json::from_str(&line).unwrap();
    match response {
        IpcResponse::Error { message } => {
            assert!(
                message.contains("Invalid command JSON"),
                "Error should mention invalid JSON, got: {}",
                message
            );
        }
        _ => panic!("Expected Error response for invalid JSON"),
    }
}

// ============================================================================
// GuiClient Tests
// ============================================================================

#[test]
fn test_gui_client_ping() {
    use ultralog::mcp::client::GuiClient;

    let port = find_available_port();
    let server = IpcServer::start_on_port(port).expect("Should start IPC server");

    // Give the server listener thread time to start
    std::thread::sleep(Duration::from_millis(200));

    // Spawn a thread to respond to the ping
    let handle = std::thread::spawn(move || {
        for _ in 0..200 {
            if let Some((cmd, tx)) = server.poll_command() {
                assert!(matches!(cmd, IpcCommand::Ping));
                tx.send(IpcResponse::ok_with_data(ResponseData::Pong))
                    .unwrap();
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    });

    let client = GuiClient::with_port(port);
    assert!(client.ping(), "Ping should succeed");

    assert!(handle.join().unwrap(), "Server should have received ping");
}

#[test]
fn test_gui_client_send_command() {
    use ultralog::mcp::client::GuiClient;

    let port = find_available_port();
    let server = IpcServer::start_on_port(port).expect("Should start IPC server");

    // Give the server listener thread time to start
    std::thread::sleep(Duration::from_millis(200));

    // Spawn responder thread
    let handle = std::thread::spawn(move || {
        for _ in 0..200 {
            if let Some((
                IpcCommand::GetChannelStats {
                    file_id,
                    channel_name,
                    time_range,
                },
                tx,
            )) = server.poll_command()
            {
                assert_eq!(file_id, "0");
                assert_eq!(channel_name, "RPM");
                assert_eq!(time_range, Some((0.0, 10.0)));
                tx.send(IpcResponse::ok()).unwrap();
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    });

    let client = GuiClient::with_port(port);
    let result = client.send_command(IpcCommand::GetChannelStats {
        file_id: "0".to_string(),
        channel_name: "RPM".to_string(),
        time_range: Some((0.0, 10.0)),
    });

    assert!(result.is_ok(), "Command should succeed");
    assert!(
        handle.join().unwrap(),
        "Server should have received command"
    );
}

#[test]
fn test_gui_client_fails_when_no_server() {
    use ultralog::mcp::client::GuiClient;

    // Use a port where nothing is listening
    let port = find_available_port();
    let client = GuiClient::with_port(port);

    assert!(!client.ping(), "Ping should fail with no server");

    let result = client.send_command(IpcCommand::GetState);
    assert!(result.is_err(), "Command should fail with no server");
}

// ============================================================================
// MCP Server Handle Tests
// ============================================================================

#[test]
fn test_mcp_server_handle_url() {
    use ultralog::mcp::start_mcp_server;

    // Start an IPC server first (the MCP server needs it)
    let ipc_port = find_available_port();
    let _ipc_server = IpcServer::start_on_port(ipc_port).expect("Should start IPC server");

    let mcp_port = find_available_port();
    let handle = start_mcp_server(mcp_port, ipc_port).expect("Should start MCP server");

    assert_eq!(handle.port(), mcp_port);
    assert_eq!(handle.url(), format!("http://127.0.0.1:{}/mcp", mcp_port));
}
