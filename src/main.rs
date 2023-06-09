#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// hide console window on Windows in release

mod app;
mod command;
mod interface;
mod song;

fn main() {
    app::init()
}
