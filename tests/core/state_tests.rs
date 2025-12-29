//! Comprehensive tests for core state types
//!
//! Tests cover:
//! - LoadedFile initialization and channel data detection
//! - Tab state management
//! - ToastType colors
//! - Constants and palettes
//! - ActiveTool enum

use std::path::PathBuf;
use ultralog::parsers::haltech::{ChannelType, HaltechChannel};
use ultralog::parsers::types::{EcuType, Log, Value};
use ultralog::parsers::Channel;
use ultralog::state::{
    ActiveTool, CacheKey, LoadResult, LoadedFile, LoadingState, ScatterPlotConfig,
    ScatterPlotState, SelectedChannel, SelectedHeatmapPoint, Tab, ToastType, CHART_COLORS,
    COLORBLIND_COLORS, MAX_CHANNELS, MAX_CHART_POINTS, SUPPORTED_EXTENSIONS,
};

// ============================================
// Constant Tests
// ============================================

#[test]
fn test_max_channels_reasonable() {
    assert!(MAX_CHANNELS >= 1, "Should allow at least 1 channel");
    assert!(MAX_CHANNELS <= 20, "Should not allow too many channels");
    assert_eq!(MAX_CHANNELS, 10, "Expected 10 max channels");
}

#[test]
fn test_max_chart_points_reasonable() {
    assert!(
        MAX_CHART_POINTS >= 100,
        "Should have minimum points for visualization"
    );
    assert!(MAX_CHART_POINTS <= 10000, "Should not have too many points");
    assert_eq!(MAX_CHART_POINTS, 2000, "Expected 2000 max chart points");
}

#[test]
fn test_supported_extensions_not_empty() {
    assert!(
        !SUPPORTED_EXTENSIONS.is_empty(),
        "Should have supported extensions"
    );
}

#[test]
fn test_supported_extensions_contains_common() {
    assert!(SUPPORTED_EXTENSIONS.contains(&"csv"), "Should support CSV");
    assert!(SUPPORTED_EXTENSIONS.contains(&"mlg"), "Should support MLG");
    assert!(SUPPORTED_EXTENSIONS.contains(&"xrk"), "Should support XRK");
    assert!(SUPPORTED_EXTENSIONS.contains(&"llg"), "Should support LLG");
}

// ============================================
// Color Palette Tests
// ============================================

#[test]
fn test_chart_colors_not_empty() {
    assert!(!CHART_COLORS.is_empty(), "Should have chart colors");
    assert_eq!(CHART_COLORS.len(), 10, "Should have 10 chart colors");
}

#[test]
fn test_chart_colors_valid_rgb() {
    for (i, color) in CHART_COLORS.iter().enumerate() {
        assert_eq!(color.len(), 3, "Color {} should have 3 components", i);
        // RGB values are u8, so already in 0-255 range
    }
}

#[test]
fn test_chart_colors_unique() {
    let mut unique_colors: Vec<&[u8; 3]> = Vec::new();
    for color in CHART_COLORS {
        assert!(
            !unique_colors.contains(&color),
            "Chart colors should be unique"
        );
        unique_colors.push(color);
    }
}

#[test]
fn test_colorblind_colors_not_empty() {
    assert!(
        !COLORBLIND_COLORS.is_empty(),
        "Should have colorblind colors"
    );
    assert_eq!(
        COLORBLIND_COLORS.len(),
        10,
        "Should have 10 colorblind colors"
    );
}

#[test]
fn test_colorblind_colors_valid_rgb() {
    for (i, color) in COLORBLIND_COLORS.iter().enumerate() {
        assert_eq!(
            color.len(),
            3,
            "Colorblind color {} should have 3 components",
            i
        );
    }
}

#[test]
fn test_chart_and_colorblind_same_count() {
    assert_eq!(
        CHART_COLORS.len(),
        COLORBLIND_COLORS.len(),
        "Chart and colorblind palettes should have same count"
    );
}

// ============================================
// ToastType Tests
// ============================================

#[test]
fn test_toast_type_default() {
    let toast = ToastType::default();
    assert!(matches!(toast, ToastType::Info));
}

#[test]
fn test_toast_type_colors() {
    let info_color = ToastType::Info.color();
    let success_color = ToastType::Success.color();
    let warning_color = ToastType::Warning.color();
    let error_color = ToastType::Error.color();

    // Each type should have unique color
    assert_ne!(info_color, success_color);
    assert_ne!(info_color, warning_color);
    assert_ne!(info_color, error_color);
    assert_ne!(success_color, warning_color);
    assert_ne!(success_color, error_color);
    assert_ne!(warning_color, error_color);
}

#[test]
fn test_toast_type_text_colors() {
    let info_text = ToastType::Info.text_color();
    let success_text = ToastType::Success.text_color();
    let warning_text = ToastType::Warning.text_color();
    let error_text = ToastType::Error.text_color();

    // Warning should have dark text (for amber background)
    assert_eq!(warning_text, [30, 30, 30]);

    // Others should have white text
    assert_eq!(info_text, [255, 255, 255]);
    assert_eq!(success_text, [255, 255, 255]);
    assert_eq!(error_text, [255, 255, 255]);
}

#[test]
fn test_toast_type_copy() {
    let toast1 = ToastType::Success;
    let toast2 = toast1;
    assert!(matches!(toast2, ToastType::Success));
}

// ============================================
// ActiveTool Tests
// ============================================

#[test]
fn test_active_tool_default() {
    let tool = ActiveTool::default();
    assert!(matches!(tool, ActiveTool::LogViewer));
}

#[test]
fn test_active_tool_names() {
    assert_eq!(ActiveTool::LogViewer.name(), "Log Viewer");
    assert_eq!(ActiveTool::ScatterPlot.name(), "Scatter Plots");
}

#[test]
fn test_active_tool_equality() {
    // Use pattern matching since ActiveTool doesn't implement Debug
    assert!(ActiveTool::LogViewer == ActiveTool::LogViewer);
    assert!(ActiveTool::ScatterPlot == ActiveTool::ScatterPlot);
    assert!(ActiveTool::LogViewer != ActiveTool::ScatterPlot);
}

#[test]
fn test_active_tool_copy() {
    let tool1 = ActiveTool::ScatterPlot;
    let tool2 = tool1;
    assert!(tool1 == tool2);
}

// ============================================
// CacheKey Tests
// ============================================

#[test]
fn test_cache_key_equality() {
    let key1 = CacheKey {
        file_index: 0,
        channel_index: 1,
    };
    let key2 = CacheKey {
        file_index: 0,
        channel_index: 1,
    };
    let key3 = CacheKey {
        file_index: 0,
        channel_index: 2,
    };

    // Use direct comparison since CacheKey doesn't implement Debug
    assert!(key1 == key2);
    assert!(key1 != key3);
}

#[test]
fn test_cache_key_hash() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    let key1 = CacheKey {
        file_index: 0,
        channel_index: 1,
    };
    let key2 = CacheKey {
        file_index: 0,
        channel_index: 2,
    };

    set.insert(key1.clone());
    set.insert(key2);

    assert_eq!(set.len(), 2);
    assert!(set.contains(&key1));
}

#[test]
fn test_cache_key_clone() {
    let key1 = CacheKey {
        file_index: 5,
        channel_index: 10,
    };
    let key2 = key1.clone();

    assert!(key1 == key2);
}

// ============================================
// LoadingState Tests
// ============================================

#[test]
fn test_loading_state_idle() {
    let state = LoadingState::Idle;
    assert!(matches!(state, LoadingState::Idle));
}

#[test]
fn test_loading_state_loading() {
    let state = LoadingState::Loading("test.csv".to_string());
    if let LoadingState::Loading(name) = state {
        assert_eq!(name, "test.csv");
    } else {
        panic!("Expected Loading state");
    }
}

// ============================================
// LoadResult Tests
// ============================================

#[test]
fn test_load_result_error() {
    let result = LoadResult::Error("File not found".to_string());
    if let LoadResult::Error(msg) = result {
        assert_eq!(msg, "File not found");
    } else {
        panic!("Expected Error result");
    }
}

// ============================================
// ScatterPlotConfig Tests
// ============================================

#[test]
fn test_scatter_plot_config_default() {
    let config = ScatterPlotConfig::default();

    assert!(config.file_index.is_none());
    assert!(config.x_channel.is_none());
    assert!(config.y_channel.is_none());
    assert!(config.z_channel.is_none());
    assert!(config.selected_point.is_none());
}

#[test]
fn test_scatter_plot_config_with_values() {
    let mut config = ScatterPlotConfig::default();
    config.file_index = Some(0);
    config.x_channel = Some(1);
    config.y_channel = Some(2);
    config.z_channel = Some(3);

    assert_eq!(config.file_index, Some(0));
    assert_eq!(config.x_channel, Some(1));
    assert_eq!(config.y_channel, Some(2));
    assert_eq!(config.z_channel, Some(3));
}

// ============================================
// ScatterPlotState Tests
// ============================================

#[test]
fn test_scatter_plot_state_default() {
    let state = ScatterPlotState::default();

    assert!(state.left.file_index.is_none());
    assert!(state.right.file_index.is_none());
}

#[test]
fn test_scatter_plot_state_clone() {
    let mut state = ScatterPlotState::default();
    state.left.x_channel = Some(5);
    state.right.y_channel = Some(10);

    let cloned = state.clone();

    assert_eq!(cloned.left.x_channel, Some(5));
    assert_eq!(cloned.right.y_channel, Some(10));
}

// ============================================
// SelectedHeatmapPoint Tests
// ============================================

#[test]
fn test_selected_heatmap_point_default() {
    let point = SelectedHeatmapPoint::default();

    assert_eq!(point.x_value, 0.0);
    assert_eq!(point.y_value, 0.0);
    assert_eq!(point.hits, 0);
}

#[test]
fn test_selected_heatmap_point_clone() {
    let point = SelectedHeatmapPoint {
        x_value: 1.5,
        y_value: 2.5,
        hits: 100,
    };

    let cloned = point.clone();

    assert_eq!(cloned.x_value, 1.5);
    assert_eq!(cloned.y_value, 2.5);
    assert_eq!(cloned.hits, 100);
}

// ============================================
// Tab Tests
// ============================================

#[test]
fn test_tab_new() {
    let tab = Tab::new(0, "test.csv".to_string());

    assert_eq!(tab.file_index, 0);
    assert_eq!(tab.name, "test.csv");
    assert!(tab.selected_channels.is_empty());
    assert!(tab.channel_search.is_empty());
    assert!(tab.cursor_time.is_none());
    assert!(tab.cursor_record.is_none());
    assert!(!tab.chart_interacted);
    assert!(tab.time_range.is_none());
    assert!(tab.jump_to_time.is_none());
}

#[test]
fn test_tab_scatter_plot_initialization() {
    let tab = Tab::new(5, "test.csv".to_string());

    // Scatter plot state should be initialized with this tab's file index
    assert_eq!(tab.scatter_plot_state.left.file_index, Some(5));
    assert_eq!(tab.scatter_plot_state.right.file_index, Some(5));
}

#[test]
fn test_tab_clone() {
    let mut tab = Tab::new(0, "test.csv".to_string());
    tab.cursor_time = Some(10.5);
    tab.chart_interacted = true;

    let cloned = tab.clone();

    assert_eq!(cloned.file_index, 0);
    assert_eq!(cloned.cursor_time, Some(10.5));
    assert!(cloned.chart_interacted);
}

// ============================================
// SelectedChannel Tests
// ============================================

#[test]
fn test_selected_channel_clone() {
    let channel = Channel::Haltech(HaltechChannel {
        name: "Engine Speed".to_string(),
        id: "0".to_string(),
        r#type: ChannelType::EngineSpeed,
        display_min: Some(0.0),
        display_max: Some(10000.0),
    });

    let selected = SelectedChannel {
        file_index: 0,
        channel_index: 1,
        channel: channel.clone(),
        color_index: 2,
    };

    let cloned = selected.clone();

    assert_eq!(cloned.file_index, 0);
    assert_eq!(cloned.channel_index, 1);
    assert_eq!(cloned.color_index, 2);
}

// ============================================
// LoadedFile Tests
// ============================================

fn create_test_log() -> Log {
    Log {
        meta: ultralog::parsers::types::Meta::Empty,
        channels: vec![
            Channel::Haltech(HaltechChannel {
                name: "Engine Speed".to_string(),
                id: "0".to_string(),
                r#type: ChannelType::EngineSpeed,
                display_min: Some(0.0),
                display_max: Some(10000.0),
            }),
            Channel::Haltech(HaltechChannel {
                name: "TPS".to_string(),
                id: "1".to_string(),
                r#type: ChannelType::Percentage,
                display_min: Some(0.0),
                display_max: Some(100.0),
            }),
        ],
        times: vec![0.0, 0.1, 0.2],
        data: vec![
            vec![Value::Float(5000.0), Value::Float(50.0)],
            vec![Value::Float(5100.0), Value::Float(0.0)],
            vec![Value::Float(0.0), Value::Float(0.0)],
        ],
    }
}

#[test]
fn test_loaded_file_new() {
    let log = create_test_log();
    let file = LoadedFile::new(
        PathBuf::from("/test/path.csv"),
        "path.csv".to_string(),
        EcuType::Haltech,
        log,
    );

    assert_eq!(file.path, PathBuf::from("/test/path.csv"));
    assert_eq!(file.name, "path.csv");
    assert!(matches!(file.ecu_type, EcuType::Haltech));
    assert_eq!(file.log.channels.len(), 2);
}

#[test]
fn test_loaded_file_channels_with_data() {
    let log = create_test_log();
    let file = LoadedFile::new(
        PathBuf::from("/test/path.csv"),
        "path.csv".to_string(),
        EcuType::Haltech,
        log,
    );

    // First channel has non-zero values (5000, 5100)
    assert!(file.channels_with_data[0]);

    // Second channel has some non-zero values (50.0 in first row)
    assert!(file.channels_with_data[1]);
}

#[test]
fn test_loaded_file_channel_has_data() {
    let log = create_test_log();
    let file = LoadedFile::new(
        PathBuf::from("/test/path.csv"),
        "path.csv".to_string(),
        EcuType::Haltech,
        log,
    );

    assert!(file.channel_has_data(0));
    assert!(file.channel_has_data(1));

    // Out of bounds
    assert!(!file.channel_has_data(999));
}

#[test]
fn test_loaded_file_all_zero_channel() {
    let log = Log {
        meta: ultralog::parsers::types::Meta::Empty,
        channels: vec![Channel::Haltech(HaltechChannel {
            name: "Zero Channel".to_string(),
            id: "0".to_string(),
            r#type: ChannelType::Raw,
            display_min: Some(0.0),
            display_max: Some(100.0),
        })],
        times: vec![0.0, 0.1, 0.2],
        data: vec![
            vec![Value::Float(0.0)],
            vec![Value::Float(0.0)],
            vec![Value::Float(0.0)],
        ],
    };

    let file = LoadedFile::new(
        PathBuf::from("/test/path.csv"),
        "path.csv".to_string(),
        EcuType::Haltech,
        log,
    );

    // All-zero channel should be marked as having no data
    assert!(!file.channel_has_data(0));
}

#[test]
fn test_loaded_file_near_zero_channel() {
    // Values very close to zero should be considered as no data
    let log = Log {
        meta: ultralog::parsers::types::Meta::Empty,
        channels: vec![Channel::Haltech(HaltechChannel {
            name: "Near Zero".to_string(),
            id: "0".to_string(),
            r#type: ChannelType::Raw,
            display_min: Some(0.0),
            display_max: Some(100.0),
        })],
        times: vec![0.0, 0.1],
        data: vec![
            vec![Value::Float(0.00001)], // Below threshold
            vec![Value::Float(0.00002)], // Below threshold
        ],
    };

    let file = LoadedFile::new(
        PathBuf::from("/test/path.csv"),
        "path.csv".to_string(),
        EcuType::Haltech,
        log,
    );

    // Values below 0.0001 threshold should be considered as no data
    assert!(!file.channel_has_data(0));
}

#[test]
fn test_loaded_file_clone() {
    let log = create_test_log();
    let file = LoadedFile::new(
        PathBuf::from("/test/path.csv"),
        "path.csv".to_string(),
        EcuType::Haltech,
        log,
    );

    let cloned = file.clone();

    assert_eq!(cloned.path, file.path);
    assert_eq!(cloned.name, file.name);
    assert_eq!(cloned.channels_with_data, file.channels_with_data);
}
