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

use ultralog::app::UltraLogApp;
use ultralog::ipc::IpcServer;
use ultralog::ipc::commands::{
    ChannelStats, DEFAULT_MAX_POINTS, IpcCommand, IpcResponse, MAX_POINTS_LIMIT, ResponseData,
};
use ultralog::mcp::UltraLogMcpServer;
use ultralog::mcp::server::MAX_RESPONSE_BYTES;

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

// ============================================================================
// Response Size Budget Tests (issue #88)
// ============================================================================
//
// Streamable-HTTP MCP clients cap a single SSE event at 1 MiB and drop anything
// larger inside their SSE decoder, so an oversized tool result surfaces to the
// caller as neither a value nor an error - the call simply never returns.
// `evaluate_formula` and `get_channel_data` used to serialize one entry per log
// record, so any log past ~22,000 rows crossed that cap and hung. These tests
// pin the two defenses: a sample budget on the data itself, and a hard byte
// guard on the serialized payload.

/// The SSE event size limit that motivated the budget, in bytes.
const SSE_EVENT_LIMIT: usize = 1024 * 1024;

#[test]
fn test_limit_samples_defaults_to_budget() {
    let n = 178_000;
    let times: Vec<f64> = (0..n).map(|i| i as f64 * 0.01).collect();
    let values: Vec<f64> = (0..n).map(|i| (i as f64).sin()).collect();

    let (t, v, downsampled) = UltraLogApp::limit_samples(times, values, None);

    assert!(downsampled, "A 178k-record series must report downsampling");
    assert_eq!(t.len(), DEFAULT_MAX_POINTS);
    assert_eq!(v.len(), DEFAULT_MAX_POINTS);
}

#[test]
fn test_limit_samples_leaves_short_series_untouched() {
    let times: Vec<f64> = (0..500).map(|i| i as f64).collect();
    let values: Vec<f64> = (0..500).map(|i| i as f64 * 2.0).collect();

    let (t, v, downsampled) = UltraLogApp::limit_samples(times.clone(), values.clone(), None);

    assert!(
        !downsampled,
        "A series under budget must not be downsampled"
    );
    assert_eq!(t, times);
    assert_eq!(v, values);
}

#[test]
fn test_limit_samples_clamps_request_to_ceiling() {
    let n = 200_000;
    let times: Vec<f64> = (0..n).map(|i| i as f64 * 0.01).collect();
    let values: Vec<f64> = (0..n).map(|i| (i as f64).cos()).collect();

    let (t, _, downsampled) = UltraLogApp::limit_samples(times, values, Some(usize::MAX));

    assert!(downsampled);
    assert_eq!(
        t.len(),
        MAX_POINTS_LIMIT,
        "An unbounded max_points must clamp to the ceiling, not honour the request"
    );
}

#[test]
fn test_limit_samples_preserves_endpoints() {
    let n = 50_000;
    let times: Vec<f64> = (0..n).map(|i| i as f64 * 0.1).collect();
    let values: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let (first_t, last_t) = (times[0], times[n - 1]);
    let (first_v, last_v) = (values[0], values[n - 1]);

    let (t, v, _) = UltraLogApp::limit_samples(times, values, Some(1000));

    assert_eq!(t.first().copied(), Some(first_t));
    assert_eq!(t.last().copied(), Some(last_t));
    assert_eq!(v.first().copied(), Some(first_v));
    assert_eq!(v.last().copied(), Some(last_v));
}

#[test]
fn test_limit_samples_handles_tiny_budgets() {
    let times: Vec<f64> = (0..10_000).map(|i| i as f64).collect();
    let values: Vec<f64> = (0..10_000).map(|i| i as f64).collect();

    for budget in [1usize, 2, 3] {
        let (t, v, downsampled) =
            UltraLogApp::limit_samples(times.clone(), values.clone(), Some(budget));
        assert!(downsampled);
        assert_eq!(
            t.len(),
            budget,
            "budget {} should be honoured exactly",
            budget
        );
        assert_eq!(v.len(), budget);
    }
}

#[test]
fn test_max_budget_payload_fits_under_sse_event_limit() {
    // Worst case a caller can ask for: the ceiling, with wide values that
    // serialize to long decimal expansions.
    let times: Vec<f64> = (0..MAX_POINTS_LIMIT)
        .map(|i| i as f64 * 0.123_456_789_012)
        .collect();
    let values: Vec<f64> = (0..MAX_POINTS_LIMIT)
        .map(|i| (i as f64).sin() * -123_456.789_012_345)
        .collect();

    let payload = serde_json::json!({
        "sample_count": times.len(),
        "total_samples": 500_000,
        "downsampled": true,
        "stats": {
            "min": -1.0, "max": 1.0, "mean": 0.5, "std_dev": 0.1,
            "median": 0.5, "count": 500_000, "min_time": 0.0, "max_time": 1.0
        },
        "times": times,
        "values": values,
    });
    let encoded = serde_json::to_string(&payload).unwrap();

    assert!(
        encoded.len() <= MAX_RESPONSE_BYTES,
        "Worst-case payload is {} bytes, over the {} byte guard",
        encoded.len(),
        MAX_RESPONSE_BYTES
    );
    assert!(
        encoded.len() < SSE_EVENT_LIMIT,
        "Worst-case payload is {} bytes, at or over the {} byte SSE event limit",
        encoded.len(),
        SSE_EVENT_LIMIT
    );
    const {
        assert!(
            MAX_RESPONSE_BYTES < SSE_EVENT_LIMIT,
            "The guard must sit below the limit it is protecting against"
        )
    };
}

#[test]
fn test_json_result_rejects_oversized_payload() {
    let oversized = serde_json::json!({ "values": vec![1.234_567_890_123_f64; 200_000] });
    let encoded_len = serde_json::to_string(&oversized).unwrap().len();
    assert!(
        encoded_len > MAX_RESPONSE_BYTES,
        "Fixture must actually exceed the guard (was {} bytes)",
        encoded_len
    );

    let err = UltraLogMcpServer::json_result(&oversized)
        .expect_err("An oversized payload must be refused, not emitted");

    // The caller has to be told what to do about it, since the transport would
    // otherwise drop the event with no diagnostic at all.
    assert!(
        err.message.contains("max_points") && err.message.contains("time range"),
        "Error should name the knobs that fix it, got: {}",
        err.message
    );
}

#[test]
fn test_json_result_accepts_budgeted_payload() {
    let times: Vec<f64> = (0..DEFAULT_MAX_POINTS).map(|i| i as f64 * 0.01).collect();
    let payload = serde_json::json!({ "sample_count": times.len(), "times": times });

    let result = UltraLogMcpServer::json_result(&payload).expect("Budgeted payload must be sent");
    assert_eq!(result.is_error, Some(false));
}

#[test]
fn test_evaluate_formula_response_roundtrips_at_full_scale() {
    // End-to-end over the real IPC transport: a 178,000-record log (the size
    // reported in issue #88) must come back bounded and fast.
    let port = find_available_port();
    let server = IpcServer::start_on_port(port).expect("Failed to start server");
    std::thread::sleep(Duration::from_millis(200));

    std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while std::time::Instant::now() < deadline {
            if let Some((command, response_tx)) = server.poll_command() {
                let IpcCommand::EvaluateFormula { max_points, .. } = command else {
                    let _ = response_tx.send(IpcResponse::error("Unexpected command"));
                    continue;
                };
                let n = 178_000;
                let times: Vec<f64> = (0..n).map(|i| i as f64 * 0.01).collect();
                let values: Vec<f64> = (0..n).map(|i| (i as f64).sin() * 1234.5678).collect();
                let total_samples = times.len();
                let (times, values, downsampled) =
                    UltraLogApp::limit_samples(times, values, max_points);
                let _ = response_tx.send(IpcResponse::ok_with_data(ResponseData::FormulaResult {
                    times,
                    values,
                    stats: ChannelStats {
                        min: -1234.5678,
                        max: 1234.5678,
                        mean: 0.0,
                        std_dev: 1.0,
                        median: 0.0,
                        count: total_samples,
                        min_time: 0.0,
                        max_time: 1779.99,
                    },
                    total_samples,
                    downsampled,
                }));
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });

    let client = ultralog::mcp::client::GuiClient::with_port(port);
    let response = client
        .send_command(IpcCommand::EvaluateFormula {
            file_id: "0".to_string(),
            formula: "RPM * 2".to_string(),
            time_range: None,
            max_points: None,
        })
        .expect("Full-scale evaluate_formula must return a response");

    let IpcResponse::Ok(Some(ResponseData::FormulaResult {
        times,
        values,
        stats,
        total_samples,
        downsampled,
    })) = response
    else {
        panic!("Expected FormulaResult, got {:?}", response);
    };

    assert_eq!(total_samples, 178_000, "The true record count must survive");
    assert_eq!(stats.count, 178_000, "Stats must describe every record");
    assert!(downsampled);
    assert_eq!(times.len(), DEFAULT_MAX_POINTS);
    assert_eq!(values.len(), DEFAULT_MAX_POINTS);

    let encoded = serde_json::to_string(&serde_json::json!({
        "sample_count": times.len(),
        "total_samples": total_samples,
        "downsampled": downsampled,
        "stats": stats,
        "times": times,
        "values": values,
    }))
    .unwrap();
    assert!(
        encoded.len() < SSE_EVENT_LIMIT,
        "Full-scale response is {} bytes, which the transport would drop",
        encoded.len()
    );
}

#[test]
fn test_require_aligned_rejects_ragged_series() {
    // `Log::get_channel_data` is a `filter_map` that drops a row missing the
    // column, so a ragged log yields fewer values than times - misaligned, not
    // merely short. Feeding that pair to LTTB indexes `values` off `times.len()`
    // and panics on the GUI thread, so it has to be refused up front.
    let times: Vec<f64> = (0..100).map(|i| i as f64).collect();
    let values: Vec<f64> = (0..97).map(|i| i as f64).collect();

    let err = UltraLogApp::require_aligned("MAP", &times, &values)
        .expect_err("A ragged series must be refused");
    assert!(
        err.contains("MAP") && err.contains("97") && err.contains("100"),
        "Error should name the channel and both counts, got: {}",
        err
    );

    UltraLogApp::require_aligned("MAP", &times, &times).expect("Aligned series must pass");
    UltraLogApp::require_aligned("MAP", &[], &[]).expect("Empty series must pass");
}

#[test]
fn test_limit_samples_never_panics_on_aligned_series() {
    // Guards the LTTB call itself: the budget path must hold for a range of
    // lengths straddling DEFAULT_MAX_POINTS, not just the big ones.
    for n in [0usize, 1, 2, 3, 4, 1999, 2000, 2001, 5000] {
        let times: Vec<f64> = (0..n).map(|i| i as f64 * 0.01).collect();
        let values: Vec<f64> = (0..n).map(|i| (i as f64).sin()).collect();
        let (t, v, _) = UltraLogApp::limit_samples(times, values, None);
        assert_eq!(t.len(), v.len(), "n={} produced a misaligned result", n);
        assert_eq!(
            t.len(),
            n.min(DEFAULT_MAX_POINTS),
            "n={} exceeded budget",
            n
        );
    }
}
