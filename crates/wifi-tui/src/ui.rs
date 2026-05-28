use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Clear, Paragraph, Row, Table, Wrap},
    Frame,
};

use net_tui_core::hotbar::hotkey_spans;
use net_tui_core::overlay::{centered_rect, detail_row, draw_separator, truncate};
use net_tui_core::theme::{CYAN, DIM, GRAY, GREEN, MAGENTA, RED, WHITE, YELLOW};

use crate::app::{App, DisplayRow, View};
use crate::wifi;

const COL_WIDTHS: [Constraint; 11] = [
    Constraint::Length(2),  // icon
    Constraint::Length(20), // ssid / bssid
    Constraint::Length(8),  // band
    Constraint::Length(4),  // ch
    Constraint::Length(12), // signal
    Constraint::Length(9),  // security / width
    Constraint::Length(7),  // gen
    Constraint::Length(3),  // SS
    Constraint::Length(4),  // clients
    Constraint::Length(5),  // load
    Constraint::Length(9),  // count / features
];

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
    draw_network_table(f, app, chunks[2]);
    draw_separator(f, chunks[3]);
    draw_hotkey_bar(f, app, chunks[4]);

    match &app.view {
        View::ConnInfo => draw_conninfo_overlay(f, app),
        View::Password => draw_password_overlay(f, app),
        View::Help => draw_help_overlay(f, app),
        View::List => {}
    }
}

// ── Status line ─────────────────────────────────────────────────────────────

fn draw_status_line(f: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![Span::styled(
        " WiFi ",
        Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
    )];

    if let Some(ds) = &app.device_state {
        match ds.state.as_str() {
            "connected" => {
                if let Some(conn) = &app.connection {
                    let w = area.width as usize;
                    // Always: SSID + signal + band/ch + gen
                    spans.push(Span::styled(
                        &conn.ssid,
                        Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::raw("  "));
                    if let Some(dbm) = conn.signal {
                        let (bar, color) = signal_bar_str(dbm as f64);
                        spans.push(Span::styled(bar, Style::default().fg(color)));
                        spans.push(Span::styled(
                            format!(" {dbm}dBm"),
                            Style::default().fg(GRAY),
                        ));
                    }
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled(&conn.band, Style::default().fg(CYAN)));
                    if !conn.channel.is_empty() {
                        spans.push(Span::styled(
                            format!("/ch{}", conn.channel),
                            Style::default().fg(GRAY),
                        ));
                    }
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled(
                        &conn.gen,
                        Style::default().fg(gen_color(&conn.gen)),
                    ));
                    // ≥80: uptime
                    if w >= 80 && conn.connected_time > 0 {
                        spans.push(Span::raw("  "));
                        spans.push(Span::styled(
                            wifi::format_time(conn.connected_time),
                            Style::default().fg(GRAY),
                        ));
                    }
                    // ≥100: TX/RX rates
                    if w >= 100 {
                        spans.push(Span::raw("  "));
                        spans.push(Span::styled(
                            format!("↑{:.0}", conn.tx_bitrate),
                            Style::default().fg(GREEN),
                        ));
                        spans.push(Span::styled(
                            format!(" ↓{:.0} Mbps", conn.rx_bitrate),
                            Style::default().fg(CYAN),
                        ));
                    }
                    // ≥125: TX retries
                    if w >= 125 {
                        let (retry_str, retry_color) =
                            wifi::format_retries(conn.tx_retries, conn.tx_packets);
                        spans.push(Span::raw("  "));
                        spans.push(Span::styled("retry:", Style::default().fg(GRAY)));
                        spans.push(Span::styled(
                            retry_str,
                            Style::default().fg(match retry_color {
                                "green" => GREEN,
                                "yellow" => YELLOW,
                                "red" => RED,
                                _ => GRAY,
                            }),
                        ));
                    }
                    // ≥150: traffic totals
                    if w >= 150 {
                        spans.push(Span::raw("  "));
                        spans.push(Span::styled(
                            format!(
                                "↑{} ↓{}",
                                wifi::format_bytes(conn.tx_bytes),
                                wifi::format_bytes(conn.rx_bytes)
                            ),
                            Style::default().fg(GRAY),
                        ));
                    }
                    // ≥180: width + region
                    if w >= 180 {
                        if !conn.width.is_empty() {
                            spans.push(Span::raw("  "));
                            spans.push(Span::styled(
                                &conn.width,
                                Style::default().fg(GRAY),
                            ));
                        }
                        if !conn.regdom.is_empty() {
                            spans.push(Span::styled(
                                format!(" {}", conn.regdom),
                                Style::default().fg(GRAY),
                            ));
                        }
                    }
                } else {
                    spans.push(Span::styled(
                        &ds.connection,
                        Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
                    ));
                }
            }
            s if s.starts_with("connecting") => {
                let phase = s
                    .strip_prefix("connecting (")
                    .and_then(|s| s.strip_suffix(')'))
                    .unwrap_or("...");
                spans.push(Span::styled("Connecting", Style::default().fg(YELLOW)));
                spans.push(Span::styled(
                    format!(" ({phase})"),
                    Style::default().fg(GRAY),
                ));
                if !ds.connection.is_empty() && ds.connection != "--" {
                    spans.push(Span::styled(
                        format!("  {}", ds.connection),
                        Style::default().fg(WHITE),
                    ));
                }
            }
            "disconnected" => {
                spans.push(Span::styled("Disconnected", Style::default().fg(GRAY)));
            }
            "unavailable" => {
                spans.push(Span::styled("WiFi OFF", Style::default().fg(RED)));
            }
            "deactivating" => {
                spans.push(Span::styled(
                    "Disconnecting...",
                    Style::default().fg(YELLOW),
                ));
            }
            other => {
                spans.push(Span::styled(other, Style::default().fg(GRAY)));
            }
        }
    } else {
        spans.push(Span::styled("...", Style::default().fg(GRAY)));
    }

    if !app.network_label.is_empty() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("[{}]", app.network_label),
            Style::default().fg(GRAY),
        ));
    }

    if let Some(msg) = app.status.current() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(format!("[{msg}]"), Style::default().fg(YELLOW)));
    }

    // Right-side badges
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

    if let Some(ds) = &app.device_state {
        match ds.state.as_str() {
            "connected" => right.extend(badge("Connected", Color::Black, GREEN)),
            "disconnected" => right.extend(badge("Disconnected", WHITE, Color::Rgb(80, 80, 80))),
            "unavailable" => right.extend(badge("Off", WHITE, RED)),
            s if s.starts_with("connecting") => right.extend(badge("Connecting", Color::Black, YELLOW)),
            _ => {}
        }
    }
    if app.scanning || app.auto_scan {
        right.extend(badge("Scanning", Color::Black, CYAN));
    }

    let left_width: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let right_width: usize = right.iter().map(|s| s.content.chars().count()).sum();
    let pad = (area.width as usize).saturating_sub(left_width + right_width);
    spans.push(Span::raw(" ".repeat(pad)));
    spans.extend(right);

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ── Network table ───────────────────────────────────────────────────────────

fn draw_network_table(f: &mut Frame, app: &mut App, area: Rect) {
    let rows: Vec<Row> = app
        .display_rows
        .iter()
        .map(|dr| match dr {
            DisplayRow::Group {
                ssid,
                count,
                best_signal,
                security,
                best_gen,
                associated,
                saved,
                ..
            } => {
                let icon = if *associated {
                    Span::styled("●", Style::default().fg(GREEN))
                } else if *saved {
                    Span::styled("○", Style::default().fg(GRAY))
                } else {
                    Span::raw(" ")
                };
                let ssid_style = if *associated {
                    Style::default().fg(GREEN).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(WHITE).add_modifier(Modifier::BOLD)
                };
                let (sig_bar, sig_color) = signal_bar_str(*best_signal);
                let sec_color = security_color(security);
                let label = if *count == 1 { "AP" } else { "APs" };

                Row::new(vec![
                    Cell::from(icon),
                    Cell::from(Span::styled(truncate(ssid, 20), ssid_style)),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(Line::from(vec![
                        Span::styled(sig_bar, Style::default().fg(sig_color)),
                        Span::styled(format!(" {:.0}", best_signal), Style::default().fg(GRAY)),
                    ])),
                    Cell::from(Span::styled(security.as_str(), Style::default().fg(sec_color))),
                    Cell::from(Span::styled(
                        best_gen.as_str(),
                        Style::default().fg(gen_color(best_gen)),
                    )),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(Span::styled(
                        format!("{count} {label}"),
                        Style::default().fg(GRAY),
                    )),
                ])
            }

            DisplayRow::Ap(net) => {
                let icon = if net.associated {
                    Span::styled("▸", Style::default().fg(GREEN))
                } else {
                    Span::raw(" ")
                };
                let bssid_style = if net.associated {
                    Style::default().fg(GREEN)
                } else {
                    Style::default().fg(Color::Rgb(140, 140, 140))
                };
                let (sig_bar, sig_color) = signal_bar_str(net.signal);

                let ss_str = if net.streams > 0 {
                    format!(" {}", net.streams)
                } else {
                    " ─".into()
                };
                let clients_str = match net.clients {
                    Some(n) => format!("{n}"),
                    None => "─".into(),
                };
                let clients_color = match net.clients {
                    Some(0) => GRAY,
                    Some(n) if n <= 5 => GREEN,
                    Some(n) if n <= 15 => YELLOW,
                    Some(_) => RED,
                    None => GRAY,
                };
                let load_str = match net.util {
                    Some(pct) => format!("{pct:.0}%"),
                    None => "─".into(),
                };
                let load_color = match net.util {
                    Some(p) if p < 30.0 => GREEN,
                    Some(p) if p < 60.0 => YELLOW,
                    Some(_) => RED,
                    None => GRAY,
                };

                let mut features = Vec::new();
                if net.wps {
                    features.push(Span::styled("WPS", Style::default().fg(YELLOW)));
                }
                if net.mu_mimo {
                    features.push(Span::styled("MU", Style::default().fg(CYAN)));
                }
                if net.twt {
                    features.push(Span::styled("TWT", Style::default().fg(GREEN)));
                }
                let feat_line = if features.is_empty() {
                    Line::from(Span::styled("─", Style::default().fg(GRAY)))
                } else {
                    let mut spans = Vec::new();
                    for (i, s) in features.into_iter().enumerate() {
                        if i > 0 {
                            spans.push(Span::raw(" "));
                        }
                        spans.push(s);
                    }
                    Line::from(spans)
                };

                Row::new(vec![
                    Cell::from(icon),
                    Cell::from(Span::styled(
                        format!("  {}", truncate(&net.bssid, 18)),
                        bssid_style,
                    )),
                    Cell::from(Span::styled(
                        net.band.as_str(),
                        Style::default().fg(band_color(&net.band)),
                    )),
                    Cell::from(Span::styled(
                        if net.channel.is_empty() { "─" } else { &net.channel },
                        Style::default().fg(WHITE),
                    )),
                    Cell::from(Line::from(vec![
                        Span::styled(sig_bar, Style::default().fg(sig_color)),
                        Span::styled(format!(" {:.0}", net.signal), Style::default().fg(GRAY)),
                    ])),
                    Cell::from(Span::styled(
                        if net.channel_width.is_empty() {
                            "─"
                        } else {
                            &net.channel_width
                        },
                        Style::default().fg(GRAY),
                    )),
                    Cell::from(""),
                    Cell::from(Span::styled(ss_str, Style::default().fg(WHITE))),
                    Cell::from(Span::styled(clients_str, Style::default().fg(clients_color))),
                    Cell::from(Span::styled(load_str, Style::default().fg(load_color))),
                    Cell::from(feat_line),
                ])
            }

            DisplayRow::FlatRow(aps) => {
                let best = &aps[0];
                let associated = aps.iter().any(|n| n.associated);
                let icon = if associated {
                    Span::styled("●", Style::default().fg(GREEN))
                } else if app.saved.contains(&best.ssid) {
                    Span::styled("○", Style::default().fg(GRAY))
                } else {
                    Span::raw(" ")
                };
                let ssid_style = if associated {
                    Style::default().fg(GREEN).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(WHITE)
                };
                let (sig_bar, sig_color) = signal_bar_str(best.signal);

                // Aggregate bands (deduplicated, sorted by freq)
                let bands_str = join_bands(aps);

                // Aggregate channels
                let ch_str = join_channels(aps);

                // Aggregate security (deduplicated)
                let sec_str = join_unique(aps.iter().map(|n| n.security.as_str()));
                let sec_color = security_color(&sec_str);

                // Best gen across all APs
                let best_gen = aps
                    .iter()
                    .max_by_key(|n| gen_rank_ui(&n.gen))
                    .map(|n| n.gen.as_str())
                    .unwrap_or("─");

                // Max spatial streams
                let max_ss = aps.iter().map(|n| n.streams).max().unwrap_or(0);
                let ss_str = if max_ss > 0 {
                    format!("{max_ss}")
                } else {
                    "─".into()
                };

                // Sum clients
                let total_clients: Option<u32> = {
                    let vals: Vec<u32> = aps.iter().filter_map(|n| n.clients).collect();
                    if vals.is_empty() { None } else { Some(vals.iter().sum()) }
                };
                let clients_str = match total_clients {
                    Some(n) => format!("{n}"),
                    None => "─".into(),
                };
                let clients_color = match total_clients {
                    Some(0) => GRAY,
                    Some(n) if n <= 5 => GREEN,
                    Some(n) if n <= 15 => YELLOW,
                    Some(_) => RED,
                    None => GRAY,
                };

                // Aggregate load (show range or max)
                let load_str = join_loads(aps);
                let max_load = aps.iter().filter_map(|n| n.util).fold(None, |acc, v| {
                    Some(acc.map_or(v, |a: f64| a.max(v)))
                });
                let load_color = match max_load {
                    Some(p) if p < 30.0 => GREEN,
                    Some(p) if p < 60.0 => YELLOW,
                    Some(_) => RED,
                    None => GRAY,
                };

                let ap_count = aps.len();
                let label = if ap_count == 1 { "AP" } else { "APs" };

                Row::new(vec![
                    Cell::from(icon),
                    Cell::from(Span::styled(truncate(&best.ssid, 20), ssid_style)),
                    Cell::from(Span::styled(bands_str, Style::default().fg(CYAN))),
                    Cell::from(Span::styled(ch_str, Style::default().fg(WHITE))),
                    Cell::from(Line::from(vec![
                        Span::styled(sig_bar, Style::default().fg(sig_color)),
                        Span::styled(
                            format!(" {:.0}", best.signal),
                            Style::default().fg(GRAY),
                        ),
                    ])),
                    Cell::from(Span::styled(sec_str.clone(), Style::default().fg(sec_color))),
                    Cell::from(Span::styled(
                        best_gen,
                        Style::default().fg(gen_color(best_gen)),
                    )),
                    Cell::from(Span::styled(ss_str, Style::default().fg(WHITE))),
                    Cell::from(Span::styled(clients_str, Style::default().fg(clients_color))),
                    Cell::from(Span::styled(load_str, Style::default().fg(load_color))),
                    Cell::from(Span::styled(
                        format!("{ap_count} {label}"),
                        Style::default().fg(GRAY),
                    )),
                ])
            }

            DisplayRow::Separator => {
                let sep: Vec<Cell> = COL_WIDTHS
                    .iter()
                    .map(|c| {
                        let w = match c {
                            Constraint::Length(n) => *n as usize,
                            _ => 6,
                        };
                        Cell::from(Span::styled("·".repeat(w), Style::default().fg(DIM)))
                    })
                    .collect();
                Row::new(sep)
            }
        })
        .collect();

    let title_line = if app.filtering {
        format!(" /{}█", app.filter)
    } else if !app.filter.is_empty() {
        format!(" [{}]", app.filter)
    } else {
        String::new()
    };

    let header = Row::new(vec![
        Cell::from(""),
        Cell::from(Span::styled("Network", Style::default().fg(GRAY))),
        Cell::from(Span::styled("Band", Style::default().fg(GRAY))),
        Cell::from(Span::styled("Ch", Style::default().fg(GRAY))),
        Cell::from(Span::styled("Signal", Style::default().fg(GRAY))),
        Cell::from(Span::styled("Sec/W", Style::default().fg(GRAY))),
        Cell::from(Span::styled("Gen", Style::default().fg(GRAY))),
        Cell::from(Span::styled("SS", Style::default().fg(GRAY))),
        Cell::from(Span::styled("👥", Style::default().fg(GRAY))),
        Cell::from(Span::styled("Load", Style::default().fg(GRAY))),
        Cell::from(Span::styled(
            format!("Info{title_line}"),
            Style::default().fg(GRAY),
        )),
    ])
    .height(1);

    let table = Table::new(rows, COL_WIDTHS)
        .header(header)
        .row_highlight_style(
            Style::default()
                .bg(Color::Rgb(35, 35, 45))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    f.render_stateful_widget(table, area, &mut app.table_state);
}

// ── Hotkey bar ──────────────────────────────────────────────────────────────

fn draw_hotkey_bar(f: &mut Frame, app: &App, area: Rect) {
    let auto_label = if app.auto_scan { "Auto Scan OFF" } else { "Auto Scan" };

    let hotkeys: Vec<(&str, &str)> = match &app.view {
        View::List if app.filtering => vec![
            ("Esc", "Cancel"),
            ("Enter", "Apply"),
            ("", "Type to filter..."),
        ],
        View::List => vec![
            ("c", "Connect"),
            ("p", if wifi::is_wifi_on() { "Power OFF" } else { "Power ON" }),
            ("s", "Scan"),
            ("a", auto_label),
            ("t", "Toggle View"),
            ("i", "Info"),
            ("d", "Disconnect"),
            ("f", "Filter"),
            ("h", "Help"),
            ("q", "Quit"),
        ],
        View::ConnInfo => vec![("Esc", "Back")],
        View::Password => vec![("⏎", "Submit"), ("Tab", "Show/Hide"), ("Esc", "Cancel")],
        View::Help => vec![("↑↓", "Scroll"), ("Esc", "Back")],
    };

    let spans: Vec<Span> = hotkeys
        .iter()
        .flat_map(|(key, desc)| hotkey_spans(key, desc))
        .collect();

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ── Connection info overlay ──────────────────────────────────────────────────

fn draw_conninfo_overlay(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 80, f.area());
    f.render_widget(Clear, area);

    let Some(conn) = &app.connection else {
        return;
    };

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    detail_row(&mut lines, "SSID", &conn.ssid, WHITE);
    detail_row(&mut lines, "BSSID", &conn.bssid, WHITE);
    detail_row(
        &mut lines,
        "Band",
        &format!(
            "{}  ch{}  ({:.0} MHz)",
            conn.band, conn.channel, conn.freq
        ),
        band_color(&conn.band),
    );
    if !conn.width.is_empty() {
        detail_row(&mut lines, "Width", &conn.width, WHITE);
    }
    detail_row(&mut lines, "Generation", &conn.gen, gen_color(&conn.gen));
    if !conn.regdom.is_empty() {
        detail_row(&mut lines, "Region", &conn.regdom, WHITE);
    }

    lines.push(Line::from(""));

    if let Some(dbm) = conn.signal {
        let (bar, color) = signal_bar_str(dbm as f64);
        lines.push(Line::from(vec![
            Span::styled("  Signal      ", Style::default().fg(GRAY)),
            Span::styled(bar, Style::default().fg(color)),
            Span::styled(
                format!(" {} dBm  ", dbm),
                Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                wifi::signal_quality(dbm as f64),
                Style::default().fg(color),
            ),
        ]));
    }
    if let Some(avg) = conn.signal_avg {
        let (abar, acolor) = signal_bar_str(avg as f64);
        lines.push(Line::from(vec![
            Span::styled("  Average     ", Style::default().fg(GRAY)),
            Span::styled(abar, Style::default().fg(acolor)),
            Span::styled(format!(" {avg} dBm"), Style::default().fg(WHITE)),
        ]));
    }
    if let Some(chains) = &conn.signal_chains {
        detail_row(&mut lines, "Chains", &format!("{chains} dBm"), WHITE);
    }

    lines.push(Line::from(""));

    let tx_label = format!("{:.0} Mbps{}", conn.tx_bitrate, mod_suffix(&conn.tx_mod));
    detail_row(&mut lines, "TX rate", &tx_label, GREEN);
    let rx_label = format!("{:.0} Mbps{}", conn.rx_bitrate, mod_suffix(&conn.rx_mod));
    detail_row(&mut lines, "RX rate", &rx_label, CYAN);

    lines.push(Line::from(""));

    let (retry_str, retry_color) = wifi::format_retries(conn.tx_retries, conn.tx_packets);
    detail_row(
        &mut lines,
        "TX retries",
        &retry_str,
        match retry_color {
            "green" => GREEN,
            "yellow" => YELLOW,
            "red" => RED,
            _ => GRAY,
        },
    );
    if conn.tx_failed > 0 {
        detail_row(&mut lines, "TX failed", &conn.tx_failed.to_string(), RED);
    }
    detail_row(
        &mut lines,
        "Beacons",
        &if conn.beacon_loss > 0 {
            format!("{} lost", conn.beacon_loss)
        } else {
            "0 lost".into()
        },
        if conn.beacon_loss > 0 { RED } else { GREEN },
    );

    lines.push(Line::from(""));

    detail_row(&mut lines, "TX total", &wifi::format_bytes(conn.tx_bytes), WHITE);
    detail_row(&mut lines, "RX total", &wifi::format_bytes(conn.rx_bytes), WHITE);
    detail_row(
        &mut lines,
        "Uptime",
        &wifi::format_time(conn.connected_time),
        WHITE,
    );
    if let Some(pwr) = conn.tx_power {
        detail_row(&mut lines, "TX power", &format!("{pwr:.1} dBm"), WHITE);
    }

    let title = format!(" {} — Connection Details ", conn.ssid);
    let border_block = ratatui::widgets::Block::default()
        .title(title)
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(Style::default().fg(CYAN));
    let para = Paragraph::new(lines)
        .block(border_block)
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

// ── Password overlay ────────────────────────────────────────────────────────

fn draw_password_overlay(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 30, f.area());
    let area = Rect {
        height: area.height.min(7),
        ..area
    };
    f.render_widget(Clear, area);

    let ssid = app
        .selected_ssid()
        .unwrap_or_else(|| "?".into());

    let display_pw = if app.password_visible {
        app.password.clone()
    } else {
        "●".repeat(app.password.len())
    };

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Password: ", Style::default().fg(GRAY)),
            Span::styled(&display_pw, Style::default().fg(WHITE)),
            Span::styled("█", Style::default().fg(CYAN)),
        ]),
        Line::from(""),
    ];

    let title = format!(" Connect to \"{ssid}\" ");
    let border_block = ratatui::widgets::Block::default()
        .title(title)
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(Style::default().fg(CYAN));
    let para = Paragraph::new(lines).block(border_block);
    f.render_widget(para, area);
}

// ── Help overlay ────────────────────────────────────────────────────────────

fn draw_help_overlay(f: &mut Frame, app: &App) {
    let area = centered_rect(65, 90, f.area());
    f.render_widget(Clear, area);

    let b = Style::default().fg(CYAN).add_modifier(Modifier::BOLD);
    let h = Style::default().fg(WHITE).add_modifier(Modifier::BOLD);
    let d = Style::default().fg(GRAY);
    let g = Style::default().fg(GREEN);
    let y = Style::default().fg(YELLOW);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled("  Keybindings", h)),
        Line::from(""),
        Line::from(vec![Span::styled("  ↑↓ / j k    ", b), Span::styled("Navigate network list", d)]),
        Line::from(vec![Span::styled("  g / G       ", b), Span::styled("Jump to first / last", d)]),
        Line::from(vec![Span::styled("  ⏎ / c       ", b), Span::styled("Connect to selected network", d)]),
        Line::from(vec![Span::styled("  D           ", b), Span::styled("Disconnect from current network", d)]),
        Line::from(vec![Span::styled("  s           ", b), Span::styled("Trigger a Wi-Fi scan", d)]),
        Line::from(vec![Span::styled("  P           ", b), Span::styled("Toggle WiFi radio on/off", d)]),
        Line::from(vec![Span::styled("  S           ", b), Span::styled("Toggle auto-scan (rescan every 2s)", d)]),
        Line::from(vec![Span::styled("  o           ", b), Span::styled("Cycle sort mode (signal / name)", d)]),
        Line::from(vec![Span::styled("  v           ", b), Span::styled("Toggle view (grouped BSSIDs / flat SSIDs)", d)]),
        Line::from(vec![Span::styled("  d           ", b), Span::styled("Detail overlay for selected AP", d)]),
        Line::from(vec![Span::styled("  i           ", b), Span::styled("Connection info overlay (active link stats)", d)]),
        Line::from(vec![Span::styled("  /           ", b), Span::styled("Filter networks by SSID", d)]),
        Line::from(vec![Span::styled("  h           ", b), Span::styled("This help screen", d)]),
        Line::from(vec![Span::styled("  q / Ctrl+C  ", b), Span::styled("Quit", d)]),
        Line::from(""),
        Line::from(Span::styled("  Table Columns", h)),
        Line::from(""),
        Line::from(vec![Span::styled("  Network  ", Style::default().fg(WHITE)), Span::styled("SSID name (grouped view) or BSSID (sub-rows)", d)]),
        Line::from(vec![Span::styled("  Band     ", Style::default().fg(CYAN)), Span::styled("2.4 GHz / 5 GHz / 6 GHz frequency band", d)]),
        Line::from(vec![Span::styled("  Ch       ", Style::default().fg(WHITE)), Span::styled("Channel number", d)]),
        Line::from(vec![Span::styled("  Signal   ", g), Span::styled("RSSI in dBm with signal bar", d)]),
        Line::from(vec![Span::styled("  Sec/W    ", Style::default().fg(WHITE)), Span::styled("Security (group row) or channel width (AP row)", d)]),
        Line::from(vec![Span::styled("  Gen      ", Style::default().fg(WHITE)), Span::styled("Wi-Fi generation (see below)", d)]),
        Line::from(vec![Span::styled("  SS       ", Style::default().fg(WHITE)), Span::styled("Spatial streams (MIMO antenna count)", d)]),
        Line::from(vec![Span::styled("  👥       ", Style::default().fg(WHITE)), Span::styled("Connected station/client count", d)]),
        Line::from(vec![Span::styled("  Load     ", Style::default().fg(WHITE)), Span::styled("Channel utilisation percentage", d)]),
        Line::from(vec![Span::styled("  Info     ", Style::default().fg(WHITE)), Span::styled("AP count (group) or feature flags (AP row)", d)]),
        Line::from(""),
        Line::from(Span::styled("  Row Icons", h)),
        Line::from(""),
        Line::from(vec![Span::styled("  ● ", g), Span::styled("Connected    ", d), Span::styled("○ ", Style::default().fg(GRAY)), Span::styled("Saved    ", d), Span::styled("  ", Style::default()), Span::styled("(blank) New", d)]),
        Line::from(""),
        Line::from(Span::styled("  Wi-Fi Generations", h)),
        Line::from(""),
        Line::from(vec![Span::styled("  Gen 7  ", Style::default().fg(GEN7_COLOR)), Span::styled("802.11be (Wi-Fi 7)  — up to 46 Gbps, 320 MHz", d)]),
        Line::from(vec![Span::styled("  Gen 6E ", Style::default().fg(GEN6E_COLOR)), Span::styled("802.11ax on 6 GHz   — less congestion, wider channels", d)]),
        Line::from(vec![Span::styled("  Gen 6  ", Style::default().fg(GEN6_COLOR)), Span::styled("802.11ax (Wi-Fi 6)  — up to 9.6 Gbps, OFDMA, TWT", d)]),
        Line::from(vec![Span::styled("  Gen 5  ", Style::default().fg(GEN5_COLOR)), Span::styled("802.11ac (Wi-Fi 5)  — up to 3.5 Gbps, 5 GHz", d)]),
        Line::from(vec![Span::styled("  Gen 4  ", Style::default().fg(GEN4_COLOR)), Span::styled("802.11n  (Wi-Fi 4)  — up to 600 Mbps, MIMO", d)]),
        Line::from(vec![Span::styled("  Legacy ", Style::default().fg(GEN_LEGACY_COLOR)), Span::styled("802.11a/b/g          — up to 54 Mbps", d)]),
        Line::from(""),
        Line::from(Span::styled("  Security Types", h)),
        Line::from(""),
        Line::from(vec![Span::styled("  WPA3    ", g), Span::styled("Latest, SAE handshake (strongest)", d)]),
        Line::from(vec![Span::styled("  WPA2/3  ", g), Span::styled("Transition mode, accepts both", d)]),
        Line::from(vec![Span::styled("  WPA2    ", Style::default().fg(WHITE)), Span::styled("Standard PSK (most common)", d)]),
        Line::from(vec![Span::styled("  802.1X  ", Style::default().fg(CYAN)), Span::styled("Enterprise (RADIUS authentication)", d)]),
        Line::from(vec![Span::styled("  Open    ", Style::default().fg(RED)), Span::styled("No encryption (avoid for sensitive traffic)", d)]),
        Line::from(""),
        Line::from(Span::styled("  Feature Flags", h)),
        Line::from(""),
        Line::from(vec![Span::styled("  WPS  ", y), Span::styled("Wi-Fi Protected Setup (push-button pairing)", d)]),
        Line::from(vec![Span::styled("  MU   ", Style::default().fg(CYAN)), Span::styled("MU-MIMO beamforming (multi-user simultaneous)", d)]),
        Line::from(vec![Span::styled("  TWT  ", g), Span::styled("Target Wake Time (power saving for clients)", d)]),
        Line::from(""),
        Line::from(Span::styled("  Signal Strength", h)),
        Line::from(""),
        Line::from(vec![Span::styled("  ████ ", g), Span::styled("≥ -50 dBm Excellent   ", d), Span::styled("███░ ", g), Span::styled("≥ -60 dBm Good", d)]),
        Line::from(vec![Span::styled("  ██░░ ", y), Span::styled("≥ -70 dBm Fair        ", d), Span::styled("█░░░ ", Style::default().fg(RED)), Span::styled("≥ -80 dBm Weak", d)]),
        Line::from(""),
        Line::from(Span::styled("  Band Colors", h)),
        Line::from(""),
        Line::from(vec![Span::styled("  2.4 GHz ", y), Span::styled("Longer range, more congested, slower", d)]),
        Line::from(vec![Span::styled("  5 GHz   ", Style::default().fg(CYAN)), Span::styled("Faster, shorter range, less congestion", d)]),
        Line::from(vec![Span::styled("  6 GHz   ", Style::default().fg(MAGENTA)), Span::styled("Fastest, shortest range, least congestion (Wi-Fi 6E/7)", d)]),
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

fn signal_bar_str(dbm: f64) -> (String, Color) {
    if dbm >= -50.0 {
        ("████".into(), GREEN)
    } else if dbm >= -60.0 {
        ("███░".into(), GREEN)
    } else if dbm >= -70.0 {
        ("██░░".into(), YELLOW)
    } else if dbm >= -80.0 {
        ("█░░░".into(), RED)
    } else {
        ("░░░░".into(), RED)
    }
}

const GEN7_COLOR: Color = Color::Rgb(198, 120, 221);  // purple
const GEN6E_COLOR: Color = Color::Rgb(86, 182, 194);  // teal
const GEN6_COLOR: Color = Color::Rgb(152, 195, 121);  // green
const GEN5_COLOR: Color = Color::Rgb(229, 192, 123);  // amber
const GEN4_COLOR: Color = Color::Rgb(224, 108, 117);   // red
const GEN_LEGACY_COLOR: Color = Color::Rgb(92, 99, 112); // dim gray

fn gen_color(gen: &str) -> Color {
    match gen {
        s if s.contains('7') => GEN7_COLOR,
        s if s.contains("6E") => GEN6E_COLOR,
        s if s.contains('6') => GEN6_COLOR,
        s if s.contains('5') => GEN5_COLOR,
        s if s.contains('4') => GEN4_COLOR,
        _ => GEN_LEGACY_COLOR,
    }
}

fn band_color(band: &str) -> Color {
    if band.starts_with('6') {
        MAGENTA
    } else if band.starts_with('5') {
        CYAN
    } else {
        YELLOW
    }
}

fn security_color(sec: &str) -> Color {
    match sec {
        "Open" => RED,
        "WPA3" | "WPA2/3" => GREEN,
        "Enterprise" => CYAN,
        _ => WHITE,
    }
}

fn mod_suffix(m: &wifi::Modulation) -> String {
    if m.mod_type.is_empty() {
        return String::new();
    }
    format!("  {}-MCS {} NSS {} GI {}", m.mod_type, m.mcs, m.nss, m.gi)
}

fn gen_rank_ui(gen: &str) -> u8 {
    match gen {
        "Gen 7" => 6,
        "Gen 6E" => 5,
        "Gen 6" => 4,
        "Gen 5" => 3,
        "Gen 4" => 2,
        _ => 1,
    }
}

fn join_bands(aps: &[wifi::Network]) -> String {
    let mut has_24 = false;
    let mut has_5 = false;
    let mut has_6 = false;
    for ap in aps {
        if ap.freq >= 5925.0 {
            has_6 = true;
        } else if ap.freq >= 5000.0 {
            has_5 = true;
        } else {
            has_24 = true;
        }
    }
    let mut parts = Vec::new();
    if has_24 { parts.push("2.4"); }
    if has_5 { parts.push("5"); }
    if has_6 { parts.push("6"); }
    if parts.is_empty() {
        "─".into()
    } else {
        format!("{} GHz", parts.join("/"))
    }
}

fn join_channels(aps: &[wifi::Network]) -> String {
    let mut chs: Vec<u32> = aps
        .iter()
        .filter_map(|n| n.channel.parse::<u32>().ok())
        .collect();
    chs.sort_unstable();
    chs.dedup();
    if chs.is_empty() {
        "─".into()
    } else if chs.len() <= 3 {
        chs.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",")
    } else {
        format!("{}…", chs.iter().take(2).map(|c| c.to_string()).collect::<Vec<_>>().join(","))
    }
}

fn join_unique<'a>(vals: impl Iterator<Item = &'a str>) -> String {
    let mut seen = Vec::new();
    for v in vals {
        if !v.is_empty() && !seen.contains(&v) {
            seen.push(v);
        }
    }
    if seen.is_empty() { "─".into() } else { seen.join("/") }
}

fn join_loads(aps: &[wifi::Network]) -> String {
    let vals: Vec<f64> = aps.iter().filter_map(|n| n.util).collect();
    if vals.is_empty() {
        return "─".into();
    }
    let max = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    format!("{max:.0}%")
}

