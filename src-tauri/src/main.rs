// Prevents additional console window on Windows in release, DO NOT REMOVE!!

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::sync::Mutex;

use tauri::State;

mod parsers;

use parsers::types::{Channel, ChannelValue, Log, Parseable};

fn main() {
    tauri::Builder::default()
      .manage(Store::default())
      .invoke_handler(tauri::generate_handler![add_file, get_channel_data])
      .run(tauri::generate_context!())
      .expect("error while running tauri application");
}

#[derive(Default)]
struct Store {
  pub log: Mutex<Log>,
}

#[tauri::command]
fn add_file(file_path: String, store: State<Store>) -> Result<Vec<Channel>, String> {
  let contents = match fs::read_to_string(&file_path) {
    Ok(c) => c,
    Err(e) => {
      eprintln!("Error reading file: {}", e);
      return Err("Error reading file".to_string());
    }
  };

  // Parse with haltech
  let haltech = parsers::haltech::Haltech {};
  match haltech.parse(&contents) {
    Ok(log) => {
      *store.log.lock().unwrap() = log.clone();
      Ok(log.channels)
    },
    Err(e) => {
      eprintln!("Error parsing Haltech file: {}", e);
      Err("Error parsing file".to_string())
    }
  }
}

#[tauri::command]
fn get_channel_data(channel_name: String, store: State<Store>) -> Result<Vec<ChannelValue>, String> {
  let log = store.log.lock().unwrap();

  // Finds the channel index
  let channel_index = match log.channels.iter().position(|c| c.name() == channel_name) {
    Some(i) => i,
    None => {
      eprintln!("Channel not found: {}", channel_name);
      return Err("Channel not found".to_string());
    }
  };

  let data = log.data.iter().map(|d| d[channel_index].clone()).collect();
  Ok(data)
}