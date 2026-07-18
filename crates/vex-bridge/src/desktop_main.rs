#![cfg_attr(windows, windows_subsystem = "windows")]

use std::panic::{self, AssertUnwindSafe};

fn main() {
    // Install first, before daemon supervision or window/webview creation can
    // panic, so every failure mode below is captured in a redacted report.
    vex_bridge::crash_report::install();
    match panic::catch_unwind(AssertUnwindSafe(vex_bridge::desktop::run)) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            vex_bridge::desktop::show_startup_error(&error.to_string());
            eprintln!("vex-desktop: {error:#}");
            std::process::exit(1);
        }
        Err(_) => {
            // A panic unwound out of the desktop event loop. The panic hook
            // installed above already wrote a redacted crash report; surface
            // a clear native dialog rather than letting the windowed process
            // silently disappear.
            vex_bridge::desktop::show_crash_dialog(vex_bridge::crash_report::last_report_path());
            std::process::exit(1);
        }
    }
}
