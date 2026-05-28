mod app;
mod bt;
mod input;
mod ui;

use std::io;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    for arg in &args {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("bt-tui — Bluetooth device manager TUI");
                println!();
                println!("KEYS:");
                println!("  ↑↓/jk   Navigate         ⏎  Connect/disconnect");
                println!("  s        Toggle scan       D  Toggle discoverable");
                println!("  p        Pair/unpair       t  Trust/untrust");
                println!("  r        Remove device     i  Detail overlay");
                println!("  /        Filter by name    h  Help");
                println!("  q        Quit");
                return Ok(());
            }
            other => {
                eprintln!("Unknown option: {other} (try --help)");
                std::process::exit(1);
            }
        }
    }

    let mut app = app::App::new();
    app.initial_load();

    net_tui_core::runtime::run(&mut app)
}
