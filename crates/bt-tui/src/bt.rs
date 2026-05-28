use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Device {
    pub address: String,
    pub name: String,
    pub icon: String,
    pub paired: bool,
    pub bonded: bool,
    pub trusted: bool,
    pub blocked: bool,
    pub connected: bool,
    pub rssi: Option<i32>,
    pub modalias: String,
    pub uuids: Vec<String>,
    pub battery: Option<u8>,
    pub transport: String, // "BREDR", "LE", or "dual"
}

#[derive(Debug, Clone)]
pub struct Controller {
    pub address: String,
    pub name: String,
    pub powered: bool,
    pub discoverable: bool,
    pub pairable: bool,
    pub discovering: bool,
}

impl Default for Controller {
    fn default() -> Self {
        Self {
            address: String::new(),
            name: String::new(),
            powered: false,
            discoverable: false,
            pairable: false,
            discovering: false,
        }
    }
}

pub fn get_controller() -> Controller {
    let out = run_cmd("bluetoothctl", &["show"]);
    let mut ctrl = Controller::default();

    for line in out.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Controller ") {
            if let Some((addr, _)) = rest.split_once(' ') {
                ctrl.address = addr.to_string();
            }
        } else if let Some(v) = t.strip_prefix("Name: ") {
            ctrl.name = v.to_string();
        } else if let Some(v) = t.strip_prefix("Alias: ") {
            if ctrl.name.is_empty() {
                ctrl.name = v.to_string();
            }
        } else if let Some(v) = t.strip_prefix("Powered: ") {
            ctrl.powered = v == "yes";
        } else if let Some(v) = t.strip_prefix("Discoverable: ") {
            ctrl.discoverable = v == "yes";
        } else if let Some(v) = t.strip_prefix("Pairable: ") {
            ctrl.pairable = v == "yes";
        } else if let Some(v) = t.strip_prefix("Discovering: ") {
            ctrl.discovering = v == "yes";
        }
    }
    ctrl
}

pub fn get_devices() -> Vec<Device> {
    let all_out = run_cmd("bluetoothctl", &["devices"]);
    let paired_out = run_cmd("bluetoothctl", &["devices", "Paired"]);
    let connected_out = run_cmd("bluetoothctl", &["devices", "Connected"]);
    let trusted_out = run_cmd("bluetoothctl", &["devices", "Trusted"]);

    let paired_set = parse_device_list(&paired_out);
    let connected_set = parse_device_list(&connected_out);
    let trusted_set = parse_device_list(&trusted_out);

    let mut devices = Vec::new();

    for line in all_out.lines() {
        let parts: Vec<&str> = line.splitn(3, ' ').collect();
        if parts.len() < 3 || parts[0] != "Device" {
            continue;
        }
        let addr = parts[1].to_string();
        let name = parts[2].to_string();

        let info = get_device_info(&addr);
        let icon = info.get("Icon").cloned().unwrap_or_default();
        let bonded = info.get("Bonded").map(|v| v == "yes").unwrap_or(false);
        let blocked = info.get("Blocked").map(|v| v == "yes").unwrap_or(false);
        let modalias = info.get("Modalias").cloned().unwrap_or_default();

        let rssi = info.get("RSSI").and_then(|v| parse_rssi(v));

        let uuids: Vec<String> = info
            .iter()
            .filter(|(k, _)| k.starts_with("UUID"))
            .map(|(_, v)| v.clone())
            .collect();

        let transport = if info.contains_key("BREDR.Connected") && info.contains_key("LE.Connected") {
            "dual".to_string()
        } else if info.contains_key("LE.Connected") || info.contains_key("LE.Paired") {
            "LE".to_string()
        } else {
            "BR/EDR".to_string()
        };

        let battery = get_battery_level(&addr);

        devices.push(Device {
            address: addr.clone(),
            name,
            icon,
            paired: paired_set.contains(&addr),
            bonded,
            trusted: trusted_set.contains(&addr),
            blocked,
            connected: connected_set.contains(&addr),
            rssi,
            modalias,
            uuids,
            battery,
            transport,
        });
    }

    devices.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then_with(|| b.paired.cmp(&a.paired))
            .then_with(|| b.trusted.cmp(&a.trusted))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    devices
}

fn get_device_info(addr: &str) -> HashMap<String, String> {
    let out = run_cmd("bluetoothctl", &["info", addr]);
    let mut map = HashMap::new();
    let mut uuid_idx = 0;

    for line in out.lines() {
        let t = line.trim();
        if let Some((key, val)) = t.split_once(": ") {
            let key = key.trim();
            let val = val.trim();
            if key == "UUID" {
                map.insert(format!("UUID_{uuid_idx}"), val.to_string());
                uuid_idx += 1;
            } else {
                map.insert(key.to_string(), val.to_string());
            }
        } else if t.contains(".Connected:") || t.contains(".Paired:") || t.contains(".Bonded:") {
            if let Some((key, val)) = t.split_once(": ") {
                map.insert(key.trim().to_string(), val.trim().to_string());
            }
        }
    }
    map
}

fn get_battery_level(addr: &str) -> Option<u8> {
    let mac_path = addr.replace(':', "_");
    let out = Command::new("dbus-send")
        .args([
            "--system",
            "--print-reply",
            "--dest=org.bluez",
            &format!("/org/bluez/hci0/dev_{mac_path}"),
            "org.freedesktop.DBus.Properties.Get",
            "string:org.bluez.Battery1",
            "string:Percentage",
        ])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        let t = line.trim();
        if t.starts_with("variant") || t.starts_with("byte") {
            if let Some(num) = t.split_whitespace().last() {
                return num.parse().ok();
            }
        }
    }
    None
}

fn parse_rssi(val: &str) -> Option<i32> {
    // Format: "0xffffffaa (-86)" — prefer the decimal in parens
    if let Some(start) = val.find('(') {
        if let Some(end) = val.find(')') {
            if let Ok(n) = val[start + 1..end].trim().parse::<i32>() {
                return Some(n);
            }
        }
    }
    // Fallback: parse hex as signed i32
    let token = val.split_whitespace().next()?;
    if let Some(hex) = token.strip_prefix("0x") {
        let unsigned = u32::from_str_radix(hex, 16).ok()?;
        Some(unsigned as i32)
    } else {
        token.parse().ok()
    }
}

fn parse_device_list(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(3, ' ').collect();
            if parts.len() >= 2 && parts[0] == "Device" {
                Some(parts[1].to_string())
            } else {
                None
            }
        })
        .collect()
}

pub fn set_power(on: bool) -> Result<String, String> {
    let arg = if on { "on" } else { "off" };
    let output = Command::new("bluetoothctl")
        .args(["power", arg])
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("succeeded") || output.status.success() {
        Ok(if on { "Power ON".into() } else { "Power OFF".into() })
    } else {
        Err("Failed to change power state".into())
    }
}

pub fn set_discoverable(on: bool) -> Result<String, String> {
    let arg = if on { "on" } else { "off" };
    let output = Command::new("bluetoothctl")
        .args(["discoverable", arg])
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("succeeded") || output.status.success() {
        Ok(if on { "Discoverable ON".into() } else { "Discoverable OFF".into() })
    } else {
        Err("Failed to change discoverable state".into())
    }
}

use std::process::Child;
use std::process::Stdio;

pub fn spawn_scan() -> Result<Child, String> {
    // Start bluetoothctl with piped stdin so it stays alive.
    // Send "scan on" after a brief delay for bluetoothd connection.
    let child = Command::new("bluetoothctl")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(child)
}

pub fn send_scan_on(child: &mut Child) {
    if let Some(ref mut stdin) = child.stdin {
        use std::io::Write;
        let _ = stdin.write_all(b"scan on\n");
        let _ = stdin.flush();
    }
}

pub fn stop_scan(mut child: Child) {
    std::thread::spawn(move || {
        if let Some(ref mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(b"scan off\n");
            let _ = stdin.flush();
        }
        // dropping stdin closes the pipe; wait reaps the zombie
        let _ = child.wait();
    });
}

pub fn connect(addr: &str) -> Result<String, String> {
    let output = Command::new("bluetoothctl")
        .args(["connect", addr])
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("Connection successful") {
        Ok("Connected".into())
    } else {
        let msg = stdout
            .lines()
            .find(|l| l.contains("Failed") || l.contains("Error"))
            .unwrap_or("Connection failed")
            .to_string();
        Err(msg)
    }
}

pub fn disconnect(addr: &str) -> Result<String, String> {
    let output = Command::new("bluetoothctl")
        .args(["disconnect", addr])
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("Successful") || output.status.success() {
        Ok("Disconnected".into())
    } else {
        Err("Disconnect failed".into())
    }
}

pub fn pair(addr: &str) -> Result<String, String> {
    let output = Command::new("bluetoothctl")
        .args(["pair", addr])
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("Pairing successful") || stdout.contains("Already Paired") {
        Ok("Paired".into())
    } else {
        let msg = stdout
            .lines()
            .find(|l| l.contains("Failed") || l.contains("Error"))
            .unwrap_or("Pairing failed")
            .to_string();
        Err(msg)
    }
}

pub fn unpair(addr: &str) -> Result<String, String> {
    let output = Command::new("bluetoothctl")
        .args(["remove", addr])
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("removed") || output.status.success() {
        Ok("Removed".into())
    } else {
        Err("Remove failed".into())
    }
}

pub fn trust(addr: &str) -> Result<String, String> {
    let output = Command::new("bluetoothctl")
        .args(["trust", addr])
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("trust succeeded") || stdout.contains("Trusted: yes") {
        Ok("Trusted".into())
    } else {
        Err("Trust failed".into())
    }
}

pub fn untrust(addr: &str) -> Result<String, String> {
    let output = Command::new("bluetoothctl")
        .args(["untrust", addr])
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("untrust succeeded") || stdout.contains("Trusted: no") {
        Ok("Untrusted".into())
    } else {
        Err("Untrust failed".into())
    }
}

fn run_cmd(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

pub fn device_type_label(icon: &str) -> &str {
    match icon {
        "audio-headphones" => "Headphones",
        "audio-headset" => "Headset",
        "audio-card" => "Speaker",
        "phone" => "Phone",
        "computer" => "Computer",
        "input-keyboard" => "Keyboard",
        "input-mouse" => "Mouse",
        "input-gaming" => "Gamepad",
        "input-tablet" => "Tablet",
        "camera-photo" => "Camera",
        "camera-video" => "Webcam",
        "printer" => "Printer",
        "scanner" => "Scanner",
        "modem" => "Modem",
        "network-wireless" => "Network",
        "video-display" => "Display",
        _ => if icon.is_empty() { "Unknown" } else { icon },
    }
}

pub fn device_type_icon(icon: &str) -> &str {
    match icon {
        "audio-headphones" | "audio-headset" => "🎧",
        "audio-card" => "🔊",
        "phone" => "📱",
        "computer" => "💻",
        "input-keyboard" => "⌨️",
        "input-mouse" => "🖱️",
        "input-gaming" => "🎮",
        "input-tablet" => "📝",
        "camera-photo" | "camera-video" => "📷",
        "printer" => "🖨️",
        "video-display" => "🖥️",
        "network-wireless" => "📡",
        _ => "📶",
    }
}

pub fn vendor_from_modalias(modalias: &str) -> &str {
    if let Some(rest) = modalias.strip_prefix("bluetooth:v") {
        let vid = &rest[..4.min(rest.len())];
        match vid.to_uppercase().as_str() {
            "004C" => "Apple",
            "000F" => "Broadcom",
            "0075" => "Samsung",
            "001D" => "Qualcomm",
            "000A" => "CSR",
            "0046" => "MediaTek",
            "000D" => "Texas Instruments",
            "0006" => "Microsoft",
            "0059" | "038F" => "LG",
            "0009" => "Intel",
            "0002" => "Intel",
            "000E" => "Ericsson",
            "0056" => "Sony",
            "0094" => "Bose",
            "012D" => "JBL/Harman",
            "0131" => "Sennheiser",
            _ => "",
        }
    } else {
        ""
    }
}
