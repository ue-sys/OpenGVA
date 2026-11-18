//! What a view is told to be: its configuration.

use std::fmt;
use std::fs;
use std::path::PathBuf;

/// The port a VNC server listens on unless it has been told another: that of
/// its display 0.
const USUAL_PORT: u16 = 5900;

/// A view of a VNC server's screen, as its configuration describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    /// The server whose screen it is.
    pub server: Server,
    /// The name the viewer gives its window - on Wayland, its application
    /// id. It is what a functional area is given to.
    pub window: String,
    /// Whether the screen is only to be looked at. If it is, nothing done on
    /// the window is passed on to the server.
    pub view_only: bool,
    /// The file the server's password is in, for a server that asks for one.
    /// A configuration can be read by anyone; this need not be.
    pub password: Option<PathBuf>,
    /// How the screen is fitted into a window that is not its size.
    pub scale: Scale,
}

/// Where a VNC server is: a host, by name or by address, and a TCP port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub host: String,
    pub port: u16,
}

/// How a screen is fitted into a window that is not its size.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Scale {
    /// As large as the window will hold, in its own shape, with black round
    /// what it leaves.
    #[default]
    Fit,
    /// A pixel of the window to a pixel of the screen: in the middle of a
    /// window that is larger, and as much of it as there is room for, from
    /// its top left corner, in one that is smaller.
    None,
}

impl View {
    /// Read a view's configuration: a name and a value to a line, `#`
    /// starting a comment, as the files that give functional areas away are
    /// written.
    ///
    /// ```text
    /// server    192.168.1.20:5900       # the port is 5900 unless said
    /// window    engine-room             # what an area is given to
    /// view-only yes                     # optional: no, unless said
    /// password  /etc/opengva/vnc.d/engine-room.password    # optional
    /// scale     fit                     # optional: fit (unless said) or none
    /// ```
    pub fn parse(text: &str) -> Result<Self, String> {
        let (mut server, mut window, mut view_only, mut password, mut scale) = (None, None, None, None, None);
        for (number, line) in (1..).zip(text.lines()) {
            let line = line.split('#').next().unwrap_or_default().trim();
            if line.is_empty() {
                continue;
            }
            let (name, value) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            let value = value.trim();
            if value.is_empty() {
                return Err(format!("line {number}: '{name}' has no value"));
            }
            let slot = match name {
                "server" => &mut server,
                "window" => &mut window,
                "view-only" => &mut view_only,
                "password" => &mut password,
                "scale" => &mut scale,
                _ => return Err(format!("line {number}: '{name}' is not something a view has")),
            };
            if slot.replace((number, value)).is_some() {
                return Err(format!("line {number}: '{name}' is given twice"));
            }
        }

        let (line, server) = server.ok_or("no 'server'")?;
        let server = Server::parse(server).ok_or_else(|| {
            format!("line {line}: 'server' is a host and perhaps a port, as 192.168.1.20:5900 is, not '{server}'")
        })?;
        let (_, window) = window.ok_or("no 'window'")?;
        let view_only = match view_only {
            Some((_, "yes")) => true,
            Some((_, "no")) | None => false,
            Some((line, other)) => return Err(format!("line {line}: 'view-only' is yes or no, not '{other}'")),
        };
        let scale = match scale {
            Some((_, "fit")) | None => Scale::Fit,
            Some((_, "none")) => Scale::None,
            Some((line, other)) => return Err(format!("line {line}: 'scale' is fit or none, not '{other}'")),
        };
        Ok(View {
            server,
            window: window.to_string(),
            view_only,
            password: password.map(|(_, file)| PathBuf::from(file)),
            scale,
        })
    }

    /// The server's password, if the configuration names a file for one: the
    /// file's first line.
    pub fn password(&self) -> Result<Option<String>, String> {
        let Some(file) = &self.password else { return Ok(None) };
        let text = fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
        Ok(Some(text.lines().next().unwrap_or_default().to_string()))
    }
}

impl Server {
    /// A server as a configuration writes it: `host` or `host:port`. An
    /// IPv6 address has colons of its own, and is in square brackets if a
    /// port follows it.
    fn parse(text: &str) -> Option<Server> {
        let (host, port) = match text.strip_prefix('[') {
            Some(rest) => {
                let (host, after) = rest.split_once(']')?;
                (host, if after.is_empty() { None } else { Some(after.strip_prefix(':')?) })
            }
            None => match text.split_once(':') {
                Some((host, port)) if !port.contains(':') => (host, Some(port)),
                _ => (text, None),
            },
        };
        let port = match port {
            Some(port) => port.parse().ok().filter(|&port| port != 0)?,
            None => USUAL_PORT,
        };
        let named = !host.is_empty() && !host.contains(|c: char| c.is_whitespace() || "[]/".contains(c));
        named.then(|| Server { host: host.to_string(), port })
    }
}

/// A server as it is written, port and all: what it is called on the screen
/// and in the log.
impl fmt::Display for Server {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.host.contains(':') {
            true => write!(f, "[{}]:{}", self.host, self.port),
            false => write!(f, "{}:{}", self.host, self.port),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(host: &str, port: u16) -> Server {
        Server { host: host.to_string(), port }
    }

    #[test]
    fn a_configuration_names_a_server_and_a_window() {
        let view = View::parse("server 192.168.1.20:5901\nwindow engine-room\n").unwrap();
        assert_eq!(
            view,
            View {
                server: server("192.168.1.20", 5901),
                window: "engine-room".to_string(),
                view_only: false,
                password: None,
                scale: Scale::Fit,
            }
        );
    }

    #[test]
    fn a_configuration_can_say_more() {
        let text = "# The engine room's screen\n\nwindow\tEngine Room   # a name can have spaces\n\
                    server engines.vehicle\nview-only yes\npassword /etc/opengva/vnc.d/engines.password\nscale none\n";
        assert_eq!(
            View::parse(text),
            Ok(View {
                server: server("engines.vehicle", 5900),
                window: "Engine Room".to_string(),
                view_only: true,
                password: Some(PathBuf::from("/etc/opengva/vnc.d/engines.password")),
                scale: Scale::None,
            })
        );
        // What is the case unless said, can be said.
        let view = View::parse("server s\nwindow w\nview-only no\nscale fit").unwrap();
        assert_eq!((view.view_only, view.scale), (false, Scale::Fit));
    }

    #[test]
    fn a_server_is_a_host_and_perhaps_a_port() {
        let parsed = |text: &str| View::parse(&format!("window w\nserver {text}")).map(|view| view.server);
        assert_eq!(parsed("10.0.0.7"), Ok(server("10.0.0.7", 5900)));
        assert_eq!(parsed("10.0.0.7:5975"), Ok(server("10.0.0.7", 5975)));
        assert_eq!(parsed("sight.turret:5901"), Ok(server("sight.turret", 5901)));
        // An IPv6 address, alone or in brackets with a port after it.
        assert_eq!(parsed("fe80::1"), Ok(server("fe80::1", 5900)));
        assert_eq!(parsed("[fe80::1]"), Ok(server("fe80::1", 5900)));
        assert_eq!(parsed("[::1]:5975"), Ok(server("::1", 5975)));

        for wrong in [":5900", "host:", "host:port", "host:0", "host:65536", "[::1", "[::1]5900", "[]:5900", "a b"] {
            assert!(parsed(wrong).unwrap_err().contains("line 2: 'server'"), "{wrong}");
        }
    }

    #[test]
    fn a_server_is_written_as_it_is_configured() {
        assert_eq!(server("10.0.0.7", 5975).to_string(), "10.0.0.7:5975");
        assert_eq!(server("sight.turret", 5900).to_string(), "sight.turret:5900");
        assert_eq!(server("::1", 5900).to_string(), "[::1]:5900");
    }

    #[test]
    fn a_configuration_that_cannot_be_right_says_why() {
        let error = |text| View::parse(text).unwrap_err();
        assert!(error("window w").contains("no 'server'"));
        assert!(error("server s").contains("no 'window'"));
        assert!(error("server s\nwindow").contains("line 2: 'window' has no value"));
        assert!(error("server s\nwindow w\nserver t").contains("line 3: 'server' is given twice"));
        assert!(error("server s\nwindow w\ncolour red").contains("line 3: 'colour'"));
        assert!(error("server s\nwindow w\nview-only maybe").contains("line 3: 'view-only' is yes or no"));
        assert!(error("server s\nwindow w\nscale stretch").contains("line 3: 'scale' is fit or none"));
        // The password is in a file of its own, never here: this is a path.
        assert!(error("server s\nwindow w\npassword").contains("line 3: 'password' has no value"));
    }

    #[test]
    fn a_password_is_the_first_line_of_its_file() {
        let file = std::env::temp_dir().join(format!("vnc-view-{}.password", std::process::id()));
        let view = View::parse(&format!("server s\nwindow w\npassword {}", file.display())).unwrap();

        fs::write(&file, "s3cret\n").unwrap();
        assert_eq!(view.password(), Ok(Some("s3cret".to_string())));
        fs::write(&file, "two words \nand a second line\n").unwrap();
        assert_eq!(view.password(), Ok(Some("two words ".to_string())));
        fs::remove_file(&file).unwrap();

        // A file that was to be there and is not, is a fault; none named
        // at all is a server that asks for no password.
        assert!(view.password().unwrap_err().contains("vnc-view-"));
        assert_eq!(View::parse("server s\nwindow w").unwrap().password(), Ok(None));
    }
}
