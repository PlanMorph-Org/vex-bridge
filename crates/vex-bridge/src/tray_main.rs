fn main() {
    if let Err(error) = vex_bridge::tray::run() {
        eprintln!("vex-tray: {error:#}");
        std::process::exit(1);
    }
}
