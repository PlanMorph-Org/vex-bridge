#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    if let Err(error) = vex_bridge::desktop::run() {
        vex_bridge::desktop::show_startup_error(&error.to_string());
        eprintln!("vex-desktop: {error:#}");
        std::process::exit(1);
    }
}
