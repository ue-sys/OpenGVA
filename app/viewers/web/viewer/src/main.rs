//! The web viewer: one web page in a window with nothing round it.
//!
//! ```text
//! opengva-web-viewer NAME
//! ```
//!
//! shows the page that `/etc/opengva/web.d/NAME.conf` names (`web_view::View`
//! is what such a file says). There is no address bar and no menu, nothing is
//! opened in another window and nothing is downloaded, and the view goes
//! nowhere but the places its configuration allows. The HMI can be given the
//! window for a functional area's page, by the name the configuration gives
//! it; what the page is, is no business of the HMI's, and what a functional
//! area is, is none of this program's.
//!
//! The page is WebKit's to fetch and to draw. What is done here is what a
//! browser leaves to whoever is sitting at it:
//!
//! - **Who this machine is.** A server may ask for a certificate. WebKit
//!   passes the question on (the web view's `authenticate` signal), and it is
//!   answered with the certificate the configuration names - for the places
//!   the view may go, and for no other server that asks.
//! - **Whether the server is the right one.** WebKit checks a server's
//!   certificate and does not load a page from a server that does not pass
//!   (`load-failed-with-tls-errors`). That check is never switched off; what
//!   differs is what it is a check against. A view that names no authority
//!   is held to the authorities the system knows. A view that names one is
//!   held to that one and to no other: this program and the processes
//!   WebKit starts for it are told, before any of them has looked, that the
//!   one in that file is all the authorities there are (see `only`). Nothing
//!   else on the machine comes to trust the authority by it, and nothing
//!   the system trusts is trusted by the view.
//! - **What to do when the page cannot be had.** The server is not there, or
//!   refuses, or the certificate files are not there yet: the window says so,
//!   white on black, and the page is tried again - a second later, then two,
//!   up to a quarter of a minute. The program does not end for any of it.
//!
//! Nothing is kept from one start to the next: no cookies, no cache, nothing
//! a page stores.
//!
//! The line on stderr that starts `OPENGVA ` is a marker, as the HMI's are:
//! what starts the image's services copies it to the console. It is printed
//! when the page has first loaded. "uptime" is seconds since the kernel
//! started.

use gtk::prelude::*;
use gtk::{gdk, gio, glib, pango};
use std::cell::{Cell, RefCell};
use std::io::{self, Write};
use std::path::Path;
use std::rc::Rc;
use std::{env, fmt, fs, process};
use web_view::{Backoff, View};
use webkit2gtk::{
    AuthenticationRequest, AuthenticationRequestExt, AuthenticationScheme, ColorChooserRequestExt, Credential,
    CredentialPersistence, DownloadExt, FileChooserRequestExt, LoadEvent, NavigationPolicyDecision,
    NavigationPolicyDecisionExt, NetworkError, PermissionRequestExt, PolicyDecisionExt, PolicyDecisionType, PolicyError,
    ResponsePolicyDecision, ResponsePolicyDecisionExt, ScriptDialog, ScriptDialogType, SettingsExt, TLSErrorsPolicy,
    URIRequestExt, URIResponseExt, WebContext, WebContextExt, WebView, WebViewExt, WebsiteDataManagerExt,
};

/// Where the views are configured: one `.conf` file each, as [`View::parse`]
/// reads them.
const VIEWS_DIR: &str = "/etc/opengva/web.d";

/// What the window has that is not the page: white on black, in the
/// monospace face the HMI is set in.
const STYLE: &str = "
window, stack, box, label { background-color: #000000; color: #ffffff; }
label { font-family: monospace; font-size: 16px; }
button { min-width: 160px; min-height: 64px; margin: 12px; padding: 0; background-image: none;
         background-color: #000000; color: #ffffff; border: 2px solid #ffffff; border-radius: 0;
         box-shadow: none; text-shadow: none; -gtk-icon-shadow: none; }
button:active { background-color: #ffffff; color: #000000; }
button:active label { background-color: #ffffff; color: #000000; }
";

/// The three things the window can be showing, by the names its stack knows
/// them by.
mod showing {
    /// What is to be said in the page's place.
    pub const WORDS: &str = "words";
    pub const PAGE: &str = "page";
    /// Something the page has asked whoever is at the screen.
    pub const QUESTION: &str = "question";
}

struct Viewer {
    /// Which view this is: the name of its configuration.
    name: String,
    view: View,
    stack: gtk::Stack,
    words: gtk::Label,
    question: gtk::Label,
    /// The button that says no to a question: there only for one that can be
    /// said no to.
    decline: gtk::Button,
    web: WebView,
    /// Who this machine is, as the files the configuration names had it when
    /// the page was last tried.
    identity: RefCell<Option<gio::TlsCertificate>>,
    /// The authority the view names, as its file had it when the page was
    /// first tried. The processes that fetch pages read it once, then: if
    /// the file has another since, they do not know it.
    authority: RefCell<Option<Vec<u8>>>,
    backoff: RefCell<Backoff>,
    /// Whether the view is still trying to get its page, the first time or
    /// after losing it: until it has, a server's refusal is a reason to try
    /// again and not a page to show.
    trying: Cell<bool>,
    /// Whether the load in hand has failed: its end is then not a page's.
    failed: Cell<bool>,
    /// Why this program stopped the load in hand, if it did.
    stopped: RefCell<Option<String>>,
    /// Whether another try is already on its way.
    waiting: Cell<bool>,
    /// The question a page is waiting for an answer to.
    asked: RefCell<Option<ScriptDialog>>,
    /// What has been written to the log since a page last loaded: the same
    /// thing is said once, however long it stays so and whatever is said
    /// between.
    noted: RefCell<Vec<String>>,
    /// Whether the marker has been printed.
    marked: Cell<bool>,
}

impl Viewer {
    /// Write something to the log, unless it has been written already.
    fn note(&self, what: &str) {
        let mut noted = self.noted.borrow_mut();
        if noted.iter().any(|said| said == what) {
            return;
        }
        // A page that finds a great many things to be told no about is not
        // remembered for ever.
        if noted.len() >= 32 {
            noted.clear();
        }
        noted.push(what.to_string());
        log(format_args!("opengva-web-viewer {}: {}", self.name, what.replace('\n', " ")));
    }

    /// Put words in the page's place.
    fn say(&self, what: &str) {
        self.words.set_text(what);
        self.stack.set_visible_child_name(showing::WORDS);
    }

    /// Read what the configuration names of TLS, afresh: a certificate that
    /// was not there may be there now, and one that was may have been
    /// replaced. Says what is the matter, if anything is.
    fn credentials(&self) -> Result<(), String> {
        if let Some(identity) = &self.view.identity {
            for file in [&identity.certificate, &identity.key] {
                if !file.exists() {
                    return Err(format!("This machine's certificate is not here yet.\n{} is missing.", file.display()));
                }
            }
            let certificate = gio::TlsCertificate::from_files(&identity.certificate, &identity.key).map_err(|why| {
                let file = identity.certificate.display();
                format!("This machine's certificate cannot be used.\n{file}: {}", why.message())
            })?;
            self.identity.replace(Some(certificate));
        }
        if let Some(file) = &self.view.authority {
            let now = fs::read(file).map_err(|why| match why.kind() {
                io::ErrorKind::NotFound => {
                    format!("The server's authority is not here yet.\n{} is missing.", file.display())
                }
                _ => format!("The server's authority cannot be read.\n{}: {why}", file.display()),
            })?;
            gio::TlsCertificate::from_file(file).map_err(|why| {
                format!("The server's authority cannot be used.\n{}: {}", file.display(), why.message())
            })?;
            // Held to it, or not let near a server at all: an image whose
            // TLS could not be told would hold the view to the system's
            // authorities instead, and say nothing of it.
            if !told() {
                return Err("The view names an authority, and this image's TLS cannot be held to one.\n\
                            No server is asked for anything."
                    .to_string());
            }
            let mut known = self.authority.borrow_mut();
            match known.as_ref() {
                // The file is another's now. This program can read it again;
                // the processes that fetch pages for it have read it already
                // and will not. Ended, the program is started again by
                // whatever keeps it, and all of them with it.
                Some(then) if *then != now => {
                    log(format_args!(
                        "opengva-web-viewer {}: the server's authority has changed: starting again, to know the server by it",
                        self.name
                    ));
                    process::exit(0);
                }
                Some(_) => {}
                None => *known = Some(now),
            }
        }
        Ok(())
    }

    /// Try for the page, from the start.
    fn load(self: &Rc<Self>) {
        self.waiting.set(false);
        if let Err(why) = self.credentials() {
            return self.lost(&why);
        }
        self.go(&self.view.url);
    }

    /// Go to an address - once WebKit has done with whatever it is in the
    /// middle of telling this program, which may be the end of the load
    /// this one replaces.
    fn go(self: &Rc<Self>, address: &str) {
        let (viewer, address) = (self.clone(), address.to_string());
        glib::idle_add_local(move || {
            viewer.failed.set(false);
            viewer.stopped.take();
            viewer.web.load_uri(&address);
            glib::ControlFlow::Break
        });
    }

    /// The page cannot be had, for now: say why, and try again in a while.
    fn lost(self: &Rc<Self>, why: &str) {
        self.failed.set(true);
        self.trying.set(true);
        self.answer_question(false);
        self.note(why);
        if self.waiting.replace(true) {
            return;
        }
        let wait = self.backoff.borrow_mut().wait();
        self.say(&format!("{}\n\n{why}\n\nTrying again in {} s.", self.view.url, wait.as_secs()));
        let viewer = self.clone();
        glib::timeout_add_local(wait, move || {
            viewer.load();
            glib::ControlFlow::Break
        });
    }

    /// A load has come to its end. If nothing went wrong on the way, the
    /// page is there.
    fn finished(&self) {
        if self.failed.get() {
            return;
        }
        self.backoff.borrow_mut().reset();
        if self.asked.borrow().is_none() {
            self.show_page();
        }
        // Said when the page is had, the first time or again: not for every
        // page gone to from it.
        if self.trying.replace(false) {
            self.noted.borrow_mut().clear();
            self.note(&format!("showing {}", self.web.uri().unwrap_or_default()));
        }
        if !self.marked.replace(true) {
            marker(format_args!("web {} loaded", self.name));
        }
    }

    /// Put a question a page has asked to whoever is at the screen, on the
    /// window: there is no other window for it to be asked in.
    fn ask(&self, dialog: &ScriptDialog) {
        self.answer_question(false);
        self.question.set_text(dialog.message().as_deref().unwrap_or_default());
        self.decline.set_visible(dialog.dialog_type() == ScriptDialogType::Confirm);
        self.asked.replace(Some(dialog.clone()));
        self.stack.set_visible_child_name(showing::QUESTION);
    }

    /// Give the page its answer, if it is waiting for one, and the window
    /// back.
    fn answer_question(&self, yes: bool) {
        let Some(dialog) = self.asked.take() else { return };
        if dialog.dialog_type() == ScriptDialogType::Confirm {
            dialog.confirm_set_confirmed(yes);
        }
        dialog.close();
        if !self.failed.get() {
            self.show_page();
        }
    }

    /// Put the page on the window, and have what is typed at the window go
    /// to it. The HMI's on-screen keyboard types into this window when the
    /// crew has it up; a key that found nothing of the window's to go to
    /// would be spent on finding the page, and the first Tab pressed to
    /// reach a field of the page would reach none.
    fn show_page(&self) {
        self.stack.set_visible_child_name(showing::PAGE);
        self.web.grab_focus();
    }
}

/// What is the matter with a certificate, in words - where there is more to
/// say of it than that nobody known vouches for it, which is for whoever
/// asks to say.
fn faults(flags: gio::TlsCertificateFlags) -> Option<String> {
    use gio::TlsCertificateFlags as Fault;
    let said: Vec<&str> = [
        (Fault::BAD_IDENTITY, "is in another server's name"),
        (Fault::NOT_ACTIVATED, "is not yet in date"),
        (Fault::EXPIRED, "is out of date"),
        (Fault::REVOKED, "has been withdrawn"),
        (Fault::INSECURE, "is too weakly made to be trusted"),
    ]
    .into_iter()
    .filter_map(|(fault, words)| flags.contains(fault).then_some(words))
    .collect();
    (!said.is_empty()).then(|| said.join(", and "))
}

/// Where there are no certificates to be found.
const NOWHERE: &str = "/nonexistent";

/// Have this program, and every process WebKit starts for it, know one
/// authority and no other: the one in `file`.
///
/// What a program trusts is its TLS library's to say. Of the two that GLib
/// can be using, OpenSSL's is the one that can be told from outside: where
/// the certificates it trusts are, is in its environment. So the view is put
/// on OpenSSL's, with that file for what it trusts and nowhere to look for
/// more. It is done before there is a display or a web view, because the
/// processes that fetch pages start with whatever this one's environment is
/// then - and they read the file when they first need to, which is not
/// before this program has seen that it is there.
fn only(file: &Path) {
    // Nothing else is running in this program yet, to be reading its
    // environment while it changes.
    unsafe {
        env::set_var("GIO_USE_TLS", "openssl");
        env::set_var("SSL_CERT_FILE", file);
        env::set_var("SSL_CERT_DIR", NOWHERE);
    }
}

/// Whether this program's TLS is the one that [`only`] can tell. Asking for
/// it does not make it so: an image that has not got it uses what it has.
fn told() -> bool {
    gio::TlsBackend::default().type_().name().contains("Openssl")
}

/// Hand a request its answer. The WebKit crate has the signal that asks and
/// the credential to answer with, but not the call that gives the one to the
/// other: that is made here, on WebKit's own function.
fn answer(request: &AuthenticationRequest, credential: &Credential) {
    use glib::translate::{ToGlibPtr, mut_override};
    // Both are WebKit's objects, alive for the length of the call; the
    // function copies what it wants of the credential and keeps neither.
    unsafe {
        webkit2gtk::ffi::webkit_authentication_request_authenticate(
            request.to_glib_none().0,
            mut_override(credential.to_glib_none().0),
        );
    }
}

/// Seconds since the kernel started. `None` where there is no /proc.
fn uptime() -> Option<f64> {
    fs::read_to_string("/proc/uptime").ok()?.split_whitespace().next()?.parse().ok()
}

/// Write a line to stderr, whole. The log is one that WebKit's own processes
/// write to as well, when they like, and a line put together a piece at a
/// time comes out with theirs in the middle of it.
fn log(line: impl fmt::Display) {
    let _ = io::stderr().write_all(format!("{line}\n").as_bytes());
}

fn marker(what: impl fmt::Display) {
    log(format_args!("OPENGVA {what} uptime={:.2}", uptime().unwrap_or(-1.0)));
}

/// The window, bare: black, with a place for words in the middle of it.
fn window() -> (gtk::Window, gtk::Stack, gtk::Label) {
    let style = gtk::CssProvider::new();
    // The style is this file's own, and parses.
    let _ = style.load_from_data(STYLE.as_bytes());
    if let Some(screen) = gdk::Screen::default() {
        gtk::StyleContext::add_provider_for_screen(&screen, &style, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_decorated(false);
    window.set_default_size(1024, 768);
    // A window that has gone has nothing to show a page in: end, and be
    // started again by whatever keeps this program.
    window.connect_destroy(|_| gtk::main_quit());

    let (stack, label) = (gtk::Stack::new(), words());
    stack.add_named(&label, showing::WORDS);
    window.add(&stack);
    (window, stack, label)
}

/// A place for words: in the middle, as many lines as they take.
fn words() -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_line_wrap(true);
    label.set_line_wrap_mode(pango::WrapMode::WordChar);
    label.set_max_width_chars(64);
    label.set_justify(gtk::Justification::Center);
    label.set_halign(gtk::Align::Center);
    label.set_valign(gtk::Align::Center);
    label.set_margin(24);
    label
}

fn main() {
    let mut arguments = env::args().skip(1);
    let name = match (arguments.next(), arguments.next()) {
        (Some(name), None) if !name.is_empty() && !name.contains('/') => name,
        _ => {
            log(format_args!("usage: opengva-web-viewer NAME    (the view that {VIEWS_DIR}/NAME.conf describes)"));
            process::exit(2);
        }
    };
    let file = Path::new(VIEWS_DIR).join(format!("{name}.conf"));
    let view = fs::read_to_string(&file).map_err(|why| why.to_string()).and_then(|text| View::parse(&text));

    // An authority the view names is the only one it is to know: said now,
    // before anything that would look is started.
    if let Ok(View { authority: Some(file), .. }) = &view {
        only(file);
    }

    // A window is known to the display server by its program's name, and to
    // the HMI by what the display server knows it by: the name is given
    // before there is a window to have it. With no configuration to say what
    // it is, it is one that says which view could not be shown.
    let known_as = view.as_ref().map_or_else(|_| format!("opengva-web-{name}"), |view| view.window.clone());
    glib::set_prgname(Some(known_as.as_str()));
    if let Err(why) = gtk::init() {
        log(format_args!("opengva-web-viewer {name}: there is no display to show a window on: {why}"));
        process::exit(1);
    }
    let (window, stack, words_label) = window();

    // A configuration that cannot be used is said on the window, where
    // whoever wrote it will be looking, and the program stays: ended, it
    // would only be started again to say the same.
    let view = match view {
        Ok(view) => view,
        Err(why) => {
            log(format_args!("opengva-web-viewer {name}: {}: {why}", file.display()));
            words_label.set_text(&format!("{}\n\n{why}", file.display()));
            window.show_all();
            gtk::main();
            return;
        }
    };

    // WebKit, keeping nothing: what a page stores is gone when this ends.
    // A certificate that does not pass stops the load - said here, though it
    // is what WebKit does unless told otherwise - and no credential is kept
    // by the system's keyring, or looked for in it.
    let context = WebContext::new_ephemeral();
    if let Some(data) = context.website_data_manager() {
        data.set_tls_errors_policy(TLSErrorsPolicy::Fail);
        data.set_persistent_credential_storage_enabled(false);
    }
    // Nothing is downloaded: there is nowhere for it to go and nobody to ask.
    context.connect_download_started(|_, download| download.cancel());

    let web = WebView::with_context(&context);
    web.set_background_color(&gdk::RGBA::BLACK);
    if let Some(settings) = WebViewExt::settings(&web) {
        settings.set_javascript_can_open_windows_automatically(false);
        settings.set_enable_developer_extras(false);
        settings.set_enable_back_forward_navigation_gestures(false);
    }
    stack.add_named(&web, showing::PAGE);

    // A question from a page: its words, and under them yes and no.
    let question = words();
    let (accept, decline) = (gtk::Button::with_label("OK"), gtk::Button::with_label("Cancel"));
    let answers = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    answers.set_halign(gtk::Align::Center);
    answers.add(&accept);
    answers.add(&decline);
    let asking = gtk::Box::new(gtk::Orientation::Vertical, 0);
    asking.set_valign(gtk::Align::Center);
    asking.add(&question);
    asking.add(&answers);
    stack.add_named(&asking, showing::QUESTION);

    let viewer = Rc::new(Viewer {
        name,
        view,
        stack,
        words: words_label,
        question,
        decline: decline.clone(),
        web: web.clone(),
        identity: RefCell::new(None),
        authority: RefCell::new(None),
        backoff: RefCell::new(Backoff::default()),
        trying: Cell::new(true),
        failed: Cell::new(false),
        stopped: RefCell::new(None),
        waiting: Cell::new(false),
        asked: RefCell::new(None),
        noted: RefCell::new(Vec::new()),
        marked: Cell::new(false),
    });

    let asked = viewer.clone();
    accept.connect_clicked(move |_| asked.answer_question(true));
    let asked = viewer.clone();
    decline.connect_clicked(move |_| asked.answer_question(false));

    // Where a load has got to. What a page is made of is on the screen as
    // it arrives; the page is there when the load ends with nothing having
    // gone wrong.
    let loading = viewer.clone();
    web.connect_load_changed(move |_, event| match event {
        LoadEvent::Started => {
            loading.failed.set(false);
            loading.answer_question(false);
        }
        LoadEvent::Committed if !loading.failed.get() => loading.show_page(),
        LoadEvent::Finished => loading.finished(),
        _ => {}
    });

    // A load that failed. One that this program stopped for a reason has
    // the reason here. One that was put aside for another, or that came to
    // nothing because of what this program will not do - go somewhere, take
    // a download - is no failure at all: what was on the screen still is.
    let loading = viewer.clone();
    web.connect_load_failed(move |_, _, _, why| {
        if let Some(stopped) = loading.stopped.take() {
            loading.lost(&stopped);
        } else if !why.matches(NetworkError::Cancelled) && why.kind::<PolicyError>().is_none() {
            loading.lost(why.message());
        }
        // Said, here: WebKit has no page of its own to put up about it.
        true
    });

    // A server's certificate that WebKit will not take: by the authority
    // the view names, if it names one, and by the system's if it does not.
    let loading = viewer.clone();
    web.connect_load_failed_with_tls_errors(move |_, address, _, refused| {
        let server = web_view::server(address).map_or_else(|| address.to_string(), |(host, _)| host);
        let mut why = if loading.view.authority.is_some() {
            format!("The authority does not vouch for {server}.")
        } else {
            format!("{server} is not a server this machine knows.")
        };
        if let Some(faults) = faults(refused) {
            why.push_str(&format!("\nIts certificate {faults}."));
        }
        loading.lost(&why);
        true
    });

    // A server asking who is calling. This machine's certificate is for the
    // places the view may go; any other server that asks is not told. And a
    // name and a password would have to be typed, on a keyboard there is
    // not: whatever asks for those goes without.
    let asked = viewer.clone();
    web.connect_authenticate(move |_, request| {
        if request.scheme() != AuthenticationScheme::ClientCertificateRequested {
            request.cancel();
            return true;
        }
        let host = request.host().unwrap_or_default();
        let port = u16::try_from(request.port()).unwrap_or_default();
        let identity = asked.identity.borrow();
        let told = identity.as_ref().filter(|_| !request.is_for_proxy() && asked.view.places.has_secure(&host, port));
        // Kept for as long as this program runs: whatever else the page
        // fetches from that server, and the sockets it opens to it, are let
        // in by the same certificate without the question being put again.
        let kept = if told.is_some() { CredentialPersistence::ForSession } else { CredentialPersistence::None };
        if told.is_none() {
            asked.note(&format!("{host}:{port} asks who this machine is, and is not told"));
        }
        answer(request, &Credential::for_certificate(told, kept));
        true
    });

    // Where the view may go, and what it does with what it is sent.
    let going = viewer.clone();
    web.connect_decide_policy(move |_, decision, kind| match kind {
        PolicyDecisionType::NavigationAction | PolicyDecisionType::NewWindowAction => {
            let address = decision
                .downcast_ref::<NavigationPolicyDecision>()
                .and_then(|decision| decision.navigation_action())
                .and_then(|action| action.request())
                .and_then(|request| request.uri())
                .unwrap_or_default();
            if !going.view.places.has(&address) {
                let why = format!("{address} is not a place this view may go");
                decision.ignore();
                // Sent there on the way to its page, the view has not got
                // its page; sent there from its page, it stays where it is.
                if going.trying.get() { going.lost(&why) } else { going.note(&why) }
                true
            } else if kind == PolicyDecisionType::NewWindowAction {
                // There is no other window: what would open in one, opens
                // in this.
                decision.ignore();
                going.go(&address);
                true
            } else {
                false
            }
        }
        PolicyDecisionType::Response => {
            let Some(decision) = decision.downcast_ref::<ResponsePolicyDecision>() else { return false };
            let response = decision.response();
            let status = response.as_ref().map_or(0, |response| response.status_code());
            let address = response.and_then(|response| response.uri()).unwrap_or_default();
            let server = web_view::server(&address).map_or_else(|| "The server".to_string(), |(host, _)| host);
            // While the view is trying for its page, what comes in the
            // page's place is a reason to try again.
            let page = decision.is_main_frame_main_resource();
            let wanted = going.trying.get() && page;
            if wanted && status >= 400 {
                // The server is there and will not give the page - not yet,
                // or not to this machine. Its own page saying so would stay
                // on the screen for ever: this program's is tried again.
                going.stopped.replace(Some(format!("{server} answered with an error: {status}.")));
                decision.ignore();
                true
            } else if page && status >= 400 {
                // From its page, the view is not taken to an error either:
                // there is nothing on this window to come back from one
                // with. It stays where it is.
                going.note(&format!("{server} answers {address} with an error, {status}: the view stays where it is"));
                decision.ignore();
                true
            } else if !decision.is_mime_type_supported() {
                // What cannot be shown would be offered as a download.
                if wanted {
                    going.stopped.replace(Some(format!("What {server} sent is not a page that can be shown.")));
                }
                decision.ignore();
                true
            } else {
                false
            }
        }
        _ => false,
    });

    // What a browser would open another window for, or put up a window of
    // its own about: none of it is done, but a page's questions, which are
    // asked on this window.
    web.connect_create(|_, _| None);
    web.connect_context_menu(|_, _, _, _| true);
    web.connect_print(|_, _| true);
    web.connect_show_notification(|_, _| true);
    web.connect_enter_fullscreen(|_| true);
    web.connect_permission_request(|_, request| {
        request.deny();
        true
    });
    web.connect_run_file_chooser(|_, request| {
        request.cancel();
        true
    });
    web.connect_run_color_chooser(|_, request| {
        request.cancel();
        true
    });
    let asking = viewer.clone();
    web.connect_script_dialog(move |_, dialog| {
        match dialog.dialog_type() {
            ScriptDialogType::Alert | ScriptDialogType::Confirm => asking.ask(dialog),
            // Leaving a page is not the page's to stop.
            ScriptDialogType::BeforeUnloadConfirm => dialog.confirm_set_confirmed(true),
            // An answer would have to be typed: the page is told there is none.
            _ => {}
        }
        true
    });

    // The page's own process has ended under it - it failed, or the system
    // was short of memory. The next load starts another.
    let loading = viewer.clone();
    web.connect_web_process_terminated(move |_, _| loading.lost("The page stopped working."));

    window.show_all();
    viewer.say(&format!("{}\n\nFetching the page.", viewer.view.url));
    viewer.load();
    gtk::main();
}
