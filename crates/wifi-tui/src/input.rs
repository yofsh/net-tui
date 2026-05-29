use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use net_tui_core::filter;
use net_tui_core::hotbar::find_clicked_item_wrapped;

use crate::app::{App, View};
use crate::wifi;

/// Single source of truth for the bottom hotkey bar: both the renderer and the
/// click handler build from this, so labels and click targets never drift.
pub fn list_hotkeys(app: &App) -> Vec<net_tui_core::hotbar::Hotkey<'static>> {
    match &app.view {
        View::List if app.filtering => vec![
            ("Esc", "Cancel", false),
            ("Enter", "Apply", false),
            ("", "Type to filter...", false),
        ],
        View::List => vec![
            ("c", "Connect", false),
            ("p", "Power", wifi::is_wifi_on()),
            ("s", "Scan", app.auto_scan),
            ("t", "Toggle View", false),
            ("i", "Info", false),
            ("d", "Disconnect", false),
            ("f", "Filter", false),
            ("h", "Help", false),
            ("q", "Quit", false),
        ],
        View::ConnInfo => vec![("Esc", "Back", false)],
        View::Password => vec![
            ("⏎", "Submit", false),
            ("Tab", "Show/Hide", false),
            ("Esc", "Cancel", false),
        ],
        View::Help => vec![("↑↓", "Scroll", false), ("Esc", "Back", false)],
    }
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    match &app.view {
        View::List if app.filtering => filter::handle_filter(app, key),
        View::List => handle_list(app, key),
        View::ConnInfo => handle_detail(app, key),
        View::Help => handle_help(app, key),
        View::Password => handle_password(app, key),
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
        KeyCode::Enter | KeyCode::Char('c') => app.connect_to_selected(),
        KeyCode::Char('s') => app.toggle_scan(),
        KeyCode::Char('i') => {
            if app.connection.is_some() {
                app.view = View::ConnInfo;
            }
        }
        KeyCode::Char('p') => app.toggle_power(),
        KeyCode::Char('d') => app.disconnect(),
        KeyCode::Char('o') => app.cycle_sort(),
        KeyCode::Char('t') | KeyCode::Char('v') => app.toggle_view_mode(),
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

pub fn handle_hotbar_click(app: &mut App, width: u16, line: u16, x: u16) {
    let items = list_hotkeys(app);
    if let Some(i) = find_clicked_item_wrapped(&items, width, line, x) {
        dispatch_wifi_hotbar(app, i);
    }
}

fn dispatch_wifi_hotbar(app: &mut App, index: usize) {
    match &app.view {
        View::List if app.filtering => match index {
            0 => { app.filtering = false; app.filter.clear(); app.rebuild(); }
            1 => { app.filtering = false; }
            _ => {}
        },
        View::List => match index {
            0 => app.connect_to_selected(),
            1 => app.toggle_power(),
            2 => app.toggle_scan(),
            3 => app.toggle_view_mode(),
            4 => { if app.connection.is_some() { app.view = View::ConnInfo; } }
            5 => app.disconnect(),
            6 => { app.filtering = true; app.filter.clear(); }
            7 => { app.view = View::Help; }
            8 => { app.should_quit = true; }
            _ => {}
        },
        View::ConnInfo => match index {
            0 => { app.view = View::List; }
            1 => { app.view = View::List; app.connect_to_selected(); }
            _ => {}
        },
        View::Help => match index {
            1 => { app.help_scroll = 0; app.view = View::List; }
            _ => {}
        },
        View::Password => match index {
            0 => { if !app.password.is_empty() { app.submit_password(); } }
            1 => { app.password_visible = !app.password_visible; }
            2 => { app.view = View::List; app.password.clear(); }
            _ => {}
        },
    }
}

fn handle_detail(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('d') | KeyCode::Char('i') | KeyCode::Char('h') => {
            app.view = View::List;
        }
        KeyCode::Enter => {
            app.view = View::List;
            app.connect_to_selected();
        }
        _ => {}
    }
}

fn handle_help(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('h') => {
            app.help_scroll = 0;
            app.view = View::List;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.help_scroll = app.help_scroll.saturating_add(1);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.help_scroll = app.help_scroll.saturating_sub(1);
        }
        _ => {}
    }
}

fn handle_password(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.view = View::List;
            app.password.clear();
        }
        KeyCode::Enter => {
            if !app.password.is_empty() {
                app.submit_password();
            }
        }
        KeyCode::Backspace => {
            app.password.pop();
        }
        KeyCode::Tab => {
            app.password_visible = !app.password_visible;
        }
        KeyCode::Char(c) => {
            app.password.push(c);
        }
        _ => {}
    }
}
