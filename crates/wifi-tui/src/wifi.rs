use std::collections::HashSet;
use std::process::Command;
use std::sync::LazyLock;

use regex::Regex;

// ── Data types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Network {
    pub ssid: String,
    pub bssid: String,
    pub freq: f64,
    pub signal: f64,
    pub security: String,
    pub gen: String,
    pub band: String,
    pub channel: String,
    pub channel_width: String,
    pub streams: u8,
    pub associated: bool,
    pub wps: bool,
    pub clients: Option<u32>,
    pub util: Option<f64>,
    pub mu_mimo: bool,
    pub twt: bool,
    pub vendor: String,
}

#[derive(Debug, Clone, Default)]
pub struct Modulation {
    pub mod_type: String,
    pub mcs: String,
    pub nss: String,
    pub gi: String,
}

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub ssid: String,
    pub bssid: String,
    pub freq: f64,
    pub signal: Option<i32>,
    pub signal_avg: Option<i32>,
    pub signal_chains: Option<String>,
    pub tx_bitrate: f64,
    pub rx_bitrate: f64,
    pub tx_mod: Modulation,
    pub rx_mod: Modulation,
    pub tx_retries: u64,
    pub tx_packets: u64,
    pub tx_failed: u64,
    pub beacon_loss: u64,
    pub connected_time: u64,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub channel: String,
    pub width: String,
    pub regdom: String,
    pub gen: String,
    pub band: String,
    pub tx_power: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct DeviceState {
    pub state: String,
    pub connection: String,
}

// ── Regex patterns ──────────────────────────────────────────────────────────

static RE_BSSID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"BSS ([0-9a-f:]{17})").unwrap());
static RE_FREQ: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"freq: ([\d.]+)").unwrap());
static RE_SIGNAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"signal: (-?[\d.]+)").unwrap());
static RE_SSID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s+SSID: (.+)$").unwrap());
static RE_PRIMARY_CH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"primary channel: (\d+)").unwrap());
static RE_CH_WIDTH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"channel width: \d+ \(([^)]+)\)").unwrap());
static RE_STREAMS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+) streams: MCS").unwrap());
static RE_STATION_COUNT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"station count: (\d+)").unwrap());
static RE_CH_UTIL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"channel utilisation: (\d+)/(\d+)").unwrap());
static RE_AUTH_SUITES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Authentication suites: ([^\n*]+)").unwrap());
static RE_WPS_MANUFACTURER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Manufacturer: ([^\n*]+)").unwrap());
static RE_WPS_DEVICE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Device name: ([^\n*]+)").unwrap());

static RE_CONNECTED_TO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Connected to ([0-9a-f:]{17})").unwrap());
static RE_BITRATE_MCS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(EHT|HE|VHT|HT)-MCS (\d+)").unwrap());
static RE_NSS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:EHT|HE|VHT)-NSS (\d+)").unwrap());
static RE_GI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:EHT|HE|VHT)-GI ([\d.]+)").unwrap());
static RE_IW_CHANNEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"channel (\d+) \([\d.]+ MHz\), width: (\d+ MHz)").unwrap());

// ── Helpers ─────────────────────────────────────────────────────────────────

pub fn get_interface() -> Option<String> {
    let out = run_cmd("iw", &["dev"]);
    for line in out.lines() {
        let t = line.trim();
        if let Some(iface) = t.strip_prefix("Interface ") {
            return Some(iface.to_string());
        }
    }
    None
}

fn run_cmd(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

pub fn wifi_gen(caps: &HashSet<&str>, freq: f64) -> &'static str {
    if caps.contains("EHT") {
        return "Gen 7";
    }
    if caps.contains("HE") {
        return if freq >= 5925.0 { "Gen 6E" } else { "Gen 6" };
    }
    if caps.contains("VHT") {
        return "Gen 5";
    }
    if caps.contains("HT") {
        return "Gen 4";
    }
    "Legacy"
}

fn gen_from_bitrate(bitrate_str: &str, freq: f64) -> &'static str {
    if bitrate_str.contains("EHT") {
        return "Wi-Fi 7";
    }
    if bitrate_str.contains("HE") {
        return if freq >= 5925.0 { "Wi-Fi 6E" } else { "Wi-Fi 6" };
    }
    if bitrate_str.contains("VHT") {
        return "Wi-Fi 5";
    }
    if bitrate_str.contains("MCS") {
        return "Wi-Fi 4";
    }
    "Legacy"
}

pub fn band_label(freq: f64) -> &'static str {
    if freq >= 5925.0 {
        "6 GHz"
    } else if freq >= 5000.0 {
        "5 GHz"
    } else {
        "2.4 GHz"
    }
}

pub fn signal_quality(dbm: f64) -> &'static str {
    if dbm >= -50.0 {
        "Excellent"
    } else if dbm >= -60.0 {
        "Good"
    } else if dbm >= -67.0 {
        "Fair"
    } else if dbm >= -75.0 {
        "Weak"
    } else {
        "Poor"
    }
}

pub fn format_bytes(b: u64) -> String {
    if b >= 1_073_741_824 {
        format!("{:.1} GB", b as f64 / 1_073_741_824.0)
    } else if b >= 1_048_576 {
        format!("{:.1} MB", b as f64 / 1_048_576.0)
    } else if b >= 1024 {
        format!("{:.1} KB", b as f64 / 1024.0)
    } else {
        format!("{b} B")
    }
}

pub fn format_time(secs: u64) -> String {
    if secs >= 86400 {
        format!("{}d {}h", secs / 86400, (secs % 86400) / 3600)
    } else if secs >= 3600 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

pub fn format_retries(retries: u64, total: u64) -> (String, &'static str) {
    if total == 0 {
        return ("-".into(), "dim");
    }
    if retries > total * 10 || retries > 4_000_000_000 {
        return ("overflow".into(), "dim");
    }
    let pct = retries as f64 / total as f64 * 100.0;
    let color = if pct < 5.0 {
        "green"
    } else if pct < 15.0 {
        "yellow"
    } else {
        "red"
    };
    (format!("{pct:.1}% ({retries}/{total})"), color)
}

fn parse_modulation(text: &str) -> Modulation {
    let mut m = Modulation::default();
    if let Some(caps) = RE_BITRATE_MCS.captures(text) {
        m.mod_type = caps[1].to_string();
        m.mcs = caps[2].to_string();
    }
    if let Some(caps) = RE_NSS.captures(text) {
        m.nss = caps[1].to_string();
    }
    if let Some(caps) = RE_GI.captures(text) {
        m.gi = caps[1].to_string();
    }
    m
}

fn get_channel_width(block: &str) -> String {
    if let Some(caps) = RE_CH_WIDTH.captures(block) {
        return caps[1].to_string();
    }
    if block.contains("HT20/HT40") {
        return "40 MHz".into();
    }
    if block.contains("HT20") {
        return "20 MHz".into();
    }
    String::new()
}

fn get_max_streams(block: &str) -> u8 {
    let mut max_ss = 0u8;
    for caps in RE_STREAMS.captures_iter(block) {
        if let Ok(n) = caps[1].parse::<u8>() {
            max_ss = max_ss.max(n);
        }
    }
    max_ss
}

// ── Scan parsing ────────────────────────────────────────────────────────────

fn split_bss_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        if line.starts_with("BSS ") && !current.is_empty() {
            blocks.push(std::mem::take(&mut current));
        }
        current.push_str(line);
        current.push('\n');
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    blocks
}

fn parse_bss_block(block: &str) -> Option<Network> {
    let bssid = RE_BSSID.captures(block).map(|c| c[1].to_string())?;
    let ssid = RE_SSID
        .captures(block)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();
    if ssid.is_empty() {
        return None;
    }

    let freq = RE_FREQ
        .captures(block)
        .and_then(|c| c[1].parse::<f64>().ok())
        .unwrap_or(0.0);
    let signal = RE_SIGNAL
        .captures(block)
        .and_then(|c| c[1].parse::<f64>().ok())
        .unwrap_or(-100.0);

    let associated = block.contains("-- associated");

    // Security
    let mut security = if block.contains("RSN:") {
        "WPA2"
    } else if block.contains("WPA:") {
        "WPA"
    } else {
        "Open"
    };
    if block.contains("802.1X") {
        security = "Enterprise";
    }
    let auth = RE_AUTH_SUITES
        .captures(block)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();
    if auth.contains("PSK SAE") {
        security = "WPA2/3";
    } else if auth.contains("SAE") {
        security = "WPA3";
    }

    // Capabilities
    let mut caps: HashSet<&str> = HashSet::new();
    if block.contains("HT capabilities:") || block.contains("HT operation:") {
        caps.insert("HT");
    }
    if block.contains("VHT capabilities:") || block.contains("VHT operation:") {
        caps.insert("VHT");
    }
    if block.contains("HE capabilities:") || block.contains("HE Operation:") {
        caps.insert("HE");
    }
    if block.contains("EHT capabilities:") || block.contains("EHT Operation:") {
        caps.insert("EHT");
    }

    let channel = RE_PRIMARY_CH
        .captures(block)
        .map(|c| c[1].to_string())
        .unwrap_or_default();
    let channel_width = get_channel_width(block);
    let streams = get_max_streams(block);
    let wps = block.contains("WPS:") || block.contains("Wi-Fi Protected Setup");
    let mu_mimo = block.contains("MU Beamformer");
    let twt = block.contains("TWT Responder");

    let clients = RE_STATION_COUNT
        .captures(block)
        .and_then(|c| c[1].parse().ok());
    let util = RE_CH_UTIL.captures(block).and_then(|c| {
        let num: f64 = c[1].parse().ok()?;
        let den: f64 = c[2].parse().ok()?;
        if den > 0.0 {
            Some(num / den * 100.0)
        } else {
            None
        }
    });

    let vendor = RE_WPS_MANUFACTURER
        .captures(block)
        .or_else(|| RE_WPS_DEVICE.captures(block))
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    let gen = wifi_gen(&caps, freq).to_string();
    let band = band_label(freq).to_string();

    Some(Network {
        ssid,
        bssid,
        freq,
        signal,
        security: security.to_string(),
        gen,
        band,
        channel,
        channel_width,
        streams,
        associated,
        wps,
        clients,
        util,
        mu_mimo,
        twt,
        vendor,
    })
}

pub fn parse_scan(text: &str) -> Vec<Network> {
    let blocks = split_bss_blocks(text);
    let mut networks: Vec<Network> = blocks.iter().filter_map(|b| parse_bss_block(b)).collect();

    // Sort: associated first, then alphabetical by SSID, then by signal (strongest first)
    networks.sort_by(|a, b| {
        a.associated
            .cmp(&b.associated)
            .reverse()
            .then_with(|| a.ssid.to_lowercase().cmp(&b.ssid.to_lowercase()))
            .then_with(|| b.signal.partial_cmp(&a.signal).unwrap_or(std::cmp::Ordering::Equal))
    });
    networks
}

// ── High-level operations ───────────────────────────────────────────────────

pub fn get_device_state(iface: &str) -> DeviceState {
    let out = run_cmd("nmcli", &["-t", "device", "status"]);
    for line in out.lines() {
        let parts: Vec<&str> = line.splitn(4, ':').collect();
        if parts.len() >= 3 && parts[0] == iface {
            return DeviceState {
                state: parts[2].to_string(),
                connection: parts.get(3).map(|s| s.replace("\\:", ":")).unwrap_or_default(),
            };
        }
    }
    DeviceState {
        state: "unknown".into(),
        connection: String::new(),
    }
}

pub fn scan_networks(iface: &str) -> Vec<Network> {
    // Trigger rescan via NetworkManager
    let _ = Command::new("nmcli")
        .args(["device", "wifi", "rescan"])
        .output();

    // Read cached scan results
    let text = run_cmd("iw", &["dev", iface, "scan", "dump"]);
    if text.is_empty() {
        // Fallback: try nmcli for basic list
        return scan_via_nmcli();
    }
    parse_scan(&text)
}

fn scan_via_nmcli() -> Vec<Network> {
    let out = run_cmd(
        "nmcli",
        &[
            "-t",
            "-f",
            "SSID,BSSID,CHAN,FREQ,SIGNAL,SECURITY,IN-USE",
            "device",
            "wifi",
            "list",
        ],
    );
    let mut networks = Vec::new();
    for line in out.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() < 7 {
            continue;
        }
        let ssid = fields[0].replace("\\:", ":").trim().to_string();
        if ssid.is_empty() {
            continue;
        }
        let bssid = fields[1..7].join(":").trim().to_string();
        let remaining = &fields[7..];
        if remaining.len() < 5 {
            continue;
        }
        let chan = remaining[0].to_string();
        let freq: f64 = remaining[1]
            .trim_end_matches(" MHz")
            .parse()
            .unwrap_or(0.0);
        let signal_pct: f64 = remaining[2].parse().unwrap_or(0.0);
        let signal_dbm = (signal_pct / 2.0) - 100.0;
        let security = remaining[3].to_string();
        let in_use = remaining[4].contains('*');

        networks.push(Network {
            ssid,
            bssid,
            freq,
            signal: signal_dbm,
            security,
            gen: String::new(),
            band: band_label(freq).to_string(),
            channel: chan,
            channel_width: String::new(),
            streams: 0,
            associated: in_use,
            wps: false,
            clients: None,
            util: None,
            mu_mimo: false,
            twt: false,
            vendor: String::new(),
        });
    }
    networks.sort_by(|a, b| {
        a.associated
            .cmp(&b.associated)
            .reverse()
            .then_with(|| b.signal.partial_cmp(&a.signal).unwrap_or(std::cmp::Ordering::Equal))
    });
    networks
}

pub fn get_connection_info(iface: &str) -> Option<ConnectionInfo> {
    let link = run_cmd("iw", &["dev", iface, "link"]);
    if link.contains("Not connected") || link.is_empty() {
        return None;
    }
    let station = run_cmd("iw", &["dev", iface, "station", "dump"]);
    let info = run_cmd("iw", &["dev", iface, "info"]);
    let reg = run_cmd("iw", &["reg", "get"]);

    let bssid = RE_CONNECTED_TO
        .captures(&link)
        .map(|c| c[1].to_string())
        .unwrap_or_default();
    let ssid = RE_SSID
        .captures(&link)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();
    let freq = RE_FREQ
        .captures(&link)
        .and_then(|c| c[1].parse().ok())
        .unwrap_or(0.0);
    let signal = Regex::new(r"signal: (-?\d+)")
        .ok()
        .and_then(|re| re.captures(&link))
        .and_then(|c| c[1].parse().ok());

    let rx_bytes = Regex::new(r"RX: (\d+) bytes")
        .ok()
        .and_then(|re| re.captures(&link))
        .and_then(|c| c[1].parse().ok())
        .unwrap_or(0);
    let tx_bytes = Regex::new(r"TX: (\d+) bytes")
        .ok()
        .and_then(|re| re.captures(&link))
        .and_then(|c| c[1].parse().ok())
        .unwrap_or(0);

    let tx_bitrate_str = Regex::new(r"tx bitrate: (.+)")
        .ok()
        .and_then(|re| re.captures(&link))
        .map(|c| c[1].to_string())
        .unwrap_or_default();
    let rx_bitrate_str = Regex::new(r"rx bitrate: (.+)")
        .ok()
        .and_then(|re| re.captures(&link))
        .map(|c| c[1].to_string())
        .unwrap_or_default();

    let tx_bitrate = tx_bitrate_str
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let rx_bitrate = rx_bitrate_str
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let signal_avg = Regex::new(r"signal avg:\s*(-?\d+)")
        .ok()
        .and_then(|re| re.captures(&station))
        .and_then(|c| c[1].parse().ok());
    let signal_chains = Regex::new(r"signal:\s*-?\d+ \[([^\]]+)\]")
        .ok()
        .and_then(|re| re.captures(&station))
        .map(|c| c[1].to_string());

    let tx_retries = Regex::new(r"tx retries:\s*(\d+)")
        .ok()
        .and_then(|re| re.captures(&station))
        .and_then(|c| c[1].parse().ok())
        .unwrap_or(0);
    let tx_packets = Regex::new(r"tx packets:\s*(\d+)")
        .ok()
        .and_then(|re| re.captures(&station))
        .and_then(|c| c[1].parse().ok())
        .unwrap_or(0);
    let tx_failed = Regex::new(r"tx failed:\s*(\d+)")
        .ok()
        .and_then(|re| re.captures(&station))
        .and_then(|c| c[1].parse().ok())
        .unwrap_or(0);
    let beacon_loss = Regex::new(r"beacon loss:\s*(\d+)")
        .ok()
        .and_then(|re| re.captures(&station))
        .and_then(|c| c[1].parse().ok())
        .unwrap_or(0);
    let connected_time = Regex::new(r"connected time:\s*(\d+)")
        .ok()
        .and_then(|re| re.captures(&station))
        .and_then(|c| c[1].parse().ok())
        .unwrap_or(0);

    let (channel, width) = RE_IW_CHANNEL
        .captures(&info)
        .map(|c| (c[1].to_string(), c[2].to_string()))
        .unwrap_or_default();
    let tx_power = Regex::new(r"txpower ([\d.]+) dBm")
        .ok()
        .and_then(|re| re.captures(&info))
        .and_then(|c| c[1].parse().ok());
    let regdom = Regex::new(r"country (\w+):")
        .ok()
        .and_then(|re| re.captures(&reg))
        .map(|c| c[1].to_string())
        .unwrap_or_default();

    let gen = gen_from_bitrate(&tx_bitrate_str, freq).to_string();
    let band = band_label(freq).to_string();

    Some(ConnectionInfo {
        ssid,
        bssid,
        freq,
        signal,
        signal_avg,
        signal_chains,
        tx_bitrate,
        rx_bitrate,
        tx_mod: parse_modulation(&tx_bitrate_str),
        rx_mod: parse_modulation(&rx_bitrate_str),
        tx_retries,
        tx_packets,
        tx_failed,
        beacon_loss,
        connected_time,
        tx_bytes,
        rx_bytes,
        channel,
        width,
        regdom,
        gen,
        band,
        tx_power,
    })
}

pub fn connect(
    ssid: &str,
    bssid: Option<&str>,
    password: Option<&str>,
) -> Result<String, String> {
    let mut args = vec!["device", "wifi", "connect", ssid];
    if let Some(b) = bssid {
        args.extend(["bssid", b]);
    }
    if let Some(pw) = password {
        args.extend(["password", pw]);
    }
    let output = Command::new("nmcli")
        .args(&args)
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let label = if let Some(b) = bssid {
        format!("{ssid} ({b})")
    } else {
        ssid.to_string()
    };

    if output.status.success() {
        Ok(format!("Connected to {label}"))
    } else {
        let msg = if stderr.trim().is_empty() {
            stdout
        } else {
            stderr.trim().to_string()
        };
        Err(msg)
    }
}

pub fn disconnect(iface: &str) -> Result<String, String> {
    let output = Command::new("nmcli")
        .args(["device", "disconnect", iface])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok("Disconnected".into())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(stderr.trim().to_string())
    }
}

pub fn set_wifi_power(on: bool) -> Result<String, String> {
    let arg = if on { "on" } else { "off" };
    let output = Command::new("nmcli")
        .args(["radio", "wifi", arg])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(if on { "WiFi ON".into() } else { "WiFi OFF".into() })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(stderr.trim().to_string())
    }
}

pub fn is_wifi_on() -> bool {
    let out = run_cmd("nmcli", &["radio", "wifi"]);
    out.trim() == "enabled"
}

pub fn saved_connections() -> HashSet<String> {
    let out = run_cmd("nmcli", &["-t", "-f", "NAME", "connection", "show"]);
    out.lines().map(|l| l.trim().to_string()).collect()
}
