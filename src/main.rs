#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod interface;
mod song;

fn main() {
    app::init()
}
