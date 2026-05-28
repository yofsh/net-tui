use std::process::Child;
use std::sync::mpsc;
use std::thread;

use crossterm::event::KeyEvent;
use net_tui_core::filter::Filterable;
use net_tui_core::runtime::TuiApp;
use net_tui_core::status::Status;
use ratatui::widgets::TableState;
use ratatui::Frame;

use crate::bt::{self, Controller, Device};

pub enum View {
    List,
    Detail,
    Help,
}

pub enum BgMsg {
    DeviceList(Vec<Device>),
    ControllerInfo(Controller),
    ActionResult(Result<String, String>),
}

pub struct App {
    pub devices: Vec<Device>,
    pub filtered: Vec<usize>,
    pub controller: Controller,
    pub table_state: TableState,
    pub view: View,
    pub should_quit: bool,
    pub scanning: bool,
    pub status: Status,
    pub filter: String,
    pub filtering: bool,
    pub help_scroll: u16,
    refresh_counter: u32,
    scan_child: Option<Child>,
    scan_pending: u32,
    bg_tx: mpsc::Sender<BgMsg>,
    bg_rx: mpsc::Receiver<BgMsg>,
}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let mut app = Self {
            devices: Vec::new(),
            filtered: Vec::new(),
            controller: Controller::default(),
            table_state: TableState::default(),
            view: View::List,
            should_quit: false,
            scanning: false,
            status: Status::new(),
            filter: String::new(),
            filtering: false,
            help_scroll: 0,
            refresh_counter: 0,
            scan_child: None,
            scan_pending: 0,
            bg_tx: tx,
            bg_rx: rx,
        };
        app.table_state.select(Some(0));
        app
    }

    pub fn initial_load(&mut self) {
        self.controller = bt::get_controller();
        self.devices = bt::get_devices();
        self.rebuild();
    }

    pub fn refresh_devices(&mut self) {
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let devices = bt::get_devices();
            let _ = tx.send(BgMsg::DeviceList(devices));
        });
    }

    pub fn refresh_controller(&mut self) {
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let ctrl = bt::get_controller();
            let _ = tx.send(BgMsg::ControllerInfo(ctrl));
        });
    }

    pub fn toggle_scan(&mut self) {
        if let Some(child) = self.scan_child.take() {
            bt::stop_scan(child);
            self.scanning = false;
            self.scan_pending = 0;
            self.status.set("Scan OFF");
            self.refresh_devices();
        } else {
            match bt::spawn_scan() {
                Ok(child) => {
                    self.scan_child = Some(child);
                    self.scanning = true;
                    // wait 2 ticks (500ms) for bluetoothctl to connect before sending command
                    self.scan_pending = 2;
                    self.status.set("Scan ON");
                }
                Err(e) => {
                    self.status.set(format!("Scan failed: {e}"));
                }
            }
        }
    }

    pub fn toggle_power(&mut self) {
        let on = !self.controller.powered;
        let label = if on { "Power ON..." } else { "Power OFF..." };
        self.status.set(label);
        if !on {
            if let Some(child) = self.scan_child.take() {
                bt::stop_scan(child);
                self.scanning = false;
            }
        }
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let result = bt::set_power(on);
            let _ = tx.send(BgMsg::ActionResult(result));
        });
    }

    pub fn toggle_discoverable(&mut self) {
        let on = !self.controller.discoverable;
        let label = if on { "Discoverable ON" } else { "Discoverable OFF" };
        self.status.set(label);
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let result = bt::set_discoverable(on);
            let _ = tx.send(BgMsg::ActionResult(result));
        });
    }

    pub fn rebuild(&mut self) {
        let filter_lower = self.filter.to_lowercase();
        self.filtered = if self.filter.is_empty() {
            (0..self.devices.len()).collect()
        } else {
            self.devices
                .iter()
                .enumerate()
                .filter(|(_, d)| d.name.to_lowercase().contains(&filter_lower))
                .map(|(i, _)| i)
                .collect()
        };
        self.clamp_selection();
    }

    pub fn selected_device(&self) -> Option<&Device> {
        let idx = self.table_state.selected()?;
        self.filtered.get(idx).and_then(|&i| self.devices.get(i))
    }

    pub fn connect_selected(&mut self) {
        let Some(dev) = self.selected_device() else { return };
        let addr = dev.address.clone();
        let name = dev.name.clone();

        if dev.connected {
            self.status.set(format!("Disconnecting {name}..."));
            let tx = self.bg_tx.clone();
            thread::spawn(move || {
                let result = bt::disconnect(&addr);
                let _ = tx.send(BgMsg::ActionResult(result));
            });
        } else {
            self.status.set(format!("Connecting {name}..."));
            let tx = self.bg_tx.clone();
            thread::spawn(move || {
                let result = bt::connect(&addr);
                let _ = tx.send(BgMsg::ActionResult(result));
            });
        }
    }

    pub fn pair_selected(&mut self) {
        let Some(dev) = self.selected_device() else { return };
        let addr = dev.address.clone();
        let name = dev.name.clone();

        if dev.paired {
            self.status.set(format!("Removing {name}..."));
            let tx = self.bg_tx.clone();
            thread::spawn(move || {
                let result = bt::unpair(&addr);
                let _ = tx.send(BgMsg::ActionResult(result));
            });
        } else {
            self.status.set(format!("Pairing {name}..."));
            let tx = self.bg_tx.clone();
            thread::spawn(move || {
                let result = bt::pair(&addr);
                let _ = tx.send(BgMsg::ActionResult(result));
            });
        }
    }

    pub fn trust_selected(&mut self) {
        let Some(dev) = self.selected_device() else { return };
        let addr = dev.address.clone();
        let name = dev.name.clone();

        if dev.trusted {
            self.status.set(format!("Untrusting {name}..."));
            let tx = self.bg_tx.clone();
            thread::spawn(move || {
                let result = bt::untrust(&addr);
                let _ = tx.send(BgMsg::ActionResult(result));
            });
        } else {
            self.status.set(format!("Trusting {name}..."));
            let tx = self.bg_tx.clone();
            thread::spawn(move || {
                let result = bt::trust(&addr);
                let _ = tx.send(BgMsg::ActionResult(result));
            });
        }
    }

    pub fn remove_selected(&mut self) {
        let Some(dev) = self.selected_device() else { return };
        let addr = dev.address.clone();
        let name = dev.name.clone();
        self.status.set(format!("Removing {name}..."));
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let result = bt::unpair(&addr);
            let _ = tx.send(BgMsg::ActionResult(result));
        });
    }

    pub fn select_next(&mut self) {
        let len = self.filtered.len();
        if len == 0 {
            return;
        }
        let next = self.table_state.selected().unwrap_or(0) + 1;
        if next < len {
            self.table_state.select(Some(next));
        }
    }

    pub fn select_prev(&mut self) {
        let prev = self.table_state.selected().unwrap_or(0).saturating_sub(1);
        self.table_state.select(Some(prev));
    }

    pub fn select_first(&mut self) {
        self.table_state.select(Some(0));
    }

    pub fn select_last(&mut self) {
        if !self.filtered.is_empty() {
            self.table_state.select(Some(self.filtered.len() - 1));
        }
    }

    fn clamp_selection(&mut self) {
        let len = self.filtered.len();
        if len == 0 {
            self.table_state.select(Some(0));
            return;
        }
        let idx = self.table_state.selected().unwrap_or(0).min(len - 1);
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
                BgMsg::DeviceList(devices) => {
                    self.devices = devices;
                    let n = self.devices.len();
                    let connected = self.devices.iter().filter(|d| d.connected).count();
                    self.rebuild();
                    self.status.set(format!("{n} devices, {connected} connected"));
                }
                BgMsg::ControllerInfo(ctrl) => {
                    self.controller = ctrl;
                }
                BgMsg::ActionResult(result) => {
                    match result {
                        Ok(msg) => self.status.set(msg),
                        Err(e) => self.status.set(format!("Error: {e}")),
                    }
                    self.refresh_devices();
                    self.refresh_controller();
                }
            }
        }

        self.status.tick();

        if self.scan_pending > 0 {
            self.scan_pending -= 1;
            if self.scan_pending == 0 {
                if let Some(ref mut child) = self.scan_child {
                    bt::send_scan_on(child);
                }
            }
        }

        self.refresh_counter += 1;
        if self.refresh_counter >= 8 {
            self.refresh_counter = 0;
            self.refresh_devices();
            self.refresh_controller();
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

    fn handle_left_click(&mut self, row: u16, col: u16, term_height: u16) {
        if row == term_height.saturating_sub(1) {
            crate::input::handle_hotbar_click(self, col);
        } else if row >= 3 {
            let clicked = (row - 3) as usize;
            if clicked < self.filtered.len() {
                self.table_state.select(Some(clicked));
            }
        }
    }

    fn should_quit(&self) -> bool {
        self.should_quit
    }

    fn cleanup(&mut self) {
        if let Some(child) = self.scan_child.take() {
            bt::stop_scan(child);
        }
    }
}
