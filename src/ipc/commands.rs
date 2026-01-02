//! IPC command and response types for GUI-MCP communication

use serde::{Deserialize, Serialize};

/// Commands that can be sent from the MCP server to the GUI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum IpcCommand {
    /// Ping to check if the GUI is running
    Ping,

    /// Get the current state of the application
    GetState,

    /// Load a log file
    LoadFile { path: String },

    /// Close a loaded file
    CloseFile { file_id: String },

    /// List all channels in a loaded file
    ListChannels { file_id: String },

    /// Get data for a specific channel
    GetChannelData {
        file_id: String,
        channel_name: String,
        /// Optional time range (start, end) in seconds
        time_range: Option<(f64, f64)>,
    },

    /// Get statistics for a channel
    GetChannelStats {
        file_id: String,
        channel_name: String,
        /// Optional time range for stats calculation
        time_range: Option<(f64, f64)>,
    },

    /// Select a channel to display on the chart
    SelectChannel {
        file_id: String,
        channel_name: String,
    },

    /// Deselect a channel from the chart
    DeselectChannel {
        file_id: String,
        channel_name: String,
    },

    /// Deselect all channels
    DeselectAllChannels,

    /// Create a computed channel
    CreateComputedChannel {
        name: String,
        formula: String,
        unit: String,
        description: Option<String>,
    },

    /// Delete a computed channel
    DeleteComputedChannel { name: String },

    /// List all computed channel templates
    ListComputedChannels,

    /// Evaluate a formula without creating a permanent channel
    EvaluateFormula {
        file_id: String,
        formula: String,
        /// Optional time range
        time_range: Option<(f64, f64)>,
    },

    /// Set the visible time range on the chart
    SetTimeRange { start: f64, end: f64 },

    /// Set the cursor position
    SetCursor { time: f64 },

    /// Start playback
    Play { speed: Option<f64> },

    /// Pause playback
    Pause,

    /// Stop playback and reset cursor
    Stop,

    /// Get values at the current cursor position
    GetCursorValues { file_id: String },

    /// Find peaks in a channel
    FindPeaks {
        file_id: String,
        channel_name: String,
        /// Minimum prominence for peak detection
        min_prominence: Option<f64>,
    },

    /// Correlate two channels
    CorrelateChannels {
        file_id: String,
        channel_a: String,
        channel_b: String,
    },

    /// Switch to scatter plot view
    ShowScatterPlot {
        file_id: String,
        x_channel: String,
        y_channel: String,
    },

    /// Switch back to time series chart view
    ShowChart,
}

/// Responses from the GUI to the MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "data")]
pub enum IpcResponse {
    /// Successful response with optional data
    Ok(Option<ResponseData>),

    /// Error response
    Error { message: String },
}

/// Data that can be returned in a successful response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ResponseData {
    /// Simple acknowledgment
    Ack,

    /// Pong response
    Pong,

    /// Application state
    State(AppState),

    /// File was loaded successfully
    FileLoaded(FileInfo),

    /// List of channels
    Channels(Vec<ChannelInfo>),

    /// Channel time series data
    ChannelData { times: Vec<f64>, values: Vec<f64> },

    /// Channel statistics
    Stats(ChannelStats),

    /// Formula evaluation result
    FormulaResult {
        times: Vec<f64>,
        values: Vec<f64>,
        stats: ChannelStats,
    },

    /// Values at cursor position
    CursorValues(Vec<CursorValue>),

    /// List of computed channel templates
    ComputedChannels(Vec<ComputedChannelInfo>),

    /// Peak detection results
    Peaks(Vec<Peak>),

    /// Correlation result
    Correlation {
        coefficient: f64,
        interpretation: String,
    },
}

/// Current application state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    /// List of loaded files
    pub files: Vec<FileInfo>,
    /// Currently active file ID
    pub active_file: Option<String>,
    /// Currently selected channels
    pub selected_channels: Vec<SelectedChannelInfo>,
    /// Current cursor time
    pub cursor_time: Option<f64>,
    /// Visible time range
    pub visible_time_range: Option<(f64, f64)>,
    /// Whether playback is active
    pub is_playing: bool,
    /// Current view mode
    pub view_mode: String,
}

/// Information about a loaded file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    /// Unique identifier for the file
    pub id: String,
    /// File path
    pub path: String,
    /// File name (for display)
    pub name: String,
    /// ECU type detected
    pub ecu_type: String,
    /// Number of channels
    pub channel_count: usize,
    /// Number of data records
    pub record_count: usize,
    /// Total duration in seconds
    pub duration: f64,
    /// Sample rate (records per second)
    pub sample_rate: f64,
}

/// Information about a channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    /// Channel name
    pub name: String,
    /// Channel unit
    pub unit: String,
    /// Channel type/category
    pub channel_type: String,
    /// Whether this is a computed channel
    pub is_computed: bool,
    /// Min value in the data
    pub min_value: Option<f64>,
    /// Max value in the data
    pub max_value: Option<f64>,
}

/// Information about a selected channel on the chart
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectedChannelInfo {
    /// File ID
    pub file_id: String,
    /// Channel name
    pub channel_name: String,
    /// Display color (hex)
    pub color: String,
}

/// Channel statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelStats {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub std_dev: f64,
    pub median: f64,
    /// Number of samples
    pub count: usize,
    /// Time of minimum value
    pub min_time: f64,
    /// Time of maximum value
    pub max_time: f64,
}

/// Value at cursor position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorValue {
    pub channel_name: String,
    pub value: f64,
    pub unit: String,
}

/// Information about a computed channel template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputedChannelInfo {
    pub id: String,
    pub name: String,
    pub formula: String,
    pub unit: String,
    pub description: String,
}

/// A detected peak in the data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peak {
    pub time: f64,
    pub value: f64,
    pub prominence: f64,
}

impl IpcResponse {
    /// Create a simple OK response
    pub fn ok() -> Self {
        Self::Ok(Some(ResponseData::Ack))
    }

    /// Create an OK response with data
    pub fn ok_with_data(data: ResponseData) -> Self {
        Self::Ok(Some(data))
    }

    /// Create an error response
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // IPC Command Serialization Tests
    // ========================================================================

    #[test]
    fn test_ping_command_roundtrip() {
        let cmd = IpcCommand::Ping;
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: IpcCommand = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, IpcCommand::Ping));
    }

    #[test]
    fn test_get_state_command_roundtrip() {
        let cmd = IpcCommand::GetState;
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: IpcCommand = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, IpcCommand::GetState));
    }

    #[test]
    fn test_load_file_command_roundtrip() {
        let cmd = IpcCommand::LoadFile {
            path: "/path/to/file.csv".to_string(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: IpcCommand = serde_json::from_str(&json).unwrap();
        if let IpcCommand::LoadFile { path } = parsed {
            assert_eq!(path, "/path/to/file.csv");
        } else {
            panic!("Expected LoadFile command");
        }
    }

    #[test]
    fn test_get_channel_data_with_time_range() {
        let cmd = IpcCommand::GetChannelData {
            file_id: "0".to_string(),
            channel_name: "RPM".to_string(),
            time_range: Some((10.0, 20.0)),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: IpcCommand = serde_json::from_str(&json).unwrap();
        if let IpcCommand::GetChannelData {
            file_id,
            channel_name,
            time_range,
        } = parsed
        {
            assert_eq!(file_id, "0");
            assert_eq!(channel_name, "RPM");
            assert_eq!(time_range, Some((10.0, 20.0)));
        } else {
            panic!("Expected GetChannelData command");
        }
    }

    #[test]
    fn test_get_channel_data_without_time_range() {
        let cmd = IpcCommand::GetChannelData {
            file_id: "0".to_string(),
            channel_name: "Boost".to_string(),
            time_range: None,
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: IpcCommand = serde_json::from_str(&json).unwrap();
        if let IpcCommand::GetChannelData {
            file_id,
            channel_name,
            time_range,
        } = parsed
        {
            assert_eq!(file_id, "0");
            assert_eq!(channel_name, "Boost");
            assert!(time_range.is_none());
        } else {
            panic!("Expected GetChannelData command");
        }
    }

    #[test]
    fn test_create_computed_channel_command() {
        let cmd = IpcCommand::CreateComputedChannel {
            name: "Boost PSI".to_string(),
            formula: "Manifold_Pressure_kPa / 6.895".to_string(),
            unit: "PSI".to_string(),
            description: Some("Boost in PSI".to_string()),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: IpcCommand = serde_json::from_str(&json).unwrap();
        if let IpcCommand::CreateComputedChannel {
            name,
            formula,
            unit,
            description,
        } = parsed
        {
            assert_eq!(name, "Boost PSI");
            assert_eq!(formula, "Manifold_Pressure_kPa / 6.895");
            assert_eq!(unit, "PSI");
            assert_eq!(description, Some("Boost in PSI".to_string()));
        } else {
            panic!("Expected CreateComputedChannel command");
        }
    }

    #[test]
    fn test_play_command_with_speed() {
        let cmd = IpcCommand::Play { speed: Some(2.0) };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: IpcCommand = serde_json::from_str(&json).unwrap();
        if let IpcCommand::Play { speed } = parsed {
            assert_eq!(speed, Some(2.0));
        } else {
            panic!("Expected Play command");
        }
    }

    #[test]
    fn test_find_peaks_command() {
        let cmd = IpcCommand::FindPeaks {
            file_id: "0".to_string(),
            channel_name: "RPM".to_string(),
            min_prominence: Some(100.0),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: IpcCommand = serde_json::from_str(&json).unwrap();
        if let IpcCommand::FindPeaks {
            file_id,
            channel_name,
            min_prominence,
        } = parsed
        {
            assert_eq!(file_id, "0");
            assert_eq!(channel_name, "RPM");
            assert_eq!(min_prominence, Some(100.0));
        } else {
            panic!("Expected FindPeaks command");
        }
    }

    // ========================================================================
    // IPC Response Serialization Tests
    // ========================================================================

    #[test]
    fn test_ok_response_roundtrip() {
        let resp = IpcResponse::ok();
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: IpcResponse = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, IpcResponse::Ok(Some(ResponseData::Ack))));
    }

    #[test]
    fn test_error_response_roundtrip() {
        let resp = IpcResponse::error("Something went wrong");
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: IpcResponse = serde_json::from_str(&json).unwrap();
        if let IpcResponse::Error { message } = parsed {
            assert_eq!(message, "Something went wrong");
        } else {
            panic!("Expected Error response");
        }
    }

    #[test]
    fn test_pong_response_roundtrip() {
        let resp = IpcResponse::ok_with_data(ResponseData::Pong);
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: IpcResponse = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, IpcResponse::Ok(Some(ResponseData::Pong))));
    }

    #[test]
    fn test_channel_data_response_roundtrip() {
        let resp = IpcResponse::ok_with_data(ResponseData::ChannelData {
            times: vec![0.0, 0.1, 0.2, 0.3],
            values: vec![1000.0, 1500.0, 2000.0, 2500.0],
        });
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: IpcResponse = serde_json::from_str(&json).unwrap();
        if let IpcResponse::Ok(Some(ResponseData::ChannelData { times, values })) = parsed {
            assert_eq!(times, vec![0.0, 0.1, 0.2, 0.3]);
            assert_eq!(values, vec![1000.0, 1500.0, 2000.0, 2500.0]);
        } else {
            panic!("Expected ChannelData response");
        }
    }

    #[test]
    fn test_stats_response_roundtrip() {
        let stats = ChannelStats {
            min: 800.0,
            max: 7500.0,
            mean: 3500.0,
            std_dev: 1200.0,
            median: 3200.0,
            count: 1000,
            min_time: 5.2,
            max_time: 42.8,
        };
        let resp = IpcResponse::ok_with_data(ResponseData::Stats(stats));
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: IpcResponse = serde_json::from_str(&json).unwrap();
        if let IpcResponse::Ok(Some(ResponseData::Stats(s))) = parsed {
            assert_eq!(s.min, 800.0);
            assert_eq!(s.max, 7500.0);
            assert_eq!(s.mean, 3500.0);
            assert_eq!(s.count, 1000);
        } else {
            panic!("Expected Stats response");
        }
    }

    #[test]
    fn test_app_state_response_roundtrip() {
        let state = AppState {
            files: vec![FileInfo {
                id: "0".to_string(),
                path: "/path/to/log.csv".to_string(),
                name: "log.csv".to_string(),
                ecu_type: "Haltech".to_string(),
                channel_count: 50,
                record_count: 10000,
                duration: 120.5,
                sample_rate: 100.0,
            }],
            active_file: Some("0".to_string()),
            selected_channels: vec![SelectedChannelInfo {
                file_id: "0".to_string(),
                channel_name: "RPM".to_string(),
                color: "#FF0000".to_string(),
            }],
            cursor_time: Some(15.5),
            visible_time_range: Some((10.0, 30.0)),
            is_playing: false,
            view_mode: "chart".to_string(),
        };
        let resp = IpcResponse::ok_with_data(ResponseData::State(state));
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: IpcResponse = serde_json::from_str(&json).unwrap();
        if let IpcResponse::Ok(Some(ResponseData::State(s))) = parsed {
            assert_eq!(s.files.len(), 1);
            assert_eq!(s.files[0].name, "log.csv");
            assert_eq!(s.selected_channels.len(), 1);
            assert_eq!(s.cursor_time, Some(15.5));
            assert!(!s.is_playing);
        } else {
            panic!("Expected State response");
        }
    }

    #[test]
    fn test_correlation_response_roundtrip() {
        let resp = IpcResponse::ok_with_data(ResponseData::Correlation {
            coefficient: 0.87,
            interpretation: "Strong positive correlation".to_string(),
        });
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: IpcResponse = serde_json::from_str(&json).unwrap();
        if let IpcResponse::Ok(Some(ResponseData::Correlation {
            coefficient,
            interpretation,
        })) = parsed
        {
            assert!((coefficient - 0.87).abs() < 0.001);
            assert_eq!(interpretation, "Strong positive correlation");
        } else {
            panic!("Expected Correlation response");
        }
    }

    #[test]
    fn test_peaks_response_roundtrip() {
        let peaks = vec![
            Peak {
                time: 10.5,
                value: 7200.0,
                prominence: 500.0,
            },
            Peak {
                time: 25.3,
                value: 7500.0,
                prominence: 800.0,
            },
        ];
        let resp = IpcResponse::ok_with_data(ResponseData::Peaks(peaks));
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: IpcResponse = serde_json::from_str(&json).unwrap();
        if let IpcResponse::Ok(Some(ResponseData::Peaks(p))) = parsed {
            assert_eq!(p.len(), 2);
            assert_eq!(p[0].time, 10.5);
            assert_eq!(p[1].value, 7500.0);
        } else {
            panic!("Expected Peaks response");
        }
    }

    // ========================================================================
    // JSON Format Compatibility Tests
    // ========================================================================

    #[test]
    fn test_command_json_format_is_stable() {
        // Ensure the JSON format is what MCP clients expect
        let cmd = IpcCommand::LoadFile {
            path: "/test.csv".to_string(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        // Should use tagged enum format
        assert!(json.contains("\"type\":\"LoadFile\""));
        assert!(json.contains("\"payload\""));
        assert!(json.contains("\"/test.csv\""));
    }

    #[test]
    fn test_response_json_format_is_stable() {
        // Ensure the JSON format is what MCP clients expect
        let resp = IpcResponse::ok();
        let json = serde_json::to_string(&resp).unwrap();
        // Should use tagged enum format
        assert!(json.contains("\"status\":\"Ok\""));

        let err = IpcResponse::error("test error");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"status\":\"Error\""));
        assert!(json.contains("\"test error\""));
    }

    #[test]
    fn test_command_can_be_parsed_from_external_json() {
        // Test parsing JSON that might come from an external MCP client
        let json = r#"{"type":"GetChannelData","payload":{"file_id":"0","channel_name":"RPM","time_range":[0.0,10.0]}}"#;
        let cmd: IpcCommand = serde_json::from_str(json).unwrap();
        if let IpcCommand::GetChannelData {
            file_id,
            channel_name,
            time_range,
        } = cmd
        {
            assert_eq!(file_id, "0");
            assert_eq!(channel_name, "RPM");
            assert_eq!(time_range, Some((0.0, 10.0)));
        } else {
            panic!("Expected GetChannelData command");
        }
    }
}
