#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] 
// hide console window on Windows in release

mod app;
mod interface;
mod song;
mod command;

fn main() {
    app::init()
}
