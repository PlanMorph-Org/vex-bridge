use std::process::ExitCode;

fn main() -> ExitCode {
    if let Err(err) = vex_bridge::cli::run() {
        eprintln!("vex-bridge: {err:#}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
