#![cfg_attr(windows, windows_subsystem = "windows")]

use std::panic::{self, AssertUnwindSafe};

fn main() {
    // Install first, before daemon supervision or tray/menu creation can
    // panic, so every failure mode below is captured in a redacted report.
    vex_bridge::crash_report::install();
    match panic::catch_unwind(AssertUnwindSafe(vex_bridge::tray::run)) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            eprintln!("vex-tray: {error:#}");
            std::process::exit(1);
        }
        Err(_) => {
            // A panic unwound out of the tray's event loop. The panic hook
            // installed above already wrote a redacted crash report; the
            // tray runs windowless with no console, so surface a clear
            // native notification rather than letting it silently vanish.
            vex_bridge::tray::notify_crash(vex_bridge::crash_report::last_report_path());
            std::process::exit(1);
        }
    }
}
