//! Talks to the Voyager over ZSA's Oryx raw-HID protocol (the one Live Training uses).
//!
//! The board can tell us which Oryx layout/revision is flashed, and once paired it
//! streams physical key down/up positions and the active layer. It cannot tell us the
//! keymap itself; that comes from Oryx (see `oryx.rs`).
//!
//! Protocol reference: zsa/qmk_modules `oryx/oryx.c` (protocol version 5).

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use hidapi::{DeviceInfo, HidApi, HidDevice};

use crate::geometry;

const ZSA_VENDOR_ID: u16 = 0x3297;
const VOYAGER_PRODUCT_ID: u16 = 0x1977;
const RAW_USAGE_PAGE: u16 = 0xFF60;
const RAW_INTERFACE: i32 = 1;
const REPORT_SIZE: usize = 32;

const CMD_GET_FW_VERSION: u8 = 0x00;
const CMD_PAIRING_INIT: u8 = 0x01;
const CMD_GET_PROTOCOL_VERSION: u8 = 0xFE;

const EVT_GET_FW_VERSION: u8 = 0x00;
const EVT_LAYER: u8 = 0x05;
const EVT_KEYDOWN: u8 = 0x06;
const EVT_KEYUP: u8 = 0x07;
const EVT_GET_PROTOCOL_VERSION: u8 = 0xFE;
const STOP_BYTE: u8 = 0xFE;

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceEvent {
    /// A Voyager is plugged in. `firmware_id` is Oryx's `<layoutId>/<revisionId>` when available.
    /// `live` is false when the board doesn't speak the Oryx protocol (no key/layer events).
    Connected { firmware_id: Option<String>, protocol: Option<u8>, live: bool },
    /// A ZSA keyboard that isn't a Voyager.
    WrongBoard(String),
    Missing,
    KeyDown(usize),
    KeyUp(usize),
    Layer(usize),
}

pub fn spawn() -> Receiver<DeviceEvent> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || run(tx));
    rx
}

enum Found {
    Voyager { raw: Option<DeviceInfo>, serial: Option<String> },
    Other(String),
    Nothing,
}

fn find(api: &HidApi) -> Found {
    let zsa: Vec<&DeviceInfo> = api.device_list().filter(|d| d.vendor_id() == ZSA_VENDOR_ID).collect();
    let voyager: Vec<&&DeviceInfo> = zsa.iter().filter(|d| d.product_id() == VOYAGER_PRODUCT_ID).collect();
    if let Some(first) = voyager.first() {
        let raw = voyager
            .iter()
            .find(|d| d.usage_page() == RAW_USAGE_PAGE)
            .or_else(|| voyager.iter().find(|d| d.usage_page() == 0 && d.interface_number() == RAW_INTERFACE))
            .map(|d| (**d).clone());
        let serial = first.serial_number().filter(|s| !s.is_empty()).map(String::from);
        return Found::Voyager { raw, serial };
    }
    match zsa.first() {
        Some(d) => Found::Other(d.product_string().unwrap_or("ZSA keyboard").to_string()),
        None => Found::Nothing,
    }
}

fn run(tx: Sender<DeviceEvent>) {
    let Ok(mut api) = HidApi::new() else {
        let _ = tx.send(DeviceEvent::Missing);
        return;
    };
    // Don't lock out Keymapp or Oryx Live Training.
    #[cfg(target_os = "macos")]
    api.set_open_exclusive(false);

    // Last status sent, so polling doesn't repeat it every cycle.
    let mut last: Option<DeviceEvent> = None;
    loop {
        let _ = api.refresh_devices();
        let status = match find(&api) {
            Found::Voyager { raw: Some(info), serial } => match session(&api, &info, serial.clone(), &tx) {
                SessionEnd::AppGone => return,
                SessionEnd::Disconnected => {
                    last = None;
                    thread::sleep(Duration::from_millis(300));
                    continue;
                }
                // e.g. missing udev rules on Linux: we know it's there but can't talk to it.
                SessionEnd::OpenFailed => DeviceEvent::Connected { firmware_id: serial, protocol: None, live: false },
            },
            Found::Voyager { raw: None, serial } => {
                DeviceEvent::Connected { firmware_id: serial, protocol: None, live: false }
            }
            Found::Other(name) => DeviceEvent::WrongBoard(name),
            Found::Nothing => DeviceEvent::Missing,
        };
        if last.as_ref() != Some(&status) {
            if tx.send(status.clone()).is_err() {
                return;
            }
            last = Some(status);
        }
        thread::sleep(Duration::from_millis(700));
    }
}

enum SessionEnd {
    /// The board went away (unplugged, reset into the bootloader, ...).
    Disconnected,
    /// The raw HID interface couldn't be opened.
    OpenFailed,
    /// The receiving side hung up; stop the thread.
    AppGone,
}

fn session(api: &HidApi, info: &DeviceInfo, serial: Option<String>, tx: &Sender<DeviceEvent>) -> SessionEnd {
    match run_session(api, info, serial, tx) {
        Ok(never) => match never {},
        Err(end) => end,
    }
}

fn run_session(
    api: &HidApi,
    info: &DeviceInfo,
    serial: Option<String>,
    tx: &Sender<DeviceEvent>,
) -> Result<std::convert::Infallible, SessionEnd> {
    let dev = info.open_device(api).map_err(|_| SessionEnd::OpenFailed)?;

    let protocol = request(&dev, CMD_GET_PROTOCOL_VERSION, EVT_GET_PROTOCOL_VERSION).map(|r| r[1]);
    let firmware_id = request(&dev, CMD_GET_FW_VERSION, EVT_GET_FW_VERSION)
        .map(|r| {
            let body = &r[1..];
            let end = body.iter().position(|&b| b == STOP_BYTE || b == 0).unwrap_or(body.len());
            String::from_utf8_lossy(&body[..end]).into_owned()
        })
        .filter(|s| !s.is_empty())
        .or(serial);

    tx.send(DeviceEvent::Connected { firmware_id, protocol, live: protocol.is_some() })
        .map_err(|_| SessionEnd::AppGone)?;
    write(&dev, CMD_PAIRING_INIT).map_err(|_| SessionEnd::Disconnected)?;

    let mut buf = [0u8; REPORT_SIZE];
    loop {
        let n = dev.read_timeout(&mut buf, 250).map_err(|_| SessionEnd::Disconnected)?;
        if n == 0 {
            continue;
        }
        let ev = match buf[0] {
            EVT_KEYDOWN | EVT_KEYUP => {
                let (col, row) = (buf[1], buf[2]);
                geometry::key_at_matrix(row, col).map(|k| {
                    if buf[0] == EVT_KEYDOWN {
                        DeviceEvent::KeyDown(k)
                    } else {
                        DeviceEvent::KeyUp(k)
                    }
                })
            }
            EVT_LAYER => Some(DeviceEvent::Layer(buf[1] as usize)),
            _ => None,
        };
        if let Some(ev) = ev {
            tx.send(ev).map_err(|_| SessionEnd::AppGone)?;
        }
    }
}

fn write(dev: &HidDevice, cmd: u8) -> hidapi::HidResult<usize> {
    // First byte is the report ID (0: none).
    let mut buf = [0u8; REPORT_SIZE + 1];
    buf[1] = cmd;
    dev.write(&buf)
}

/// Send a command and wait briefly for its reply, skipping unrelated events.
fn request(dev: &HidDevice, cmd: u8, reply: u8) -> Option<[u8; REPORT_SIZE]> {
    write(dev, cmd).ok()?;
    let deadline = Instant::now() + Duration::from_millis(500);
    let mut buf = [0u8; REPORT_SIZE];
    while Instant::now() < deadline {
        let n = dev.read_timeout(&mut buf, 100).ok()?;
        if n > 0 && buf[0] == reply {
            return Some(buf);
        }
    }
    None
}

/// `--hid-debug`: print raw device events, to check the protocol against a real board.
pub fn debug_print() {
    println!("Listening for Voyager events. Press keys / switch layers. Ctrl+C to stop.");
    let rx = spawn();
    let start = Instant::now();
    for ev in rx {
        let t = start.elapsed().as_secs_f32();
        match ev {
            DeviceEvent::KeyDown(k) | DeviceEvent::KeyUp(k) => {
                let g = geometry::KEYS[k];
                println!("{t:7.3}s  {ev:?}  (matrix row {} col {})", g.row, g.col);
            }
            other => println!("{t:7.3}s  {other:?}"),
        }
    }
}
