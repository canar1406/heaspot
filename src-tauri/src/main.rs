// Ẩn console window trên bản release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    winspot_lib::run()
}
