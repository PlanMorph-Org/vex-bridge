#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    if let Err(error) = vex_bridge::desktop::run() {
        eprintln!("vex-desktop: {error:#}");
        std::process::exit(1);
    }
}
