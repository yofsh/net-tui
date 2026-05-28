mod app;
mod input;
mod ui;
mod wifi;

use std::io;

use app::{SortMode, ViewMode};

fn main() -> io::Result<()> {
    let mut sort = SortMode::Signal;
    let mut view = ViewMode::Grouped;
    let mut auto_scan = false;

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
            "--auto" | "-a" => auto_scan = true,
            "--help" | "-h" => {
                println!("wifi-tui — WiFi network manager TUI");
                println!();
                println!("OPTIONS:");
                println!("  -s, --sort <mode>    Sort: signal (default), name");
                println!("  -v, --view <mode>    View: grouped (default), flat");
                println!("  -a, --auto           Enable auto-scan (2s interval)");
                println!("  -h, --help           Show this help");
                println!();
                println!("KEYS:");
                println!("  ↑↓/jk   Navigate         ⏎  Connect to selected");
                println!("  s        Scan             S  Toggle auto-scan");
                println!("  o        Cycle sort       v  Toggle view mode");
                println!("  d        Detail overlay   D  Disconnect");
                println!("  i        Connection info  h  Help overlay");
                println!("  /        Filter by SSID   q  Quit");
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
