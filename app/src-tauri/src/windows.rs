//! Other programs' windows: which are open, and which are shown under the HMI.
//!
//! The display server tells of them, and is told which to show, over the
//! wlr-foreign-toplevel-management protocol. Cage offers it, keeps the HMI's
//! window in front and shows the others under it
//! (os/board/opengva/patches/cage): one, in all that the HMI's window leaves
//! clear, or several, each in the rectangle of the HMI's window that it is
//! given. On a display server that does not offer it there is nothing to see
//! or to ask for, and an area that is another program's stays unavailable.
//!
//! A window is given its rectangle with the protocol's `set_rectangle`
//! request, by which a program says where in a surface of its own it has a
//! window represented: Cage, as patched, puts the window there.
//!
//! And what the crew types on the on-screen keyboard for the program whose
//! window has the page to itself goes to it from here, as the keys of a
//! keyboard: one of this connection's own, made with the virtual-keyboard
//! protocol when the on-screen keyboard comes up over the window and
//! destroyed when it is put away. For as long as there is one Cage, as
//! patched, lends the window on show the keyboard focus, and gives it what
//! this keyboard types and nothing that any other does - the keys of the
//! machine's own keyboards, bezel buttons among them, are never a program's.
//! The keys are those of a keyboard laid out as the United States' are
//! (`us-keyboard`), which is all a program is told: it makes characters of
//! them itself.
//!
//! A program can say for itself that it wants typing, where it speaks the
//! text-input protocol: its toolkit tells the display server that a field of
//! text has the focus in its window. So that it reaches the HMI, this
//! connection is the display server's input method (the input-method
//! protocol): Cage, as patched once more, has that for the HMI alone, tells
//! it when a field of the window on show takes the focus and what the field
//! is for, and when none has it any more - and, for as long as there is such
//! an input method, leaves the window on show the keyboard focus, without
//! which a toolkit takes none of its fields to be typed into. Nothing more
//! is made of being an input method: what is typed goes as keys, as it does
//! to a program that says nothing of its fields at all. (One that says it
//! of its own window to the HMI itself, having no word for it here, is
//! heard elsewhere: `asks`.)

use crate::asks::Asks;
use hmi_core::{Kind, Stroke};
use std::fs::{self, File};
use std::io::{self, Write};
use std::os::fd::AsFd;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::{env, process, thread};
use wayland_client::protocol::{wl_compositor::WlCompositor, wl_registry, wl_seat::WlSeat, wl_surface::WlSurface};
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum, event_created_child};
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3::ContentPurpose;
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_manager_v2::ZwpInputMethodManagerV2 as InputMethodManager,
    zwp_input_method_v2::{self as input_method, ZwpInputMethodV2 as InputMethod},
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1 as KeyboardManager,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1 as Keyboard,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self as handle, ZwlrForeignToplevelHandleV1 as Handle},
    zwlr_foreign_toplevel_manager_v1::{self as manager, ZwlrForeignToplevelManagerV1 as Manager},
};

/// The connection to the display server that this is done over. It is the
/// shell's own: the one the window is drawn through belongs to GTK.
pub struct Windows {
    connection: Connection,
    /// For what is made on the connection after the watching has begun.
    queue: QueueHandle<Watcher>,
    open: Arc<Mutex<Open>>,
    /// What the times of this connection's keys are counted from.
    started: Instant,
}

#[derive(Default)]
struct Open {
    seat: Option<WlSeat>,
    compositor: Option<WlCompositor>,
    /// A surface of this connection's with nothing on it. A rectangle for a
    /// window is said to be a rectangle of a surface, and the surface the
    /// HMI is drawn on belongs to GTK's connection, where it cannot be named
    /// from here. Cage takes the rectangle to be of the HMI's window
    /// whichever surface is named.
    surface: Option<WlSurface>,
    windows: Vec<Window>,
    /// The window that was last asked to be shown with the page to itself,
    /// while that still stands: it is asked for once, and not again each
    /// time the HMI has something new to draw.
    alone: Option<Handle>,
    /// What makes a keyboard of this connection's own, if the display
    /// server has it.
    keyboards: Option<KeyboardManager>,
    /// That keyboard, while the on-screen keyboard is up over another
    /// program's page: what is typed on that is sent as this one's keys.
    keyboard: Option<Keyboard>,
}

struct Window {
    handle: Handle,
    /// The name its program gives it, once that is known.
    name: String,
    /// Whether it has been given a rectangle of its own, and still has it.
    placed: bool,
}

struct Watcher {
    open: Arc<Mutex<Open>>,
    listed: bool,
    /// The names last told of.
    told: Vec<String>,
    changed: Box<dyn Fn(Vec<String>) + Send>,
    /// What makes this connection the display server's input method, if the
    /// display server has such a thing.
    methods: Option<InputMethodManager>,
    /// What the display server has said of the field with the focus in the
    /// window on show since it last said it was done saying: whether there
    /// is one, and what it is for.
    saying: (bool, WEnum<ContentPurpose>),
    /// Where that is passed on, once it is done saying it.
    asks: Arc<Asks>,
}

/// What a field takes, by what its program says the field is for: digits
/// alone, a number that may have a point in it, or - whatever else is said,
/// or nothing - text.
fn kind(purpose: WEnum<ContentPurpose>) -> Kind {
    match purpose {
        WEnum::Value(ContentPurpose::Digits | ContentPurpose::Pin) => Kind::Number,
        WEnum::Value(ContentPurpose::Number) => Kind::Decimal,
        _ => Kind::Text,
    }
}

impl Windows {
    /// Start watching. `changed` is called, on a thread of its own, with the
    /// names of the windows open whenever those change. `asks` is told when
    /// the program whose window is on show says that a field of its page
    /// has the focus - with what such a field takes - and, with `None`,
    /// when it says that none has it now; again, when it says so again.
    pub fn watch(changed: impl Fn(Vec<String>) + Send + 'static, asks: Arc<Asks>) -> Result<Self, String> {
        let connection = Connection::connect_to_env().map_err(|e| e.to_string())?;
        let mut queue = connection.new_event_queue();
        connection.display().get_registry(&queue.handle(), ());

        let open = Arc::new(Mutex::new(Open::default()));
        let mut watcher = Watcher {
            open: open.clone(),
            listed: false,
            told: Vec::new(),
            changed: Box::new(changed),
            methods: None,
            saying: (false, WEnum::Value(ContentPurpose::Normal)),
            asks,
        };
        queue.roundtrip(&mut watcher).map_err(|e| e.to_string())?;
        if !watcher.listed {
            return Err("the display server does not tell of other programs' windows".to_string());
        }

        let handle = queue.handle();
        // Be the input method, where there is a place for one: it is what
        // has a program's fields told of, and nothing is lost where there is
        // none - the crew bring the keyboard up themselves.
        if let (Some(methods), Some(seat)) = (&watcher.methods, &open.lock().unwrap().seat) {
            methods.get_input_method(seat, &handle, ());
            let _ = connection.flush();
        }
        thread::spawn(move || while queue.blocking_dispatch(&mut watcher).is_ok() {});
        Ok(Windows { connection, queue: handle, open, started: Instant::now() })
    }

    /// Have these windows shown under the HMI's, and no others. Each is
    /// named, and has the rectangle of the HMI's window it is to go in -
    /// left, top, width and height - or `None`, to go in all that the HMI's
    /// window leaves clear.
    pub fn show(&self, windows: &[(&str, Option<[i32; 4]>)]) {
        let mut open = self.open.lock().unwrap();
        let open = &mut *open;
        match windows {
            // One, with the page to itself: it is shown in place of any other.
            [(name, None)] => {
                let window = open.windows.iter_mut().find(|window| window.name == *name);
                if let (Some(window), Some(seat)) = (window, &open.seat) {
                    // A rectangle it was given, in another area's tiles, it gives back.
                    if let (true, Some(surface)) = (window.placed, &open.surface) {
                        window.handle.set_rectangle(surface, 0, 0, 0, 0);
                        window.placed = false;
                    }
                    // Asked for when it is not what is showing already. The
                    // display server tells a window that it is shown as
                    // often as it is asked to show it, and a program can
                    // take being told for news: Android lays its screen out
                    // afresh, and that at every key typed into it.
                    if open.alone.as_ref() != Some(&window.handle) {
                        window.handle.activate(seat);
                        open.alone = Some(window.handle.clone());
                    }
                }
            }
            // Several, each in its rectangle, or none at all: every window
            // is shown or hidden for itself.
            _ => {
                open.alone = None;
                if open.surface.is_none() && windows.iter().any(|(_, place)| place.is_some()) {
                    open.surface = open.compositor.as_ref().map(|compositor| compositor.create_surface(&self.queue, ()));
                }
                for window in &mut open.windows {
                    match windows.iter().find(|(name, _)| window.name == *name) {
                        Some((_, place)) => {
                            if let (Some([x, y, width, height]), Some(surface)) = (place, &open.surface) {
                                window.handle.set_rectangle(surface, *x, *y, *width, *height);
                                window.placed = true;
                            }
                            window.handle.unset_minimized();
                        }
                        // The HMI's own window is among these; that request is ignored.
                        None => window.handle.set_minimized(),
                    }
                }
            }
        }
        let _ = self.connection.flush();
    }

    /// Type into the window that has the page to itself. `typing` is
    /// whether the on-screen keyboard is up over it. While it is, this
    /// connection has a keyboard, which is what has the display server lend
    /// that window the keyboard focus; `strokes` are sent as its keys, each
    /// pressed and let go. When it no longer is, the keyboard is destroyed,
    /// and the focus is the HMI's again.
    ///
    /// A stroke that no key of the keyboard types - a character that is not
    /// ASCII - is passed over. Where the display server has no such
    /// keyboards to give, nothing is typed.
    pub fn type_into(&self, typing: bool, strokes: &[Stroke]) {
        let mut open = self.open.lock().unwrap();
        if !typing {
            if let Some(keyboard) = open.keyboard.take() {
                keyboard.destroy();
                let _ = self.connection.flush();
            }
            return;
        }
        if open.keyboard.is_none() {
            open.keyboard = self.keyboard(&open);
        }
        let Some(keyboard) = &open.keyboard else { return };
        let time = self.started.elapsed().as_millis() as u32;
        for stroke in strokes {
            let key = match *stroke {
                Stroke::Character(character) => us_keyboard::key(character),
                Stroke::Backspace => Some((us_keyboard::BACKSPACE, false)),
                Stroke::Enter => Some((us_keyboard::ENTER, false)),
                Stroke::Tab => Some((us_keyboard::TAB, false)),
                Stroke::BackTab => Some((us_keyboard::TAB, true)),
            };
            if let Some((code, shift)) = key {
                strike(keyboard, time, code, shift);
            }
        }
        let _ = self.connection.flush();
    }

    /// Make this connection's keyboard, and give it the keymap that says
    /// what its keys type: no key can be sent before that.
    fn keyboard(&self, open: &Open) -> Option<Keyboard> {
        let (keyboards, seat) = (open.keyboards.as_ref()?, open.seat.as_ref()?);
        let keymap = match keymap_file() {
            Ok(keymap) => keymap,
            Err(why) => {
                eprintln!("opengva-hmi: no keymap for what the on-screen keyboard types: {why}");
                return None;
            }
        };
        let keyboard = keyboards.create_virtual_keyboard(seat, &self.queue, ());
        keyboard.keymap(XKB_V1, keymap.as_fd(), us_keyboard::KEYMAP.len() as u32 + 1);
        // The file is the display server's to read once it has been told of.
        let _ = self.connection.flush();
        Some(keyboard)
    }
}

/// The format of a keymap in XKB's text, as `wl_keyboard` numbers it.
const XKB_V1: u32 = 1;

/// The keymap in a file for the display server to read, with the zero byte
/// at its end that a keymap is to have. The file has no name by the time
/// this returns: it lasts as long as somebody has it open.
fn keymap_file() -> io::Result<File> {
    let dir = env::var_os("XDG_RUNTIME_DIR").map_or_else(env::temp_dir, PathBuf::from);
    let path = dir.join(format!("opengva-hmi-{}.keymap", process::id()));
    let mut file = File::options().read(true).write(true).create(true).truncate(true).open(&path)?;
    let written = file.write_all(us_keyboard::KEYMAP.as_bytes()).and_then(|()| file.write_all(&[0]));
    fs::remove_file(&path)?;
    written.map(|()| file)
}

/// Press a key of the keyboard and let it go, with Shift held if it is to
/// be. Shift is a key like any other, pressed first and let go last, for a
/// program that reads keys and nothing else; and the modifiers are said
/// too, which is what one that goes by the keymap listens to.
fn strike(keyboard: &Keyboard, time: u32, code: us_keyboard::Code, shift: bool) {
    const RELEASED: u32 = 0;
    const PRESSED: u32 = 1;
    if shift {
        keyboard.key(time, us_keyboard::SHIFT, PRESSED);
        keyboard.modifiers(us_keyboard::SHIFT_MODIFIER, 0, 0, 0);
    }
    keyboard.key(time, code, PRESSED);
    keyboard.key(time, code, RELEASED);
    if shift {
        keyboard.key(time, us_keyboard::SHIFT, RELEASED);
        keyboard.modifiers(0, 0, 0, 0);
    }
}

impl Watcher {
    /// Tell of the windows open, if they are not what was last told of. The
    /// display server says something of a window whenever it is shown or
    /// hidden, which is not news of what is open.
    fn tell(&mut self) {
        let names: Vec<String> = self.open.lock().unwrap().windows.iter().map(|window| window.name.clone()).collect();
        if names != self.told {
            self.told.clone_from(&names);
            (self.changed)(names);
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Watcher {
    fn event(
        watcher: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        queue: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global { name, interface, version } = event else { return };
        match interface.as_str() {
            // A request to bring a window forward names the seat it is for.
            "wl_seat" => {
                let seat = registry.bind::<WlSeat, _, _>(name, 1, queue, ());
                watcher.open.lock().unwrap().seat.get_or_insert(seat);
            }
            // And one that gives a window a rectangle names a surface, which
            // this makes when the first rectangle is given.
            "wl_compositor" => {
                let compositor = registry.bind::<WlCompositor, _, _>(name, 1, queue, ());
                watcher.open.lock().unwrap().compositor.get_or_insert(compositor);
            }
            "zwlr_foreign_toplevel_manager_v1" => {
                registry.bind::<Manager, _, _>(name, version.min(3), queue, ());
                watcher.listed = true;
            }
            // What the on-screen keyboard types into another program's page
            // is sent as the keys of a keyboard made with this.
            "zwp_virtual_keyboard_manager_v1" => {
                let keyboards = registry.bind::<KeyboardManager, _, _>(name, 1, queue, ());
                watcher.open.lock().unwrap().keyboards.get_or_insert(keyboards);
            }
            // And a program that says a field of its page has the focus is
            // heard by the input method made with this.
            "zwp_input_method_manager_v2" => {
                let methods = registry.bind::<InputMethodManager, _, _>(name, 1, queue, ());
                watcher.methods.get_or_insert(methods);
            }
            _ => {}
        }
    }
}

impl Dispatch<WlSeat, ()> for Watcher {
    fn event(_: &mut Self, _: &WlSeat, _: <WlSeat as wayland_client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<WlCompositor, ()> for Watcher {
    fn event(_: &mut Self, _: &WlCompositor, _: <WlCompositor as wayland_client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<WlSurface, ()> for Watcher {
    fn event(_: &mut Self, _: &WlSurface, _: <WlSurface as wayland_client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<InputMethodManager, ()> for Watcher {
    fn event(_: &mut Self, _: &InputMethodManager, _: <InputMethodManager as wayland_client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<InputMethod, ()> for Watcher {
    fn event(watcher: &mut Self, _: &InputMethod, event: input_method::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match event {
            // A field has taken the focus; what it is for follows, if its
            // program says.
            input_method::Event::Activate => watcher.saying = (true, WEnum::Value(ContentPurpose::Normal)),
            input_method::Event::Deactivate => watcher.saying.0 = false,
            input_method::Event::ContentType { purpose, .. } => watcher.saying.1 = purpose,
            // That is all of it, for now: said of the window on show, which
            // is the one the display server speaks of.
            input_method::Event::Done => {
                let (there, purpose) = watcher.saying;
                watcher.asks.say(None, there.then(|| kind(purpose)));
            }
            input_method::Event::Unavailable => {
                eprintln!("opengva-hmi: the display server has no place for the HMI as its input method: no keyboard comes up by itself");
            }
            _ => {}
        }
    }
}

impl Dispatch<KeyboardManager, ()> for Watcher {
    fn event(_: &mut Self, _: &KeyboardManager, _: <KeyboardManager as wayland_client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<Keyboard, ()> for Watcher {
    fn event(_: &mut Self, _: &Keyboard, _: <Keyboard as wayland_client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<Manager, ()> for Watcher {
    fn event(watcher: &mut Self, _: &Manager, event: manager::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let manager::Event::Toplevel { toplevel } = event {
            let window = Window { handle: toplevel, name: String::new(), placed: false };
            watcher.open.lock().unwrap().windows.push(window);
        }
    }

    event_created_child!(Watcher, Manager, [
        manager::EVT_TOPLEVEL_OPCODE => (Handle, ()),
    ]);
}

impl Dispatch<Handle, ()> for Watcher {
    fn event(watcher: &mut Self, window: &Handle, event: handle::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match event {
            handle::Event::AppId { app_id } => {
                let mut open = watcher.open.lock().unwrap();
                if let Some(known) = open.windows.iter_mut().find(|known| known.handle == *window) {
                    known.name = app_id;
                }
            }
            // What was sent about the window so far is now to be taken together.
            handle::Event::Done => watcher.tell(),
            handle::Event::Closed => {
                let mut open = watcher.open.lock().unwrap();
                open.windows.retain(|known| known.handle != *window);
                if open.alone.as_ref() == Some(window) {
                    open.alone = None;
                }
                drop(open);
                window.destroy();
                watcher.tell();
            }
            _ => {}
        }
    }
}
