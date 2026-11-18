//! The line the feed comes down.
//!
//! What the HMI is told about the vehicle, and what it says back, are lines
//! of text (`hmi_core::feed` has what they say). This is where they come from
//! and go to: the other end of a path, held by whatever knows the vehicle.
//! The path is one of two things:
//!
//! - a Unix socket that the other end listens on, which the HMI connects to;
//! - a character device, such as the port a virtual machine's host can talk
//!   to its guest through, which the HMI opens.
//!
//! Neither has to be there when the HMI starts, or to stay: the feed is
//! looked for until it is found, and again when it goes. Each time there
//! comes to be someone at the other end the HMI says so to them, with a line
//! of the caller's choosing: when a socket is connected to, and when a
//! device's other end is opened, if it is of a kind that says (a virtual
//! machine's port does; a serial line does not, and is greeted once). A line
//! for an end that is not listening is dropped, not kept for later.

use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// A line longer than this is not a line of the feed.
const LONGEST_LINE: usize = 64 * 1024;

/// How often a device with nobody at its other end is asked whether there is
/// now. It cannot be waited on: it is always ready to say that there is not.
const ASKED_EVERY: Duration = Duration::from_millis(250);

/// The feed, for sending down. What comes up it goes to the channel that
/// [`Feed::follow`] was given.
pub struct Feed {
    end: Arc<Mutex<Option<End>>>,
}

/// The HMI's end of the line. Neither kind makes its reader or writer wait.
enum End {
    Socket(UnixStream),
    Device(File),
}

impl End {
    /// Whatever is at the first of `paths` that there is anything at.
    fn open(paths: &[PathBuf]) -> Option<End> {
        paths.iter().find_map(|path| End::open_one(path))
    }

    fn open_one(path: &Path) -> Option<End> {
        let kind = path.metadata().ok()?.file_type();
        if kind.is_socket() {
            let socket = UnixStream::connect(path).ok()?;
            socket.set_nonblocking(true).ok()?;
            Some(End::Socket(socket))
        } else if kind.is_char_device() {
            let options = OpenOptions::new().read(true).write(true).custom_flags(libc::O_NONBLOCK).open(path);
            Some(End::Device(options.ok()?))
        } else {
            None
        }
    }

    fn try_clone(&self) -> Option<End> {
        match self {
            End::Socket(socket) => socket.try_clone().ok().map(End::Socket),
            End::Device(device) => device.try_clone().ok().map(End::Device),
        }
    }

    fn fd(&self) -> RawFd {
        match self {
            End::Socket(socket) => socket.as_raw_fd(),
            End::Device(device) => device.as_raw_fd(),
        }
    }

    fn read(&mut self, into: &mut [u8]) -> std::io::Result<usize> {
        match self {
            End::Socket(socket) => socket.read(into),
            End::Device(device) => device.read(into),
        }
    }

    fn write(&mut self, from: &[u8]) -> std::io::Result<usize> {
        match self {
            End::Socket(socket) => socket.write(from),
            End::Device(device) => device.write(from),
        }
    }
}

impl Feed {
    /// Follow the feed at the first of `paths` that is there, for as long as
    /// `lines` is listened to. Each line that comes up it is sent to `lines`,
    /// without its line ending. `greeting` is sent down it whenever there
    /// comes to be someone at its other end.
    pub fn follow(paths: Vec<PathBuf>, greeting: &str, lines: Sender<String>) -> Feed {
        let end = Arc::new(Mutex::new(None));
        let shared = end.clone();
        let greeting = format!("{greeting}\n");
        thread::spawn(move || {
            loop {
                let Some(mut end) = End::open(&paths) else {
                    thread::sleep(Duration::from_secs(1));
                    continue;
                };
                *shared.lock().unwrap() = end.try_clone();
                let listened_to = read_lines(&mut end, greeting.as_bytes(), &lines);
                *shared.lock().unwrap() = None;
                if !listened_to {
                    return;
                }
                thread::sleep(Duration::from_secs(1));
            }
        });
        Feed { end }
    }

    /// Send a line down the feed, if there is anyone at the other end.
    pub fn send(&self, line: &str) {
        if let Some(end) = self.end.lock().unwrap().as_mut() {
            // In one piece or not at all: an end that cannot take it now
            // does not get the rest of it later, in the middle of another.
            let _ = end.write(format!("{line}\n").as_bytes());
        }
    }
}

/// Send each line that comes up `end` to `lines`, until the end is closed,
/// and `greeting` down it when there comes to be someone at the other end.
/// Returns whether `lines` is still being listened to.
fn read_lines(end: &mut End, greeting: &[u8], lines: &Sender<String>) -> bool {
    let mut waiting = libc::pollfd { fd: end.fd(), events: libc::POLLIN, revents: 0 };
    let mut pending: Vec<u8> = Vec::new();
    let mut some = [0u8; 4096];
    // Whether there is known to be someone at the other end.
    let mut there = false;
    loop {
        if there {
            // SAFETY: `waiting` is one pollfd, as the count says, and outlives the call.
            unsafe { libc::poll(&mut waiting, 1, 1000) };
        }
        let read = match end.read(&mut some) {
            // A socket that has nothing more to say has been closed. A device
            // with nobody at its other end says the same, and is not closed
            // for it - what arrives while it is closed is lost - but asked
            // again in a while.
            Ok(0) => match end {
                End::Socket(_) => return true,
                End::Device(_) => {
                    there = false;
                    pending.clear();
                    thread::sleep(ASKED_EVERY);
                    continue;
                }
            },
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::WouldBlock => 0,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return true,
        };
        // Something to read, or nothing yet: either way from somebody.
        if !there {
            there = true;
            let _ = end.write(greeting);
        }
        pending.extend_from_slice(&some[..read]);
        while let Some(ending) = pending.iter().position(|&byte| byte == b'\n') {
            let line: Vec<u8> = pending.drain(..=ending).collect();
            let line = String::from_utf8_lossy(&line);
            if lines.send(line.trim_end_matches(['\r', '\n']).to_string()).is_err() {
                return false;
            }
        }
        if pending.len() > LONGEST_LINE {
            pending.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;
    use std::io::{BufRead, BufReader};
    use std::os::fd::FromRawFd;
    use std::os::unix::net::UnixListener;
    use std::sync::mpsc::{Receiver, channel};

    fn next(lines: &Receiver<String>) -> String {
        lines.recv_timeout(Duration::from_secs(5)).expect("a line within five seconds")
    }

    /// A path in a directory of this test's own.
    fn path(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hmi-feed-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("feed")
    }

    #[test]
    fn lines_come_up_a_socket_and_go_down_it() {
        let path = path("socket");
        let listener = UnixListener::bind(&path).unwrap();
        let (up, lines) = channel();
        let feed = Feed::follow(vec![PathBuf::from("/nonexistent/feed"), path], "hello", up);

        let (mut source, _) = listener.accept().unwrap();
        let mut from_the_hmi = BufReader::new(source.try_clone().unwrap());
        let mut greeting = String::new();
        from_the_hmi.read_line(&mut greeting).unwrap();
        assert_eq!(greeting, "hello\n");

        source.write_all(b"one\ntwo\r\nthr").unwrap();
        assert_eq!((next(&lines), next(&lines)), ("one".to_string(), "two".to_string()));
        source.write_all(b"ee\n").unwrap();
        assert_eq!(next(&lines), "three", "a line is a line however it arrives");

        feed.send("back");
        let mut back = String::new();
        from_the_hmi.read_line(&mut back).unwrap();
        assert_eq!(back, "back\n");
    }

    #[test]
    fn a_source_that_goes_and_comes_back_is_found_again() {
        let path = path("again");
        let (up, lines) = channel();
        let feed = Feed::follow(vec![path.clone()], "hello", up);
        feed.send("to nobody: there is no source yet");

        // It starts after the HMI...
        thread::sleep(Duration::from_millis(300));
        let listener = UnixListener::bind(&path).unwrap();
        let (mut source, _) = listener.accept().unwrap();
        source.write_all(b"first\n").unwrap();
        assert_eq!(next(&lines), "first");

        // ...and starts again, and is greeted again.
        drop(source);
        let (mut source, _) = listener.accept().unwrap();
        let mut greeting = String::new();
        BufReader::new(&source).read_line(&mut greeting).unwrap();
        assert_eq!(greeting, "hello\n");
        source.write_all(b"second\n").unwrap();
        assert_eq!(next(&lines), "second");
    }

    #[test]
    fn lines_come_up_a_character_device_and_go_down_it() {
        // A pseudo-terminal is a character device anyone can make: the feed
        // gets one end by its path, and this test holds the other.
        // SAFETY: plain calls on a descriptor this test owns; ptsname's
        // result is copied before anything else can change it.
        let (host, path) = unsafe {
            let host = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY);
            assert!(host >= 0 && libc::grantpt(host) == 0 && libc::unlockpt(host) == 0);
            let mut plain: libc::termios = std::mem::zeroed();
            assert_eq!(libc::tcgetattr(host, &mut plain), 0);
            libc::cfmakeraw(&mut plain);
            assert_eq!(libc::tcsetattr(host, libc::TCSANOW, &plain), 0);
            let path = CStr::from_ptr(libc::ptsname(host)).to_str().unwrap().to_string();
            (File::from_raw_fd(host), PathBuf::from(path))
        };
        let (up, lines) = channel();
        let feed = Feed::follow(vec![path], "hello", up);
        let mut from_the_hmi = BufReader::new(&host);
        let mut greeting = String::new();
        from_the_hmi.read_line(&mut greeting).unwrap();
        assert_eq!(greeting.trim_end(), "hello");

        (&host).write_all(b"{\"resolved\":\"oil\"}\n").unwrap();
        assert_eq!(next(&lines), "{\"resolved\":\"oil\"}");

        feed.send("back");
        let mut back = String::new();
        from_the_hmi.read_line(&mut back).unwrap();
        assert_eq!(back.trim_end(), "back");
    }
}
