//! ADB client — device management and logcat streaming.

use std::process::Stdio;
use tokio::process::{Child, Command};

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceState {
    Device,
    Offline,
    Unauthorized,
    Unknown(String),
}

impl DeviceState {
    fn from_str(s: &str) -> Self {
        match s {
            "device" => Self::Device,
            "offline" => Self::Offline,
            "unauthorized" => Self::Unauthorized,
            other => Self::Unknown(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Device => "device",
            Self::Offline => "offline",
            Self::Unauthorized => "unauthorized",
            Self::Unknown(s) => s,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionType {
    Usb,
    Tcp,
}

#[derive(Debug, Clone)]
pub struct Device {
    pub serial: String,
    pub state: DeviceState,
    pub model: String,
    pub product: String,
}

impl Device {
    pub fn connection_type(&self) -> ConnectionType {
        if self.serial.contains(':') {
            ConnectionType::Tcp
        } else {
            ConnectionType::Usb
        }
    }

    pub fn is_online(&self) -> bool {
        self.state == DeviceState::Device
    }

    pub fn display_name(&self) -> String {
        let name = if !self.model.is_empty() {
            &self.model
        } else if !self.product.is_empty() {
            &self.product
        } else {
            &self.serial
        };
        let conn = match self.connection_type() {
            ConnectionType::Usb => "USB",
            ConnectionType::Tcp => "TCP",
        };
        format!("{name} [{conn}] ({serial})", serial = self.serial)
    }
}

pub struct AdbClient {
    adb_path: String,
    pub selected_device: Option<Device>,
    devices: Vec<Device>,
}

impl AdbClient {
    pub fn new() -> Self {
        let adb_path = std::env::var("ADB_PATH").unwrap_or_else(|_| "adb".to_string());
        Self {
            adb_path,
            selected_device: None,
            devices: Vec::new(),
        }
    }

    fn run_sync(&self, args: &[&str], device: Option<&str>) -> std::io::Result<std::process::Output> {
        let mut cmd = std::process::Command::new(&self.adb_path);
        if let Some(serial) = device {
            cmd.args(["-s", serial]);
        }
        cmd.args(args);
        cmd.output()
    }

    fn device_serial(&self) -> Option<&str> {
        self.selected_device.as_ref().map(|d| d.serial.as_str())
    }

    // --- Device management ---

    pub fn list_devices(&mut self) -> &[Device] {
        let output = match self.run_sync(&["devices", "-l"], None) {
            Ok(o) => o,
            Err(_) => return &self.devices,
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut devices = Vec::new();

        for line in stdout.lines().skip(1) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            let serial = parts[0].to_string();
            let state = DeviceState::from_str(parts[1]);
            let mut model = String::new();
            let mut product = String::new();

            for part in &parts[2..] {
                if let Some(val) = part.strip_prefix("model:") {
                    model = val.to_string();
                } else if let Some(val) = part.strip_prefix("product:") {
                    product = val.to_string();
                }
            }

            devices.push(Device {
                serial,
                state,
                model,
                product,
            });
        }

        self.devices = devices;
        &self.devices
    }

    pub fn select_device(&mut self, serial: &str) -> Option<&Device> {
        self.list_devices();
        if let Some(idx) = self.devices.iter().position(|d| d.serial == serial) {
            self.selected_device = Some(self.devices[idx].clone());
            self.selected_device.as_ref()
        } else {
            None
        }
    }

    pub fn auto_select(&mut self) -> Option<&Device> {
        self.list_devices();
        let online: Vec<_> = self.devices.iter().filter(|d| d.is_online()).collect();
        if online.len() == 1 {
            self.selected_device = Some(online[0].clone());
            self.selected_device.as_ref()
        } else {
            None
        }
    }

    pub fn connect_tcp(&mut self, address: &str) -> (bool, String) {
        let addr = if address.contains(':') {
            address.to_string()
        } else {
            format!("{address}:5555")
        };

        match self.run_sync(&["connect", &addr], None) {
            Ok(output) => {
                let msg = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let success = msg.to_lowercase().contains("connected");
                if success {
                    self.list_devices();
                    self.select_device(&addr);
                }
                (success, msg)
            }
            Err(e) => (false, format!("Error: {e}")),
        }
    }

    pub fn disconnect(&mut self, serial: Option<&str>) -> (bool, String) {
        let mut args = vec!["disconnect"];
        let target = serial
            .map(|s| s.to_string())
            .or_else(|| {
                self.selected_device
                    .as_ref()
                    .filter(|d| d.connection_type() == ConnectionType::Tcp)
                    .map(|d| d.serial.clone())
            });

        if let Some(ref t) = target {
            args.push(t);
        }

        match self.run_sync(&args, None) {
            Ok(output) => {
                let msg = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if serial.is_none()
                    || self
                        .selected_device
                        .as_ref()
                        .is_some_and(|d| Some(d.serial.as_str()) == serial)
                {
                    self.selected_device = None;
                }
                (true, msg)
            }
            Err(e) => (false, format!("Error: {e}")),
        }
    }

    // --- Package utilities ---

    pub fn get_pid(&self, package: &str) -> Option<u32> {
        let serial = self.device_serial();
        if let Ok(output) = self.run_sync(&["shell", &format!("pidof {package}")], serial) {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Ok(pid) = s.parse::<u32>() {
                return Some(pid);
            }
        }
        // Fallback: ps
        if let Ok(output) = self.run_sync(&["shell", &format!("ps -A | grep {package}")], serial) {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && line.contains(package) {
                    if let Ok(pid) = parts[1].parse::<u32>() {
                        return Some(pid);
                    }
                }
            }
        }
        None
    }

    // --- Logcat streaming ---

    pub fn start_logcat(&self, clear_first: bool) -> std::io::Result<Child> {
        let serial = self.device_serial();

        if clear_first {
            let mut cmd = std::process::Command::new(&self.adb_path);
            if let Some(s) = serial {
                cmd.args(["-s", s]);
            }
            cmd.args(["logcat", "-c"]);
            let _ = cmd.output();
        }

        let mut cmd = Command::new(&self.adb_path);
        if let Some(s) = serial {
            cmd.args(["-s", s]);
        }
        cmd.args(["logcat", "-v", "threadtime"]);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::null());
        cmd.spawn()
    }

    /// Kill the local adb server. Returns (ok, message).
    pub fn kill_server(&self) -> (bool, String) {
        match self.run_sync(&["kill-server"], None) {
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                let msg = format!("{}{}", stdout.trim(), stderr.trim());
                let msg = if msg.is_empty() {
                    "adb server killed".to_string()
                } else {
                    msg
                };
                (output.status.success(), msg)
            }
            Err(e) => (false, format!("adb kill-server failed: {e}")),
        }
    }

    /// Start the local adb server. Returns (ok, message).
    pub fn start_server(&self) -> (bool, String) {
        match self.run_sync(&["start-server"], None) {
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                let msg = format!("{}{}", stdout.trim(), stderr.trim());
                let msg = if msg.is_empty() {
                    "adb server started".to_string()
                } else {
                    msg
                };
                (output.status.success(), msg)
            }
            Err(e) => (false, format!("adb start-server failed: {e}")),
        }
    }

    pub fn check_adb(&self) -> (bool, String) {
        match self.run_sync(&["version"], None) {
            Ok(output) => {
                let msg = String::from_utf8_lossy(&output.stdout).trim().to_string();
                (true, msg)
            }
            Err(e) => (false, format!("adb not found: {e}")),
        }
    }

    /// Get the package name of the currently foreground (resumed) activity.
    pub fn get_foreground_package(&self) -> Option<String> {
        let serial = self.device_serial();
        if let Ok(output) = self.run_sync(
            &["shell", "dumpsys", "activity", "activities"],
            serial,
        ) {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("mResumedActivity") || line.contains("topResumedActivity") {
                    // Format: "mResumedActivity: ActivityRecord{... com.pkg/.Activity ...}"
                    let tokens: Vec<&str> = line.split_whitespace().collect();
                    for token in tokens {
                        if token.contains('/') {
                            let pkg = token.split('/').next().unwrap_or("");
                            if pkg.contains('.') {
                                return Some(pkg.to_string());
                            }
                        }
                    }
                }
            }
        }
        None
    }
}
