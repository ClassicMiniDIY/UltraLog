// Prevents additional console window on Windows in release, DO NOT REMOVE!!

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;

mod parsers;

use parsers::types::Parser;

// Prevents additional console window on Windows in release, DO NOT REMOVE!!

fn main() {
    tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![add_file, greet])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn add_file(file_path: String) -> String {
  let contents = fs::read_to_string(&file_path)
    .expect("Error parsing file");

  // Parse with haltech
  let haltech = parsers::haltech::Haltech {};
  let log = haltech.parse(&contents)
    .expect("Error parsing Haltech file");

  serde_json::to_string(&log.channels).unwrap()
}