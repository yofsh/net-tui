use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Clear, Paragraph, Row, Table, Wrap},
    Frame,
};

use net_tui_core::hotbar::hotkey_spans;
use net_tui_core::overlay::{centered_rect, detail_row, draw_separator, truncate};
use net_tui_core::theme::{CYAN, GRAY, GREEN, MAGENTA, RED, WHITE, YELLOW};

use crate::app::{App, View};
use crate::bt;

struct Columns {
    widths: Vec<Constraint>,
    show_type: bool,
    show_address: bool,
    show_signal: bool,
    show_battery: bool,
    show_transport: bool,
    name_width: u16,
}

fn compute_columns(app: &App, width: u16) -> Columns {
    let max_name = app
        .filtered
        .iter()
        .filter_map(|&i| app.devices.get(i))
        .map(|d| d.name.len() as u16)
        .max()
        .unwrap_or(6)
        .clamp(8, 40);

    // fixed columns: icon(2) + status(10) = 12 always shown
    // progressive: name, type(12), address(18), signal(8), battery(7), transport(7)
    // total at full: 2 + name + 12 + 18 + 10 + 8 + 7 + 7 = 64 + name
    let avail = width.saturating_sub(4); // account for highlight symbol + margin

    let show_transport = avail >= 64 + max_name;
    let show_battery = avail >= 57 + max_name;
    let show_signal = avail >= 50 + max_name;
    let show_address = avail >= 42 + max_name;
    let show_type = avail >= 24 + max_name;

    // shrink name if still too tight
    let fixed: u16 = 2 + 10
        + if show_type { 12 } else { 0 }
        + if show_address { 18 } else { 0 }
        + if show_signal { 8 } else { 0 }
        + if show_battery { 7 } else { 0 }
        + if show_transport { 7 } else { 0 };
    let name_width = max_name.min(avail.saturating_sub(fixed)).max(6);

    let mut widths = vec![
        Constraint::Length(2),          // icon
        Constraint::Length(name_width), // name
    ];
    if show_type { widths.push(Constraint::Length(12)); }
    if show_address { widths.push(Constraint::Length(18)); }
    widths.push(Constraint::Length(10)); // status — always
    if show_signal { widths.push(Constraint::Length(8)); }
    if show_battery { widths.push(Constraint::Length(7)); }
    if show_transport { widths.push(Constraint::Length(7)); }

    Columns { widths, show_type, show_address, show_signal, show_battery, show_transport, name_width }
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // status
            Constraint::Length(1), // separator
            Constraint::Min(3),   // table
            Constraint::Length(1), // separator
            Constraint::Length(1), // hotkeys
        ])
        .split(f.area());

    draw_status_line(f, app, chunks[0]);
    draw_separator(f, chunks[1]);
    draw_device_table(f, app, chunks[2]);
    draw_separator(f, chunks[3]);
    draw_hotkey_bar(f, app, chunks[4]);

    match &app.view {
        View::Detail => draw_detail_overlay(f, app),
        View::Help => draw_help_overlay(f, app),
        View::List => {}
    }
}

fn draw_status_line(f: &mut Frame, app: &App, area: Rect) {
    let mut left: Vec<Span> = vec![Span::styled(
        " BT ",
        Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
    )];

    let ctrl = &app.controller;
    if ctrl.powered {
        left.push(Span::styled(
            &ctrl.name,
            Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
        ));
        left.push(Span::raw("  "));

        let connected: Vec<&bt::Device> = app.devices.iter().filter(|d| d.connected).collect();
        if connected.is_empty() {
            left.push(Span::styled("No connections", Style::default().fg(GRAY)));
        } else {
            for (i, dev) in connected.iter().enumerate() {
                if i > 0 {
                    left.push(Span::styled(", ", Style::default().fg(GRAY)));
                }
                left.push(Span::styled(
                    bt::device_type_icon(&dev.icon),
                    Style::default(),
                ));
                left.push(Span::styled(
                    &dev.name,
                    Style::default().fg(GREEN),
                ));
                if let Some(bat) = dev.battery {
                    left.push(Span::styled(
                        format!(" {bat}%"),
                        Style::default().fg(battery_color(bat)),
                    ));
                }
            }
        }

        if let Some(msg) = app.status.current() {
            left.push(Span::raw("  "));
            left.push(Span::styled(format!("[{msg}]"), Style::default().fg(YELLOW)));
        }
    } else {
        left.push(Span::styled(
            "Bluetooth OFF",
            Style::default().fg(RED).add_modifier(Modifier::BOLD),
        ));
    }

    // Build right-side badges
    let badge = |label: &str, fg: Color, bg: Color| -> Vec<Span<'static>> {
        vec![
            Span::styled(
                format!(" {label} "),
                Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]
    };

    let mut right: Vec<Span> = Vec::new();
    if ctrl.powered {
        right.extend(badge("Powered", Color::Black, GREEN));
    } else {
        right.extend(badge("Off", WHITE, RED));
    }
    if app.scanning {
        right.extend(badge("Scanning", Color::Black, YELLOW));
    }
    if ctrl.discoverable {
        right.extend(badge("Discoverable", Color::Black, MAGENTA));
    }
    if ctrl.pairable {
        right.extend(badge("Pairable", Color::Black, Color::Rgb(180, 142, 255)));
    }

    // Pad between left and right so badges sit at the right edge
    let left_width: usize = left.iter().map(|s| s.content.chars().count()).sum();
    let right_width: usize = right.iter().map(|s| s.content.chars().count()).sum();
    let pad = (area.width as usize).saturating_sub(left_width + right_width);
    left.push(Span::raw(" ".repeat(pad)));
    left.extend(right);

    f.render_widget(Paragraph::new(Line::from(left)), area);
}

fn draw_device_table(f: &mut Frame, app: &mut App, area: Rect) {
    let cols = compute_columns(app, area.width);
    let name_width = cols.name_width as usize;

    let rows: Vec<Row> = app
        .filtered
        .iter()
        .map(|&idx| {
            let dev = &app.devices[idx];

            let icon = if dev.connected {
                Span::styled("●", Style::default().fg(GREEN))
            } else if dev.paired {
                Span::styled("○", Style::default().fg(CYAN))
            } else {
                Span::styled("·", Style::default().fg(GRAY))
            };

            let name_style = if dev.connected {
                Style::default().fg(GREEN).add_modifier(Modifier::BOLD)
            } else if dev.paired {
                Style::default().fg(WHITE)
            } else {
                Style::default().fg(GRAY)
            };

            let mut status_spans = Vec::new();
            if dev.connected {
                status_spans.push(Span::styled("C", Style::default().fg(GREEN)));
            }
            if dev.paired {
                status_spans.push(Span::styled("P", Style::default().fg(CYAN)));
            }
            if dev.trusted {
                status_spans.push(Span::styled("T", Style::default().fg(YELLOW)));
            }
            if dev.blocked {
                status_spans.push(Span::styled("B", Style::default().fg(RED)));
            }
            if status_spans.is_empty() {
                status_spans.push(Span::styled("─", Style::default().fg(GRAY)));
            } else {
                let len = status_spans.len();
                let mut interspersed = Vec::with_capacity(len * 2 - 1);
                for (i, s) in status_spans.into_iter().enumerate() {
                    if i > 0 {
                        interspersed.push(Span::styled(" ", Style::default()));
                    }
                    interspersed.push(s);
                }
                status_spans = interspersed;
            }

            let mut cells = vec![
                Cell::from(icon),
                Cell::from(Span::styled(truncate(&dev.name, name_width), name_style)),
            ];
            if cols.show_type {
                let type_label = bt::device_type_label(&dev.icon);
                let type_icon = bt::device_type_icon(&dev.icon);
                cells.push(Cell::from(Line::from(vec![
                    Span::raw(format!("{type_icon} ")),
                    Span::styled(type_label, Style::default().fg(GRAY)),
                ])));
            }
            if cols.show_address {
                cells.push(Cell::from(Span::styled(&dev.address, Style::default().fg(GRAY))));
            }
            cells.push(Cell::from(Line::from(status_spans)));
            if cols.show_signal {
                cells.push(match dev.rssi {
                    Some(rssi) => {
                        let (bar, color) = rssi_bar(rssi);
                        Cell::from(Line::from(vec![
                            Span::styled(bar, Style::default().fg(color)),
                            Span::styled(format!(" {rssi}"), Style::default().fg(GRAY)),
                        ]))
                    }
                    None => Cell::from(Span::styled("─", Style::default().fg(GRAY))),
                });
            }
            if cols.show_battery {
                cells.push(match dev.battery {
                    Some(pct) => Cell::from(Span::styled(
                        format!("{pct}%"),
                        Style::default().fg(battery_color(pct)),
                    )),
                    None => Cell::from(Span::styled("─", Style::default().fg(GRAY))),
                });
            }
            if cols.show_transport {
                let transport_color = match dev.transport.as_str() {
                    "LE" => MAGENTA,
                    "dual" => CYAN,
                    _ => GRAY,
                };
                cells.push(Cell::from(Span::styled(
                    dev.transport.as_str(),
                    Style::default().fg(transport_color),
                )));
            }

            Row::new(cells)
        })
        .collect();

    let title_line = if app.filtering {
        format!(" /{}█", app.filter)
    } else if !app.filter.is_empty() {
        format!(" [{}]", app.filter)
    } else {
        String::new()
    };

    let mut hdr_cells = vec![
        Cell::from(""),
        Cell::from(Span::styled("Device", Style::default().fg(GRAY))),
    ];
    if cols.show_type { hdr_cells.push(Cell::from(Span::styled("Type", Style::default().fg(GRAY)))); }
    if cols.show_address { hdr_cells.push(Cell::from(Span::styled("Address", Style::default().fg(GRAY)))); }
    hdr_cells.push(Cell::from(Span::styled("Status", Style::default().fg(GRAY))));
    if cols.show_signal { hdr_cells.push(Cell::from(Span::styled("RSSI", Style::default().fg(GRAY)))); }
    if cols.show_battery { hdr_cells.push(Cell::from(Span::styled("Bat", Style::default().fg(GRAY)))); }
    if cols.show_transport {
        hdr_cells.push(Cell::from(Span::styled(
            format!("Trans{title_line}"),
            Style::default().fg(GRAY),
        )));
    } else {
        // put filter indicator on last visible column
        if !title_line.is_empty() {
            if let Some(last) = hdr_cells.last_mut() {
                *last = Cell::from(Span::styled(title_line, Style::default().fg(GRAY)));
            }
        }
    }
    let header = Row::new(hdr_cells).height(1);

    let table = Table::new(rows, &cols.widths)
        .header(header)
        .row_highlight_style(
            Style::default()
                .bg(Color::Rgb(35, 35, 45))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    f.render_stateful_widget(table, area, &mut app.table_state);
}

fn draw_hotkey_bar(f: &mut Frame, app: &App, area: Rect) {
    let power_label = if app.controller.powered { "Power OFF" } else { "Power ON" };
    let scan_label = if app.scanning { "Scan OFF" } else { "Scan" };
    let disc_label = if app.controller.discoverable { "Hide" } else { "Discoverable" };

    let hotkeys: Vec<(&str, &str)> = match &app.view {
        View::List if app.filtering => vec![
            ("Esc", "Cancel"),
            ("Enter", "Apply"),
            ("", "Type to filter..."),
        ],
        View::List => {
            let connect_label = app
                .selected_device()
                .map(|d| if d.connected { "Disconnect" } else { "Connect" })
                .unwrap_or("Connect");
            let pair_label = app
                .selected_device()
                .map(|d| if d.paired { "Unpair" } else { "Pair" })
                .unwrap_or("Pair");
            let trust_label = app
                .selected_device()
                .map(|d| if d.trusted { "Untrust" } else { "Trust" })
                .unwrap_or("Trust");

            vec![
                ("c", connect_label),
                ("p", pair_label),
                ("t", trust_label),
                ("P", power_label),
                ("s", scan_label),
                ("D", disc_label),
                ("i", "Detail"),
                ("r", "Remove"),
                ("f", "Filter"),
                ("h", "Help"),
                ("q", "Quit"),
            ]
        }
        View::Detail => vec![("Esc", "Back"), ("⏎", "Connect")],
        View::Help => vec![("↑↓", "Scroll"), ("Esc", "Back")],
    };

    let spans: Vec<Span> = hotkeys
        .iter()
        .flat_map(|(key, desc)| hotkey_spans(key, desc))
        .collect();

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_detail_overlay(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 80, f.area());
    f.render_widget(Clear, area);

    let Some(dev) = app.selected_device() else {
        return;
    };

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    detail_row(&mut lines, "Name", &dev.name, WHITE);
    detail_row(&mut lines, "Address", &dev.address, WHITE);
    detail_row(
        &mut lines,
        "Type",
        &format!("{} {}", bt::device_type_icon(&dev.icon), bt::device_type_label(&dev.icon)),
        WHITE,
    );
    detail_row(
        &mut lines,
        "Transport",
        &dev.transport,
        match dev.transport.as_str() {
            "LE" => MAGENTA,
            "dual" => CYAN,
            _ => WHITE,
        },
    );

    let vendor = bt::vendor_from_modalias(&dev.modalias);
    if !vendor.is_empty() {
        detail_row(&mut lines, "Vendor", vendor, GRAY);
    }
    if !dev.modalias.is_empty() {
        detail_row(&mut lines, "Modalias", &dev.modalias, GRAY);
    }

    lines.push(Line::from(""));

    let connected_color = if dev.connected { GREEN } else { GRAY };
    detail_row(&mut lines, "Connected", if dev.connected { "Yes" } else { "No" }, connected_color);
    detail_row(&mut lines, "Paired", if dev.paired { "Yes" } else { "No" }, if dev.paired { CYAN } else { GRAY });
    detail_row(&mut lines, "Bonded", if dev.bonded { "Yes" } else { "No" }, if dev.bonded { CYAN } else { GRAY });
    detail_row(&mut lines, "Trusted", if dev.trusted { "Yes" } else { "No" }, if dev.trusted { YELLOW } else { GRAY });
    if dev.blocked {
        detail_row(&mut lines, "Blocked", "Yes", RED);
    }

    lines.push(Line::from(""));

    if let Some(rssi) = dev.rssi {
        let (bar, color) = rssi_bar(rssi);
        lines.push(Line::from(vec![
            Span::styled("  RSSI        ", Style::default().fg(GRAY)),
            Span::styled(bar, Style::default().fg(color)),
            Span::styled(
                format!(" {} dBm  ", rssi),
                Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
            ),
            Span::styled(rssi_quality(rssi), Style::default().fg(color)),
        ]));
    }

    if let Some(bat) = dev.battery {
        let bat_bar = battery_bar(bat);
        let bat_color = battery_color(bat);
        lines.push(Line::from(vec![
            Span::styled("  Battery     ", Style::default().fg(GRAY)),
            Span::styled(bat_bar, Style::default().fg(bat_color)),
            Span::styled(
                format!(" {bat}%"),
                Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    if !dev.uuids.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Services",
            Style::default().fg(GRAY).add_modifier(Modifier::BOLD),
        )));
        let mut service_names: Vec<String> = dev
            .uuids
            .iter()
            .filter_map(|uuid| {
                let name = uuid.split('(').next().unwrap_or(uuid).trim().trim_end_matches("..");
                if name.starts_with("Vendor") { None } else { Some(name.to_string()) }
            })
            .collect();
        service_names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        for name in service_names {
            lines.push(Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(name, Style::default().fg(GRAY)),
            ]));
        }
    }

    let title = format!(" {} ", dev.name);
    let border_block = ratatui::widgets::Block::default()
        .title(title)
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(Style::default().fg(CYAN));
    let para = Paragraph::new(lines)
        .block(border_block)
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

fn draw_help_overlay(f: &mut Frame, app: &App) {
    let area = centered_rect(65, 85, f.area());
    f.render_widget(Clear, area);

    let b = Style::default().fg(CYAN).add_modifier(Modifier::BOLD);
    let h = Style::default().fg(WHITE).add_modifier(Modifier::BOLD);
    let d = Style::default().fg(GRAY);
    let g = Style::default().fg(GREEN);
    let y = Style::default().fg(YELLOW);
    let m = Style::default().fg(MAGENTA);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled("  Keybindings", h)),
        Line::from(""),
        Line::from(vec![Span::styled("  ↑↓ / j k    ", b), Span::styled("Navigate device list", d)]),
        Line::from(vec![Span::styled("  g / G       ", b), Span::styled("Jump to first / last", d)]),
        Line::from(vec![Span::styled("  ⏎ / c       ", b), Span::styled("Connect (or disconnect if connected)", d)]),
        Line::from(vec![Span::styled("  p           ", b), Span::styled("Pair (or unpair if paired)", d)]),
        Line::from(vec![Span::styled("  t           ", b), Span::styled("Trust (or untrust if trusted)", d)]),
        Line::from(vec![Span::styled("  r           ", b), Span::styled("Remove device (unpair + forget)", d)]),
        Line::from(vec![Span::styled("  P           ", b), Span::styled("Toggle Bluetooth power on/off", d)]),
        Line::from(vec![Span::styled("  s           ", b), Span::styled("Toggle scan on/off (discover nearby devices)", d)]),
        Line::from(vec![Span::styled("  D           ", b), Span::styled("Toggle discoverable (visible to other devices)", d)]),
        Line::from(vec![Span::styled("  i           ", b), Span::styled("Detail overlay for selected device", d)]),
        Line::from(vec![Span::styled("  /           ", b), Span::styled("Filter devices by name", d)]),
        Line::from(vec![Span::styled("  h           ", b), Span::styled("This help screen", d)]),
        Line::from(vec![Span::styled("  q / Ctrl+C  ", b), Span::styled("Quit", d)]),
        Line::from(""),
        Line::from(Span::styled("  Status Column", h)),
        Line::from(""),
        Line::from(vec![Span::styled("  C ", g), Span::styled("Connected — active link to this device", d)]),
        Line::from(vec![Span::styled("  P ", Style::default().fg(CYAN)), Span::styled("Paired — keys exchanged, can reconnect without re-pairing", d)]),
        Line::from(vec![Span::styled("  T ", y), Span::styled("Trusted — auto-connect allowed, no confirmation needed", d)]),
        Line::from(vec![Span::styled("  B ", Style::default().fg(RED)), Span::styled("Blocked — device is rejected", d)]),
        Line::from(""),
        Line::from(Span::styled("  Device Icons", h)),
        Line::from(""),
        Line::from(vec![Span::styled("  ● ", g), Span::styled("Connected    ", d), Span::styled("○ ", Style::default().fg(CYAN)), Span::styled("Paired    ", d), Span::styled("· ", Style::default().fg(GRAY)), Span::styled("Discovered only", d)]),
        Line::from(""),
        Line::from(Span::styled("  Transport Types", h)),
        Line::from(""),
        Line::from(vec![Span::styled("  BR/EDR  ", d), Span::styled("Classic Bluetooth (audio, file transfer, HID)", d)]),
        Line::from(vec![Span::styled("  LE      ", m), Span::styled("Bluetooth Low Energy (sensors, beacons, IoT)", d)]),
        Line::from(vec![Span::styled("  dual    ", Style::default().fg(CYAN)), Span::styled("Supports both BR/EDR and LE", d)]),
        Line::from(""),
        Line::from(Span::styled("  RSSI Signal Strength", h)),
        Line::from(""),
        Line::from(vec![Span::styled("  ████ ", g), Span::styled("≥ -50 dBm Excellent   ", d), Span::styled("███░ ", g), Span::styled("≥ -60 dBm Good", d)]),
        Line::from(vec![Span::styled("  ██░░ ", y), Span::styled("≥ -70 dBm Fair        ", d), Span::styled("█░░░ ", Style::default().fg(RED)), Span::styled("≥ -80 dBm Weak", d)]),
        Line::from(""),
    ];

    let border_block = ratatui::widgets::Block::default()
        .title(" Help (↑↓ scroll, Esc close) ")
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(Style::default().fg(CYAN));
    let para = Paragraph::new(lines)
        .block(border_block)
        .wrap(Wrap { trim: false })
        .scroll((app.help_scroll, 0));
    f.render_widget(para, area);
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn rssi_bar(rssi: i32) -> (String, Color) {
    if rssi >= -50 {
        ("████".into(), GREEN)
    } else if rssi >= -60 {
        ("███░".into(), GREEN)
    } else if rssi >= -70 {
        ("██░░".into(), YELLOW)
    } else if rssi >= -80 {
        ("█░░░".into(), RED)
    } else {
        ("░░░░".into(), RED)
    }
}

fn rssi_quality(rssi: i32) -> &'static str {
    if rssi >= -50 {
        "Excellent"
    } else if rssi >= -60 {
        "Good"
    } else if rssi >= -70 {
        "Fair"
    } else if rssi >= -80 {
        "Weak"
    } else {
        "Poor"
    }
}

fn battery_color(pct: u8) -> Color {
    if pct >= 60 {
        GREEN
    } else if pct >= 20 {
        YELLOW
    } else {
        RED
    }
}

fn battery_bar(pct: u8) -> String {
    let filled = (pct as usize * 4) / 100;
    let empty = 4 - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

