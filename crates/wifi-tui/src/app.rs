use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::thread;

use crossterm::event::KeyEvent;
use net_tui_core::filter::Filterable;
use net_tui_core::runtime::TuiApp;
use net_tui_core::status::Status;
use ratatui::widgets::TableState;
use ratatui::Frame;

use crate::wifi::{self, ConnectionInfo, DeviceState, Network};

pub enum View {
    List,
    ConnInfo,
    Password,
    Help,
}

pub enum BgMsg {
    ScanResult(Vec<Network>, HashSet<String>),
    StatusUpdate(DeviceState, Option<ConnectionInfo>),
    ConnectResult(Result<String, String>),
    DisconnectResult(Result<String, String>),
}

#[derive(Clone)]
pub enum DisplayRow {
    Group {
        ssid: String,
        count: usize,
        best_signal: f64,
        security: String,
        best_gen: String,
        associated: bool,
        saved: bool,
    },
    Ap(Network),
    FlatRow(Vec<Network>),
    Separator,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SortMode {
    Signal,
    Name,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ViewMode {
    Grouped,
    Flat,
}

fn gen_rank(gen: &str) -> u8 {
    match gen {
        "Gen 7" => 6,
        "Gen 6E" => 5,
        "Gen 6" => 4,
        "Gen 5" => 3,
        "Gen 4" => 2,
        _ => 1,
    }
}

pub struct App {
    pub networks: Vec<Network>,
    pub saved: HashSet<String>,
    pub device_state: Option<DeviceState>,
    pub connection: Option<ConnectionInfo>,
    pub display_rows: Vec<DisplayRow>,
    pub table_state: TableState,
    pub view: View,
    pub scanning: bool,
    pub interface: String,
    pub should_quit: bool,
    pub password: String,
    pub password_visible: bool,
    pub status: Status,
    pub network_label: String,
    pub filter: String,
    pub filtering: bool,
    pub auto_scan: bool,
    pub sort_mode: SortMode,
    pub view_mode: ViewMode,
    pub help_scroll: u16,
    refresh_counter: u32,
    auto_scan_counter: u32,
    bg_tx: mpsc::Sender<BgMsg>,
    bg_rx: mpsc::Receiver<BgMsg>,
}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let iface = wifi::get_interface().unwrap_or_default();
        let mut app = Self {
            networks: Vec::new(),
            saved: HashSet::new(),
            device_state: None,
            connection: None,
            display_rows: Vec::new(),
            table_state: TableState::default(),
            view: View::List,
            scanning: false,
            interface: iface,
            should_quit: false,
            password: String::new(),
            password_visible: false,
            status: Status::new(),
            network_label: String::new(),
            filter: String::new(),
            filtering: false,
            auto_scan: false,
            sort_mode: SortMode::Signal,
            view_mode: ViewMode::Grouped,
            help_scroll: 0,
            refresh_counter: 0,
            auto_scan_counter: 0,
            bg_tx: tx,
            bg_rx: rx,
        };
        app.table_state.select(Some(0));
        app
    }

    pub fn initial_load(&mut self) {
        // Quick, near-instant reads so the first frame has real device state.
        // The network scan is slow, so it runs in the background instead of
        // blocking the first render — the TUI shows immediately, then fills in.
        let state = wifi::get_device_state(&self.interface);
        if state.state == "connected" {
            self.connection = wifi::get_connection_info(&self.interface);
        }
        self.device_state = Some(state);
        self.saved = wifi::saved_connections();
        self.rebuild();

        // Kick off the first scan right away without blocking the first frame.
        self.scan();
    }

    /// Spawn a background network scan. No-op if one is already in flight.
    pub fn scan(&mut self) {
        if self.scanning {
            return;
        }
        self.scanning = true;
        let tx = self.bg_tx.clone();
        let iface = self.interface.clone();
        thread::spawn(move || {
            let networks = wifi::scan_networks(&iface);
            let saved = wifi::saved_connections();
            let _ = tx.send(BgMsg::ScanResult(networks, saved));
        });
    }

    pub fn refresh_connection(&mut self) {
        let tx = self.bg_tx.clone();
        let iface = self.interface.clone();
        thread::spawn(move || {
            let state = wifi::get_device_state(&iface);
            let info = if state.state == "connected" {
                wifi::get_connection_info(&iface)
            } else {
                None
            };
            let _ = tx.send(BgMsg::StatusUpdate(state, info));
        });
    }

    pub fn cycle_sort(&mut self) {
        self.sort_mode = match self.sort_mode {
            SortMode::Signal => SortMode::Name,
            SortMode::Name => SortMode::Signal,
        };
        let label = match self.sort_mode {
            SortMode::Signal => "Sort: signal",
            SortMode::Name => "Sort: name",
        };
        self.status.set(label);
        self.rebuild();
    }

    pub fn toggle_view_mode(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::Grouped => ViewMode::Flat,
            ViewMode::Flat => ViewMode::Grouped,
        };
        let label = match self.view_mode {
            ViewMode::Grouped => "View: grouped (BSSIDs)",
            ViewMode::Flat => "View: flat (SSIDs)",
        };
        self.status.set(label);
        self.rebuild();
    }

    /// Toggle continuous scanning. When on, the tick loop rescans every ~2s and
    /// the status line shows a "Scanning" badge. When off, the network list is
    /// left as-is.
    pub fn toggle_scan(&mut self) {
        self.auto_scan = !self.auto_scan;
        self.auto_scan_counter = 0;
        if self.auto_scan {
            self.status.set("Scan ON");
            self.scan();
        } else {
            self.status.set("Scan OFF");
        }
    }

    pub fn rebuild(&mut self) {
        let filter_lower = self.filter.to_lowercase();
        let filtered: Vec<&Network> = if self.filter.is_empty() {
            self.networks.iter().collect()
        } else {
            self.networks
                .iter()
                .filter(|n| n.ssid.to_lowercase().contains(&filter_lower))
                .collect()
        };

        let mut groups: Vec<(String, Vec<Network>)> = Vec::new();
        let mut seen: HashMap<String, usize> = HashMap::new();
        for net in &filtered {
            if let Some(&idx) = seen.get(&net.ssid) {
                groups[idx].1.push((*net).clone());
            } else {
                seen.insert(net.ssid.clone(), groups.len());
                groups.push((net.ssid.clone(), vec![(*net).clone()]));
            }
        }

        let sort_mode = self.sort_mode;
        groups.sort_by(|a, b| {
            let a_assoc = a.1.iter().any(|n| n.associated);
            let b_assoc = b.1.iter().any(|n| n.associated);
            let a_best = a.1.iter().map(|n| n.signal).fold(f64::NEG_INFINITY, f64::max);
            let b_best = b.1.iter().map(|n| n.signal).fold(f64::NEG_INFINITY, f64::max);
            // Associated always first, then sort by mode
            b_assoc.cmp(&a_assoc).then_with(|| match sort_mode {
                SortMode::Name => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
                SortMode::Signal => b_best
                    .partial_cmp(&a_best)
                    .unwrap_or(std::cmp::Ordering::Equal),
            })
        });

        self.display_rows.clear();
        let view_mode = self.view_mode;
        for (i, (ssid, mut aps)) in groups.into_iter().enumerate() {
            aps.sort_by(|a, b| {
                b.signal
                    .partial_cmp(&a.signal)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let best_signal = aps.first().map(|a| a.signal).unwrap_or(-100.0);
            let associated = aps.iter().any(|a| a.associated);
            let security = aps.first().map(|a| a.security.clone()).unwrap_or_default();
            let best_gen = aps
                .iter()
                .max_by_key(|a| gen_rank(&a.gen))
                .map(|a| a.gen.clone())
                .unwrap_or_default();
            let saved = self.saved.contains(&ssid);
            let count = aps.len();

            match view_mode {
                ViewMode::Flat => {
                    if !aps.is_empty() {
                        self.display_rows.push(DisplayRow::FlatRow(aps));
                    }
                }
                ViewMode::Grouped => {
                    if i > 0 {
                        self.display_rows.push(DisplayRow::Separator);
                    }
                    self.display_rows.push(DisplayRow::Group {
                        ssid,
                        count,
                        best_signal,
                        security,
                        best_gen,
                        associated,
                        saved,
                    });
                    for ap in aps {
                        self.display_rows.push(DisplayRow::Ap(ap));
                    }
                }
            }
        }
        self.clamp_selection();
    }

    pub fn selected_ssid(&self) -> Option<String> {
        match self.display_rows.get(self.selected_index())? {
            DisplayRow::Group { ssid, .. } => Some(ssid.clone()),
            DisplayRow::Ap(net) => Some(net.ssid.clone()),
            DisplayRow::FlatRow(aps) => aps.first().map(|n| n.ssid.clone()),
            DisplayRow::Separator => None,
        }
    }

    pub fn selected_security(&self) -> Option<String> {
        match self.display_rows.get(self.selected_index())? {
            DisplayRow::Group { security, .. } => Some(security.clone()),
            DisplayRow::Ap(net) => Some(net.security.clone()),
            DisplayRow::FlatRow(aps) => aps.first().map(|n| n.security.clone()),
            DisplayRow::Separator => None,
        }
    }

    /// Returns BSSID only when a specific AP row is selected (not a group/flat header).
    pub fn selected_bssid(&self) -> Option<String> {
        match self.display_rows.get(self.selected_index())? {
            DisplayRow::Ap(net) => Some(net.bssid.clone()),
            _ => None,
        }
    }

    pub fn connect_to_selected(&mut self) {
        let Some(ssid) = self.selected_ssid() else {
            return;
        };
        let bssid = self.selected_bssid();
        let security = self.selected_security().unwrap_or_default();
        if security == "Open" || self.saved.contains(&ssid) {
            let tx = self.bg_tx.clone();
            let label = bssid
                .as_deref()
                .map(|b| format!("{ssid} ({b})"))
                .unwrap_or_else(|| ssid.clone());
            self.status.set(format!("Connecting to {label}..."));
            thread::spawn(move || {
                let result = wifi::connect(&ssid, bssid.as_deref(), None);
                let _ = tx.send(BgMsg::ConnectResult(result));
            });
        } else {
            self.password.clear();
            self.password_visible = false;
            self.view = View::Password;
        }
    }

    pub fn submit_password(&mut self) {
        let Some(ssid) = self.selected_ssid() else {
            return;
        };
        let bssid = self.selected_bssid();
        let password = self.password.clone();
        let tx = self.bg_tx.clone();
        let label = bssid
            .as_deref()
            .map(|b| format!("{ssid} ({b})"))
            .unwrap_or_else(|| ssid.clone());
        self.status.set(format!("Connecting to {label}..."));
        self.view = View::List;
        thread::spawn(move || {
            let result = wifi::connect(&ssid, bssid.as_deref(), Some(&password));
            let _ = tx.send(BgMsg::ConnectResult(result));
        });
    }

    pub fn disconnect(&mut self) {
        let tx = self.bg_tx.clone();
        let iface = self.interface.clone();
        self.status.set("Disconnecting...");
        thread::spawn(move || {
            let result = wifi::disconnect(&iface);
            let _ = tx.send(BgMsg::DisconnectResult(result));
        });
    }

    pub fn toggle_power(&mut self) {
        let on = !wifi::is_wifi_on();
        let label = if on { "WiFi ON..." } else { "WiFi OFF..." };
        self.status.set(label);
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let result = wifi::set_wifi_power(on);
            let _ = tx.send(BgMsg::ConnectResult(result));
        });
    }

    pub fn selected_index(&self) -> usize {
        self.table_state.selected().unwrap_or(0)
    }

    pub fn network_count(&self) -> usize {
        self.networks.len()
    }

    pub fn group_count(&self) -> usize {
        self.display_rows
            .iter()
            .filter(|r| matches!(r, DisplayRow::Group { .. }))
            .count()
    }

    pub fn select_next(&mut self) {
        let len = self.display_rows.len();
        if len == 0 {
            return;
        }
        let mut next = self.selected_index() + 1;
        while next < len && matches!(self.display_rows[next], DisplayRow::Separator) {
            next += 1;
        }
        if next < len {
            self.table_state.select(Some(next));
        }
    }

    pub fn select_prev(&mut self) {
        let mut prev = self.selected_index().saturating_sub(1);
        while prev > 0 && matches!(self.display_rows.get(prev), Some(DisplayRow::Separator)) {
            prev -= 1;
        }
        if !matches!(self.display_rows.get(prev), Some(DisplayRow::Separator)) {
            self.table_state.select(Some(prev));
        }
    }

    pub fn select_first(&mut self) {
        let mut idx = 0;
        while idx < self.display_rows.len()
            && matches!(self.display_rows[idx], DisplayRow::Separator)
        {
            idx += 1;
        }
        self.table_state.select(Some(idx));
    }

    pub fn select_last(&mut self) {
        if self.display_rows.is_empty() {
            return;
        }
        let mut idx = self.display_rows.len() - 1;
        while idx > 0 && matches!(self.display_rows[idx], DisplayRow::Separator) {
            idx -= 1;
        }
        self.table_state.select(Some(idx));
    }

    fn clamp_selection(&mut self) {
        let len = self.display_rows.len();
        if len == 0 {
            self.table_state.select(Some(0));
            return;
        }
        let mut idx = self.selected_index().min(len - 1);
        while idx < len && matches!(self.display_rows[idx], DisplayRow::Separator) {
            idx += 1;
        }
        if idx >= len {
            idx = len.saturating_sub(1);
            while idx > 0 && matches!(self.display_rows[idx], DisplayRow::Separator) {
                idx -= 1;
            }
        }
        self.table_state.select(Some(idx));
    }
}

impl Filterable for App {
    fn filter_mut(&mut self) -> &mut String {
        &mut self.filter
    }
    fn set_filtering(&mut self, on: bool) {
        self.filtering = on;
    }
    fn rebuild(&mut self) {
        App::rebuild(self);
    }
}

impl TuiApp for App {
    fn draw(&mut self, frame: &mut Frame) {
        crate::ui::draw(frame, self);
    }

    fn tick(&mut self) {
        while let Ok(msg) = self.bg_rx.try_recv() {
            match msg {
                BgMsg::ScanResult(networks, saved) => {
                    self.networks = networks;
                    self.saved = saved;
                    self.scanning = false;
                    let aps = self.network_count();
                    self.rebuild();
                    let groups = self.group_count();
                    self.network_label = format!("{groups} networks, {aps} APs");
                }
                BgMsg::StatusUpdate(state, info) => {
                    self.device_state = Some(state);
                    self.connection = info;
                }
                BgMsg::ConnectResult(result) | BgMsg::DisconnectResult(result) => match result {
                    Ok(msg) => {
                        self.status.set(msg);
                        self.refresh_connection();
                    }
                    Err(e) => {
                        self.status.set(format!("Error: {e}"));
                    }
                },
            }
        }

        self.status.tick();

        self.refresh_counter += 1;
        if self.refresh_counter >= 4 {
            self.refresh_counter = 0;
            self.refresh_connection();
        }

        if self.auto_scan {
            self.auto_scan_counter += 1;
            if self.auto_scan_counter >= 8 {
                self.auto_scan_counter = 0;
                self.scan();
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        crate::input::handle_key(self, key);
    }

    fn handle_scroll_up(&mut self) {
        self.select_prev();
    }

    fn handle_scroll_down(&mut self) {
        self.select_next();
    }

    fn handle_left_click(&mut self, row: u16, col: u16, term_width: u16, term_height: u16) {
        let hotkeys = crate::input::list_hotkeys(self);
        let hotbar_rows = net_tui_core::hotbar::rows_needed(&hotkeys, term_width);
        let hotbar_top = term_height.saturating_sub(hotbar_rows);
        if row >= hotbar_top {
            crate::input::handle_hotbar_click(self, term_width, row - hotbar_top, col);
        } else if row >= 3 {
            let clicked = (row - 3) as usize;
            if clicked < self.display_rows.len()
                && !matches!(self.display_rows[clicked], DisplayRow::Separator)
            {
                self.table_state.select(Some(clicked));
            }
        }
    }

    fn should_quit(&self) -> bool {
        self.should_quit
    }
}
