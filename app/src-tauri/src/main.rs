//! The HMI shell: opens the window, feeds the core and answers the UI.
//!
//! What crosses to the UI, all of it in the HMI's own terms:
//!
//! - the `frame` command returns the [`Frame`] to draw;
//! - a `frame` event carries a new one whenever it changes;
//! - the `first_frame` command is the UI saying it has painted one;
//! - the `solid` command is the UI saying which parts of the window are the
//!   bars and labels, whether a touch on the rest is another program's, and
//!   where in the window it has drawn the display;
//! - the `touch` command is the UI saying what of the frame has been
//!   touched: a soft-key label, a row of a table, a key of the on-screen
//!   keyboard, or on a form's page a field;
//! - the `lift` command is the UI saying that the finger on such a key has
//!   gone.
//!
//! Bezel buttons do not come through the UI. They are read from the kernel's
//! input devices (`bezel-input`) and handed to the core here.
//!
//! A functional area can be another program's window (`hmi_core::ExternalArea`),
//! or several programs' windows, some of them shown at a time in tiles
//! (`hmi_core::TiledArea`) or one at a time, chosen from the area's menu
//! (`hmi_core::ViewedArea`): each file in [`AREAS_DIR`] gives one away. The
//! HMI's window is transparent, the other programs' are under it, and
//! `windows` is how the shell learns which are open and has them shown - and
//! how what the crew types on the on-screen keyboard, where it is up over a
//! program's page, reaches the program: as the keys of a keyboard. That
//! keyboard comes up by itself when the program says a field of its page has
//! the focus, which it says through the display server or, if it has no way
//! to, to the shell itself (`asks`).
//!
//! What the vehicle has to say - its alarms, and the threats its sensors find
//! - comes up the feed, and what the crew does about it goes back down
//! (`hmi_core::feed` for what is said, `hmi-feed` for the line). The feed is at one of the paths in the
//! environment's [`FEED`], if any of them is there; with none there is no
//! vehicle to hear from, and the HMI is as it is without one.
//!
//! Lines on stderr that start `OPENGVA ` are markers; what starts the image's
//! services copies them to the console, where whatever boots the image -
//! a test rig, a bench - can read them. "uptime" is seconds since the kernel
//! started.

mod asks;
mod verifier;
mod windows;

use base64::Engine;
use gtk::cairo::{RectangleInt, Region};
use gtk::prelude::*;
use asks::Asks;
use hmi_core::{
    AccessConfig, AreaState, Asking, Audit, External, FUNCTIONS, Frame, Hmi, Page, Report, Row, Session, ThreatsView,
    Time, Touched,
};
use hmi_feed::Feed;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{env, fs, thread};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use windows::Windows;

/// Where the areas that are other programs' windows are configured: one
/// `.conf` file each, as [`External::parse`] reads them.
const AREAS_DIR: &str = "/etc/opengva/gva-fa.d";

/// Who may have what at this crew station: its roles, and which of the HMI's
/// own functions need one, as [`AccessConfig::read`] reads them. Most crew
/// stations have no such file, and nothing at them needs anybody's PIN.
const ACCESS: &str = "/etc/opengva/access.conf";

/// The environment variable that says where the feed may be: paths, with
/// colons between them. The first that is there is the feed.
const FEED: &str = "OPENGVA_FEED";

struct Shell {
    hmi: Mutex<Hmi>,
    /// Other programs' windows, if the display server tells of them.
    windows: OnceLock<Option<Windows>>,
    /// Where the UI has drawn the display in the window, as it last said.
    display: Mutex<Display>,
    /// The line to whatever knows the vehicle, if there may be one.
    feed: OnceLock<Feed>,
    /// The lines to the programs that listen for the HMI, by their areas.
    apps: OnceLock<HashMap<u8, Feed>>,
    /// Wakes the thread that keeps time: what it is waiting for may have changed.
    timekeeper: OnceLock<mpsc::Sender<()>>,
    /// The markers as last printed, by what they are about.
    said: Mutex<HashMap<&'static str, String>>,
    started: Instant,
    /// Seconds since the kernel started, when each stage was reached.
    app_start: Option<f64>,
    first_frame: OnceLock<Option<f64>>,
}

impl Shell {
    fn frame(&self) -> Frame {
        let rows = vec![
            Row::new("HMI version", env!("CARGO_PKG_VERSION")),
            Row::new("Kernel", kernel_release()),
            Row::new("HMI started", seconds(self.app_start)),
            Row::new("First frame", seconds(self.first_frame.get().copied().flatten())),
        ];
        self.hmi.lock().unwrap().frame(self.now(), rows)
    }

    /// The time as the core is told it: by the calendar, and since the HMI
    /// started.
    fn now(&self) -> Time {
        Time { unix_s: unix_time(), uptime_ms: self.started.elapsed().as_millis() as u64 }
    }

    /// Print a marker about `what`, if it says something it did not last time.
    fn say(&self, what: &'static str, saying: String) {
        let mut said = self.said.lock().unwrap();
        if said.get(what) != Some(&saying) {
            marker(format_args!("{what} {saying}"));
            said.insert(what, saying);
        }
    }
}

/// The HMI with the areas that [`AREAS_DIR`] gives to other programs. A file
/// that cannot be used is reported and left out.
fn configured_hmi() -> Hmi {
    let mut hmi = Hmi::new();
    // First, so that an area which asks for a role there is not is refused.
    hmi.set_access(access());
    let mut files: Vec<PathBuf> = fs::read_dir(AREAS_DIR)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "conf"))
        .collect();
    files.sort();
    for file in files {
        let added = fs::read_to_string(&file)
            .map_err(|e| e.to_string())
            .and_then(|text| External::parse(&text))
            .and_then(|external| match external {
                External::Window(mut area) => {
                    area.overlay = area.overlay.map(|image| data_url(Path::new(&image))).transpose()?;
                    hmi.add_external(area)
                }
                External::Tiles(area) => hmi.add_tiled(area),
                External::Views(mut area) => {
                    for view in &mut area.views {
                        view.overlay = view.overlay.take().map(|image| data_url(Path::new(&image))).transpose()?;
                    }
                    hmi.add_viewed(area)
                }
                External::Form(area) => hmi.add_form(area),
            });
        if let Err(why) = added {
            eprintln!("opengva-hmi: {}: {why}", file.display());
        }
    }
    hmi
}

/// Who may have what, as [`ACCESS`] says. With no such file there are no
/// roles and nothing needs one. With one that cannot be read, or cannot be
/// right, the mistake is reported and what it might have kept for some is
/// kept from all: a broken file is not a way in.
fn access() -> AccessConfig {
    let text = match fs::read_to_string(ACCESS) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return AccessConfig::default(),
        Err(e) => {
            eprintln!("opengva-hmi: {ACCESS}: {e}");
            return AccessConfig::shut(&FUNCTIONS);
        }
    };
    AccessConfig::read(&text, &FUNCTIONS, verifier::members).unwrap_or_else(|why| {
        eprintln!("opengva-hmi: {ACCESS}: {why}");
        AccessConfig::shut(&FUNCTIONS)
    })
}

/// Write down something that has happened about access: in the service's
/// log, with whatever else the HMI says, and in the system's, where PAM's
/// own word on each PIN is. Never a PIN: there is none in it.
fn record(what: &Audit) {
    eprintln!("opengva-hmi: access: {what}");
    if let Ok(line) = std::ffi::CString::new(what.to_string()) {
        // SAFETY: both strings end in a NUL and outlive the call, and "%s"
        // takes the one argument it is given.
        unsafe { libc::syslog(libc::LOG_AUTH | libc::LOG_NOTICE, c"%s".as_ptr(), line.as_ptr()) };
    }
}

/// An image file as a URL that carries it, which the UI can show without
/// being let near the file system.
fn data_url(image: &Path) -> Result<String, String> {
    let kind = match image.extension().and_then(|e| e.to_str()) {
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        _ => return Err(format!("{}: an overlay is an .svg or a .png", image.display())),
    };
    let bytes = fs::read(image).map_err(|e| format!("{}: {e}", image.display()))?;
    Ok(format!("data:{kind};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

fn unix_time() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Seconds since the kernel started. `None` where there is no /proc.
fn uptime() -> Option<f64> {
    std::fs::read_to_string("/proc/uptime").ok()?.split_whitespace().next()?.parse().ok()
}

fn seconds(t: Option<f64>) -> String {
    t.map_or_else(|| "n/a".to_string(), |s| format!("{s:.2} s"))
}

fn kernel_release() -> String {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map_or_else(|_| "n/a".to_string(), |s| s.trim().to_string())
}

/// Print a marker, in one write: stderr is shared with whatever else the
/// service prints - WebKit's own processes among them - and a line that goes
/// out in pieces can have somebody else's in the middle of it.
fn marker(what: impl std::fmt::Display) {
    use std::io::Write;
    let line = format!("OPENGVA {what} uptime={:.2}\n", uptime().unwrap_or(-1.0));
    let _ = std::io::stderr().write_all(line.as_bytes());
}

/// Bring everything outside the core up to date with it: the selected area's
/// window under the HMI's if it is another program's - or its windows, each
/// where its tile is - and no window there if it is not, what the crew has
/// done about alarms and threats to whatever told of them, and the frame to
/// the UI.
fn refresh(app: &AppHandle) {
    let shell = app.state::<Shell>();
    let (requests, check, audit, to_apps) = {
        let mut hmi = shell.hmi.lock().unwrap();
        if let Some(Some(windows)) = shell.windows.get() {
            // A tile is a part of the display, and the display server is
            // told which part of the window that is.
            let display = *shell.display.lock().unwrap();
            let shown: Vec<_> = hmi
                .windows_to_show()
                .into_iter()
                .map(|shown| (shown.window, shown.at.map(|at| at.on_screen(display.x, display.y, display.scale))))
                .collect();
            windows.show(&shown);
            // What the crew has typed on the on-screen keyboard for the
            // program whose window that is goes to it. How much was typed
            // is for the console; what, is not.
            let strokes = hmi.take_strokes();
            if !strokes.is_empty() {
                marker(format_args!("typed {}", strokes.len()));
            }
            windows.type_into(hmi.typing_into_window(), &strokes);
        }
        (hmi.take_requests(), hmi.take_check(), hmi.take_audit(), hmi.take_to_apps())
    };
    audit.iter().for_each(record);
    // A PIN the crew have entered is put to the verifier, which takes a
    // moment over one - and longer over a wrong one - on a thread of its
    // own. What it says is the core's to act on, and brings a frame.
    if let Some(check) = check {
        let app = app.clone();
        thread::spawn(move || {
            let answer = verifier::check(&check.account, check.pin.reveal());
            let shell = app.state::<Shell>();
            if shell.hmi.lock().unwrap().verified(check.serial, answer, shell.now()) {
                refresh(&app);
            }
        });
    }
    if let Some(feed) = shell.feed.get() {
        requests.iter().for_each(|request| feed.send(&request.line()));
    }
    if let Some(apps) = shell.apps.get() {
        for (area, told) in &to_apps {
            if let Some(app) = apps.get(area) {
                app.send(&told.line());
            }
        }
    }
    // What the core next wants to be woken for may not be what it was.
    if let Some(timekeeper) = shell.timekeeper.get() {
        let _ = timekeeper.send(());
    }
    let frame = shell.frame();

    // Where things stand, on the console when it changes.
    let areas: Vec<String> = frame
        .areas
        .iter()
        .filter(|area| area.state != AreaState::Absent)
        .map(|area| format!("{}={}", area.id, format!("{:?}", area.state).to_lowercase()))
        .collect();
    shell.say("areas", areas.join(" "));
    shell.say(
        "page",
        match &frame.page {
            // Whose it is, of the HMI's own: SYS's, or the menu of an area
            // that has views.
            Page::Own { title, .. } => format!("own {title}"),
            // Another program's, and the on-screen keyboard if it is up
            // over it, with whether a key of it is lit.
            Page::External { keyboard: None, .. } => "external".to_string(),
            Page::External { keyboard: Some(keyboard), .. } => format!(
                "external keyboard{}{}",
                // The pad, where the field is a number's.
                if keyboard.rows == 4 { " pad" } else { "" },
                if keyboard.keys.iter().any(|key| key.pressed) { " lit" } else { "" },
            ),
            Page::Tiles { tiles } if tiles.is_empty() => "tiles none".to_string(),
            Page::Tiles { tiles } => {
                format!("tiles {}", tiles.iter().map(|tile| tile.label.as_str()).collect::<Vec<_>>().join(" "))
            }
            Page::Alarms { page, pages, rows, .. } => format!("alarms {page}/{pages} rows={}", rows.len()),
            Page::Threats { view: ThreatsView::Table { page, pages, rows }, .. } => {
                format!("threats {page}/{pages} rows={}", rows.len())
            }
            Page::Threats { view: ThreatsView::Plot(plot), .. } => format!(
                "threats plotted={} elsewhere={} to {} m",
                plot.threats.len(),
                plot.elsewhere,
                plot.range_m
            ),
            // A PIN being asked for, and how that stands. Nothing of the PIN
            // is for the console: not what, and not how much of it.
            Page::Pin(pin) => format!("pin {}", format!("{:?}", pin.standing).to_lowercase()),
            // A form: where the cursor is and how its data stands; the
            // keyboard, if it is up, with the key the highlight is on and
            // the key that is lit - or the question asked before changes
            // are discarded; and what the fields on the page hold.
            // What has been typed is not for the console: how much of it,
            // field by field, is.
            Page::Form(form) => format!(
                "form field {}/{} {}{} {:?}",
                form.field,
                form.fields,
                format!("{:?}", form.standing).to_lowercase(),
                match &form.keyboard {
                    Some(keyboard) => format!(
                        " keyboard={}{}",
                        format!("{:?}", keyboard.layout).to_lowercase(),
                        if keyboard.keys.iter().any(|key| key.pressed) { " lit" } else { "" },
                    ),
                    None if form.discard => " discard?".to_string(),
                    None => String::new(),
                },
                form.rows.iter().map(|row| row.value.chars().count()).collect::<Vec<_>>(),
            ),
        },
    );
    let counts = frame.status.alarms;
    let showing = frame.alarm.as_ref().map_or_else(
        || "nothing".to_string(),
        |m| format!("\"{}\"{}", m.text, if m.acknowledged { " acknowledged" } else { "" }),
    );
    shell.say(
        "alarms",
        format!("W{} C{} A{} showing {showing}", counts.warnings, counts.cautions, counts.advisories),
    );

    let showing = frame.threat.as_ref().map_or_else(
        || "nothing".to_string(),
        |m| format!("\"{}\"{}", m.text, if m.acknowledged { " acknowledged" } else { "" }),
    );
    let marked = frame.ppi.as_ref().map_or_else(
        || "no ppi".to_string(),
        |ppi| format!("ppi {:?}", ppi.threats.iter().map(|threat| threat.bearing_mils).collect::<Vec<_>>()),
    );
    shell.say("threats", format!("showing {showing} {marked}"));

    // What F19 has hidden. The Plan Position Indicator, hidden, is not in
    // the frame: "no ppi" above says so.
    shell.say(
        "hidden",
        match (frame.hidden.labels, frame.hidden.status) {
            (false, _) => "nothing",
            (true, false) => "labels",
            (true, true) => "labels status",
        }
        .to_string(),
    );
    shell.say("touch-screen", if frame.touch { "on" } else { "off" }.to_string());
    // Who is logged in, where anybody can be.
    let who = match shell.hmi.lock().unwrap().session() {
        Session::Nobody => "nobody".to_string(),
        Session::Somebody { account, roles } => format!("{account} roles={}", roles.join(",")),
    };
    shell.say("access", who);

    // No window yet, or none any more: there is nobody to tell.
    let _ = app.emit("frame", frame);
}

#[tauri::command]
fn frame(shell: State<Shell>) -> Frame {
    shell.frame()
}

#[tauri::command]
fn first_frame(app: AppHandle, shell: State<Shell>) {
    if shell.first_frame.set(uptime()).is_ok() {
        marker("first-frame");
        // For whoever starts the other programs: the HMI is on the screen,
        // and what they cost to start no longer delays it.
        if let Some(dir) = env::var_os("XDG_RUNTIME_DIR") {
            let _ = fs::write(Path::new(&dir).join("opengva-hmi.ready"), "");
        }
        refresh(&app);
    }
}

/// What of the frame has been touched, as the UI says it: a soft-key label
/// by its button's name ("F14"), or a row of the table on the page - or a
/// threat's symbol on the threats' picture - by the name the frame gave it;
/// or a field of a form's page by its number, or a key of the on-screen
/// keyboard - a form's, or the one up over another program's page - by its
/// place among the keys.
#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Touch {
    Label { key: String },
    Row { id: String },
    Field { number: usize },
    Key { at: usize },
}

/// The UI saying that something the frame had drawn has been touched. What
/// that does is the core's to say.
#[tauri::command]
fn touch(app: AppHandle, shell: State<Shell>, touched: Touch) {
    let touched = match touched {
        Touch::Label { key } => match key.parse() {
            Ok(button) => Touched::Label(button),
            Err(()) => return,
        },
        Touch::Row { id } => Touched::Row(id),
        Touch::Field { number } => Touched::Field(number),
        Touch::Key { at } => Touched::Key(at),
    };
    // A key of a PIN's pad is a digit of the PIN: that one was touched is
    // for the console, and which is not.
    let changed = {
        let mut hmi = shell.hmi.lock().unwrap();
        if hmi.untold(&touched) { marker("touch pin") } else { marker(format_args!("touch {touched}")) }
        hmi.touch(touched, shell.now())
    };
    if changed {
        refresh(&app);
    }
}

/// The UI saying that the finger which touched a key of the on-screen
/// keyboard has gone: the key is lit until then.
#[tauri::command]
fn lift(app: AppHandle, shell: State<Shell>) {
    marker("lift");
    if shell.hmi.lock().unwrap().lift() {
        refresh(&app);
    }
}

/// A rectangle of the window, in the UI's pixels.
#[derive(serde::Deserialize)]
struct Rect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

/// Where the UI has drawn the display in the window: its top left corner,
/// in the UI's pixels, and how many of those one of the display's own is.
/// The display is drawn at its reference size and scaled to the screen.
#[derive(Clone, Copy, PartialEq, serde::Deserialize)]
struct Display {
    x: f64,
    y: f64,
    scale: f64,
}

/// The UI saying which parts of the window are the bars and labels - what
/// stays put whatever the page - and whether a touch on the rest goes
/// `through` to another program, whose picture is there. The display server
/// is told both:
///
/// - the bars and labels are the window's opaque region. What is left is
///   where another program's window is put, so that the bars take nothing
///   off its picture - and when F19 hides them, it has their room;
/// - while a touch on the rest is the other program's, the window's input
///   region is the bars and labels only. Otherwise it is the whole window:
///   the page is the HMI's own, or the touch screen is off
///   and no touch is to reach anything.
///
/// It says where the `display` is in the window too, which is what says
/// where a tile of it is: windows in tiles are put there again if that has
/// changed.
#[tauri::command]
fn solid(
    app: AppHandle,
    window: WebviewWindow,
    shell: State<Shell>,
    rects: Vec<Rect>,
    through: bool,
    display: Display,
) {
    if display.scale > 0.0 {
        let was = std::mem::replace(&mut *shell.display.lock().unwrap(), display);
        if was != display {
            refresh(&app);
        }
    }
    // GTK is only to be touched from the thread it runs on.
    let _ = app.run_on_main_thread(move || {
        let Ok(window) = window.gtk_window() else { return };
        let rects: Vec<_> =
            rects.iter().map(|r| RectangleInt::new(r.x, r.y, r.width, r.height)).collect();
        let solid = Region::create_rectangles(&rects);
        window.input_shape_combine_region(through.then_some(&solid));
        if let Some(surface) = window.window() {
            surface.set_opaque_region(Some(&solid));
        }
    });
}

fn main() {
    let app_start = uptime();
    marker("app-start");
    // What is written down about access goes to the system's log by this
    // name ([`record`]).
    // SAFETY: the name is a constant, there for as long as the process is.
    unsafe { libc::openlog(c"opengva-access".as_ptr(), 0, libc::LOG_AUTH) };

    tauri::Builder::default()
        .manage(Shell {
            hmi: Mutex::new(configured_hmi()),
            windows: OnceLock::new(),
            display: Mutex::new(Display { x: 0.0, y: 0.0, scale: 1.0 }),
            feed: OnceLock::new(),
            apps: OnceLock::new(),
            timekeeper: OnceLock::new(),
            said: Mutex::new(HashMap::new()),
            started: Instant::now(),
            app_start,
            first_frame: OnceLock::new(),
        })
        .invoke_handler(tauri::generate_handler![frame, first_frame, solid, touch, lift])
        // The window is made hidden (tauri.conf.json) and shown here, once
        // its page has started to load. Shown from the start it is drawn
        // before the process that renders the page is ready, and WebKit then
        // holds this one for half a second, waiting for a picture that cannot
        // come: the other is waiting for this one to answer it.
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Started {
                let _ = webview.window().show();
            }
        })
        .setup(|app| {
            // A page that never starts to load still has its window, late.
            if let Some(window) = app.get_webview_window("main") {
                thread::spawn(move || {
                    thread::sleep(Duration::from_secs(3));
                    if !window.is_visible().unwrap_or(false) {
                        let _ = window.show();
                    }
                });
            }

            // Bezel buttons: from the input devices, to the core, to the screen.
            let (events, buttons) = mpsc::channel();
            bezel_input::watch(events);
            let handle = app.handle().clone();
            thread::spawn(move || {
                for event in buttons {
                    let shell = handle.state::<Shell>();
                    let changed = {
                        let mut hmi = shell.hmi.lock().unwrap();
                        // A button that is a digit of a PIN has the digit's
                        // name: that one was pressed is for the console,
                        // and which is not.
                        if hmi.unspoken(event.button) {
                            marker(format_args!("button pin {}", event.action));
                        } else {
                            marker(format_args!("button {event}"));
                        }
                        hmi.button(event, shell.now())
                    };
                    if changed {
                        refresh(&handle);
                    }
                }
            });

            // Other programs' windows: an area that is one can be selected
            // while the window is open.
            let handle = app.handle().clone();
            // A program says that a field of its page has the focus, or that
            // none has now: the on-screen keyboard comes up by itself, and
            // goes. The display server says it for the program whose window
            // is on show; a program that the display server hears nothing
            // of says it for itself, of its own window by name.
            let asking = app.handle().clone();
            let asks = Asks::new(move |window, kind| {
                let shell = asking.state::<Shell>();
                let wants = kind.map_or_else(|| "nothing".to_string(), |kind| format!("{kind:?}").to_lowercase());
                let changed = match window {
                    // A field says it again each time something is typed
                    // into it: the console is told when what is asked for
                    // is not what was last asked for.
                    None => {
                        shell.say("asked", wants);
                        shell.hmi.lock().unwrap().asked(kind)
                    }
                    // A program says it when its field is touched again,
                    // and that is what the console is told.
                    Some(window) => {
                        marker(format_args!("asked-for {window} {wants}"));
                        shell.hmi.lock().unwrap().asked_for(&Asking { window: window.to_string(), kind })
                    }
                };
                if changed {
                    refresh(&asking);
                }
            });
            let windows = Windows::watch(
                move |open| {
                    // In the log, for whoever is writing a file for AREAS_DIR
                    // and needs to know what their program's window is called.
                    eprintln!("opengva-hmi: windows open: {open:?}");
                    if handle.state::<Shell>().hmi.lock().unwrap().windows(&open) {
                        refresh(&handle);
                    }
                },
                asks.clone(),
            );
            if let Err(why) = &windows {
                eprintln!("opengva-hmi: no other program's window can be shown: {why}");
            }
            let _ = app.state::<Shell>().windows.set(windows.ok());
            asks::listen(asks);

            // The feed: what the vehicle has to say, to the core, to the
            // screen. A line that says nothing the HMI knows of is reported
            // and passed over.
            if let Some(paths) = env::var_os(FEED) {
                let (up, lines) = mpsc::channel();
                let paths = env::split_paths(&paths).collect();
                let _ = app.state::<Shell>().feed.set(Feed::follow(paths, hmi_core::feed::READY, up));
                let handle = app.handle().clone();
                thread::spawn(move || {
                    for line in lines {
                        match Report::parse(&line) {
                            Ok(report) => {
                                let shell = handle.state::<Shell>();
                                shell.hmi.lock().unwrap().report(report, shell.now());
                                refresh(&handle);
                            }
                            Err(why) => eprintln!("opengva-hmi: feed: {why}: {line}"),
                        }
                    }
                });
            }

            // The programs that listen for the HMI: told of their buttons,
            // and saying what those are. Each is looked for until it is
            // there, and again when it goes, as the feed is.
            let sockets = app.state::<Shell>().hmi.lock().unwrap().app_sockets();
            let mut apps = HashMap::new();
            for (area, socket) in sockets {
                let (up, lines) = mpsc::channel();
                apps.insert(area, Feed::follow(vec![PathBuf::from(&socket)], hmi_core::feed::READY, up));
                let handle = app.handle().clone();
                thread::spawn(move || {
                    for line in lines {
                        let shell = handle.state::<Shell>();
                        let said = shell.hmi.lock().unwrap().from_app(area, &line);
                        match said {
                            Ok(true) => refresh(&handle),
                            Ok(false) => {}
                            Err(why) => eprintln!("opengva-hmi: {socket}: {why}: {line}"),
                        }
                    }
                });
            }
            let _ = app.state::<Shell>().apps.set(apps);

            // Time: the core is told of it when it has said it wants to be -
            // an alarm's message has had its ten seconds, say - and the
            // date-time group shows minutes, so a new frame is due as each
            // one starts.
            let (wake, woken) = mpsc::channel();
            let _ = app.state::<Shell>().timekeeper.set(wake);
            let handle = app.handle().clone();
            thread::spawn(move || {
                let shell = handle.state::<Shell>();
                let mut minute = unix_time() / 60;
                loop {
                    let now = shell.now();
                    let to_the_minute = (60 - now.unix_s % 60) * 1000;
                    let wait = match shell.hmi.lock().unwrap().deadline() {
                        Some(deadline) => to_the_minute.min(deadline.saturating_sub(now.uptime_ms)),
                        None => to_the_minute,
                    };
                    let _ = woken.recv_timeout(Duration::from_millis(wait.max(1)));
                    while woken.try_recv().is_ok() {}

                    let changed = shell.hmi.lock().unwrap().tick(shell.now());
                    if changed || unix_time() / 60 != minute {
                        minute = unix_time() / 60;
                        refresh(&handle);
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("the HMI window could not be created");
}
