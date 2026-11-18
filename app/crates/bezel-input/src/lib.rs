//! Bezel buttons from Linux input devices.
//!
//! A bezel is a keypad: whatever it is wired to - USB, GPIO, a matrix
//! scanner - the kernel presents it as an input device that reports key
//! events. This crate reads those events and turns the keys in [`button_for`]
//! into [`ButtonEvent`]s. It knows nothing about any particular bezel, and
//! nothing about what the buttons do.
//!
//! The same path serves a virtual machine: there the "bezel" is drawn round
//! the display by a program on the host (the bezel viewer), which injects these
//! keys. The image cannot tell the difference, and does not try to.
//!
//! Every readable device under /dev/input is watched, and the directory is
//! re-scanned so that a keypad plugged in later, or one whose node appears
//! after this starts, is picked up. Reading needs membership of the `input`
//! group.

use hmi_core::{Action, Button, ButtonEvent};
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// The key each bezel button reports, as a Linux input event code
/// (`KEY_*` in linux/input-event-codes.h).
///
/// | Buttons    | Keys                                |
/// |------------|-------------------------------------|
/// | F1 to F12  | `KEY_F1` to `KEY_F12`               |
/// | F13 to F20 | `KEY_F13` to `KEY_F20`              |
/// | SA to BMS  | `KEY_STOP` to `KEY_PASTE` (128-135) |
///
/// The top row has no natural key, so it takes a block of eight consecutive
/// codes that no ordinary keyboard has - nothing typed can be mistaken for a
/// bezel button - and that a virtual machine's keyboard can be made to send.
/// A real bezel that reports other codes needs this table changed, or made
/// configurable, when there is one.
///
/// The bezel viewer on the host holds the other end of this table.
pub fn button_for(code: u16) -> Option<Button> {
    match code {
        59..=68 => Button::f((code - 58) as u8),    // KEY_F1 to KEY_F10
        87 => Button::f(11),                        // KEY_F11
        88 => Button::f(12),                        // KEY_F12
        183..=190 => Button::f((code - 170) as u8), // KEY_F13 to KEY_F20
        128..=135 => Button::area((code - 128) as u8), // KEY_STOP to KEY_PASTE
        _ => None,
    }
}

const EV_KEY: u16 = 1;

/// One `struct input_event` on a 64-bit kernel: two 64-bit words of timestamp,
/// then the type, code and value.
const EVENT_SIZE: usize = 24;

#[cfg(not(target_pointer_width = "64"))]
compile_error!("bezel-input reads the 64-bit layout of struct input_event");

/// Decode one input event. `None` unless it is a mapped key going down or up
/// (a held key also reports auto-repeats, value 2, which are ignored).
fn decode(raw: &[u8; EVENT_SIZE]) -> Option<ButtonEvent> {
    let kind = u16::from_ne_bytes([raw[16], raw[17]]);
    let code = u16::from_ne_bytes([raw[18], raw[19]]);
    let value = i32::from_ne_bytes([raw[20], raw[21], raw[22], raw[23]]);
    if kind != EV_KEY {
        return None;
    }
    let action = match value {
        1 => Action::Pressed,
        0 => Action::Released,
        _ => return None,
    };
    Some(ButtonEvent { button: button_for(code)?, action })
}

/// Start watching for bezel buttons; events are sent on `events` for as long
/// as the receiver lives.
pub fn watch(events: Sender<ButtonEvent>) {
    thread::spawn(move || {
        // Devices being read, or that could not be opened: not tried again.
        let claimed = Arc::new(Mutex::new(HashSet::<PathBuf>::new()));
        loop {
            for path in event_devices(Path::new("/dev/input")) {
                if !claimed.lock().unwrap().insert(path.clone()) {
                    continue;
                }
                let Ok(device) = File::open(&path) else { continue };
                let (events, claimed) = (events.clone(), Arc::clone(&claimed));
                thread::spawn(move || {
                    read(device, &events);
                    // Unplugged (or the HMI is going away): let a device that
                    // reappears under the same name be opened afresh.
                    claimed.lock().unwrap().remove(&path);
                });
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
}

fn event_devices(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("event")))
        .collect()
}

fn read(mut device: File, events: &Sender<ButtonEvent>) {
    let mut raw = [0u8; EVENT_SIZE];
    while device.read_exact(&mut raw).is_ok() {
        if let Some(event) = decode(&raw) {
            if events.send(event).is_err() {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_button_has_exactly_one_key() {
        let mapped: Vec<Button> = (0..=u16::MAX).filter_map(button_for).collect();
        assert_eq!(mapped.len(), 28);
        for button in Button::all() {
            assert_eq!(mapped.iter().filter(|&&b| b == button).count(), 1, "{button}");
        }
    }

    #[test]
    fn the_keys_are_the_ones_documented() {
        let name = |code| button_for(code).map(|b| b.to_string());
        assert_eq!(name(59).as_deref(), Some("F1")); // KEY_F1
        assert_eq!(name(68).as_deref(), Some("F10")); // KEY_F10
        assert_eq!(name(87).as_deref(), Some("F11")); // KEY_F11
        assert_eq!(name(88).as_deref(), Some("F12")); // KEY_F12
        assert_eq!(name(183).as_deref(), Some("F13")); // KEY_F13
        assert_eq!(name(190).as_deref(), Some("F20")); // KEY_F20
        assert_eq!(name(128).as_deref(), Some("SA")); // KEY_STOP
        assert_eq!(name(135).as_deref(), Some("BMS")); // KEY_PASTE
        assert_eq!(name(30), None); // KEY_A: typing is not a bezel button
        assert_eq!(name(191), None); // KEY_F21
    }

    fn event(kind: u16, code: u16, value: i32) -> [u8; EVENT_SIZE] {
        let mut raw = [0u8; EVENT_SIZE];
        raw[16..18].copy_from_slice(&kind.to_ne_bytes());
        raw[18..20].copy_from_slice(&code.to_ne_bytes());
        raw[20..24].copy_from_slice(&value.to_ne_bytes());
        raw
    }

    #[test]
    fn only_a_mapped_key_going_down_or_up_is_a_button_event() {
        let f13 = Button::f(13).unwrap();
        assert_eq!(decode(&event(EV_KEY, 183, 1)), Some(ButtonEvent { button: f13, action: Action::Pressed }));
        assert_eq!(decode(&event(EV_KEY, 183, 0)), Some(ButtonEvent { button: f13, action: Action::Released }));
        assert_eq!(decode(&event(EV_KEY, 183, 2)), None, "auto-repeat");
        assert_eq!(decode(&event(EV_KEY, 30, 1)), None, "an unmapped key");
        assert_eq!(decode(&event(0, 183, 1)), None, "not a key event");
    }
}
