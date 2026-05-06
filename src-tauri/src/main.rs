// Prevents an extra console window on Windows in release.
// Debug builds keep the console so developers can see logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    klipo_lib::run()
}
