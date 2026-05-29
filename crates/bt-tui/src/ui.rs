use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Clear, Paragraph, Row, Table, Wrap},
    Frame,
};

use net_tui_core::hotbar::layout_hotkeys;
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

    // fixed columns: icon(2) + status flags(5) = 7 always shown (status sits
    // just left of the name now)
    // progressive: name, type(12), address(18), signal(8), battery(7), transport(7)
    // total at full: 2 + 5 + name + 12 + 18 + 8 + 7 + 7 = 59 + name
    let avail = width.saturating_sub(4); // account for highlight symbol + margin

    let show_transport = avail >= 59 + max_name;
    let show_battery = avail >= 52 + max_name;
    let show_signal = avail >= 45 + max_name;
    let show_address = avail >= 37 + max_name;
    let show_type = avail >= 19 + max_name;

    // shrink name if still too tight
    let fixed: u16 = 2 + 5
        + if show_type { 12 } else { 0 }
        + if show_address { 18 } else { 0 }
        + if show_signal { 8 } else { 0 }
        + if show_battery { 7 } else { 0 }
        + if show_transport { 7 } else { 0 };
    let name_width = max_name.min(avail.saturating_sub(fixed)).max(6);

    let mut widths = vec![
        Constraint::Length(2),          // icon
        Constraint::Length(5),          // status flags — always, left of name
        Constraint::Length(name_width), // name
    ];
    if show_type { widths.push(Constraint::Length(12)); }
    if show_address { widths.push(Constraint::Length(18)); }
    if show_signal { widths.push(Constraint::Length(8)); }
    if show_battery { widths.push(Constraint::Length(7)); }
    if show_transport { widths.push(Constraint::Length(7)); }

    Columns { widths, show_type, show_address, show_signal, show_battery, show_transport, name_width }
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

    // Wrap the hotkey bar onto as many rows as the width needs, then reserve
    // exactly that many rows so every hotkey stays visible on a narrow window.
    let hotkeys = crate::input::list_hotkeys(app);
    let hotbar_lines = layout_hotkeys(&hotkeys, area.width);
    let hotbar_rows = hotbar_lines.len() as u16;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),           // status
            Constraint::Length(1),           // separator
            Constraint::Min(3),              // table
            Constraint::Length(1),           // separator
            Constraint::Length(hotbar_rows), // hotkeys (1+ rows)
        ])
        .split(area);

    draw_status_line(f, app, chunks[0]);
    draw_separator(f, chunks[1]);
    draw_device_table(f, app, chunks[2]);
    draw_separator(f, chunks[3]);
    f.render_widget(Paragraph::new(hotbar_lines), chunks[4]);

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
            ctrl.name.as_str(),
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
                    dev.name.as_str(),
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

    // Right-side status badges. Each has a full label and a single-letter
    // fallback; fold to letters when the window is too narrow for both the
    // badges and the left-hand content (the colors still tell them apart).
    let badge = |label: &str, fg: Color, bg: Color| -> Vec<Span<'static>> {
        vec![
            Span::styled(
                format!(" {label} "),
                Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]
    };

    let mut badges: Vec<(&str, &str, Color, Color)> = Vec::new();
    if ctrl.powered {
        badges.push(("Powered", "P", Color::Black, GREEN));
    } else {
        badges.push(("Off", "O", WHITE, RED));
    }
    if app.scanning {
        badges.push(("Scanning", "S", Color::Black, YELLOW));
    }
    if ctrl.discoverable {
        badges.push(("Discoverable", "D", Color::Black, MAGENTA));
    }
    if ctrl.pairable {
        badges.push(("Pairable", "p", Color::Black, Color::Rgb(180, 142, 255)));
    }

    // " {label} " (label+2) plus a trailing space (1) per badge.
    let badge_w = |label: &str| label.chars().count() + 3;
    let full_w: usize = badges.iter().map(|(full, ..)| badge_w(full)).sum();
    // Fold to letters when the full badges would leave too little room for the
    // left-hand content (≈ " BT " + a readable name).
    const RESERVE: usize = 16;
    let fold = full_w + RESERVE > area.width as usize;

    let mut right: Vec<Span> = Vec::new();
    for (full, short, fg, bg) in &badges {
        let label = if fold { *short } else { *full };
        right.extend(badge(label, *fg, *bg));
    }
    let right_width: usize = right.iter().map(|s| s.content.chars().count()).sum();

    // Keep the badges glued to the right edge: clamp the left content so it
    // never overruns them, then pad the gap.
    let max_left = (area.width as usize).saturating_sub(right_width);
    let left = clamp_spans(left, max_left);
    let left_width: usize = left.iter().map(|s| s.content.chars().count()).sum();
    let pad = (area.width as usize).saturating_sub(left_width + right_width);
    let mut line = left;
    line.push(Span::raw(" ".repeat(pad)));
    line.extend(right);

    f.render_widget(Paragraph::new(Line::from(line)), area);
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

            // Connected is shown by the first-column icon, so it's not repeated
            // here. Paired → chain, Trusted → check; Blocked falls back to a
            // letter. No separators between them.
            let mut status_spans = Vec::new();
            if dev.paired {
                status_spans.push(Span::styled("🔗", Style::default().fg(CYAN)));
            }
            if dev.trusted {
                status_spans.push(Span::styled("✓", Style::default().fg(YELLOW)));
            }
            if dev.blocked {
                status_spans.push(Span::styled("B", Style::default().fg(RED)));
            }
            if status_spans.is_empty() {
                status_spans.push(Span::styled("─", Style::default().fg(GRAY)));
            }

            let mut cells = vec![
                Cell::from(icon),
                Cell::from(Line::from(status_spans)),
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
                cells.push(Cell::from(Span::styled(dev.address.as_str(), Style::default().fg(GRAY))));
            }
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
        Cell::from(""),                                          // icon
        Cell::from(Span::styled("", Style::default().fg(GRAY))), // status flags
        Cell::from(Span::styled("Device", Style::default().fg(GRAY))),
    ];
    if cols.show_type { hdr_cells.push(Cell::from(Span::styled("Type", Style::default().fg(GRAY)))); }
    if cols.show_address { hdr_cells.push(Cell::from(Span::styled("Address", Style::default().fg(GRAY)))); }
    if cols.show_signal { hdr_cells.push(Cell::from(Span::styled("RSSI", Style::default().fg(GRAY)))); }
    if cols.show_battery { hdr_cells.push(Cell::from(Span::styled("Bat", Style::default().fg(GRAY)))); }
    if cols.show_transport {
        hdr_cells.push(Cell::from(Span::styled(
            format!("Trans{title_line}"),
            Style::default().fg(GRAY),
        )));
    } else if !title_line.is_empty() {
        // put filter indicator on last visible column
        if let Some(last) = hdr_cells.last_mut() {
            *last = Cell::from(Span::styled(title_line, Style::default().fg(GRAY)));
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
        Line::from(vec![Span::styled("  d           ", b), Span::styled("Toggle discoverable (visible to other devices)", d)]),
        Line::from(vec![Span::styled("  i           ", b), Span::styled("Info overlay for selected device", d)]),
        Line::from(vec![Span::styled("  /           ", b), Span::styled("Filter devices by name", d)]),
        Line::from(vec![Span::styled("  h           ", b), Span::styled("This help screen", d)]),
        Line::from(vec![Span::styled("  q / Ctrl+C  ", b), Span::styled("Quit", d)]),
        Line::from(""),
        Line::from(Span::styled("  Status Flags", h)),
        Line::from(""),
        Line::from(vec![Span::styled("  🔗 ", Style::default().fg(CYAN)), Span::styled("Paired — keys exchanged, can reconnect without re-pairing", d)]),
        Line::from(vec![Span::styled("  ✓ ", y), Span::styled("Trusted — auto-connect allowed, no confirmation needed", d)]),
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

/// Trim a run of spans to at most `max` columns, truncating the span that
/// straddles the boundary and dropping the rest.
fn clamp_spans(spans: Vec<Span>, max: usize) -> Vec<Span> {
    let mut out = Vec::new();
    let mut used = 0usize;
    for s in spans {
        let w = s.content.chars().count();
        if used + w <= max {
            used += w;
            out.push(s);
        } else {
            let room = max - used;
            if room > 0 {
                let truncated: String = s.content.chars().take(room).collect();
                out.push(Span::styled(truncated, s.style));
            }
            break;
        }
    }
    out
}

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

