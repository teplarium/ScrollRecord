//! Binary entry point of the desktop app. All logic lives in `lib.rs`.

// Release builds on Windows must not open an extra console window next to the
// app. Debug builds keep the console so developers can see logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    sr_desktop_lib::run();
}
