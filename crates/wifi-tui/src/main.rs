mod app;
mod input;
mod ui;
mod wifi;

use std::io;

use app::{SortMode, ViewMode};

fn main() -> io::Result<()> {
    let mut sort = SortMode::Signal;
    // Flat (non-grouped) by default; `--view grouped` shows grouped on start.
    let mut view = ViewMode::Flat;
    // Continuous scan (rescan every ~2s) is on by default; --no-scan disables it.
    let mut auto_scan = true;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--sort" | "-s" => {
                i += 1;
                if i < args.len() {
                    match args[i].as_str() {
                        "name" | "n" => sort = SortMode::Name,
                        "signal" | "s" => sort = SortMode::Signal,
                        other => {
                            eprintln!("Unknown sort mode: {other} (use: name, signal)");
                            std::process::exit(1);
                        }
                    }
                }
            }
            "--view" | "-v" => {
                i += 1;
                if i < args.len() {
                    match args[i].as_str() {
                        "flat" | "f" => view = ViewMode::Flat,
                        "grouped" | "group" | "g" => view = ViewMode::Grouped,
                        other => {
                            eprintln!("Unknown view mode: {other} (use: flat, grouped)");
                            std::process::exit(1);
                        }
                    }
                }
            }
            "--no-scan" => auto_scan = false,
            "--help" | "-h" => {
                println!("wifi-tui — WiFi network manager TUI");
                println!();
                println!("OPTIONS:");
                println!("  -s, --sort <mode>    Sort: signal (default), name");
                println!("  -v, --view <mode>    View: flat (default), grouped");
                println!("      --no-scan        Don't auto-scan on start (off by default)");
                println!("  -h, --help           Show this help");
                println!();
                println!("KEYS:");
                println!("  ↑↓/jk   Navigate         ⏎  Connect to selected");
                println!("  s        Toggle scan      v  Toggle view mode");
                println!("  o        Cycle sort       p  Power on/off");
                println!("  i        Connection info  d  Disconnect");
                println!("  /        Filter by SSID   h  Help overlay");
                println!("  q        Quit");
                return Ok(());
            }
            other => {
                eprintln!("Unknown option: {other} (try --help)");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let mut app = app::App::new();
    app.sort_mode = sort;
    app.view_mode = view;
    app.auto_scan = auto_scan;
    app.initial_load();

    net_tui_core::runtime::run(&mut app)
}
