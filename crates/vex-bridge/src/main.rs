use std::process::ExitCode;

fn main() -> ExitCode {
    // Install first: every subsequent line (arg parsing, config/paths
    // discovery, the daemon loop itself) is a place a panic could originate,
    // and we want a redacted crash report even for those.
    vex_bridge::crash_report::install();
    if let Err(err) = vex_bridge::cli::run() {
        eprintln!("vex-bridge: {err:#}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
