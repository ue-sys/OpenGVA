//! The VNC viewer's shell: a window with a VNC server's screen in it.
//!
//! It is run as `opengva-vnc-viewer NAME`, and is the view that
//! [`VIEWS_DIR`]`/NAME.conf` describes (`vnc_view::View`): which server, what
//! the window is called, and whether the screen can be touched or only looked
//! at. The HMI shows the window in a functional area if a file in
//! `/etc/opengva/gva-fa.d` gives one to a window of that name. To the HMI it is
//! one more program's window, and the two say nothing to each other.
//!
//! The VNC is noVNC's, in the page. noVNC speaks down a WebSocket and a VNC
//! server listens on a TCP port, so the two are joined here
//! (`vnc_view::Bridge`), on a port of this machine's own that only the page
//! is given the key to.
//!
//! What crosses to the page:
//!
//! - the `view` command returns what it is to show, and how;
//! - the `connected` command is the page saying that it has the server's
//!   screen;
//! - the `disconnected` command is the page saying that it has not, and why.
//!
//! A server that is not there, or that goes, is not this program's end: the
//! page says so on the window and goes on trying.
//!
//! Several views can run at once, each this program again. What the web view
//! of one keeps on disk, it keeps in a directory of that view's own.
//!
//! Lines on stderr that start `OPENGVA ` are markers, as the HMI's are:
//! `OPENGVA vnc NAME connected` each time the server's screen is had, and
//! `OPENGVA vnc NAME lost` each time it goes. "uptime" is seconds since the
//! kernel started.

use std::path::Path;
use std::sync::Mutex;
use std::{env, fs, process};
use tauri::{Manager, State, WebviewWindowBuilder};
use vnc_view::{Bridge, Scale, View};

/// Where the views are configured: one `.conf` file each, as [`View::parse`]
/// reads them.
const VIEWS_DIR: &str = "/etc/opengva/vnc.d";

struct Shell {
    /// Which view this is: the name it was started with.
    name: String,
    view: View,
    /// The server's password, if it is to be given one.
    password: Option<String>,
    bridge: Bridge,
    said: Mutex<Said>,
}

/// What has been printed about the server, so that it is printed when it
/// changes and not every time the page tries again.
#[derive(Default)]
struct Said {
    connected: bool,
    why: String,
}

/// The view called `name`, and its server's password if it has one.
fn configured(name: &str) -> Result<(View, Option<String>), String> {
    // It is part of a file's name, and of a marker.
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(format!("'{name}' is not a view's name: those are letters, digits, '-' and '_'"));
    }
    let file = Path::new(VIEWS_DIR).join(format!("{name}.conf"));
    let view = fs::read_to_string(&file)
        .map_err(|e| e.to_string())
        .and_then(|text| View::parse(&text))
        .map_err(|why| format!("{}: {why}", file.display()))?;
    let password = view.password()?;
    Ok((view, password))
}

/// Seconds since the kernel started. `None` where there is no /proc.
fn uptime() -> Option<f64> {
    fs::read_to_string("/proc/uptime").ok()?.split_whitespace().next()?.parse().ok()
}

/// Print a line, in one write: stderr is shared with whatever else the
/// service prints - WebKit's own processes among them - and a line that goes
/// out in pieces can have somebody else's in the middle of it.
fn print(line: String) {
    use std::io::Write;
    let _ = std::io::stderr().write_all(format!("{line}\n").as_bytes());
}

fn marker(what: impl std::fmt::Display) {
    print(format!("OPENGVA {what} uptime={:.2}", uptime().unwrap_or(-1.0)));
}

/// What the page is to show, and how.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Showing {
    /// The server, as it is called on the window when its screen is not there.
    server: String,
    /// Where the page connects to reach it: the bridge, and the key to it.
    url: String,
    view_only: bool,
    scale: &'static str,
    /// The page is given the password because the VNC is the page's: it is
    /// noVNC that answers the server's challenge with it.
    password: Option<String>,
}

#[tauri::command]
fn view(shell: State<Shell>) -> Showing {
    Showing {
        server: shell.view.server.to_string(),
        url: shell.bridge.url(),
        view_only: shell.view.view_only,
        scale: match shell.view.scale {
            Scale::Fit => "fit",
            Scale::None => "none",
        },
        password: shell.password.clone(),
    }
}

#[tauri::command]
fn connected(shell: State<Shell>) {
    let mut said = shell.said.lock().unwrap();
    if !said.connected {
        marker(format_args!("vnc {} connected", shell.name));
    }
    *said = Said { connected: true, why: String::new() };
}

/// The page saying that it has not the server's screen: because it has just
/// lost it, or because it has tried for it again and not got it. `why` is in
/// the words of whichever end said so, for the log.
#[tauri::command]
fn disconnected(shell: State<Shell>, why: String) {
    let mut said = shell.said.lock().unwrap();
    if said.connected {
        marker(format_args!("vnc {} lost", shell.name));
    }
    if said.connected || said.why != why {
        print(format!("opengva-vnc-viewer: {}: {}: {why}", shell.name, shell.view.server));
    }
    *said = Said { connected: false, why };
}

fn main() {
    let mut arguments = env::args().skip(1);
    let (Some(name), None) = (arguments.next(), arguments.next()) else {
        eprintln!("usage: opengva-vnc-viewer NAME    (the view of {VIEWS_DIR}/NAME.conf)");
        process::exit(2);
    };
    let shell = configured(&name).and_then(|(view, password)| {
        let bridge = Bridge::open(view.server.clone()).map_err(|e| format!("cannot listen for the page: {e}"))?;
        Ok(Shell { name, view, password, bridge, said: Mutex::default() })
    });
    let shell = shell.unwrap_or_else(|why| {
        eprintln!("opengva-vnc-viewer: {why}");
        process::exit(1);
    });

    // A window is known to the display server, and through it to the HMI, by
    // its application id, which GTK takes from the program's name: so that is
    // made the name the view's configuration gives its window, before GTK
    // finds one for itself.
    glib::set_prgname(Some(shell.view.window.as_str()));

    tauri::Builder::default()
        .manage(shell)
        .invoke_handler(tauri::generate_handler![view, connected, disconnected])
        .setup(|app| {
            // The window is as the configuration has it (tauri.conf.json),
            // but for where its web view keeps what it keeps: WebKit's cache
            // and the files beside it. Left alone that is one directory for
            // the program, and so for every view of it that is running; here
            // each view has one of its own, under that.
            let window = app.config().app.windows[0].clone();
            let kept = app.path().app_local_data_dir()?.join(&app.state::<Shell>().name);
            WebviewWindowBuilder::from_config(app, &window)?.data_directory(kept).build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("the viewer's window could not be created");
}
