//! What programs say of their fields: that one has the focus and wants
//! typing, and what it takes, or that none has it now.
//!
//! It is said in one of two ways. A program that speaks the text-input
//! protocol says it to the display server, which tells the HMI as its input
//! method (`windows`): that is said of the window on show, whichever it is.
//! A program that cannot - one that takes keys and tells the display server
//! nothing of its fields - says it to the HMI itself, of its own window by
//! name: a line written to a pipe the HMI makes in its runtime directory
//! ([`PIPE`]), as [`Asking`] reads it:
//!
//! ```text
//! WINDOW text|number|decimal|nothing
//! ```
//!
//! ```text
//! echo "my-window number" > "$XDG_RUNTIME_DIR/opengva-hmi.keyboard"
//! ```
//!
//! What the HMI makes of either is the core's to say (`Hmi::asked`,
//! `Hmi::asked_for`). Here it is only left to settle first ([`Asks`]), and
//! passed on.

use hmi_core::{Asking, Kind};
use std::collections::HashMap;
use std::ffi::CString;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{env, thread};

/// The pipe, by its name in the runtime directory (`XDG_RUNTIME_DIR`). Who
/// may write to it is who may write there: the HMI's own user, and root.
const PIPE: &str = "opengva-hmi.keyboard";

/// The longest line the pipe is read for. A window's name and a word are
/// far shorter; what goes on longer without ending is not a line of this.
const LONGEST: u64 = 1024;

/// How long what a program says of its fields is left to settle before it
/// is acted on, where acting at once would have the keyboard go and come
/// back: a toolkit that goes from one field to the next says that the first
/// has let go and then that the second has the focus.
const SETTLING: Duration = Duration::from_millis(150);

/// Where what programs say of their fields is passed on, once it has
/// settled: to `asked`, with whose window it is said of - `None` for the one
/// on show, which is all the display server speaks of - and what the field
/// that has the focus takes, or `None` when no field has it.
pub struct Asks {
    asked: Box<dyn Fn(Option<&str>, Option<Kind>) + Send + Sync>,
    /// How things stand with each window that has a field, or had one a
    /// moment ago. It is held while `asked` is called, so that what is said
    /// later is passed on later.
    said: Mutex<HashMap<Option<String>, Said>>,
}

#[derive(Default)]
struct Said {
    /// How many times something has been said of the window. What was put
    /// off to settle is dropped if more has been said since.
    turn: u64,
    /// Whether a field had the focus when it was last said.
    field: bool,
}

impl Asks {
    pub fn new(asked: impl Fn(Option<&str>, Option<Kind>) + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Asks { asked: Box::new(asked), said: Mutex::new(HashMap::new()) })
    }

    /// Take what has just been said of a window, and pass it on now or once
    /// it has settled. A field that takes the focus has the keyboard at
    /// once. One that lets go may be letting go for the next, and one that
    /// says more of itself - it does when it is touched again - may be
    /// about to let go: those settle first.
    pub fn say(self: &Arc<Self>, window: Option<&str>, kind: Option<Kind>) {
        let mut said = self.said.lock().unwrap();
        let of = said.entry(window.map(str::to_string)).or_default();
        of.turn += 1;
        let was = std::mem::replace(&mut of.field, kind.is_some());
        if kind.is_some() && !was {
            (self.asked)(window, kind);
            return;
        }
        let (asks, window, turn) = (self.clone(), window.map(str::to_string), of.turn);
        thread::spawn(move || {
            thread::sleep(SETTLING);
            let mut said = asks.said.lock().unwrap();
            if said.get(&window).is_some_and(|of| of.turn == turn) {
                (asks.asked)(window.as_deref(), kind);
                // A window with no field has nothing more to be kept of it.
                if kind.is_none() {
                    said.remove(&window);
                }
            }
        });
    }
}

/// Make the pipe and hear what is written to it, on a thread of its own.
/// Without a runtime directory there is nowhere for it, and no program says
/// anything for itself: one that speaks through the display server is heard
/// all the same.
pub fn listen(asks: Arc<Asks>) {
    let Some(dir) = env::var_os("XDG_RUNTIME_DIR") else { return };
    let path = PathBuf::from(dir).join(PIPE);
    thread::spawn(move || {
        let said = |why: &dyn std::fmt::Display| eprintln!("opengva-hmi: {}: {why}", path.display());
        let pipe = match open(&path) {
            Ok(pipe) => pipe,
            Err(why) => return said(&format_args!("{why}: no program can say for itself that it wants typing")),
        };
        let mut pipe = BufReader::new(pipe);
        loop {
            match line(&mut pipe) {
                Ok(Some(line)) => match Asking::parse(&line) {
                    Ok(asking) => asks.say(Some(&asking.window), asking.kind),
                    Err(why) => said(&format_args!("{why}: {line}")),
                },
                // Not a line of text, or too long for one: passed over.
                Ok(None) => said(&"something was written that is not a line of this"),
                Err(why) => return said(&why),
            }
        }
    });
}

/// Make the pipe, if it is not there already, and open it. For writing as
/// well as reading, though nothing is written: a pipe that somebody has open
/// for writing does not end when a program that wrote to it goes away.
fn open(path: &Path) -> io::Result<File> {
    let name = CString::new(path.as_os_str().as_bytes())?;
    // SAFETY: the name ends in a NUL and outlives the call.
    if unsafe { libc::mkfifo(name.as_ptr(), 0o600) } != 0 {
        let why = io::Error::last_os_error();
        if why.kind() != io::ErrorKind::AlreadyExists {
            return Err(why);
        }
    }
    let pipe = File::options().read(true).write(true).open(path)?;
    if !pipe.metadata()?.file_type().is_fifo() {
        return Err(io::Error::other("it is there, and is not a pipe"));
    }
    Ok(pipe)
}

/// The next line written to the pipe, without what ended it; `None` for one
/// that is not text, or that goes on for longer than a line of this does -
/// the rest of which is read and dropped.
fn line(pipe: &mut impl BufRead) -> io::Result<Option<String>> {
    let (mut line, mut whole) = (Vec::new(), true);
    loop {
        if pipe.by_ref().take(LONGEST).read_until(b'\n', &mut line)? == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        if line.pop() == Some(b'\n') {
            return Ok(if whole { String::from_utf8(line).ok() } else { None });
        }
        // As much as a line can be, and it has not ended: it is no line of
        // this, and what is left of it is nobody's.
        whole = false;
        line.clear();
    }
}
