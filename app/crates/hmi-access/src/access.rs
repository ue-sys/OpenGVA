//! Who is logged in, and the PIN being asked for.
//!
//! A PIN page is a dialog: it is over the page the crew were on, F13 cancels
//! it and F20 accepts what has been entered. Its
//! digits are on the pad, to be touched, and on the buttons down the sides -
//! what touch can do, the bezel can do too. Whether the PIN is
//! right is not known here: it is handed over to be checked ([`Check`]), and
//! what comes back is taken as it is told ([`Answer`]).

use crate::config::{Config, Require};
use hmi_entry::{Keyboard, KeyboardView, Layout, Stroke, Takes};
use serde::Serialize;
use std::fmt;

/// The fewest digits Enter will hand over to be checked. A slip of a finger
/// on Enter is then not a wrong PIN, counted against the account.
pub const FEWEST_DIGITS: usize = 4;

/// The most digits a PIN can have: how long the page's box is.
pub const MOST_DIGITS: usize = 12;

/// How long a PIN page waits for a button or a touch before it goes away,
/// and the crew are where they were: what was under it is not to stay
/// covered by a question nobody is answering.
pub const ASKING_MS: u64 = 60_000;

/// What the buttons do on a PIN page.
///
/// Down the sides are the digits, a press each: 1 to 6 on the left, and 7,
/// 8, 9 and 0 on the right, with Backspace under them on the button a form
/// has it on. F13 cancels and F20 enters, as in any dialog.
/// F17 and F18 choose whose PIN it is, where more than one account would do.
/// F14, F15, F16 and F19 are not the page's at all ([`Pressed::Passed`]).
pub mod button {
    /// F1 to F9 have the digits 1 to 9, and this has 0.
    pub const ZERO: u8 = 10;
    pub const BACKSPACE: u8 = 11;
    pub const CANCEL: u8 = 13;
    pub const ALARMS: u8 = 14;
    pub const THREATS: u8 = 15;
    pub const ACKNOWLEDGE: u8 = 16;
    pub const UP: u8 = 17;
    pub const DOWN: u8 = 18;
    pub const LABELS: u8 = 19;
    pub const ENTER: u8 = 20;

    /// The digit F`n` has on it, if it has one.
    pub fn digit(n: u8) -> Option<char> {
        match n {
            1..=9 => Some(char::from(b'0' + n)),
            ZERO => Some('0'),
            _ => None,
        }
    }
}

/// What is written on the buttons that have digits, F1 first.
const DIGITS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];

/// Who is logged in.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Session {
    /// Nobody: how the crew station comes up, and what it goes back to.
    #[default]
    Nobody,
    /// The account whose PIN was last given, and the roles it holds.
    Somebody { account: String, roles: Vec<String> },
}

impl Session {
    pub fn account(&self) -> Option<&str> {
        match self {
            Session::Nobody => None,
            Session::Somebody { account, .. } => Some(account),
        }
    }

    /// The roles held: none, with nobody logged in.
    pub fn roles(&self) -> &[String] {
        match self {
            Session::Nobody => &[],
            Session::Somebody { roles, .. } => roles,
        }
    }
}

/// A PIN as it was entered, on its way to be checked. It says nothing of
/// itself but how long it is, and is wiped when it is done with.
pub struct Pin(String);

impl Pin {
    fn new() -> Self {
        // Room for all it can hold, so that a digit added never moves the
        // others and leaves them behind.
        Pin(String::with_capacity(MOST_DIGITS))
    }

    /// The digits: for whatever checks them, and nothing else.
    pub fn reveal(&self) -> &str {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn wipe(&mut self) {
        let mut digits = std::mem::take(&mut self.0).into_bytes();
        digits.fill(0);
        // The wiping is for whoever reads the memory later, which the
        // compiler does not count as a use of it.
        std::hint::black_box(&digits);
    }
}

impl fmt::Debug for Pin {
    fn fmt(&self, out: &mut fmt::Formatter) -> fmt::Result {
        write!(out, "Pin of {} digits", self.len())
    }
}

impl Drop for Pin {
    fn drop(&mut self) {
        self.wipe();
    }
}

/// A PIN to be checked: whether it is the account's. The answer comes back
/// with the same `serial` ([`Access::answer`]).
#[derive(Debug)]
pub struct Check {
    pub serial: u32,
    pub account: String,
    pub pin: Pin,
}

/// What the verifier said of a PIN.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// It is the account's.
    Accepted,
    /// It is not, or the account is not to be tried just now. `why` is the
    /// verifier's own word for it, and `wait_ms` how long it will be before
    /// it hears another PIN for the account, if it said.
    Refused { why: String, wait_ms: Option<u64> },
    /// There was nobody to check it, and this is why.
    Unavailable(String),
}

/// What became of something the crew asked for.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked<W> {
    /// It needs nothing that whoever is logged in has not: here it is back,
    /// to be shown or done.
    Allowed(W),
    /// It needs a role nobody logged in has: a PIN page is up for it.
    Pin,
    /// It needs a role that no account at this crew station holds: it is
    /// not to be had here.
    Never,
}

/// What a button did, pressed while a PIN was being asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pressed {
    /// Nothing here: it is not the PIN page's, and does what it does
    /// everywhere. Always so with no PIN page up, and always so for F14,
    /// F15, F16 and F19: the Alarms and Threats pages, the acknowledging of
    /// an alert and the hiding of labels wait on nobody's PIN.
    Passed,
    /// It was the page's, whether or not it had anything to do.
    Taken,
    /// It was F13: the page has gone, and the crew are where they were.
    Cancelled,
}

/// What a finger has come down on, of what a PIN page shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Touched {
    /// A key of the pad, by its place among the keys as the page lists them.
    Key(usize),
    /// An account, by its number ([`AccountRow::number`]).
    Account(usize),
}

/// How a PIN page stands, for it to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Standing {
    /// A PIN can be entered.
    Entering,
    /// The PIN has been handed over, and the answer is awaited.
    Checking,
    /// The last PIN was refused, and another can be entered.
    Refused,
    /// The verifier will hear no PIN for this account until a wait is over.
    Waiting,
    /// The last PIN could not be checked: there was nobody to check it.
    Unchecked,
}

/// A PIN page as it is to be drawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PinPage {
    /// What the crew asked for, as they know it.
    pub wanted: String,
    /// The account whose PIN is being asked for.
    pub account: String,
    /// The accounts whose PIN would do, where there is more than one to
    /// choose between; none, where there is one.
    pub accounts: Vec<AccountRow>,
    /// How many digits have been entered. Not which: the page never has
    /// them (ISM-1597), and nor has anything else the HMI draws or prints.
    pub entered: usize,
    /// The most there can be: how long the box is.
    pub most: usize,
    pub standing: Standing,
    /// What the verifier said, the last time it refused or could not check.
    pub said: Option<String>,
    /// How many seconds it is until the verifier will hear another PIN.
    pub wait: Option<u64>,
    /// The pad the PIN is typed on: `hmi-entry`'s phone pad, with nothing
    /// highlighted - Enter does not select a key here, it enters the PIN.
    pub pad: KeyboardView,
}

/// An account whose PIN would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AccountRow {
    /// Counted from 1: what a touch on it is told of by
    /// ([`Touched::Account`]).
    pub number: usize,
    pub name: String,
    /// Whether it is the one whose PIN is being asked for.
    pub chosen: bool,
}

/// Something that happened, to be written down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// A PIN page came up.
    Asked,
    /// A PIN was the account's: the account is logged in.
    Accepted,
    /// A PIN was refused.
    Refused,
    /// A PIN could not be checked.
    Unchecked,
    /// The crew cancelled a PIN page, or went elsewhere.
    Cancelled,
    /// A PIN page went away of itself, nobody having answered it.
    Abandoned,
    /// Whoever was logged in logged out.
    LoggedOut,
    /// Whoever was logged in left the display alone for the idle time.
    Idle,
    /// Whoever was logged in gave way to another account's PIN.
    Displaced,
}

/// A line for the record: what happened, and to whom, over what. Never a
/// PIN, nor how long one was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Audit {
    pub event: Event,
    pub account: Option<String>,
    /// The roles the account holds, or the roles that were asked for.
    pub roles: Vec<String>,
    /// What was asked for.
    pub wanted: Option<String>,
    /// What the verifier said.
    pub said: Option<String>,
}

/// As a line of a log: the event, then what is known of it, each by name.
impl fmt::Display for Audit {
    fn fmt(&self, out: &mut fmt::Formatter) -> fmt::Result {
        out.write_str(match self.event {
            Event::Asked => "asked",
            Event::Accepted => "accepted",
            Event::Refused => "refused",
            Event::Unchecked => "unchecked",
            Event::Cancelled => "cancelled",
            Event::Abandoned => "abandoned",
            Event::LoggedOut => "logged-out",
            Event::Idle => "idle",
            Event::Displaced => "displaced",
        })?;
        if let Some(account) = &self.account {
            write!(out, " account={account}")?;
        }
        if !self.roles.is_empty() {
            write!(out, " roles={}", self.roles.join(","))?;
        }
        if let Some(wanted) = &self.wanted {
            write!(out, " for={wanted:?}")?;
        }
        if let Some(said) = &self.said {
            write!(out, " said={said:?}")?;
        }
        Ok(())
    }
}

/// Where a PIN page has got to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Entering,
    /// Handed over under this serial.
    Checking(u32),
    Refused(String),
    Unchecked(String),
}

/// A PIN being asked for.
#[derive(Debug)]
struct Request<W> {
    /// What the crew asked for, to be handed back if the PIN is right...
    wanted: W,
    /// ...and what they know it as.
    what: String,
    /// The accounts whose PIN would do, and which of them is being asked.
    accounts: Vec<String>,
    chosen: usize,
    pin: Pin,
    pad: Keyboard,
    state: State,
    /// When a button or a touch was last the page's.
    touched: u64,
}

impl<W> Request<W> {
    fn account(&self) -> &str {
        &self.accounts[self.chosen]
    }

    fn checking(&self) -> bool {
        matches!(self.state, State::Checking(_))
    }

    /// Ask for another account's PIN instead: what was entered for the last
    /// is not this one's.
    fn choose(&mut self, account: usize) {
        if account < self.accounts.len() && account != self.chosen && !self.checking() {
            self.chosen = account;
            self.pin = Pin::new();
            self.state = State::Entering;
        }
    }

    /// What the pad can take: digits while there is room, never a point.
    fn takes(&self, typing: bool) -> Takes {
        Takes {
            character: typing && self.pin.len() < MOST_DIGITS,
            point: false,
            backspace: typing && !self.pin.is_empty(),
        }
    }

    /// Take what a key of the pad typed.
    fn typed(&mut self, stroke: Option<Stroke>) {
        match stroke {
            Some(Stroke::Character(digit)) if digit.is_ascii_digit() => self.pin.0.push(digit),
            Some(Stroke::Backspace) => {
                self.pin.0.pop();
            }
            _ => return,
        }
        // What the verifier said of the last PIN is not said of this one.
        self.state = State::Entering;
    }
}

/// Who is logged in at the crew station, and the PIN being asked for, if one
/// is. `W` is whatever the crew can want that may need a role: it is kept
/// while its PIN is asked for and handed back when the PIN is right.
///
/// Time is milliseconds on a clock that only goes forward, as whoever owns
/// this is told it.
#[derive(Debug)]
pub struct Access<W> {
    config: Config,
    session: Session,
    /// When the crew last pressed or touched anything.
    active: u64,
    request: Option<Request<W>>,
    /// The accounts the verifier has said to wait for, each until when.
    waits: Vec<(String, u64)>,
    /// A PIN handed over by the page and not yet taken to be checked.
    check: Option<Check>,
    serial: u32,
    /// The time as last told.
    now: u64,
    audit: Vec<Audit>,
}

/// With no configuration: nobody logged in, and nothing that needs anybody.
impl<W> Default for Access<W> {
    fn default() -> Self {
        Access::new(Config::default())
    }
}

fn note(audit: &mut Vec<Audit>, event: Event, account: Option<&str>, roles: &[String], wanted: Option<&str>, said: Option<&str>) {
    audit.push(Audit {
        event,
        account: account.map(str::to_string),
        roles: roles.to_vec(),
        wanted: wanted.map(str::to_string),
        said: said.map(str::to_string),
    });
}

impl<W> Access<W> {
    /// Nobody is logged in to begin with.
    pub fn new(config: Config) -> Self {
        Access {
            config,
            session: Session::Nobody,
            active: 0,
            request: None,
            waits: Vec::new(),
            check: None,
            serial: 0,
            now: 0,
            audit: Vec::new(),
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Whether whoever is logged in - or nobody - may have what needs this.
    pub fn allows(&self, require: &Require) -> bool {
        require.met_by(self.session.roles())
    }

    /// Whether what needs this can be had at this crew station at all: it
    /// needs nothing, or some account holds a role that will do. What cannot
    /// is unavailable, whoever asks.
    pub fn possible(&self, require: &Require) -> bool {
        require.is_nothing() || !self.config.accounts(require).is_empty()
    }

    /// The crew want something that needs `require`: `what` is what they
    /// know it as. It is handed straight back if whoever is logged in may
    /// have it; otherwise a PIN page comes up for it - in place of any that
    /// was up for something else - unless no PIN could ever get it.
    pub fn ask(&mut self, wanted: W, what: &str, require: &Require, now: u64) -> Asked<W> {
        self.now = now;
        if self.allows(require) {
            return Asked::Allowed(wanted);
        }
        let accounts = self.config.accounts(require);
        if accounts.is_empty() {
            return Asked::Never;
        }
        self.close(Event::Cancelled);
        note(&mut self.audit, Event::Asked, None, require.roles(), Some(what), None);
        self.request = Some(Request {
            wanted,
            what: what.to_string(),
            accounts,
            chosen: 0,
            pin: Pin::new(),
            pad: Keyboard::new(Layout::Phone),
            state: State::Entering,
            touched: now,
        });
        Asked::Pin
    }

    /// Whether a PIN is being asked for: its page is the one to draw, and
    /// the buttons and touches are to be offered to it first.
    pub fn asking(&self) -> bool {
        self.request.is_some()
    }

    /// Until when the verifier is to be waited for, over the account being
    /// asked about, if `now` is before then.
    fn waiting(&self, now: u64) -> Option<u64> {
        let account = self.request.as_ref()?.account();
        self.waits.iter().find(|(held, until)| held == account && *until > now).map(|&(_, until)| until)
    }

    /// Whether a PIN can be typed at `now`: one is being asked for, it is
    /// not away being checked, and the verifier is not being waited for.
    fn typing(&self, now: u64) -> bool {
        self.request.as_ref().is_some_and(|request| !request.checking()) && self.waiting(now).is_none()
    }

    /// F`n` has been pressed.
    pub fn press(&mut self, n: u8, now: u64) -> Pressed {
        use button::*;
        self.now = now;
        if self.request.is_none() || matches!(n, ALARMS | THREATS | ACKNOWLEDGE | LABELS) {
            return Pressed::Passed;
        }
        if n == CANCEL {
            self.close(Event::Cancelled);
            return Pressed::Cancelled;
        }
        let typing = self.typing(now);
        let Some(request) = &mut self.request else { return Pressed::Passed };
        request.touched = now;
        // A key still shown as pressed, the letting go of its button having
        // gone astray, is let go now.
        request.pad.release();
        let takes = request.takes(typing);
        match n {
            UP => request.choose(request.chosen.saturating_sub(1)),
            DOWN => request.choose(request.chosen + 1),
            BACKSPACE => {
                let typed = request.pad.strike(Stroke::Backspace, takes);
                request.typed(typed);
            }
            ENTER if typing && request.pin.len() >= FEWEST_DIGITS => {
                self.serial = self.serial.wrapping_add(1);
                request.state = State::Checking(self.serial);
                let pin = std::mem::replace(&mut request.pin, Pin::new());
                self.check = Some(Check { serial: self.serial, account: request.account().to_string(), pin });
            }
            _ => {
                if let Some(digit) = digit(n) {
                    let typed = request.pad.strike(Stroke::Character(digit), takes);
                    request.typed(typed);
                }
            }
        }
        Pressed::Taken
    }

    /// F`n` has been let go: a key of the pad it was selecting is no longer
    /// lit.
    pub fn release(&mut self, n: u8) {
        if n <= button::BACKSPACE {
            self.lift();
        }
    }

    /// A finger has come down on a key of the pad, or on an account.
    pub fn touch(&mut self, touched: Touched, now: u64) {
        self.now = now;
        let typing = self.typing(now);
        let Some(request) = &mut self.request else { return };
        request.touched = now;
        match touched {
            Touched::Key(key) => {
                let typed = request.pad.touch(key, request.takes(typing));
                request.typed(typed);
            }
            Touched::Account(number) => request.choose(number.wrapping_sub(1)),
        }
    }

    /// The finger has gone.
    pub fn lift(&mut self) {
        if let Some(request) = &mut self.request {
            request.pad.release();
        }
    }

    /// The crew have gone elsewhere: no PIN is being asked for any more.
    pub fn cancel(&mut self) {
        self.close(Event::Cancelled);
    }

    /// Take the PIN page down, if one is up, and say why. A PIN handed over
    /// and not yet taken to be checked is not checked.
    fn close(&mut self, event: Event) {
        if let Some(request) = self.request.take() {
            self.check = None;
            note(&mut self.audit, event, None, &[], Some(&request.what), None);
        }
    }

    /// The PIN the crew have entered, for the verifier to be asked about:
    /// once, and then it is the verifier's business.
    pub fn take_check(&mut self) -> Option<Check> {
        self.check.take()
    }

    /// What the verifier said of the PIN handed over under `serial`. If it
    /// is the account's, the account is logged in - in place of whoever was
    /// - and what the crew asked for is handed back, to be shown or done.
    /// An answer to a page that has since gone is nobody's.
    pub fn answer(&mut self, serial: u32, answer: Answer, now: u64) -> Option<W> {
        self.now = now;
        let request = self.request.as_mut().filter(|request| request.state == State::Checking(serial))?;
        request.touched = now;
        let account = request.account().to_string();
        match answer {
            Answer::Accepted => {
                let request = self.request.take()?;
                if let Session::Somebody { account: was, roles } = &self.session {
                    if *was != account {
                        note(&mut self.audit, Event::Displaced, Some(was), roles, None, None);
                    }
                }
                let roles = self.config.roles_of(&account);
                note(&mut self.audit, Event::Accepted, Some(&account), &roles, Some(&request.what), None);
                self.session = Session::Somebody { account, roles };
                self.active = now;
                Some(request.wanted)
            }
            Answer::Refused { why, wait_ms } => {
                note(&mut self.audit, Event::Refused, Some(&account), &[], Some(&request.what), Some(&why));
                request.state = State::Refused(why);
                if let Some(wait) = wait_ms {
                    self.waits.retain(|(held, _)| *held != account);
                    self.waits.push((account, now.saturating_add(wait)));
                }
                None
            }
            Answer::Unavailable(why) => {
                note(&mut self.audit, Event::Unchecked, Some(&account), &[], Some(&request.what), Some(&why));
                request.state = State::Unchecked(why);
                None
            }
        }
    }

    /// Log out: nobody is logged in.
    pub fn leave(&mut self) {
        if let Session::Somebody { account, roles } = std::mem::take(&mut self.session) {
            note(&mut self.audit, Event::LoggedOut, Some(&account), &roles, None, None);
        }
    }

    /// The crew have pressed or touched something: the display is not being
    /// left alone.
    pub fn active(&mut self, now: u64) {
        self.now = now;
        self.active = now;
    }

    /// Time has passed. Returns whether anything has changed with it:
    /// whoever was logged in no longer is, the display having been left
    /// alone for the idle time; a PIN page nobody was answering has gone;
    /// or a wait for the verifier is a second shorter, or over.
    pub fn tick(&mut self, now: u64) -> bool {
        // The wait the page shows, in whole seconds, as it was last told
        // the time and as it is now.
        let shown = |access: &Self, at: u64| access.waiting(at).map(|until| (until - at).div_ceil(1000));
        let was = shown(self, self.now);
        let mut changed = was != shown(self, now);
        self.now = now;
        if now.saturating_sub(self.active) >= self.config.idle_ms() {
            if let Session::Somebody { account, roles } = std::mem::take(&mut self.session) {
                note(&mut self.audit, Event::Idle, Some(&account), &roles, None, None);
                changed = true;
            }
        }
        if self.request.as_ref().is_some_and(|request| !request.checking() && now.saturating_sub(request.touched) >= ASKING_MS) {
            self.close(Event::Abandoned);
            changed = true;
        }
        self.waits.retain(|(_, until)| *until > now);
        changed
    }

    /// When [`Access::tick`] is next wanted, if it is: when the idle time
    /// will be up, when a PIN page will have waited long enough, or when a
    /// wait for the verifier will be a second shorter.
    pub fn deadline(&self) -> Option<u64> {
        let idle = self.session.account().map(|_| self.active.saturating_add(self.config.idle_ms()));
        let asking =
            self.request.as_ref().filter(|request| !request.checking()).map(|request| request.touched + ASKING_MS);
        let counting = self.waiting(self.now).map(|until| until - (until - self.now - 1) / 1000 * 1000);
        [idle, asking, counting].into_iter().flatten().min()
    }

    /// The PIN page to draw at `now`, while a PIN is being asked for.
    pub fn page(&self, now: u64) -> Option<PinPage> {
        let request = self.request.as_ref()?;
        let waiting = self.waiting(now);
        let mut pad = request.pad.view(request.takes(self.typing(now)));
        pad.keys.iter_mut().for_each(|key| key.highlighted = false);
        let several = request.accounts.len() > 1;
        Some(PinPage {
            wanted: request.what.clone(),
            account: request.account().to_string(),
            accounts: (1..)
                .zip(&request.accounts)
                .filter(|_| several)
                .map(|(number, name)| AccountRow { number, name: name.clone(), chosen: number - 1 == request.chosen })
                .collect(),
            entered: request.pin.len(),
            most: MOST_DIGITS,
            standing: match &request.state {
                State::Checking(_) => Standing::Checking,
                _ if waiting.is_some() => Standing::Waiting,
                State::Refused(_) => Standing::Refused,
                State::Unchecked(_) => Standing::Unchecked,
                State::Entering => Standing::Entering,
            },
            said: match &request.state {
                State::Refused(said) | State::Unchecked(said) => Some(said.clone()),
                State::Entering | State::Checking(_) => None,
            },
            wait: waiting.map(|until| (until - now).div_ceil(1000)),
            pad,
        })
    }

    /// The labels of the buttons a PIN page gives something to do, at `now`:
    /// each with the number of its button, and whether it can do anything.
    /// One that cannot is shown as unavailable. None,
    /// with no PIN page up.
    pub fn labels(&self, now: u64) -> Vec<(u8, &'static str, bool)> {
        use button::*;
        let Some(request) = &self.request else { return Vec::new() };
        let takes = request.takes(self.typing(now));
        let choosing = request.accounts.len() > 1 && !request.checking();
        let mut labels: Vec<_> = (1u8..).zip(DIGITS).map(|(n, digit)| (n, digit, takes.character)).collect();
        labels.extend([
            (BACKSPACE, "Backspace", takes.backspace),
            // F13 is 'Cancel' while a dialog is open.
            (CANCEL, "Cancel", true),
            (UP, "Up", choosing && request.chosen > 0),
            (DOWN, "Down", choosing && request.chosen + 1 < request.accounts.len()),
            (ENTER, "Enter", self.typing(now) && request.pin.len() >= FEWEST_DIGITS),
        ]);
        labels
    }

    /// What has happened since this was last asked, to be written down.
    pub fn take_audit(&mut self) -> Vec<Audit> {
        std::mem::take(&mut self.audit)
    }
}

#[cfg(test)]
mod tests {
    use super::button::*;
    use super::*;

    /// One commander, two gunners, everyone crew, and a maintainer's role
    /// that nobody at this crew station holds. Idle after five minutes.
    fn access() -> Access<&'static str> {
        let members = |role: &str| -> Vec<String> {
            let held: &[&str] = match role {
                "commander" => &["cmdr"],
                "gunner" => &["smith", "jones"],
                "crew" => &["cmdr", "smith", "jones"],
                _ => &[],
            };
            held.iter().map(|account| account.to_string()).collect()
        };
        let text = "role crew\nrole commander\nrole gunner\nrole maintainer\nidle 5m\nfunction override commander";
        Access::new(Config::read(text, &["override"], members).unwrap())
    }

    fn require(roles: &str) -> Require {
        Require::parse(roles).unwrap()
    }

    /// Press F`n` and let it go.
    fn tap(access: &mut Access<&'static str>, n: u8, ms: u64) -> Pressed {
        let pressed = access.press(n, ms);
        access.release(n);
        pressed
    }

    /// Type digits on the buttons that have them.
    fn digits(access: &mut Access<&'static str>, pin: &str, ms: u64) {
        for digit in pin.chars() {
            let n = (1..=10).find(|&n| button::digit(n) == Some(digit)).unwrap();
            assert_eq!(tap(access, n, ms), Pressed::Taken);
        }
    }

    /// Type a PIN and enter it: what there is to check.
    fn enter(access: &mut Access<&'static str>, pin: &str, ms: u64) -> Check {
        digits(access, pin, ms);
        assert_eq!(tap(access, ENTER, ms), Pressed::Taken);
        access.take_check().expect("a PIN to check")
    }

    fn page(access: &Access<&'static str>, ms: u64) -> PinPage {
        access.page(ms).expect("a PIN page")
    }

    fn label(access: &Access<&'static str>, n: u8, ms: u64) -> (&'static str, bool) {
        let (_, text, able) = access.labels(ms).into_iter().find(|&(button, ..)| button == n).unwrap();
        (text, able)
    }

    /// What has been written down, as lines.
    fn audit(access: &mut Access<&'static str>) -> Vec<String> {
        access.take_audit().iter().map(Audit::to_string).collect()
    }

    #[test]
    fn what_needs_no_role_is_had_with_nobody_logged_in_and_nothing_asked() {
        let mut access = access();
        assert_eq!(access.session(), &Session::Nobody);
        assert_eq!(access.ask("SA", "SA", &Require::nothing(), 0), Asked::Allowed("SA"));
        assert!(!access.asking());
        assert!(access.page(0).is_none() && access.labels(0).is_empty());
        assert!(access.take_check().is_none() && access.deadline().is_none());
        assert!(audit(&mut access).is_empty(), "and nothing is written down about it");

        // So it is with no configuration at all, which is every image until
        // one says otherwise.
        let mut plain: Access<&'static str> = Access::default();
        assert!(plain.config().function("override").is_nothing());
        assert_eq!(plain.ask("WPN", "WPN", &Require::nothing(), 0), Asked::Allowed("WPN"));
    }

    #[test]
    fn what_needs_a_role_asks_for_a_pin_and_the_right_one_gives_what_was_asked_for() {
        let mut access = access();
        assert!(!access.allows(&require("commander")));
        assert_eq!(access.ask("orders", "COM: Orders", &require("commander"), 1000), Asked::Pin);
        assert!(access.asking());
        let asked = page(&access, 1000);
        assert_eq!((asked.wanted.as_str(), asked.account.as_str()), ("COM: Orders", "cmdr"));
        assert_eq!((asked.entered, asked.most, asked.standing), (0, MOST_DIGITS, Standing::Entering));
        assert!(asked.accounts.is_empty(), "one account would do: there is nothing to choose");

        let check = enter(&mut access, "246810", 2000);
        assert_eq!((check.account.as_str(), check.pin.reveal()), ("cmdr", "246810"));
        assert_eq!(page(&access, 2000).standing, Standing::Checking);
        assert!(access.take_check().is_none(), "a PIN is handed over once");

        assert_eq!(access.answer(check.serial, Answer::Accepted, 2500), Some("orders"));
        assert!(!access.asking());
        assert_eq!(access.session().account(), Some("cmdr"));
        assert_eq!(access.session().roles(), ["crew", "commander"]);
        // Whoever is logged in is not asked again, for this or for anything
        // else their roles will do for.
        assert_eq!(access.ask("orders", "COM: Orders", &require("commander"), 3000), Asked::Allowed("orders"));
        assert_eq!(access.ask("net", "COM: Net", &require("gunner crew"), 3000), Asked::Allowed("net"));
        assert!(access.allows(access.config().function("override")));
    }

    #[test]
    fn cancel_gives_nothing_and_leaves_things_as_they_were() {
        let mut access = access();
        access.ask("orders", "COM: Orders", &require("commander"), 0);
        digits(&mut access, "2468", 100);
        assert_eq!(label(&access, CANCEL, 100), ("Cancel", true));
        assert_eq!(tap(&mut access, CANCEL, 200), Pressed::Cancelled);
        assert!(!access.asking() && access.page(200).is_none());
        assert_eq!(access.session(), &Session::Nobody);
        assert!(access.take_check().is_none());

        // And so does the crew's going elsewhere, a PIN having been entered
        // but not yet taken to be checked.
        access.ask("orders", "COM: Orders", &require("commander"), 300);
        digits(&mut access, "246810", 400);
        tap(&mut access, ENTER, 400);
        access.cancel();
        assert!(!access.asking() && access.take_check().is_none());
        // The page asked afresh has nothing of what was entered before.
        access.ask("orders", "COM: Orders", &require("commander"), 500);
        assert_eq!(page(&access, 500).entered, 0);
    }

    #[test]
    fn ism_1597_the_page_says_how_many_digits_and_never_which() {
        let mut access = access();
        access.ask("orders", "COM: Orders", &require("commander"), 0);
        digits(&mut access, "97531", 100);
        let shown = page(&access, 100);
        assert_eq!(shown.entered, 5);
        let drawn = serde_json::to_string(&shown).unwrap();
        assert!(!drawn.contains("97531"), "{drawn}");
        assert!(!format!("{access:?}").contains("97531"), "nor does what is printed of it for a programmer");

        // Nor does the PIN on its way to be checked, but to the verifier.
        tap(&mut access, ENTER, 200);
        assert!(format!("{access:?}").contains("Pin of 5 digits"));
        let check = access.take_check().unwrap();
        assert_eq!(format!("{:?}", check.pin), "Pin of 5 digits");
        assert_eq!(check.pin.reveal(), "97531");
    }

    #[test]
    fn the_buttons_down_the_sides_have_the_digits_and_light_their_keys_while_held() {
        let mut access = access();
        access.ask("orders", "COM: Orders", &require("commander"), 0);
        let texts: Vec<&str> = (1..=10).map(|n| label(&access, n, 0).0).collect();
        assert_eq!(texts, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"]);
        assert_eq!(label(&access, BACKSPACE, 0), ("Backspace", false), "there is nothing to take away yet");

        let lit = |access: &Access<&'static str>| -> Vec<String> {
            page(access, 0).pad.keys.into_iter().filter(|key| key.pressed).map(|key| key.text).collect()
        };
        assert_eq!(access.press(7, 0), Pressed::Taken);
        assert_eq!(lit(&access), ["7"]);
        access.release(7);
        assert!(lit(&access).is_empty());
        assert_eq!(page(&access, 0).entered, 1);
        assert_eq!(label(&access, BACKSPACE, 0), ("Backspace", true));

        tap(&mut access, ZERO, 0);
        tap(&mut access, BACKSPACE, 0);
        tap(&mut access, BACKSPACE, 0);
        tap(&mut access, BACKSPACE, 0);
        assert_eq!(page(&access, 0).entered, 0);
        // Enter enters the PIN: there is no key for it to select, and none
        // is highlighted as if there were.
        assert!(page(&access, 0).pad.keys.iter().all(|key| !key.highlighted));
        assert_eq!(tap(&mut access, 12, 0), Pressed::Taken, "a button with nothing on it is still the page's");
    }

    #[test]
    fn a_touched_key_types_what_its_button_does() {
        let mut access = access();
        access.ask("orders", "COM: Orders", &require("commander"), 0);
        let key = |access: &Access<&'static str>, text: &str| {
            page(access, 0).pad.keys.iter().position(|key| key.text == text).unwrap()
        };
        for digit in ["3", "0", "8", "5"] {
            access.touch(Touched::Key(key(&access, digit)), 0);
            assert!(page(&access, 0).pad.keys[key(&access, digit)].pressed);
            access.lift();
        }
        access.touch(Touched::Key(key(&access, "5")), 0);
        access.lift();
        access.touch(Touched::Key(key(&access, "Backspace")), 0);
        access.lift();
        // The pad's decimal point is no part of a PIN, and is drawn as a
        // key that does nothing.
        let point = key(&access, ".");
        assert!(!page(&access, 0).pad.keys[point].available);
        access.touch(Touched::Key(point), 0);
        access.touch(Touched::Key(99), 0);
        assert_eq!(page(&access, 0).entered, 4);
        tap(&mut access, ENTER, 0);
        assert_eq!(access.take_check().unwrap().pin.reveal(), "3085");
    }

    #[test]
    fn enter_wants_enough_digits_and_the_pad_takes_no_more_than_the_box_holds() {
        let mut access = access();
        access.ask("orders", "COM: Orders", &require("commander"), 0);
        digits(&mut access, "123", 0);
        assert_eq!(label(&access, ENTER, 0), ("Enter", false));
        tap(&mut access, ENTER, 0);
        assert!(access.take_check().is_none(), "three digits are a slip, not a PIN");
        assert_eq!(page(&access, 0).standing, Standing::Entering);
        digits(&mut access, "4", 0);
        assert_eq!(label(&access, ENTER, 0), ("Enter", true));

        digits(&mut access, "567890123456", 0);
        assert_eq!(page(&access, 0).entered, MOST_DIGITS);
        assert_eq!(label(&access, 1, 0), ("1", false));
        assert!(page(&access, 0).pad.keys.iter().all(|key| key.text == "Backspace" || !key.available));
        tap(&mut access, ENTER, 0);
        assert_eq!(access.take_check().unwrap().pin.reveal(), "123456789012");
    }

    #[test]
    fn a_refused_pin_says_what_the_verifier_said_and_another_can_be_tried() {
        let mut access = access();
        access.ask("orders", "COM: Orders", &require("commander"), 0);
        let check = enter(&mut access, "111111", 100);
        // Away being checked, nothing more is typed.
        digits(&mut access, "9", 150);
        assert_eq!((page(&access, 150).entered, label(&access, 9, 150).1), (0, false));

        let refused = Answer::Refused { why: "Authentication failure".to_string(), wait_ms: None };
        assert_eq!(access.answer(check.serial, refused, 200), None);
        assert!(access.asking());
        assert_eq!(access.session(), &Session::Nobody);
        let shown = page(&access, 200);
        assert_eq!((shown.standing, shown.entered, shown.wait), (Standing::Refused, 0, None));
        assert_eq!(shown.said.as_deref(), Some("Authentication failure"));

        // The first digit of the next is no longer about the last.
        digits(&mut access, "2", 300);
        assert_eq!((page(&access, 300).standing, page(&access, 300).said), (Standing::Entering, None));
        let check = enter(&mut access, "46810", 400);
        assert_eq!(check.pin.reveal(), "246810");
        assert_eq!(access.answer(check.serial, Answer::Accepted, 500), Some("orders"));
    }

    #[test]
    fn ism_1403_the_verifiers_wait_is_counted_down_and_nothing_typed_until_it_is_over() {
        let mut access = access();
        access.ask("orders", "COM: Orders", &require("commander"), 0);
        let check = enter(&mut access, "111111", 1000);
        let locked = Answer::Refused { why: "The account is locked".to_string(), wait_ms: Some(60_000) };
        access.answer(check.serial, locked, 1000);

        let shown = page(&access, 1000);
        assert_eq!((shown.standing, shown.wait), (Standing::Waiting, Some(60)));
        assert_eq!(shown.said.as_deref(), Some("The account is locked"));
        assert!(shown.pad.keys.iter().all(|key| !key.available));
        digits(&mut access, "246810", 2000);
        tap(&mut access, ENTER, 2000);
        assert_eq!(page(&access, 2000).entered, 0);
        assert!(access.take_check().is_none());
        assert_eq!((label(&access, 1, 2000).1, label(&access, ENTER, 2000).1), (false, false));

        // Time is asked for a second at a time while the wait is shown, and
        // nothing has changed until a second of it has gone.
        assert_eq!(access.deadline(), Some(3000));
        assert!(!access.tick(2500));
        assert!(access.tick(3000));
        assert!(!access.tick(3000) && !access.tick(3999), "it is not for ever being drawn again");
        assert_eq!(access.deadline(), Some(4000));
        assert_eq!(page(&access, 3000).wait, Some(58));
        assert_eq!(page(&access, 60_500).wait, Some(1));

        // The wait is the account's, whatever is asked for and however
        // often the page is cancelled.
        tap(&mut access, CANCEL, 4000);
        access.ask("net", "COM: Net", &require("commander"), 5000);
        assert_eq!((page(&access, 5000).standing, page(&access, 5000).wait), (Standing::Waiting, Some(56)));
        // And not another account's.
        access.ask("gun", "WPN", &require("gunner"), 5000);
        assert_eq!(page(&access, 5000).standing, Standing::Entering);

        access.ask("net", "COM: Net", &require("commander"), 60_900);
        assert!(access.tick(61_000), "the wait is over, and the page is to say so");
        assert_eq!((page(&access, 61_000).standing, page(&access, 61_000).wait), (Standing::Entering, None));
        let check = enter(&mut access, "246810", 62_000);
        assert_eq!(access.answer(check.serial, Answer::Accepted, 62_000), Some("net"));
    }

    #[test]
    fn with_nobody_to_check_a_pin_nothing_is_given() {
        let mut access = access();
        access.ask("orders", "COM: Orders", &require("commander"), 0);
        let check = enter(&mut access, "246810", 0);
        assert_eq!(access.answer(check.serial, Answer::Unavailable("no verifier".to_string()), 100), None);
        assert_eq!(access.session(), &Session::Nobody);
        let shown = page(&access, 100);
        assert_eq!((shown.standing, shown.said.as_deref()), (Standing::Unchecked, Some("no verifier")));
        assert_eq!(label(&access, 1, 100), ("1", true), "it can be tried again");
    }

    #[test]
    fn an_answer_to_a_page_that_has_gone_is_nobodys() {
        let mut access = access();
        access.ask("orders", "COM: Orders", &require("commander"), 0);
        let first = enter(&mut access, "246810", 0);
        tap(&mut access, CANCEL, 100);
        assert_eq!(access.answer(first.serial, Answer::Accepted, 200), None);
        assert_eq!(access.session(), &Session::Nobody);

        // Nor is it the answer to a PIN entered since, on another page.
        access.ask("gun", "WPN", &require("gunner"), 300);
        let second = enter(&mut access, "135790", 300);
        assert_eq!(access.answer(first.serial, Answer::Accepted, 400), None);
        assert_eq!(page(&access, 400).standing, Standing::Checking);
        assert_eq!(access.answer(second.serial, Answer::Accepted, 500), Some("gun"));
        assert_eq!(access.answer(second.serial, Answer::Accepted, 600), None, "nor is an answer given twice");
    }

    #[test]
    fn what_needs_a_role_nobody_here_holds_is_never_asked_for() {
        let mut access = access();
        let maintainer = require("maintainer");
        assert!(!access.possible(&maintainer));
        assert_eq!(access.ask("workshop", "SYS: Workshop", &maintainer, 0), Asked::Never);
        assert!(!access.asking());
        assert!(access.possible(&require("maintainer commander")) && access.possible(&Require::nothing()));
    }

    #[test]
    fn alerts_and_labels_are_never_the_pin_pages() {
        let mut access = access();
        for n in 1..=20 {
            assert_eq!(access.press(n, 0), Pressed::Passed, "F{n}, with no PIN page up");
        }
        access.ask("orders", "COM: Orders", &require("commander"), 0);
        digits(&mut access, "24", 0);
        for n in [ALARMS, THREATS, ACKNOWLEDGE, LABELS] {
            assert_eq!(tap(&mut access, n, 0), Pressed::Passed, "F{n}");
            assert!(access.labels(0).iter().all(|&(button, ..)| button != n), "F{n} keeps the label it has everywhere");
        }
        assert_eq!(page(&access, 0).entered, 2, "and the page is as it was");
        // While the PIN is away being checked, and while the verifier is
        // being waited for, as at any other time.
        let check = enter(&mut access, "6810", 0);
        assert_eq!(tap(&mut access, ACKNOWLEDGE, 0), Pressed::Passed);
        access.answer(check.serial, Answer::Refused { why: "locked".to_string(), wait_ms: Some(60_000) }, 0);
        assert_eq!(tap(&mut access, ALARMS, 0), Pressed::Passed);
    }

    #[test]
    fn ism_2012_after_the_idle_time_nobody_is_logged_in_and_a_press_puts_that_off() {
        let mut access = access();
        access.ask("orders", "COM: Orders", &require("commander"), 0);
        let check = enter(&mut access, "246810", 0);
        access.answer(check.serial, Answer::Accepted, 10_000);
        assert_eq!(access.deadline(), Some(310_000), "five minutes from the PIN's being accepted");

        assert!(!access.tick(200_000));
        access.active(250_000);
        assert_eq!(access.deadline(), Some(550_000));
        assert!(!access.tick(549_999));
        assert_eq!(access.session().account(), Some("cmdr"));
        assert!(access.tick(550_000));
        assert_eq!(access.session(), &Session::Nobody);
        assert_eq!(access.ask("orders", "COM: Orders", &require("commander"), 550_000), Asked::Pin);
        // With nobody logged in there is nobody to become idle.
        tap(&mut access, CANCEL, 550_000);
        assert_eq!(access.deadline(), None);
        assert!(!access.tick(10_000_000));
    }

    #[test]
    fn logging_out_leaves_nobody_logged_in() {
        let mut access = access();
        access.ask("gun", "WPN", &require("gunner"), 0);
        let check = enter(&mut access, "135790", 0);
        access.answer(check.serial, Answer::Accepted, 0);
        assert_eq!(access.session().account(), Some("smith"));
        access.leave();
        assert_eq!(access.session(), &Session::Nobody);
        assert!(!access.allows(&require("gunner")));
        access.leave();
        assert_eq!(audit(&mut access).iter().filter(|line| line.starts_with("logged-out")).count(), 1);
    }

    #[test]
    fn a_new_login_takes_the_place_of_the_last() {
        let mut access = access();
        access.ask("gun", "WPN", &require("gunner"), 0);
        let check = enter(&mut access, "135790", 0);
        access.answer(check.serial, Answer::Accepted, 0);

        // A gunner is no commander: the commander's PIN is asked for, and
        // until it is given the gunner is logged in still.
        assert_eq!(access.ask("orders", "COM: Orders", &require("commander"), 1000), Asked::Pin);
        assert_eq!(access.session().account(), Some("smith"));
        let check = enter(&mut access, "246810", 1000);
        assert_eq!(access.answer(check.serial, Answer::Accepted, 1000), Some("orders"));
        assert_eq!(access.session().account(), Some("cmdr"));
        assert!(!access.allows(&require("gunner")), "one account is logged in, and it is the last");
        assert!(audit(&mut access).contains(&"displaced account=smith roles=crew,gunner".to_string()));
    }

    #[test]
    fn a_pin_page_nobody_answers_goes_away() {
        let mut access = access();
        access.ask("orders", "COM: Orders", &require("commander"), 1000);
        digits(&mut access, "24", 5000);
        assert_eq!(access.deadline(), Some(5000 + ASKING_MS));
        assert!(!access.tick(60_000));
        assert!(access.tick(65_000));
        assert!(!access.asking());
        assert_eq!(audit(&mut access).last().map(String::as_str), Some("abandoned for=\"COM: Orders\""));

        // One whose PIN is away being checked waits for the answer.
        access.ask("orders", "COM: Orders", &require("commander"), 70_000);
        let check = enter(&mut access, "246810", 70_000);
        assert!(!access.tick(500_000));
        assert_eq!(access.answer(check.serial, Answer::Accepted, 500_000), Some("orders"));
    }

    #[test]
    fn where_several_accounts_would_do_the_crew_choose_whose_pin_it_is() {
        let mut access = access();
        access.ask("gun", "WPN", &require("gunner commander"), 0);
        let names = |access: &Access<&'static str>| -> Vec<(usize, String, bool)> {
            page(access, 0).accounts.into_iter().map(|row| (row.number, row.name, row.chosen)).collect()
        };
        let row = |number, name: &str, chosen| (number, name.to_string(), chosen);
        assert_eq!(names(&access), [row(1, "smith", true), row(2, "jones", false), row(3, "cmdr", false)]);
        assert_eq!((label(&access, UP, 0), label(&access, DOWN, 0)), (("Up", false), ("Down", true)));

        digits(&mut access, "13", 0);
        tap(&mut access, DOWN, 0);
        assert_eq!(page(&access, 0).account, "jones");
        assert_eq!(page(&access, 0).entered, 0, "what was entered was the other's");
        access.touch(Touched::Account(3), 0);
        assert_eq!(names(&access)[2], row(3, "cmdr", true));
        assert_eq!(label(&access, DOWN, 0), ("Down", false));
        tap(&mut access, DOWN, 0);
        access.touch(Touched::Account(0), 0);
        access.touch(Touched::Account(4), 0);
        assert_eq!(page(&access, 0).account, "cmdr");

        let check = enter(&mut access, "246810", 0);
        assert_eq!(check.account, "cmdr");
        // Not while that one's PIN is away being checked.
        tap(&mut access, UP, 0);
        assert_eq!((page(&access, 0).account, label(&access, UP, 0)), ("cmdr".to_string(), ("Up", false)));
        assert_eq!(access.answer(check.serial, Answer::Accepted, 0), Some("gun"));
        assert_eq!(access.session().roles(), ["crew", "commander"]);
    }

    #[test]
    fn what_happens_is_written_down_and_no_pin_with_it() {
        let mut access = access();
        access.ask("orders", "COM: Orders", &require("commander"), 0);
        let check = enter(&mut access, "111111", 0);
        access.answer(check.serial, Answer::Refused { why: "Authentication failure".to_string(), wait_ms: None }, 0);
        let check = enter(&mut access, "222222", 0);
        access.answer(check.serial, Answer::Unavailable("no verifier".to_string()), 0);
        tap(&mut access, CANCEL, 0);
        access.ask("orders", "COM: Orders", &require("commander"), 0);
        let check = enter(&mut access, "246810", 0);
        access.answer(check.serial, Answer::Accepted, 0);
        access.tick(300_000);

        let lines = audit(&mut access);
        assert_eq!(
            lines,
            [
                "asked roles=commander for=\"COM: Orders\"",
                "refused account=cmdr for=\"COM: Orders\" said=\"Authentication failure\"",
                "unchecked account=cmdr for=\"COM: Orders\" said=\"no verifier\"",
                "cancelled for=\"COM: Orders\"",
                "asked roles=commander for=\"COM: Orders\"",
                "accepted account=cmdr roles=crew,commander for=\"COM: Orders\"",
                "idle account=cmdr roles=crew,commander",
            ]
        );
        assert!(lines.iter().all(|line| !line.contains("111111") && !line.contains("246810") && !line.contains('6')));
        assert!(audit(&mut access).is_empty(), "each is written down once");
    }

    #[test]
    fn the_page_serialises_in_the_shape_the_ui_reads() {
        let mut access = access();
        access.ask("gun", "WPN", &require("gunner"), 0);
        digits(&mut access, "13", 0);
        let drawn = serde_json::to_value(page(&access, 0)).unwrap();
        assert_eq!(drawn["wanted"], "WPN");
        assert_eq!(drawn["account"], "smith");
        assert_eq!(drawn["accounts"][1], serde_json::json!({ "number": 2, "name": "jones", "chosen": false }));
        assert_eq!((&drawn["entered"], &drawn["most"]), (&serde_json::json!(2), &serde_json::json!(12)));
        assert_eq!(drawn["standing"], "entering");
        assert!(drawn["said"].is_null() && drawn["wait"].is_null());
        assert_eq!(drawn["pad"]["layout"], "phone");
        assert_eq!(drawn["pad"]["keys"].as_array().unwrap().len(), 12);
        assert_eq!(drawn["pad"]["keys"][0]["text"], "1");
    }
}
