use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use net_tui_core::filter;
use net_tui_core::hotbar::find_clicked_item;

use crate::app::{App, View};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    match &app.view {
        View::List if app.filtering => filter::handle_filter(app, key),
        View::List => handle_list(app, key),
        View::Detail => handle_overlay(app, key),
        View::Help => handle_overlay(app, key),
    }
}

fn handle_list(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true
        }
        KeyCode::Down | KeyCode::Char('j') => app.select_next(),
        KeyCode::Up | KeyCode::Char('k') => app.select_prev(),
        KeyCode::Home | KeyCode::Char('g') => app.select_first(),
        KeyCode::End | KeyCode::Char('G') => app.select_last(),
        KeyCode::Enter | KeyCode::Char('c') => app.connect_selected(),
        KeyCode::Char('p') => app.pair_selected(),
        KeyCode::Char('t') => app.trust_selected(),
        KeyCode::Char('r') => app.remove_selected(),
        KeyCode::Char('s') => app.toggle_scan(),
        KeyCode::Char('P') => app.toggle_power(),
        KeyCode::Char('D') => app.toggle_discoverable(),
        KeyCode::Char('i') => {
            if app.selected_device().is_some() {
                app.view = View::Detail;
            }
        }
        KeyCode::Char('h') => {
            app.view = View::Help;
        }
        KeyCode::Char('/') | KeyCode::Char('f') => {
            app.filtering = true;
            app.filter.clear();
        }
        KeyCode::Esc => {
            if !app.filter.is_empty() {
                app.filter.clear();
                app.rebuild();
            }
        }
        _ => {}
    }
}

pub fn handle_hotbar_click(app: &mut App, x: u16) {
    let keys: Vec<&str> = match &app.view {
        View::List if app.filtering => vec!["Esc", "Enter"],
        View::List => vec!["c", "p", "t", "P", "s", "D", "i", "r", "f", "h", "q"],
        View::Detail => vec!["Esc", "⏎"],
        View::Help => vec!["↑↓", "Esc"],
    };
    let descs: Vec<&str> = match &app.view {
        View::List if app.filtering => vec!["Cancel", "Apply"],
        View::List => {
            let cl = if app.selected_device().map(|d| d.connected).unwrap_or(false) { "Disconnect" } else { "Connect" };
            let pl = if app.selected_device().map(|d| d.paired).unwrap_or(false) { "Unpair" } else { "Pair" };
            let tl = if app.selected_device().map(|d| d.trusted).unwrap_or(false) { "Untrust" } else { "Trust" };
            vec![cl, pl, tl, if app.controller.powered { "Power OFF" } else { "Power ON" },
                 if app.scanning { "Scan OFF" } else { "Scan" },
                 if app.controller.discoverable { "Hide" } else { "Discoverable" },
                 "Detail", "Remove", "Filter", "Help", "Quit"]
        }
        View::Detail => vec!["Back", "Connect"],
        View::Help => vec!["Scroll", "Back"],
    };

    let items: Vec<(&str, &str)> = keys.iter().copied().zip(descs.iter().copied()).collect();
    if let Some(i) = find_clicked_item(&items, x) {
        dispatch_hotbar(app, i);
    }
}

fn dispatch_hotbar(app: &mut App, index: usize) {
    match &app.view {
        View::List if app.filtering => match index {
            0 => { app.filtering = false; app.filter.clear(); app.rebuild(); }
            1 => { app.filtering = false; }
            _ => {}
        },
        View::List => match index {
            0 => app.connect_selected(),
            1 => app.pair_selected(),
            2 => app.trust_selected(),
            3 => app.toggle_power(),
            4 => app.toggle_scan(),
            5 => app.toggle_discoverable(),
            6 => { if app.selected_device().is_some() { app.view = View::Detail; } }
            7 => app.remove_selected(),
            8 => { app.filtering = true; app.filter.clear(); }
            9 => { app.view = View::Help; }
            10 => { app.should_quit = true; }
            _ => {}
        },
        View::Detail => match index {
            0 => { app.help_scroll = 0; app.view = View::List; }
            1 => { app.view = View::List; app.connect_selected(); }
            _ => {}
        },
        View::Help => match index {
            // 0 = Scroll (no-op)
            1 => { app.help_scroll = 0; app.view = View::List; }
            _ => {}
        },
    }
}

fn handle_overlay(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('i') | KeyCode::Char('h') => {
            app.help_scroll = 0;
            app.view = View::List;
        }
        KeyCode::Enter => {
            app.help_scroll = 0;
            app.view = View::List;
            app.connect_selected();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if matches!(app.view, View::Help) {
                app.help_scroll = app.help_scroll.saturating_add(1);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if matches!(app.view, View::Help) {
                app.help_scroll = app.help_scroll.saturating_sub(1);
            }
        }
        _ => {}
    }
}
