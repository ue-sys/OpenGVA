//! Where a view may go.

use url::{Host, Origin, Url};

/// The places a view may go: the one its page is at, and any others its
/// configuration allows. A place is what a browser tells one server from
/// another by - how it is asked (http or https), its name and its port: an
/// origin. An address is read by the rules a browser reads it by, so that
/// one written to look like somewhere else is taken for where it leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    allowed: Vec<Origin>,
}

impl Places {
    /// The place a page is at, and no other.
    pub fn of(page: &str) -> Result<Self, String> {
        let not_a_page = || format!("'{page}' is not the address of a page: one is written https://name/...");
        let url = Url::parse(page).map_err(|_| not_a_page())?;
        let fetched = matches!(url.scheme(), "http" | "https") && url.host().is_some();
        if !fetched || !url.username().is_empty() || url.password().is_some() {
            return Err(not_a_page());
        }
        Ok(Places { allowed: vec![url.origin()] })
    }

    /// Allow another place, as a configuration writes one: `https://name`,
    /// or `https://name:8443` where the port is not the usual one.
    pub fn allow(&mut self, place: &str) -> Result<(), String> {
        let not_a_place = || format!("'{place}' is not a place: one is written https://name or https://name:port");
        let url = Url::parse(place).map_err(|_| not_a_place())?;
        // A name with a star in it is an address like any other to a browser,
        // and no server's: one place is one server.
        let only_a_place = matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some_and(|name| !name.contains('*'))
            && url.username().is_empty()
            && url.password().is_none()
            && matches!(url.path(), "" | "/")
            && url.query().is_none()
            && url.fragment().is_none();
        if !only_a_place {
            return Err(not_a_place());
        }
        let origin = url.origin();
        if !self.allowed.contains(&origin) {
            self.allowed.push(origin);
        }
        Ok(())
    }

    /// Whether a view may go to an address. It may if the address is at one
    /// of the places; if it is something a page there made for itself (a
    /// `blob:`); or if it is the nothing a frame holds before it is given
    /// anything. A file, a `data:` address and whatever else is not fetched
    /// from a server, it may not.
    pub fn has(&self, address: &str) -> bool {
        let Ok(url) = Url::parse(address) else { return false };
        match url.scheme() {
            "http" | "https" | "blob" => self.allowed.contains(&url.origin()),
            "about" => matches!(url.path(), "blank" | "srcdoc"),
            _ => false,
        }
    }

    /// Whether one of the places is this server, asked over TLS. Who this
    /// machine is - its certificate - is for these servers to be told, and
    /// for no other that asks.
    pub fn has_secure(&self, host: &str, port: u16) -> bool {
        // An IPv6 address is written in brackets in an address, and may be
        // handed over without them.
        let Ok(host) = Host::parse(host).or_else(|_| Host::parse(&format!("[{host}]"))) else {
            return false;
        };
        self.allowed.iter().any(|place| match place {
            Origin::Tuple(scheme, name, at) => scheme == "https" && *name == host && *at == port,
            Origin::Opaque(_) => false,
        })
    }
}

/// The server an address is at: its name and its port, the usual one for
/// the address if it names none.
pub fn server(address: &str) -> Option<(String, u16)> {
    let url = Url::parse(address).ok()?;
    Some((url.host_str()?.to_string(), url.port_or_known_default()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The places of a view of a page on a server's port 8443, which may
    /// also go to a map server.
    fn places() -> Places {
        let mut places = Places::of("https://server.example:8443/dashboard/index.html#!/clients").unwrap();
        places.allow("https://maps.example").unwrap();
        places
    }

    #[test]
    fn a_view_may_go_anywhere_at_the_place_its_page_is_at() {
        let places = places();
        for address in [
            "https://server.example:8443/",
            "https://server.example:8443/dashboard/index.html#!/clients",
            "https://server.example:8443/other/page?with=a&question",
            "https://SERVER.example:8443/names/are/not/case/sensitive",
        ] {
            assert!(places.has(address), "{address}");
        }
    }

    #[test]
    fn a_place_is_how_it_is_asked_its_name_and_its_port_and_all_three_must_match() {
        let places = places();
        for address in [
            "http://server.example:8443/",
            "https://server.example/",
            "https://server.example:8444/",
            "https://other.example:8443/",
            "https://server.example.other.example:8443/",
            "https://sub.server.example:8443/",
            "https://server.example.:8443/",
        ] {
            assert!(!places.has(address), "{address}");
        }
        // A port that is the usual one need not be written, and may be.
        assert!(places.has("https://maps.example/tiles/1/2/3.png"));
        assert!(places.has("https://maps.example:443/tiles/1/2/3.png"));
        assert!(!places.has("http://maps.example/tiles/1/2/3.png"));
    }

    #[test]
    fn an_address_written_to_look_like_a_place_is_taken_for_where_it_leads() {
        let places = places();
        for address in [
            // A name and a password for somewhere else...
            "https://server.example:8443@other.example/",
            "https://server.example%3A8443@other.example/",
            "https://server.example:8443:pass@other.example:8443/",
            // ...a slash a browser reads either way round...
            "https://other.example\\@server.example:8443/",
            // ...and the place as a part of an address that is elsewhere.
            "https://other.example/https://server.example:8443/",
            "https://other.example/?https://server.example:8443/",
            "https://other.example/#https://server.example:8443/",
        ] {
            assert!(!places.has(address), "{address}");
        }
        // The same, the other way about: these lead to the place.
        assert!(places.has("https://someone:secret@server.example:8443/"));
        assert!(places.has("https://server.example:8443\\@other.example/"));
    }

    #[test]
    fn what_is_not_fetched_from_a_server_is_nowhere_a_view_may_go() {
        let places = places();
        for address in [
            "file:///etc/passwd",
            "data:text/html,<p>anything</p>",
            "javascript:alert(1)",
            "ftp://server.example:8443/",
            "ws://server.example:8443/",
            "wss://server.example:8443/",
            "mailto:someone@server.example",
            "about:config",
            "//server.example:8443/",
            "/dashboard/index.html",
            "server.example:8443",
            "",
        ] {
            assert!(!places.has(address), "{address}");
        }
    }

    #[test]
    fn what_a_page_made_for_itself_is_of_its_place_and_an_empty_frame_is_of_any() {
        let places = places();
        assert!(places.has("blob:https://server.example:8443/0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0"));
        assert!(!places.has("blob:https://other.example/0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0"));
        assert!(!places.has("blob:null/0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0"));
        assert!(places.has("about:blank"));
        assert!(places.has("about:srcdoc"));
    }

    #[test]
    fn a_page_is_at_a_place_or_it_is_not_a_page() {
        for page in [
            "",
            "server.example",
            "server.example:8443/index.html",
            "file:///usr/share/doc/index.html",
            "about:blank",
            "https://",
            "https://someone:secret@server.example/",
            "ftp://server.example/",
        ] {
            let why = Places::of(page).unwrap_err();
            assert!(why.contains("is not the address of a page"), "{page}: {why}");
        }
        assert!(Places::of("http://192.168.1.20/").unwrap().has("http://192.168.1.20/status"));
        assert!(Places::of("https://[fd00::20]:8443/").unwrap().has("https://[fd00::20]:8443/status"));
    }

    #[test]
    fn a_place_is_allowed_as_it_is_written_and_only_if_it_is_only_a_place() {
        let mut places = Places::of("https://server.example/").unwrap();
        for place in [
            "",
            "maps.example",
            "https://maps.example/tiles/",
            "https://maps.example/?key=1",
            "https://maps.example/#top",
            "https://someone@maps.example",
            "file:///srv/maps",
            "*",
            "https://*.example",
        ] {
            let why = places.allow(place).unwrap_err();
            assert!(why.contains("is not a place"), "{place}: {why}");
        }
        assert!(!places.has("https://maps.example/"));

        places.allow("https://maps.example/").unwrap();
        places.allow("http://plain.example:8080").unwrap();
        assert!(places.has("https://maps.example/tiles/1.png"));
        assert!(places.has("http://plain.example:8080/"));
        // Allowed twice is allowed, and no more.
        let once = places.clone();
        places.allow("https://MAPS.example:443").unwrap();
        assert_eq!(places, once);
    }

    #[test]
    fn a_certificate_is_for_the_places_that_are_asked_over_tls_and_no_others() {
        let mut places = places();
        places.allow("http://plain.example:8080").unwrap();
        places.allow("https://[fd00::20]:8443").unwrap();
        assert!(places.has_secure("server.example", 8443));
        assert!(places.has_secure("SERVER.example", 8443));
        assert!(places.has_secure("maps.example", 443));
        assert!(places.has_secure("fd00::20", 8443));
        assert!(places.has_secure("[fd00::20]", 8443));
        // Not on another port, not another server, and not one asked in the clear.
        assert!(!places.has_secure("server.example", 443));
        assert!(!places.has_secure("other.example", 8443));
        assert!(!places.has_secure("plain.example", 8080));
        assert!(!places.has_secure("", 8443));
    }

    #[test]
    fn the_server_an_address_is_at_is_its_name_and_its_port() {
        assert_eq!(server("https://server.example:8443/a/b?c#d"), Some(("server.example".to_string(), 8443)));
        assert_eq!(server("https://Server.Example/"), Some(("server.example".to_string(), 443)));
        assert_eq!(server("http://192.168.1.20/"), Some(("192.168.1.20".to_string(), 80)));
        assert_eq!(server("about:blank"), None);
        assert_eq!(server("not an address"), None);
    }
}
