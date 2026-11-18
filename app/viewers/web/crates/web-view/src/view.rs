//! What a view is told to be: its configuration.

use crate::Places;
use std::path::PathBuf;

/// A view of a web page, as its configuration describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    /// The page: where the view starts, and where it goes back to when what
    /// it was showing can no longer be had.
    pub url: String,
    /// The name the viewer gives its window - on Wayland, its application
    /// id. It is what a functional area is given to.
    pub window: String,
    /// Who this machine is to a server that asks, if it is to say.
    pub identity: Option<Identity>,
    /// The file with the certificate of the authority that vouches for the
    /// page's server: PEM. A view that names one takes no other's word, and
    /// nothing else on the machine takes this one's because the view does.
    /// A view that names none takes the word of those the system knows.
    pub authority: Option<PathBuf>,
    /// Where the view may go: the place its page is at, and any others its
    /// configuration allows.
    pub places: Places,
}

/// Who this machine is to a server that asks: a certificate, and the key
/// that shows the certificate to be this machine's. Two PEM files. The
/// certificate is the first in its file; what follows it there is not shown
/// to anyone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub certificate: PathBuf,
    pub key: PathBuf,
}

/// A line without its comment. `#` starts a comment where a word could
/// start; inside a word it is the word's, as it is in an address.
fn said(line: &str) -> &str {
    let mut before = ' ';
    for (at, character) in line.char_indices() {
        if character == '#' && before.is_whitespace() {
            return line[..at].trim();
        }
        before = character;
    }
    line.trim()
}

impl View {
    /// Read a view's configuration: a name and a value to a line, `#`
    /// starting a comment, as the files that give functional areas away are
    /// written.
    ///
    /// ```text
    /// url          https://server:8443/status/index.html
    /// window       server-status           # what an area is given to
    /// certificate  /etc/station/certificate.pem    # optional, with its key:
    /// key          /etc/station/key.pem            # who this machine is
    /// authority    /etc/station/authority.pem      # optional: who vouches for the server
    /// allow        https://maps:8443       # optional, as often as wanted:
    ///                                      # another place the view may go
    /// ```
    pub fn parse(text: &str) -> Result<Self, String> {
        let (mut url, mut window, mut certificate, mut key, mut authority) = (None, None, None, None, None);
        let mut allowed = Vec::new();
        for (number, line) in (1..).zip(text.lines()) {
            let line = said(line);
            if line.is_empty() {
                continue;
            }
            let (name, value) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            let value = value.trim();
            if value.is_empty() {
                return Err(format!("line {number}: '{name}' has no value"));
            }
            let slot = match name {
                "url" => &mut url,
                "window" => &mut window,
                "certificate" => &mut certificate,
                "key" => &mut key,
                "authority" => &mut authority,
                // A view may be allowed as many places as it is given.
                "allow" => {
                    allowed.push((number, value));
                    continue;
                }
                _ => return Err(format!("line {number}: '{name}' is not something a view has")),
            };
            if slot.replace((number, value)).is_some() {
                return Err(format!("line {number}: '{name}' is given twice"));
            }
        }

        let (line, url) = url.ok_or("no 'url'")?;
        let (_, window) = window.ok_or("no 'window'")?;
        let mut places = Places::of(url).map_err(|why| format!("line {line}: {why}"))?;
        for (line, place) in allowed {
            places.allow(place).map_err(|why| format!("line {line}: {why}"))?;
        }
        let identity = match (certificate, key) {
            (Some((_, certificate)), Some((_, key))) => {
                Some(Identity { certificate: PathBuf::from(certificate), key: PathBuf::from(key) })
            }
            (Some((line, _)), None) => return Err(format!("line {line}: a 'certificate' is no use without its 'key'")),
            (None, Some((line, _))) => return Err(format!("line {line}: a 'key' is no use without its 'certificate'")),
            (None, None) => None,
        };
        Ok(View {
            url: url.to_string(),
            window: window.to_string(),
            identity,
            authority: authority.map(|(_, file)| PathBuf::from(file)),
            places,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_configuration_names_a_page_and_a_window() {
        let view = View::parse("url https://server:8443/status/index.html\nwindow server-status\n").unwrap();
        assert_eq!(view.url, "https://server:8443/status/index.html");
        assert_eq!(view.window, "server-status");
        assert_eq!((view.identity, view.authority), (None, None));
        // The page's own place is where it may go, and nowhere else.
        assert!(view.places.has("https://server:8443/other/page"));
        assert!(!view.places.has("https://elsewhere/"));
    }

    #[test]
    fn a_configuration_can_say_who_the_machine_is_who_vouches_for_the_server_and_where_else_to_go() {
        let text = "# The server's status page\n\
                    window\tServer Status   # a name can have spaces\n\n\
                    url          https://server:8443/status/\n\
                    certificate  /etc/station/certificate.pem\n\
                    key          /etc/station/key.pem   # nobody else's to read\n\
                    authority    /etc/station/authority.pem\n\
                    allow        https://maps:8443\n\
                    allow        http://tiles.example\n";
        let view = View::parse(text).unwrap();
        assert_eq!(view.window, "Server Status");
        assert_eq!(
            view.identity,
            Some(Identity {
                certificate: PathBuf::from("/etc/station/certificate.pem"),
                key: PathBuf::from("/etc/station/key.pem"),
            })
        );
        assert_eq!(view.authority, Some(PathBuf::from("/etc/station/authority.pem")));
        for address in ["https://server:8443/", "https://maps:8443/1/2/3.png", "http://tiles.example/1/2/3.png"] {
            assert!(view.places.has(address), "{address}");
        }
        assert!(view.places.has_secure("server", 8443));
        assert!(!view.places.has_secure("tiles.example", 80));
    }

    #[test]
    fn a_hash_starts_a_comment_only_where_a_word_could_start() {
        // An address can have one in it, and keeps it...
        let view = View::parse("url https://server/app/index.html#!/clients   # the dashboard\nwindow w").unwrap();
        assert_eq!(view.url, "https://server/app/index.html#!/clients");
        // ...and so can a file's name.
        let view = View::parse("url https://server/\nwindow w\nauthority /etc/ca#1.pem#not a comment\n#authority x").unwrap();
        assert_eq!(view.authority, Some(PathBuf::from("/etc/ca#1.pem#not a comment")));
        assert_eq!(said("  # nothing but a comment"), "");
        assert_eq!(said("name value\t# comment # and more"), "name value");
    }

    #[test]
    fn a_configuration_that_cannot_be_right_says_why() {
        let error = |text: &str| View::parse(text).unwrap_err();
        assert!(error("window w").contains("no 'url'"));
        assert!(error("url https://server/").contains("no 'window'"));
        assert!(error("url https://server/\nwindow").contains("line 2: 'window' has no value"));
        assert!(error("url https://server/\nwindow w\nzoom 2").contains("line 3: 'zoom'"));
        assert!(error("url https://server/\nurl https://other/\nwindow w").contains("line 2: 'url' is given twice"));
        assert!(error("window w\nurl server/index.html").contains("line 2: 'server/index.html' is not the address of a page"));
        assert!(error("url https://server/\nwindow w\nallow https://maps/tiles").contains("line 3: 'https://maps/tiles' is not a place"));
        // A name is a word and then its value: there is no sign between them.
        assert!(error("url = https://server/\nwindow w").contains("line 1"));
    }

    #[test]
    fn a_certificate_and_its_key_go_together() {
        let error = |text: &str| View::parse(text).unwrap_err();
        let page = "url https://server/\nwindow w\n";
        assert!(error(&format!("{page}certificate /c.pem")).contains("line 3: a 'certificate' is no use without its 'key'"));
        assert!(error(&format!("{page}\nkey /k.pem")).contains("line 4: a 'key' is no use without its 'certificate'"));
        // An authority needs neither: a server can be known without being told who is asking.
        assert!(View::parse(&format!("{page}authority /a.pem")).unwrap().identity.is_none());
    }
}
