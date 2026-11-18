//! What the HMI shows, as data, and the state behind it.
//!
//! The UI holds no state of record: it is handed a [`Frame`] and draws it.
//! Bezel buttons come in as [`ButtonEvent`]s, and what the crew touches of
//! what the frame had drawn as [`Touched`]. Everything here is in the
//! vocabulary of a GVA display (functional areas, labels, the status
//! information bar). Nothing here knows where vehicle data comes from, or what
//! device a button press arrived on - those boundaries are deliberate.
//!
//! A functional area's page can be another program's window, shown under the
//! HMI's own elements: see [`ExternalArea`]. Or several programs' windows, of
//! which the crew picks some to be shown side by side, each in a tile of the
//! page: see [`TiledArea`]. Or several programs' windows one at a time, each
//! a view of the area that the crew chooses from the area's top level menu:
//! see [`ViewedArea`]. Which programs, and what they show, is equally not
//! this crate's business.
//!
//! Or a functional area's page can be a form for the crew to fill in, with
//! the on-screen keyboard: see [`FormArea`]. Forms and the keyboard are
//! `hmi-entry`'s, and what a form is for is nobody's here. The same keyboard
//! comes up over another program's page, where the program is one that can
//! be typed into ([`ExternalArea::keyboard`]): what is typed there goes out
//! as [`Stroke`]s ([`Hmi::take_strokes`]), for the program, whatever it is.
//!
//! What the vehicle has to say - its alarms, and the threats its sensors
//! find - comes in as [`Report`]s and what the crew does about it goes out
//! as [`Request`]s: [`feed`] has how each is written down. Time comes in too
//! ([`Time`]): nothing here reads a clock.
//!
//! This crate has no windowing or I/O dependencies so that it can be tested
//! on any machine with `cargo test -p hmi-core`.

mod alarms;
mod dtg;
mod external;
pub mod feed;
mod table;
mod threats;
#[cfg(test)]
mod gated;
mod tiles;
mod views;

use alarms::Alarms;
pub use alarms::{AlarmCounts, AlarmDetail, AlarmRow, Category, Message, Raised, State, Time};
pub use dtg::date_time_group;
pub use external::{AppKey, AreaView, External, ExternalArea, FormArea, FromApp, TileWindow, TiledArea, ToApp, ViewedArea, keys_fault};
pub use feed::{Report, Request};
use hmi_access::{Access, Asked, Pressed, Require};
pub use hmi_access::{Answer, Audit, Check, Config as AccessConfig, PinPage, Session};
pub use hmi_entry::{Kind, Stroke};
use hmi_entry::{Entry, FormPage, KeyboardView, Typing};
use serde::{Deserialize, Serialize};
use std::fmt;
use table::Paged;
use threats::Threats;
use tiles::Tiles;
pub use tiles::{Aspect, Rect, Tile};
use views::Views;
pub use threats::{
    Detected, Plotted, Priority, Sensor, SensorStatus, ThreatCounts, ThreatDetail, ThreatMark, ThreatMessage,
    ThreatPlot, ThreatRow, ThreatState,
};

/// Everything on the screen at one instant.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    /// The eight functional areas, in bezel order left to right.
    pub areas: Vec<Area>,
    pub status: Status,
    /// The twenty soft-key labels, F1 to F20.
    pub labels: Vec<Label>,
    /// The alarm text message that goes directly beneath the status
    /// information bar, when there is one.
    pub alarm: Option<Message>,
    /// The threat text message that goes along the bottom of the page, when
    /// there is one: not where an alarm's goes.
    pub threat: Option<ThreatMessage>,
    /// The Plan Position Indicator, when it is on the screen.
    pub ppi: Option<Ppi>,
    pub page: Page,
    /// What F19 has taken off the screen for now.
    pub hidden: Hidden,
    /// Whether the touch screen is on. While it is off a touch
    /// does nothing, here or in another program's picture.
    pub touch: bool,
}

/// What of the frame is not to be drawn, because F19 has hidden it.
/// The Plan Position Indicator, hidden, is simply not
/// in the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Hidden {
    /// The labels of the bezel buttons: the bars under the functional-area
    /// buttons and the twenty soft-key labels.
    pub labels: bool,
    /// The status information bar.
    pub status: bool,
}

/// The Plan Position Indicator. The vehicle's front is up the
/// screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ppi {
    /// Where the current threats are.
    pub threats: Vec<ThreatMark>,
}

/// The functional areas, in the order of the top bezel buttons.
pub const AREAS: [&str; 8] = ["SA", "WPN", "DEF", "SYS", "DRV", "STR", "COM", "BMS"];

#[derive(Debug, Clone, Serialize)]
pub struct Area {
    pub id: &'static str,
    pub state: AreaState,
}

/// What the bar under a top bezel button says about its area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AreaState {
    /// Has functions and is the area on screen.
    Selected,
    /// Has functions.
    Available,
    /// Temporarily unavailable.
    Unavailable,
    /// Not implemented on this platform.
    Absent,
}

#[derive(Debug, Clone, Serialize)]
pub struct Label {
    /// The bezel button this label belongs to: "F1" to "F20".
    pub key: String,
    /// What it says. A line break in it is one on the screen.
    pub text: String,
    pub state: LabelState,
}

/// How a soft-key label is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelState {
    Available,
    Selected,
    Unavailable,
    /// The button has no function on this page: nothing is drawn.
    Blank,
}

/// The status information bar.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// Date-time group, far left.
    pub dtg: String,
    /// Grid reference in the selected format; `None` until there is a fix.
    pub position: Option<String>,
    /// Heading in NATO mils, 0 to 6399.
    pub heading_mils: Option<u16>,
    /// Active alarm counters, far right.
    pub alarms: AlarmCounts,
}

/// The reconfigurable middle of the screen.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Page {
    /// Drawn by the HMI.
    Own { title: String, rows: Vec<Row> },
    /// Another program's picture, which is under the HMI's window: the UI
    /// leaves the middle of the screen clear for it, but for `overlay`, an
    /// image to draw over the picture - and for the on-screen `keyboard`,
    /// while the crew has it up to type into the program with: that has the
    /// lower part of the page, and the picture what is left above it.
    External { overlay: Option<String>, keyboard: Option<KeyboardView> },
    /// Other programs' pictures side by side, each of which is under the
    /// HMI's window in a tile of its own: the UI leaves the tiles clear, and
    /// draws round them. As many as are picked, which can be none.
    Tiles { tiles: Vec<Tile> },
    /// The Alarms page, which F14 brings up.
    Alarms {
        /// How many alarms of each category have not been acknowledged...
        unacknowledged: AlarmCounts,
        /// ...and how many have.
        acknowledged: AlarmCounts,
        /// The page of the table that is showing, counted from 1...
        page: usize,
        /// ...of this many.
        pages: usize,
        rows: Vec<AlarmRow>,
        /// The picked alarm at length, when the crew has asked for it.
        detail: Option<AlarmDetail>,
    },
    /// The Threats page, which F15 brings up.
    Threats {
        /// How many threats are there, and how many have gone.
        counts: ThreatCounts,
        /// The sensors that look for threats, and how each is.
        sensors: Vec<Sensor>,
        /// The threats, as a table or as a picture.
        view: ThreatsView,
        /// The picked threat at length, when the crew has asked for it.
        detail: Option<ThreatDetail>,
    },
    /// A form for the crew to fill in and, while they are entering data into
    /// it, the on-screen keyboard.
    Form(FormPage),
    /// A PIN being asked for, the crew having gone to something that needs
    /// a role: a dialog, over whatever the page was (`hmi-access`).
    Pin(PinPage),
}

/// How the Threats page shows the threats: "tabular, pictorial
/// or the option for both".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ThreatsView {
    /// A table, a page of it at a time.
    Table {
        /// The page that is showing, counted from 1...
        page: usize,
        /// ...of this many.
        pages: usize,
        rows: Vec<ThreatRow>,
    },
    /// The Plan Position Indicator at the size of the page, with the threats
    /// on it where they are.
    Plot(ThreatPlot),
}

#[derive(Debug, Clone, Serialize)]
pub struct Row {
    pub name: String,
    pub value: String,
}

impl Row {
    pub fn new(name: &str, value: impl Into<String>) -> Self {
        Row { name: name.to_string(), value: value.into() }
    }
}

/// The labels of the common function buttons along the bottom, F13 to F20.
/// F13 reads "Cancel" instead while a dialog is open.
///
/// F17 and F18 read "Up" and "Down". F13, which goes up a menu level,
/// also reads "Up".
pub const COMMON_LABELS: [&str; 8] =
    ["Up", "Alarms", "Threats", "Ack", "Up", "Down", "Labels", "Enter"];

/// A bezel button: eight along the top that select a functional
/// area, six down each side (F1 to F12) and eight along the bottom (F13 to F20).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    /// A functional-area button: an index into [`AREAS`].
    Area(u8),
    /// F1 to F20.
    F(u8),
}

impl Button {
    /// The functional-area button at `index`, 0 (SA) to 7 (BMS).
    pub fn area(index: u8) -> Option<Self> {
        (usize::from(index) < AREAS.len()).then_some(Button::Area(index))
    }

    /// The button F`n`, 1 to 20.
    pub fn f(n: u8) -> Option<Self> {
        (1..=20).contains(&n).then_some(Button::F(n))
    }

    /// Every button, in bezel order: the top row, then F1 to F20.
    pub fn all() -> impl Iterator<Item = Button> {
        (0..8).map(Button::Area).chain((1..=20).map(Button::F))
    }
}

/// A button by its name, as [`Button`] is written.
impl std::str::FromStr for Button {
    type Err = ();

    fn from_str(name: &str) -> Result<Self, ()> {
        if let Some(area) = (0u8..).zip(AREAS).find_map(|(i, area)| (area == name).then_some(i)) {
            return Ok(Button::Area(area));
        }
        name.strip_prefix('F').and_then(|n| n.parse().ok()).and_then(Button::f).ok_or(())
    }
}

/// "SA" ... "BMS", "F1" ... "F20": the buttons' names.
impl fmt::Display for Button {
    fn fmt(&self, out: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Button::Area(i) => out.write_str(AREAS[usize::from(i)]),
            Button::F(n) => write!(out, "F{n}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Pressed,
    Released,
}

impl fmt::Display for Action {
    fn fmt(&self, out: &mut fmt::Formatter) -> fmt::Result {
        out.write_str(match self {
            Action::Pressed => "pressed",
            Action::Released => "released",
        })
    }
}

/// A bezel button going down or coming up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonEvent {
    pub button: Button,
    pub action: Action,
}

impl fmt::Display for ButtonEvent {
    fn fmt(&self, out: &mut fmt::Formatter) -> fmt::Result {
        write!(out, "{} {}", self.button, self.action)
    }
}

/// Something the frame had drawn, which the crew has touched. Whatever can
/// be touched can be done from the bezel as well.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Touched {
    /// A soft-key label, by its button: a label is a soft key too.
    Label(Button),
    /// A row of the table on the page, or a threat's symbol on the threats'
    /// picture, by the name the frame gave it.
    Row(String),
    /// A field of the form on the page, by the number the frame gave it.
    Field(usize),
    /// A key of the on-screen keyboard - a form's, or the one up over
    /// another program's page - by its place among the keys as the frame
    /// listed them. It is lit until the finger goes ([`Hmi::lift`]).
    Key(usize),
}

impl fmt::Display for Touched {
    fn fmt(&self, out: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Touched::Label(button) => write!(out, "{button}"),
            Touched::Row(name) => write!(out, "row {name}"),
            Touched::Field(number) => write!(out, "field {number}"),
            Touched::Key(key) => write!(out, "key {key}"),
        }
    }
}

/// How much of the display F19 has hidden for now. Each
/// press hides more - the labels of the bezel buttons, then the status
/// information bar as well, then what else lies over the page that the crew
/// can do without - and the press after the last brings it all back.
/// The text message of an alarm or of a threat is not such a
/// thing, and is never hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
enum Hiding {
    #[default]
    Nothing,
    Labels,
    Status,
    Overlays,
}

impl Hiding {
    /// What the next press of F19 leaves hidden. With no `overlay` on the
    /// screen there is nothing more to hide after the status information
    /// bar, and the press shows everything again rather than do nothing.
    fn next(self, overlay: bool) -> Hiding {
        match self {
            Hiding::Nothing => Hiding::Labels,
            Hiding::Labels => Hiding::Status,
            Hiding::Status if overlay => Hiding::Overlays,
            Hiding::Status | Hiding::Overlays => Hiding::Nothing,
        }
    }
}

/// The SYS area, an index into [`AREAS`]: the one area that is the HMI's own.
/// It is what is on screen after start-up (8.4.1 suggests the SYS overview)
/// and what the HMI falls back to.
const SYS: u8 = 3;

/// What the buttons down the sides do on the SYS page.
mod sys_page {
    /// The touch screen, on and off.
    pub const TOUCH: u8 = 12;
    /// Logging out, where there are roles to be logged in with: beside the
    /// touch screen's button, and not where a slip for it would be made.
    pub const LOG_OUT: u8 = 11;
}

/// What a button down the sides does on a page that is another program's
/// and can be typed into ([`ExternalArea::keyboard`]).
mod typed_page {
    /// Brings the on-screen keyboard up over the page, and puts it away
    /// again: the same button whichever program's page it is.
    /// It is one that a form's page gives nothing to do, so that while the
    /// keyboard is up every button of data entry is where a form has it
    /// (`hmi_entry::button`) and this one is still there to put the keyboard
    /// away with. And it is low on the left: beside the keyboard while that
    /// is up, and while it is not, its label - the one thing the HMI draws
    /// over such a picture's edge - is clear of the top, where a program's
    /// own menus are.
    pub const KEYBOARD: u8 = 5;
}

/// The common function button that hides labels and shows them.
const LABELS: u8 = 19;

/// What the buttons down the sides do on any page with a table. The table's
/// pick is moved with F17 and F18, and F20 shows the picked row at length as
/// F4 does.
mod table_page {
    pub const DETAIL: u8 = 4;
    pub const PAGE_UP: u8 = 5;
    pub const PAGE_DOWN: u8 = 6;
}

/// What the buttons down the sides do on the Alarms page.
mod alarms_page {
    pub use super::table_page::*;
    pub const SORT_BY_CATEGORY: u8 = 1;
    pub const SORT_BY_TIME: u8 = 2;
    pub const ACKNOWLEDGED: u8 = 3;
    pub const ACKNOWLEDGE: u8 = 7;
    pub const ACKNOWLEDGE_ALL: u8 = 8;
    pub const CLEAR: u8 = 9;
    pub const CLEAR_ALL: u8 = 10;
    pub const OVERRIDE: u8 = 11;
}

/// What the buttons down the sides do on the Threats page:
/// each on the button that does the like on the Alarms page.
mod threats_page {
    pub const SORT_BY_TYPE: u8 = 1;
    pub const SORT_BY_TIME: u8 = 2;
    pub const PAST: u8 = 3;
    pub const CANCEL: u8 = 9;
    pub const CANCEL_ALL: u8 = 10;
    /// The table, or the picture.
    pub const VIEW: u8 = 12;
}

/// The HMI's own functions that a configuration can keep for a role
/// (`function` in access.conf), by the names it has for them there: what
/// silences an alarm's source, what takes alarms off the list, and what
/// takes threats off theirs. Seeing alarms and threats, and acknowledging
/// them, are not among them and cannot be made so (`hmi_access::NEVER`).
pub const FUNCTIONS: [&str; 3] = ["alarms.override", "alarms.clear", "threats.cancel"];

/// One of those functions, with what it is to be done to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Function {
    OverrideAlarm(String),
    ClearAlarm(String),
    ClearAlarms,
    CancelThreat(String),
    CancelThreats,
}

impl Function {
    /// Its name in a configuration, and what the crew know it as.
    fn names(&self) -> (&'static str, &'static str) {
        match self {
            Function::OverrideAlarm(_) => (FUNCTIONS[0], "Override Alarm"),
            Function::ClearAlarm(_) => (FUNCTIONS[1], "Clear Alarm"),
            Function::ClearAlarms => (FUNCTIONS[1], "Clear All Alarms"),
            Function::CancelThreat(_) => (FUNCTIONS[2], "Cancel Threat"),
            Function::CancelThreats => (FUNCTIONS[2], "Cancel All Threats"),
        }
    }
}

/// Something the crew can go to that may need a role: what is kept while a
/// PIN is asked for it, and shown or done when the PIN is right.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Wanted {
    /// A functional area: an index into [`AREAS`].
    Area(u8),
    /// A view of an area of views, by its place among them.
    View { area: u8, view: usize },
    Function(Function),
}

/// The state of the HMI.
///
/// The functional-area buttons select an area. F14 brings up the Alarms page
/// and F15 the Threats page, F16 acknowledges the alarm and the threat
/// whose messages are showing, and F19 hides the labels and more, a press at
/// a time. On the page of an area of tiles the buttons
/// down the sides show its windows and stop showing them. Of the other
/// buttons, those that have no function yet are remembered and shown, so
/// that the path from a bezel to here can be seen working.
#[derive(Debug)]
pub struct Hmi {
    selected: u8,
    given: Vec<Given>,
    alarms: Alarms,
    threats: Threats,
    /// The Alarms page or the Threats page, while one of them is the page
    /// on screen.
    menu: Option<Menu>,
    /// What F19 has hidden.
    hiding: Hiding,
    /// Whether the touch screen is on.
    touch: bool,
    /// The on-screen keyboards the crew has up over other programs' pages,
    /// each with the name of the window it types into: a page that is left
    /// with the keyboard up has it up when it is come back to.
    typing: Vec<Up>,
    /// What has been typed on one of them and not yet been asked for.
    strokes: Vec<Stroke>,
    /// Who is logged in, and the PIN being asked for, if one is: nobody and
    /// none, unless something of the crew station's needs a role.
    access: Access<Wanted>,
    last_button: Option<ButtonEvent>,
    /// What programs are to be told of their buttons, each by its area, and
    /// not yet taken ([`Hmi::take_to_apps`]).
    to_apps: Vec<(u8, ToApp)>,
    /// A program's button that is down: its area, its number and its id. It
    /// is told when it is let go, whatever page is up by then.
    held: Option<(u8, u8, String)>,
}

/// An on-screen keyboard that is up over another program's page.
#[derive(Debug)]
struct Up {
    /// The name of the window it types into.
    window: String,
    typing: Typing,
    /// Who brought it up, which is who puts it away again.
    by: By,
}

/// Who brought an on-screen keyboard up over another program's page. One
/// that came up by itself, a field of the page having taken the focus, goes
/// away by itself when the field lets go - as whoever said the one says the
/// other. One the crew brought up is the crew's to put away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum By {
    /// The crew, with the button a page that can be typed into has for it.
    Crew,
    /// The display server, which said that a field of the window on show
    /// had taken the focus ([`Hmi::asked`]).
    Display,
    /// The window's own program, which said so for itself
    /// ([`Hmi::asked_for`]).
    Program,
}

/// What a program says for itself of its own window, where it cannot say it
/// through the display server: that a field of its page has the focus, and
/// what such a field takes, or that none has now. One line each time:
///
/// ```text
/// map-viewer text
/// map-viewer number
/// map-viewer decimal
/// map-viewer nothing
/// ```
///
/// The window is named as the configuration that gives it an area names it,
/// in as many words as that takes; what it wants is the line's last word.
/// Where the lines come from is not this crate's business
/// ([`Hmi::asked_for`] is where they go).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asking {
    /// The name the program gives its window.
    pub window: String,
    /// What the field that has the focus takes, or `None`: no field of the
    /// window has it now.
    pub kind: Option<Kind>,
}

impl Asking {
    /// Read one, as a program writes it.
    pub fn parse(line: &str) -> Result<Self, String> {
        let line = line.trim();
        let (window, wants) = line.rsplit_once(char::is_whitespace).unwrap_or(("", line));
        let kind = match wants {
            "text" => Some(Kind::Text),
            "number" => Some(Kind::Number),
            "decimal" => Some(Kind::Decimal),
            "nothing" => None,
            _ => return Err(format!("'{wants}' is not 'text', 'number', 'decimal' or 'nothing'")),
        };
        match window.trim_end() {
            "" => Err(format!("'{wants}' is said of no window")),
            window => Ok(Asking { window: window.to_string(), kind }),
        }
    }
}

/// A page that a common function button brings up over the selected area's,
/// with the table on it as the crew has it set.
#[derive(Debug)]
enum Menu {
    Alarms(alarms::Table),
    Threats(threats::Table),
}

/// What a button did to the table on such a page.
enum Browsed {
    /// What any table has it do: it has been done.
    Done,
    /// F13, with no dialog to cancel: up a level, off the page.
    Left,
    /// Nothing: it is for the page to say what the button does - to the row
    /// picked, if it is to one.
    Not(Option<String>),
}

/// Do what a button does on any page with a table.
fn browse<T: Paged>(table: &mut T, from: &T::From, n: u8) -> Browsed {
    use table_page::*;
    match n {
        // A row shown at length is a dialog: F13 cancels it, F20 accepts it,
        // F17 and F18 show the row before or after instead, and nothing down
        // the sides answers.
        13 | 20 if table.showing() => table.close(),
        17 => table.step(false, from),
        18 => table.step(true, from),
        _ if table.showing() => {}
        // Up a level: back to the page of the selected area.
        13 => return Browsed::Left,
        PAGE_UP => table.turn_page(false, from),
        PAGE_DOWN => table.turn_page(true, from),
        DETAIL | 20 => table.show(from),
        _ => return Browsed::Not(table.selected(from).map(str::to_string)),
    }
    Browsed::Done
}

/// A functional area that has been given to other programs, and how their
/// windows stand.
#[derive(Debug)]
enum Given {
    /// To one program, whose window has the page to itself.
    Window {
        area: ExternalArea,
        /// Whether the program's window is there to be shown.
        present: bool,
    },
    /// To several, whose windows have a tile of the page each.
    Tiles(Tiles),
    /// To several, whose windows have the page one at a time: the one the
    /// crew has chosen from the area's menu.
    Views(Views),
    /// To no program: it is a form, which is the HMI's own to show and the
    /// crew's to fill in.
    Form {
        /// The functional area: an index into [`AREAS`].
        area: u8,
        entry: Entry,
        /// The roles any one of which will do to have it.
        require: Require,
    },
}

impl Given {
    /// The functional area: an index into [`AREAS`].
    fn area(&self) -> u8 {
        match self {
            Given::Window { area, .. } => area.area,
            Given::Tiles(tiles) => tiles.area(),
            Given::Views(views) => views.area(),
            Given::Form { area, .. } => *area,
        }
    }

    /// The roles any one of which will do to have the area: none, unless
    /// its configuration says.
    fn require(&self) -> &Require {
        match self {
            Given::Window { area, .. } => &area.require,
            Given::Tiles(tiles) => tiles.require(),
            Given::Views(views) => views.area_requires(),
            Given::Form { require, .. } => require,
        }
    }

    /// Whether there is a window open to be shown: the area has functions.
    fn present(&self) -> bool {
        match self {
            Given::Window { present, .. } => *present,
            Given::Tiles(tiles) => tiles.present(),
            Given::Views(views) => views.present(),
            // A form is the HMI's own, and there from the start.
            Given::Form { .. } => true,
        }
    }

    /// Take the windows now open. Returns whether anything changed.
    fn windows<S: AsRef<str>>(&mut self, open: &[S]) -> bool {
        match self {
            Given::Window { area, present } => {
                let was = std::mem::replace(present, open.iter().any(|w| w.as_ref() == area.window));
                was != *present
            }
            Given::Tiles(tiles) => tiles.windows(open),
            Given::Views(views) => views.windows(open),
            Given::Form { .. } => false,
        }
    }
}

/// A window that belongs under the HMI's now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown<'a> {
    /// The name its program gives it.
    pub window: &'a str,
    /// The part of the display it is to have, if it is in a tile. A window
    /// that has the page to itself has all that the HMI's bars leave of the
    /// screen, which is for the display server to find.
    pub at: Option<Rect>,
}

impl Default for Hmi {
    fn default() -> Self {
        Hmi {
            selected: SYS,
            given: Vec::new(),
            alarms: Alarms::default(),
            threats: Threats::default(),
            menu: None,
            hiding: Hiding::Nothing,
            touch: true,
            typing: Vec::new(),
            strokes: Vec::new(),
            access: Access::default(),
            last_button: None,
            to_apps: Vec::new(),
            held: None,
        }
    }
}

impl Hmi {
    pub fn new() -> Self {
        Self::default()
    }

    /// Say who may have what at this crew station: its roles, and which of
    /// the HMI's own functions need one. To be said before any area is
    /// given away: an area that asks for a role there is not is a mistake,
    /// and is refused. Without this there are no roles, nothing needs one,
    /// and nobody is ever asked for anything.
    pub fn set_access(&mut self, config: AccessConfig) {
        self.access = Access::new(config);
    }

    /// Give a functional area to another program. It is unavailable until
    /// the program's window is seen ([`Hmi::windows`]).
    pub fn add_external(&mut self, area: ExternalArea) -> Result<(), String> {
        self.to_give(area.area)?;
        self.access.config().knows(&area.require)?;
        if let Some(fault) = keys_fault(&area.keys, area.keyboard) {
            return Err(fault);
        }
        self.given.push(Given::Window { area, present: false });
        Ok(())
    }

    /// The areas whose programs listen for the HMI, and where: each by its
    /// area, an index into [`AREAS`].
    pub fn app_sockets(&self) -> Vec<(u8, String)> {
        self.given
            .iter()
            .filter_map(|given| match given {
                Given::Window { area, .. } => Some((area.area, area.socket.clone()?)),
                _ => None,
            })
            .collect()
    }

    /// Take a line from the program of `area`, over its socket. Returns
    /// whether the frame changed; a line that cannot be taken says why, and
    /// changes nothing.
    pub fn from_app(&mut self, area: u8, line: &str) -> Result<bool, String> {
        let FromApp::Keys(keys) = FromApp::parse(line)?;
        let Some(Given::Window { area: given, .. }) = self.given.iter_mut().find(|given| given.area() == area) else {
            return Err(format!("{} is no program's", AREAS[usize::from(area)]));
        };
        if let Some(fault) = keys_fault(&keys, given.keyboard) {
            return Err(fault);
        }
        let changed = given.keys != keys;
        given.keys = keys;
        Ok(changed)
    }

    /// What programs are to be told of their buttons since this was last
    /// asked: each line with the area whose program it is for.
    pub fn take_to_apps(&mut self) -> Vec<(u8, ToApp)> {
        std::mem::take(&mut self.to_apps)
    }

    /// Give a functional area to several programs, whose windows are shown
    /// in tiles. It is unavailable until one of the windows is seen
    /// ([`Hmi::windows`]), and none is shown until the crew picks it.
    pub fn add_tiled(&mut self, area: TiledArea) -> Result<(), String> {
        self.to_give(area.area)?;
        self.access.config().knows(&area.require)?;
        if let Some(fault) = area.fault() {
            return Err(fault);
        }
        self.given.push(Given::Tiles(Tiles::new(area)));
        Ok(())
    }

    /// Give a functional area to several programs, whose windows have its
    /// page one at a time. It is unavailable until one of the windows is
    /// seen ([`Hmi::windows`]). The first is what it shows until the crew
    /// chooses another from its top level menu.
    pub fn add_viewed(&mut self, area: ViewedArea) -> Result<(), String> {
        self.to_give(area.area)?;
        self.access.config().knows(&area.require)?;
        for view in &area.views {
            self.access.config().knows(&view.require)?;
        }
        if let Some(fault) = area.fault() {
            return Err(fault);
        }
        self.given.push(Given::Views(Views::new(area)));
        Ok(())
    }

    /// Give a functional area a form for the crew to fill in. Its page is
    /// the HMI's own, and the area has functions from the start.
    pub fn add_form(&mut self, area: FormArea) -> Result<(), String> {
        self.to_give(area.area)?;
        self.access.config().knows(&area.require)?;
        let entry = Entry::new(area.form)?;
        self.given.push(Given::Form { area: area.area, entry, require: area.require });
        Ok(())
    }

    /// Whether a functional area is there to be given away, and if not, why.
    fn to_give(&self, area: u8) -> Result<(), String> {
        let name = AREAS[usize::from(area)];
        if area == SYS {
            return Err(format!("{name} is the HMI's own"));
        }
        if self.given(area).is_some() {
            return Err(format!("{name} already has a program"));
        }
        Ok(())
    }

    fn given(&self, area: u8) -> Option<&Given> {
        self.given.iter().find(|given| given.area() == area)
    }

    /// The forms, whichever areas' they are.
    fn forms(&mut self) -> impl Iterator<Item = &mut Entry> {
        self.given.iter_mut().filter_map(|given| match given {
            Given::Form { entry, .. } => Some(entry),
            Given::Window { .. } | Given::Tiles(_) | Given::Views(_) => None,
        })
    }

    /// The form whose page is the one on screen, if a form's is.
    fn form(&mut self) -> Option<&mut Entry> {
        let (selected, on_screen) = (self.selected, self.menu.is_none());
        self.given.iter_mut().find_map(|given| match given {
            Given::Form { area, entry, .. } if on_screen && *area == selected => Some(entry),
            _ => None,
        })
    }

    /// The window that has the page to itself now, if its program can be
    /// typed into: the one the on-screen keyboard is brought up over. Not a
    /// tile's: with several programs' pictures on the page there is no
    /// saying which of them a key would be for.
    fn typed_into(&self) -> Option<&str> {
        // Nor while a PIN is being asked for: the page is the HMI's then, and
        // what is typed is the PIN, which is no program's.
        if self.menu.is_some() || self.access.asking() {
            return None;
        }
        match self.given(self.selected)? {
            Given::Window { area, .. } => area.keyboard.then_some(area.window.as_str()),
            Given::Views(views) => views.showing().filter(|view| view.keyboard).map(|view| view.window.as_str()),
            Given::Tiles(_) | Given::Form { .. } => None,
        }
    }

    /// The on-screen keyboard that is up over the page on screen, if that
    /// page is another program's with a window to itself - whether or not
    /// it is one the crew can bring a keyboard up over - and the crew has
    /// brought it up there, or the program has asked for it.
    fn typing(&self) -> Option<&Typing> {
        let window = self.window_to_show()?;
        self.typing.iter().find_map(|up| (up.window == window).then_some(&up.typing))
    }

    fn typing_mut(&mut self) -> Option<&mut Typing> {
        let window = self.window_to_show()?.to_string();
        self.typing.iter_mut().find_map(|up| (up.window == window).then_some(&mut up.typing))
    }

    /// The program whose window has the page to itself says that a field of
    /// its page has taken the focus, and what such a field takes - or, with
    /// `None`, that no field of it has the focus now. Returns whether the
    /// frame changed.
    ///
    /// A field that takes the focus has the on-screen keyboard come up over
    /// the page by itself, and one that lets go has it go away again:
    /// the keyboard a form has for a field of that kind. A
    /// program says it again when a field that has the focus is touched
    /// again, which is what brings back a keyboard the crew had put away;
    /// until then it stays away. A keyboard the crew brought up themselves
    /// is not changed by what the program says, nor put away by it. And
    /// nothing a program says brings a keyboard up while the page is not
    /// its own to be seen on: the HMI's pages have none of its fields.
    ///
    /// This is what the display server says, of the window that has the
    /// keyboard's focus: the one on show. A program that says it for itself
    /// is heard by [`Hmi::asked_for`].
    pub fn asked(&mut self, kind: Option<Kind>) -> bool {
        match (kind, self.window_to_show().map(str::to_string)) {
            (Some(kind), Some(window)) => self.bring_up(window, kind, By::Display),
            (Some(_), None) => false,
            // No field has the focus now, in whichever window one had it.
            (None, _) => self.put_away(|up| up.by == By::Display),
        }
    }

    /// The same, said by a program for itself, of the window it names: for
    /// one that cannot say it through the display server. Returns whether
    /// the frame changed.
    ///
    /// The rules are [`Hmi::asked`]'s. That a field has the focus is heard
    /// only while the window has the page to itself: said of a window that
    /// is not to be seen - another area's, or one that a page of the HMI's
    /// own is over, a PIN's among them - it brings no keyboard up, then or
    /// when the window is next on the screen, and the field asks again when
    /// it is touched again. That no field has the focus is heard wherever
    /// the crew are: the keyboard that came up for the window is not there
    /// when it is come back to. A keyboard is put away by whoever brought
    /// it up: this takes away none that the crew did, nor one the display
    /// server asked for.
    pub fn asked_for(&mut self, asking: &Asking) -> bool {
        match asking.kind {
            Some(kind) if self.window_to_show() == Some(asking.window.as_str()) => {
                self.bring_up(asking.window.clone(), kind, By::Program)
            }
            Some(_) => false,
            None => {
                let up = self.typing_into_window();
                self.put_away(|up| up.by == By::Program && up.window == asking.window);
                up != self.typing_into_window()
            }
        }
    }

    /// A field of `window`, which has the page to itself, has the focus:
    /// see that the keyboard for what it takes is up over it. Returns
    /// whether the frame changed.
    fn bring_up(&mut self, window: String, kind: Kind, by: By) -> bool {
        match self.typing.iter_mut().find(|up| up.window == window) {
            Some(up) if up.by == By::Crew || up.typing.kind() == kind => false,
            // On to a field of another kind: its keyboard, in place of this.
            Some(up) => {
                up.typing = Typing::of(kind);
                true
            }
            None => {
                self.typing.push(Up { window, typing: Typing::of(kind), by });
                true
            }
        }
    }

    /// Put away the keyboards that `which` picks out. Returns whether there
    /// was one.
    fn put_away(&mut self, which: impl Fn(&Up) -> bool) -> bool {
        let up = self.typing.len();
        self.typing.retain(|up| !which(up));
        self.typing.len() != up
    }

    /// Whether the on-screen keyboard is up over another program's page
    /// now: what is typed on it is for that program, whose window is the
    /// one [`Hmi::window_to_show`] names.
    pub fn typing_into_window(&self) -> bool {
        self.typing().is_some()
    }

    /// What the crew has typed for the program whose page is on screen since
    /// this was last asked, in the order it was typed. What was typed for a
    /// page that has since gone from the screen is nobody's.
    pub fn take_strokes(&mut self) -> Vec<Stroke> {
        let strokes = std::mem::take(&mut self.strokes);
        if self.typing_into_window() { strokes } else { Vec::new() }
    }

    /// Do what F`n` does to the on-screen keyboard over another program's
    /// page, if the page on screen is such a page with a window to itself.
    /// Returns whether the button was the keyboard's.
    fn type_with(&mut self, n: u8) -> bool {
        use hmi_entry::button::CANCEL;
        use typed_page::KEYBOARD;
        let Some(window) = self.window_to_show().map(str::to_string) else { return false };
        // The crew bring the keyboard up themselves where the page is one
        // that says they can: a program that cannot ask for it needs that.
        let offered = self.typed_into().is_some();
        let up = self.typing.iter().position(|up| up.window == window);
        match (n, up) {
            (KEYBOARD, None) if offered => self.typing.push(Up { window, typing: Typing::new(), by: By::Crew }),
            // The button that brings it up puts it away, and so does F13:
            // it cancels data entry here as it does on a form.
            // Whichever way it came up, it is away at once,
            // and stays away until it is asked for again.
            (KEYBOARD, Some(up)) if offered => {
                self.typing.remove(up);
            }
            (CANCEL, Some(up)) => {
                self.typing.remove(up);
            }
            (_, Some(up)) => {
                let typing = &mut self.typing[up].typing;
                let taken = typing.press(n);
                self.strokes.append(&mut typing.take_strokes());
                return taken;
            }
            (_, None) => return false,
        }
        true
    }

    /// Take what the vehicle has to say. The frame is to be drawn again.
    pub fn report(&mut self, report: Report, now: Time) {
        match report {
            Report::Alarm(alarm) => self.alarms.raise(alarm, now),
            Report::Threat(threat) => self.threats.detect(threat, now),
            Report::Resolved(id) => {
                self.alarms.resolve(&id, now);
                self.threats.end(&id);
            }
            Report::Sensor(sensor) => self.threats.sensor(sensor),
        }
        self.settle();
    }

    /// Bring the table on screen, if there is one, up to date with what it
    /// is a table of.
    fn settle(&mut self) {
        match &mut self.menu {
            Some(Menu::Alarms(table)) => table.settle(&self.alarms),
            Some(Menu::Threats(table)) => table.settle(&self.threats),
            None => {}
        }
        self.keep_out();
    }

    /// See that the crew are nowhere that needs a role nobody logged in
    /// holds - whoever held it having logged out, or left the display alone
    /// too long, or never been there. An area that does is left for SYS, and
    /// a view that does for its area's menu: which is where an area is, too,
    /// whose first view needs a role, until the crew chooses. Returns
    /// whether anything was left.
    fn keep_out(&mut self) -> bool {
        let selected = self.selected;
        let Some(given) = self.given(selected) else { return false };
        if !self.access.allows(given.require()) {
            self.selected = SYS;
            return true;
        }
        let kept_from_view = match given {
            Given::Views(views) => views.viewed().is_some_and(|view| !self.access.allows(views.requires(view))),
            Given::Window { .. } | Given::Tiles(_) | Given::Form { .. } => false,
        };
        if kept_from_view {
            if let Some(Given::Views(views)) = self.given.iter_mut().find(|given| given.area() == selected) {
                views.to_menu();
            }
        }
        kept_from_view
    }

    /// The crew want something that may need a role. It is shown, or done,
    /// if whoever is logged in may have it - which, where it needs none, is
    /// anybody. Otherwise a PIN is asked for it: the page is the PIN's until
    /// the right one is given ([`Hmi::verified`]) or the crew think better
    /// of it.
    fn want(&mut self, wanted: Wanted, now: Time) {
        let (what, require) = match &wanted {
            Wanted::Area(area) => (
                AREAS[usize::from(*area)].to_string(),
                self.given(*area).map_or_else(Require::nothing, |given| given.require().clone()),
            ),
            Wanted::View { area, view } => match self.given(*area) {
                Some(Given::Views(views)) => {
                    (format!("{}: {}", AREAS[usize::from(*area)], views.label(*view)), views.requires(*view).clone())
                }
                _ => return,
            },
            Wanted::Function(function) => {
                let (name, what) = function.names();
                (what.to_string(), self.access.config().function(name).clone())
            }
        };
        if let Asked::Allowed(wanted) = self.access.ask(wanted, &what, &require, now.uptime_ms) {
            self.grant(wanted, now);
        }
    }

    /// Show, or do, what the crew wanted and may have.
    fn grant(&mut self, wanted: Wanted, now: Time) {
        match wanted {
            Wanted::Area(area) => {
                self.selected = area;
                self.menu = None;
            }
            Wanted::View { area, view } => {
                if let Some(Given::Views(views)) = self.given.iter_mut().find(|given| given.area() == area) {
                    views.choose(view);
                }
            }
            Wanted::Function(Function::OverrideAlarm(id)) => self.alarms.override_alarm(&id, now),
            Wanted::Function(Function::ClearAlarm(id)) => self.alarms.clear(&id, now),
            Wanted::Function(Function::ClearAlarms) => self.alarms.clear_all(now),
            Wanted::Function(Function::CancelThreat(id)) => self.threats.cancel(&id),
            Wanted::Function(Function::CancelThreats) => self.threats.cancel_all(),
        }
    }

    /// The PIN the crew have entered, for the verifier to be asked whether
    /// it is the account's: once, and the answer to come back with its
    /// serial ([`Hmi::verified`]). Whether a PIN is right is not known here.
    pub fn take_check(&mut self) -> Option<Check> {
        self.access.take_check()
    }

    /// Take what the verifier said of the PIN handed over under `serial`.
    /// If it is the account's, the account is logged in and what the crew
    /// asked for is shown or done; if not, the PIN page says so. Returns
    /// whether the frame changed.
    pub fn verified(&mut self, serial: u32, answer: Answer, now: Time) -> bool {
        let asking = self.access.asking();
        if let Some(wanted) = self.access.answer(serial, answer, now.uptime_ms) {
            self.grant(wanted, now);
        }
        self.settle();
        asking
    }

    /// Whether a PIN is being asked for: the page is the PIN's, whatever it
    /// came up over ([`Page::Pin`]). While it is, nothing is typed into any
    /// program and no program's keyboard is up - `typed_into` has no
    /// window to name - and no program's window is shown.
    pub fn pin_asked(&self) -> bool {
        self.access.asking()
    }

    /// Whether a button, pressed or let go now, is part of a PIN being
    /// entered: a digit of it, or the taking back of one. That it was
    /// pressed may be written down - on a console, in a log - and which it
    /// was may not: the button has the digit's name on it.
    pub fn unspoken(&self, button: Button) -> bool {
        self.access.asking() && matches!(button, Button::F(1..=11))
    }

    /// The same of something touched: a key of the PIN's pad, or the label
    /// of such a button.
    pub fn untold(&self, touched: &Touched) -> bool {
        self.access.asking()
            && match touched {
                Touched::Key(_) => true,
                Touched::Label(button) => self.unspoken(*button),
                Touched::Row(_) | Touched::Field(_) => false,
            }
    }

    /// Who is logged in.
    pub fn session(&self) -> &Session {
        self.access.session()
    }

    /// What has happened about access since this was last asked, to be
    /// written down: never a PIN.
    pub fn take_audit(&mut self) -> Vec<Audit> {
        self.access.take_audit()
    }

    /// Time has passed. Returns whether the frame changed with it, other
    /// than in its date-time group.
    pub fn tick(&mut self, now: Time) -> bool {
        let mut changed = self.alarms.tick(now);
        // Whoever was logged in may no longer be, the display having been
        // left alone; a PIN page nobody answered may have gone.
        changed |= self.access.tick(now.uptime_ms);
        self.settle();
        changed
    }

    /// When [`Hmi::tick`] is next wanted, by the forward-running clock
    /// ([`Time::uptime_ms`]), if it is.
    pub fn deadline(&self) -> Option<u64> {
        [self.alarms.deadline(), self.access.deadline()].into_iter().flatten().min()
    }

    /// What the crew has done about alarms and threats since this was last
    /// asked, for whatever told of them to be told - and what they have
    /// entered into a form and saved, for whatever the form is for.
    pub fn take_requests(&mut self) -> Vec<Request> {
        let mut requests = self.alarms.take_requests();
        requests.append(&mut self.threats.take_requests());
        for entry in self.forms() {
            requests.extend(entry.take_saved().into_iter().map(Request::Entered));
        }
        requests
    }

    /// Take a button event. Returns whether the frame changed - or whether
    /// something was typed for another program ([`Hmi::take_strokes`]),
    /// which is as much the caller's to act on.
    pub fn button(&mut self, event: ButtonEvent, now: Time) -> bool {
        // A button that is a digit of a PIN is not remembered: the SYS page
        // says which button was last pressed.
        let secret = self.unspoken(event.button);
        let changed = secret || self.last_button != Some(event);
        if !secret {
            self.last_button = Some(event);
        }
        if event.action == Action::Pressed {
            // The display is not being left alone.
            self.access.active(now.uptime_ms);
            self.pressed(event.button, now);
            self.settle();
        } else if let Button::F(n) = event.button {
            // A key of the on-screen keyboard is lit for as long as the
            // button that selected it is down: whatever page
            // the button comes up on.
            self.forms().for_each(|entry| entry.release(n));
            self.typing.iter_mut().for_each(|up| up.typing.release(n));
            self.access.release(n);
            if let Some((area, _, id)) = self.held.take_if(|(_, held, _)| *held == n) {
                self.to_apps.push((area, ToApp::Released(id)));
            }
        }
        changed || !self.strokes.is_empty()
    }

    fn pressed(&mut self, button: Button, now: Time) {
        // A PIN being asked for has the buttons first. Four are never its
        // own: the Alarms page and the Threats page come up in its place,
        // and an alert is acknowledged and the labels hidden over it.
        // An area's button is the crew going
        // elsewhere, and the PIN is no longer asked for.
        if self.access.asking() {
            match button {
                Button::F(n) => match self.access.press(n, now.uptime_ms) {
                    Pressed::Taken | Pressed::Cancelled => return,
                    Pressed::Passed if matches!(n, 14 | 15) => self.access.cancel(),
                    Pressed::Passed => {}
                },
                Button::Area(_) => self.access.cancel(),
            }
        }
        match button {
            // An area that is unavailable or absent stays unselected.
            // Choosing an area leaves the Alarms page
            // or the Threats page. Another area's button brings that area
            // up as it was left. The selected area's own
            // brings up its top level menu - which, for an
            // area that has one page and no more, is the page it has.
            Button::Area(area) => {
                if area == SYS || self.given(area).is_some_and(Given::present) {
                    if area == self.selected {
                        if let Some(Given::Views(views)) = self.given.iter_mut().find(|given| given.area() == area) {
                            views.to_menu();
                        }
                        self.menu = None;
                    } else {
                        // Another area may need a role: it is selected if
                        // whoever is logged in may have it, and otherwise
                        // its PIN is asked for first.
                        self.want(Wanted::Area(area), now);
                    }
                }
            }
            // Each of the two pages opens as it is first seen, and stays as
            // the crew has set it for as long as it is up.
            Button::F(14) => {
                if !matches!(self.menu, Some(Menu::Alarms(_))) {
                    self.menu = Some(Menu::Alarms(alarms::Table::default()));
                }
            }
            Button::F(15) => {
                if !matches!(self.menu, Some(Menu::Threats(_))) {
                    self.menu = Some(Menu::Threats(threats::Table::default()));
                }
            }
            // F16 acknowledges alerts: the alarm and the
            // threat whose messages are on the screen.
            Button::F(16) => {
                self.alarms.acknowledge_presented(now);
                self.threats.acknowledge_presented();
            }
            // F19 hides, a press at a time, and then shows again.
            // The buttons go on doing what they did
            // while their labels are out of sight.
            Button::F(LABELS) => self.hiding = self.hiding.next(self.ppi().is_some()),
            // While the on-screen keyboard is up over another program's
            // page the buttons of data entry are its own, and the one that
            // brings it up is there whenever such a page is on screen.
            Button::F(n) if self.type_with(n) => {}
            Button::F(n) => {
                // What a button asks for that may need a role, to be seen to
                // once it is known what the button was for.
                let mut wanted = None;
                let left = match &mut self.menu {
                    // The page is the selected area's, and so are the buttons
                    // down the sides: where the area is one of
                    // tiles, each has a window to show or to stop showing.
                    None => {
                        let selected = self.selected;
                        let given = self.given.iter_mut().find(|given| given.area() == selected);
                        match given {
                            Some(Given::Tiles(tiles)) => tiles.press(n),
                            // An area of views has a view on each of the
                            // buttons while its menu is up, and F13 to go
                            // up to the menu from a view.
                            Some(Given::Views(views)) => match views.chooses(n) {
                                Some(view) => wanted = Some(Wanted::View { area: selected, view }),
                                None => views.press(n),
                            },
                            // A program's own buttons are its, while they
                            // can do anything.
                            Some(Given::Window { area, .. }) => {
                                let key = area.keys.iter().find(|key| key.n() == n);
                                if let Some(key) = key.filter(|key| matches!(key.state, LabelState::Available | LabelState::Selected)) {
                                    self.to_apps.push((selected, ToApp::Pressed(key.id.clone())));
                                    self.held = Some((selected, n, key.id.clone()));
                                }
                            }
                            // A form has the buttons its labels say, and
                            // while data is being entered the keyboard's.
                            Some(Given::Form { entry, .. }) => entry.press(n),
                            // The SYS page, which is the HMI's own.
                            None if n == sys_page::TOUCH => self.touch = !self.touch,
                            None if n == sys_page::LOG_OUT => self.access.leave(),
                            None => {}
                        }
                        false
                    }
                    Some(Menu::Alarms(table)) => {
                        use alarms_page::*;
                        match (n, browse(table, &self.alarms, n)) {
                            (_, Browsed::Left) => true,
                            (_, Browsed::Done) => false,
                            (n, Browsed::Not(picked)) => {
                                match (n, picked) {
                                    (SORT_BY_CATEGORY, _) => table.sort = alarms::Sort::Category,
                                    (SORT_BY_TIME, _) => table.sort = alarms::Sort::Time,
                                    (ACKNOWLEDGED, _) => table.acknowledged = !table.acknowledged,
                                    (ACKNOWLEDGE_ALL, _) => self.alarms.acknowledge_all(now),
                                    (ACKNOWLEDGE, Some(id)) => self.alarms.acknowledge(&id, now),
                                    // What takes an alarm off the list, or
                                    // silences its source, can be kept for
                                    // a role - where there is one to do it
                                    // to.
                                    (CLEAR_ALL, _) if !self.alarms.is_empty() => {
                                        wanted = Some(Wanted::Function(Function::ClearAlarms));
                                    }
                                    (CLEAR, Some(id)) => wanted = Some(Wanted::Function(Function::ClearAlarm(id))),
                                    (OVERRIDE, Some(id)) if table.can(&self.alarms).1 => {
                                        wanted = Some(Wanted::Function(Function::OverrideAlarm(id)));
                                    }
                                    _ => {}
                                }
                                false
                            }
                        }
                    }
                    Some(Menu::Threats(table)) => {
                        use table_page::{PAGE_DOWN, PAGE_UP};
                        use threats_page::*;
                        // On the picture every listed threat is there at
                        // once: there is no order to put them in and no page
                        // to turn.
                        let idle = table.plotted && matches!(n, SORT_BY_TYPE | SORT_BY_TIME | PAGE_UP | PAGE_DOWN);
                        match (n, if idle { Browsed::Done } else { browse(table, &self.threats, n) }) {
                            (_, Browsed::Left) => true,
                            (_, Browsed::Done) => false,
                            (n, Browsed::Not(picked)) => {
                                match (n, picked) {
                                    (SORT_BY_TYPE, _) => table.sort = threats::Sort::Type,
                                    (SORT_BY_TIME, _) => table.sort = threats::Sort::Time,
                                    (PAST, _) => table.past = !table.past,
                                    (VIEW, _) => table.plotted = !table.plotted,
                                    // So can what takes a threat off
                                    // its list.
                                    (CANCEL_ALL, _) if !self.threats.is_empty() => {
                                        wanted = Some(Wanted::Function(Function::CancelThreats));
                                    }
                                    (CANCEL, Some(id)) => wanted = Some(Wanted::Function(Function::CancelThreat(id))),
                                    _ => {}
                                }
                                false
                            }
                        }
                    }
                };
                if left {
                    self.menu = None;
                }
                if let Some(wanted) = wanted {
                    self.want(wanted, now);
                }
            }
        }
    }

    /// Take a touch on something the frame had drawn. Returns whether the
    /// frame changed. A label does what its button does, pressed and let go.
    /// A row of a table, or a threat's symbol on the threats'
    /// picture, is picked, and touched again is shown at length - as F17,
    /// F18 and F20 would have done it. A field of a form is entered into,
    /// and a key of the on-screen keyboard selected - which, where the
    /// keyboard is up over another program's page, types for that program
    /// ([`Hmi::take_strokes`]): that too is told of as a change.
    pub fn touch(&mut self, touched: Touched, now: Time) -> bool {
        // With the touch screen off, nothing touched does anything.
        if !self.touch {
            return false;
        }
        self.access.active(now.uptime_ms);
        match touched {
            // A label that F19 has hidden is not on the screen to be touched.
            Touched::Label(_) if self.hiding != Hiding::Nothing => false,
            Touched::Label(button) => {
                let pressed = self.button(ButtonEvent { button, action: Action::Pressed }, now);
                self.button(ButtonEvent { button, action: Action::Released }, now) || pressed
            }
            Touched::Row(name) => {
                let picked = match &mut self.menu {
                    // A row shown at length is a dialog, and the table under
                    // it is not to be touched through it.
                    Some(Menu::Alarms(table)) if !table.showing() => table.pick(&name, &self.alarms),
                    Some(Menu::Threats(table)) if !table.showing() => table.pick(&name, &self.threats),
                    _ => false,
                };
                self.settle();
                picked
            }
            // A field of a form is one to enter data into, and a key of the
            // on-screen keyboard is a key selected - as F17, F18 and F20
            // would have done either. Neither is there to be touched unless
            // the form's page is the one on screen.
            // But while a PIN is asked for, a key is the PIN pad's, and what
            // is told of as a field is an account whose PIN it could be.
            Touched::Field(number) if self.access.asking() => {
                self.access.touch(hmi_access::Touched::Account(number), now.uptime_ms);
                true
            }
            Touched::Key(key) if self.access.asking() => {
                self.access.touch(hmi_access::Touched::Key(key), now.uptime_ms);
                true
            }
            Touched::Field(number) => self.touch_form(hmi_entry::Touched::Field(number)),
            Touched::Key(key) => match self.typing_mut() {
                Some(typing) => {
                    typing.touch(key);
                    let mut typed = typing.take_strokes();
                    self.strokes.append(&mut typed);
                    true
                }
                None => self.touch_form(hmi_entry::Touched::Key(key)),
            },
        }
    }

    /// Take a touch on the form whose page is on screen. Returns whether the
    /// frame changed.
    fn touch_form(&mut self, touched: hmi_entry::Touched) -> bool {
        let Some(entry) = self.form() else { return false };
        let before = entry.page();
        entry.touch(touched);
        before != entry.page()
    }

    /// The finger that touched something has gone. Returns whether the frame
    /// changed. A label or a row of a table acted when the finger landed and
    /// has nothing to end; a key of the on-screen keyboard is lit for as long
    /// as it is touched, and this is the end of that.
    pub fn lift(&mut self) -> bool {
        // With the touch screen off no finger selected anything, and a key
        // that a button is holding down is the button's to let go.
        if !self.touch {
            return false;
        }
        let mut changed = false;
        for entry in self.forms() {
            let before = entry.page();
            entry.lift();
            changed |= before != entry.page();
        }
        for up in &mut self.typing {
            let before = up.typing.view();
            up.typing.lift();
            changed |= before != up.typing.view();
        }
        if self.access.asking() {
            self.access.lift();
            changed = true;
        }
        changed
    }

    /// Take the windows now open, by the names their programs give them.
    /// Returns whether the frame changed. An area whose program has no window
    /// is unavailable, and so is one of tiles or of views none of whose
    /// windows is open; if it was the one on screen, SYS takes its place. A
    /// window that had a tile and has gone leaves the tiles to those that
    /// are left, and one that was the view the selected area was at leaves
    /// that area at its menu.
    pub fn windows<S: AsRef<str>>(&mut self, open: &[S]) -> bool {
        let selected = self.selected;
        let viewing = |hmi: &Hmi| matches!(hmi.given(selected), Some(Given::Views(views)) if views.showing().is_some());
        let was_viewing = viewing(self);
        let mut changed = false;
        for given in &mut self.given {
            changed |= given.windows(open);
        }
        // A view that goes from under the crew's eyes does not have the page
        // again by itself when its window is back: the crew chose nothing.
        if was_viewing && !viewing(self) {
            if let Some(Given::Views(views)) = self.given.iter_mut().find(|given| given.area() == selected) {
                views.to_menu();
            }
        }
        if self.given(self.selected).is_some_and(|given| !given.present()) {
            self.selected = SYS;
        }
        // A window that has gone takes with it the keyboard that was up
        // over it: its program, started again, is not where it was.
        self.typing.retain(|up| open.iter().any(|open| open.as_ref() == up.window));
        // A view that needs a role does not have the page for its window's
        // having opened.
        changed |= self.keep_out();
        changed
    }

    /// The window that has the page to itself now: that of the selected
    /// area's program, where the area has been given to one. `None` while
    /// the page on screen is the HMI's own - the Alarms page and the Threats
    /// page are, whichever area is selected.
    pub fn window_to_show(&self) -> Option<&str> {
        match self.windows_to_show().as_slice() {
            [Shown { window, at: None }] => Some(window),
            _ => None,
        }
    }

    /// The windows that belong under the HMI's now, and nowhere else: the
    /// one that has the selected area's page to itself, or those that have
    /// its tiles, in the order they were picked. None while the page on
    /// screen is the HMI's own.
    pub fn windows_to_show(&self) -> Vec<Shown<'_>> {
        // A PIN page is the HMI's own, whatever it is over.
        if self.access.asking() {
            return Vec::new();
        }
        match (&self.menu, self.given(self.selected)) {
            (None, Some(Given::Window { area, .. })) => vec![Shown { window: &area.window, at: None }],
            (None, Some(Given::Tiles(tiles))) => {
                tiles.shown(self.hidden()).map(|(window, tile)| Shown { window, at: Some(tile.at) }).collect()
            }
            // The view the crew is viewing, which has the page to itself;
            // none while the area's menu has it.
            (None, Some(Given::Views(views))) => {
                views.showing().map(|view| Shown { window: &view.window, at: None }).into_iter().collect()
            }
            _ => Vec::new(),
        }
    }

    /// What F19 has taken off the screen for now, and given up the room of:
    /// a picture grows into it, whether it has the page or a tile of it.
    fn hidden(&self) -> Hidden {
        Hidden { labels: self.hiding >= Hiding::Labels, status: self.hiding >= Hiding::Status }
    }

    /// The frame to draw: the selected area's page, or the Alarms page or
    /// the Threats page over it, with the labels of what the buttons do
    /// there. `rows`, the caller's, go in the middle of the SYS page.
    pub fn frame(&self, now: Time, mut rows: Vec<Row>) -> Frame {
        let areas = (0u8..)
            .zip(AREAS)
            .map(|(i, id)| {
                let has_functions = match self.given(i) {
                    // What needs a role that nobody at this crew station
                    // holds is not to be had, whoever asks.
                    Some(given) => Some(given.present() && self.access.possible(given.require())),
                    None => (i == SYS).then_some(true),
                };
                let state = match has_functions {
                    Some(true) if i == self.selected => AreaState::Selected,
                    Some(true) => AreaState::Available,
                    Some(false) => AreaState::Unavailable,
                    None => AreaState::Absent,
                };
                Area { id, state }
            })
            .collect();

        let page = match (&self.menu, self.given(self.selected)) {
            (Some(Menu::Alarms(table)), _) => {
                let (page, pages, rows) = table.rows(&self.alarms);
                Page::Alarms {
                    unacknowledged: self.alarms.acknowledged(false),
                    acknowledged: self.alarms.acknowledged(true),
                    page,
                    pages,
                    rows,
                    detail: table.detail(&self.alarms),
                }
            }
            (Some(Menu::Threats(table)), _) => {
                let view = if table.plotted {
                    ThreatsView::Plot(table.plot(&self.threats))
                } else {
                    let (page, pages, rows) = table.rows(&self.threats);
                    ThreatsView::Table { page, pages, rows }
                };
                Page::Threats {
                    counts: self.threats.counts(),
                    sensors: self.threats.sensors().to_vec(),
                    view,
                    detail: table.detail(&self.threats),
                }
            }
            (None, Some(Given::Window { area, .. })) => {
                Page::External { overlay: area.overlay.clone(), keyboard: self.typing().map(Typing::view) }
            }
            (None, Some(Given::Tiles(tiles))) => {
                Page::Tiles { tiles: tiles.shown(self.hidden()).map(|(_, tile)| tile).collect() }
            }
            // An area of views: the view the crew is viewing, which is its
            // program's picture, or the area's top level menu, which is a
            // page of the HMI's own and says what the area has.
            (None, Some(Given::Views(views))) => match views.showing() {
                Some(view) => {
                    Page::External { overlay: view.overlay.clone(), keyboard: self.typing().map(Typing::view) }
                }
                None => Page::Own { title: AREAS[usize::from(views.area())].to_string(), rows: views.rows() },
            },
            (None, Some(Given::Form { entry, .. })) => Page::Form(entry.page()),
            (None, None) => {
                // Who is logged in, where there are roles to be logged in
                // with: nobody, until something that needs one is gone to.
                if self.access.config().has_roles() {
                    rows.push(Row::new("Logged in", self.access.session().account().unwrap_or("nobody")));
                }
                rows.push(Row::new("Touch screen", if self.touch { "On" } else { "Off" }));
                rows.push(Row::new(
                    "Last button",
                    self.last_button.map_or_else(|| "none".to_string(), |e| e.to_string()),
                ));
                Page::Own { title: "System".to_string(), rows }
            }
        };
        // A PIN being asked for is a dialog, over whatever the page was.
        let page = self.access.page(now.uptime_ms).map_or(page, Page::Pin);

        Frame {
            areas,
            status: Status {
                dtg: date_time_group(now.unix_s),
                position: None,
                heading_mils: None,
                alarms: self.alarms.active(),
            },
            labels: self.labels(now),
            alarm: self.alarms.message(),
            threat: self.threats.message(),
            // The Plan Position Indicator is an overlay that is not
            // critical: F19 hides it last.
            ppi: if self.hiding == Hiding::Overlays { None } else { self.ppi() },
            page,
            hidden: self.hidden(),
            touch: self.touch,
        }
    }

    /// The Plan Position Indicator, where the page on screen has it. It is on
    /// the HMI's own area pages and beside the threats table, with the
    /// current threats marked on it. It is on a page of tiles too, which is
    /// the HMI's to lay out, and where the pictures are of what is round the
    /// vehicle it says which way that is. Over a program's
    /// picture that has the page to itself it is not, until there is a
    /// threat to mark: then it is there too. The Alarms page
    /// is a table and nothing else, and the threats' picture is the
    /// indicator writ large and has no need of it.
    fn ppi(&self) -> Option<Ppi> {
        let picked = match &self.menu {
            Some(Menu::Threats(table)) => table.picked(&self.threats),
            _ => None,
        };
        let threats = self.threats.marks(picked);
        let there = match (&self.menu, self.given(self.selected)) {
            (Some(Menu::Alarms(_)), _) => false,
            (Some(Menu::Threats(table)), _) => !table.plotted,
            (None, Some(Given::Window { .. })) => !threats.is_empty(),
            // A view is a program's picture as much; the menu it is chosen
            // from is the HMI's own page.
            (None, Some(Given::Views(views))) => views.showing().is_none() || !threats.is_empty(),
            // A form's page is the HMI's own, and leaves the indicator its
            // corner.
            (None, Some(Given::Form { .. })) => true,
            (None, Some(Given::Tiles(_)) | None) => true,
        };
        there.then_some(Ppi { threats })
    }

    /// The twenty labels: down the sides what the page on screen gives the
    /// buttons to do, along the bottom the common functions.
    fn labels(&self, now: Time) -> Vec<Label> {
        use LabelState::{Available, Blank, Selected, Unavailable};
        use alarms_page::*;

        let mut labels: Vec<Label> = (1..=12)
            .map(|n| Label { key: format!("F{n}"), text: String::new(), state: Blank })
            .chain(COMMON_LABELS.iter().zip(13..).map(|(text, n)| Label {
                key: format!("F{n}"),
                text: (*text).to_string(),
                state: Available,
            }))
            .collect();
        let mut set = |n: u8, text: &str, state: LabelState| {
            let label = &mut labels[usize::from(n) - 1];
            (label.text, label.state) = (text.to_string(), state);
        };

        // A label says what its button will do, and is grey while there is
        // nothing for it to do that to - which is all of
        // those down the sides while a row is shown at length.
        match &self.menu {
            Some(Menu::Alarms(table)) => {
                let able = |able: bool| if able && !table.showing() { Available } else { Unavailable };
                let chosen = |chosen: bool| if chosen && !table.showing() { Selected } else { able(true) };
                let picked = table.selected(&self.alarms).is_some();
                let (acknowledge, to_override) = table.can(&self.alarms);
                let (before, after) = table.more_pages(&self.alarms);
                set(SORT_BY_CATEGORY, "Sort By\nCategory", chosen(table.sort == alarms::Sort::Category));
                set(SORT_BY_TIME, "Sort By\nTime", chosen(table.sort == alarms::Sort::Time));
                set(ACKNOWLEDGED, if table.acknowledged { "Hide\nAcked" } else { "Show\nAcked" }, able(true));
                set(DETAIL, "Detail", able(picked));
                set(PAGE_UP, "Page Up", able(before));
                set(PAGE_DOWN, "Page Down", able(after));
                set(ACKNOWLEDGE, "Ack\nSelected", able(acknowledge));
                set(ACKNOWLEDGE_ALL, "Ack All", able(self.alarms.acknowledged(false).total() > 0));
                set(CLEAR, "Clear\nSelected", able(picked));
                set(CLEAR_ALL, "Clear All", able(!self.alarms.is_empty()));
                set(OVERRIDE, "Override\nSelected", able(to_override));
                // F13 cancels a dialog and otherwise goes up a level.
                set(13, if table.showing() { "Cancel" } else { "Up" }, Available);
                set(14, COMMON_LABELS[1], Selected);
            }
            Some(Menu::Threats(table)) => {
                use threats_page::{CANCEL, CANCEL_ALL, PAST, SORT_BY_TYPE, VIEW};
                let able = |able: bool| if able && !table.showing() { Available } else { Unavailable };
                let chosen = |chosen: bool| if chosen && !table.showing() { Selected } else { able(true) };
                let picked = table.selected(&self.threats).is_some();
                let (before, after) = table.more_pages(&self.threats);
                // The picture has no order and no pages.
                let tabled = !table.plotted;
                let sorted = |by: threats::Sort| if tabled { chosen(table.sort == by) } else { Unavailable };
                set(SORT_BY_TYPE, "Sort By\nType", sorted(threats::Sort::Type));
                set(SORT_BY_TIME, "Sort By\nTime", sorted(threats::Sort::Time));
                set(PAST, if table.past { "Hide\nPast" } else { "Show\nPast" }, able(true));
                set(DETAIL, "Detail", able(picked));
                set(PAGE_UP, "Page Up", able(tabled && before));
                set(PAGE_DOWN, "Page Down", able(tabled && after));
                set(CANCEL, "Cancel\nSelected", able(picked));
                set(CANCEL_ALL, "Cancel All", able(!self.threats.is_empty()));
                set(VIEW, if table.plotted { "View\nTable" } else { "View\nPPI" }, able(true));
                set(13, if table.showing() { "Cancel" } else { "Up" }, Available);
                set(15, COMMON_LABELS[2], Selected);
            }
            None => match self.given(self.selected) {
                // An area of tiles has a window on each of its buttons.
                Some(Given::Tiles(tiles)) => tiles.labels().for_each(|(n, text, state)| set(n, text, state)),
                // An area of views has a view on each of its buttons while
                // its menu is the page, and none while a view is.
                Some(Given::Views(views)) => {
                    views.labels().iter().for_each(|(n, text, state)| set(*n, text, *state));
                }
                // A program's buttons say what it has said they do.
                Some(Given::Window { area, .. }) => area.keys.iter().for_each(|key| set(key.n(), &key.label, key.state)),
                // A form says what its buttons do, and which of them can do
                // anything now. F13 is among them while data is being
                // entered: it is 'Cancel' then.
                Some(Given::Form { entry, .. }) => entry
                    .labels()
                    .for_each(|(n, text, able)| set(n, text, if able { Available } else { Unavailable })),
                // The SYS page: the touch screen can be put off, and on
                // again. The label says which a press will do.
                None => {
                    set(sys_page::TOUCH, if self.touch { "Touch\nOff" } else { "Touch\nOn" }, Available);
                    // Where there are roles, whoever is logged in can
                    // log out: nobody is, then, until a PIN is next given.
                    if self.access.config().has_roles() {
                        let somebody = self.access.session().account().is_some();
                        set(sys_page::LOG_OUT, "Log Out", if somebody { Available } else { Unavailable });
                    }
                }
            },
        }
        // While the on-screen keyboard is up over another program's page
        // the buttons of data entry say what they do: F13 among them, which
        // is 'Cancel' then. And where the page is one the crew can bring it
        // up over themselves, it is on a button.
        let up = self.typing();
        let able = |able| if able { Available } else { Unavailable };
        up.into_iter().flat_map(Typing::labels).for_each(|(n, text, can)| set(n, text, able(can)));
        if self.typed_into().is_some() {
            set(typed_page::KEYBOARD, "Keyboard", if up.is_some() { Selected } else { Available });
        }
        // A PIN page has the buttons down the sides to itself: the digits
        // of its pad, and Backspace. Of the common ones it says which it
        // uses; F13 is 'Cancel', as in any dialog.
        let pin = self.access.labels(now.uptime_ms);
        if !pin.is_empty() {
            (1..=12).for_each(|n| set(n, "", Blank));
            pin.into_iter().for_each(|(n, text, able)| set(n, text, if able { Available } else { Unavailable }));
        }
        labels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A moment `ms` after the tests' beginning of time.
    fn at(ms: u64) -> Time {
        Time { unix_s: 1_728_045_000 + ms / 1000, uptime_ms: ms }
    }

    fn frame() -> Frame {
        Hmi::new().frame(at(0), Vec::new())
    }

    fn press(hmi: &mut Hmi, button: Button) {
        hmi.button(ButtonEvent { button, action: Action::Pressed }, at(0));
    }

    fn states(hmi: &Hmi) -> Vec<AreaState> {
        hmi.frame(at(0), Vec::new()).areas.iter().map(|a| a.state).collect()
    }

    fn label(hmi: &Hmi, n: u8) -> (String, LabelState) {
        let label = hmi.frame(at(0), Vec::new()).labels.swap_remove(usize::from(n) - 1);
        assert_eq!(label.key, format!("F{n}"));
        (label.text, label.state)
    }

    fn raise(hmi: &mut Hmi, id: &str, category: &str, ms: u64) {
        let line = format!(r#"{{"alarm":{{"id":"{id}","category":"{category}","subsystem":"{id}"}}}}"#);
        hmi.report(Report::parse(&line).unwrap(), at(ms));
    }

    /// The Alarms page of an HMI that is showing it.
    fn alarms_page(hmi: &Hmi) -> (AlarmCounts, AlarmCounts, usize, usize, Vec<AlarmRow>, Option<AlarmDetail>) {
        match hmi.frame(at(0), Vec::new()).page {
            Page::Alarms { unacknowledged, acknowledged, page, pages, rows, detail } => {
                (unacknowledged, acknowledged, page, pages, rows, detail)
            }
            other => panic!("not the Alarms page: {other:?}"),
        }
    }

    fn rows(hmi: &Hmi) -> Vec<String> {
        alarms_page(hmi).4.into_iter().map(|row| row.text).collect()
    }

    /// Give the HMI a line of the feed, `ms` after the tests' beginning of time.
    fn feed(hmi: &mut Hmi, line: &str, ms: u64) {
        hmi.report(Report::parse(line).unwrap(), at(ms));
    }

    /// The Threats page of an HMI that is showing it.
    fn threats_page(hmi: &Hmi) -> (ThreatCounts, Vec<Sensor>, usize, usize, Vec<ThreatRow>, Option<ThreatDetail>) {
        match hmi.frame(at(0), Vec::new()).page {
            Page::Threats { counts, sensors, view: ThreatsView::Table { page, pages, rows }, detail } => {
                (counts, sensors, page, pages, rows, detail)
            }
            other => panic!("not the Threats page's table: {other:?}"),
        }
    }

    fn threat_rows(hmi: &Hmi) -> Vec<String> {
        threats_page(hmi).4.into_iter().map(|row| row.text).collect()
    }

    /// An HMI that has been told of a laser and, a minute earlier, a gunshot
    /// that matters less.
    fn with_threats() -> Hmi {
        let mut hmi = Hmi::new();
        feed(&mut hmi, r#"{"threat":{"id":"shot","type":"Gunshot","priority":"low","bearing":5300}}"#, 0);
        feed(
            &mut hmi,
            r#"{"threat":{"id":"lws","type":"Laser","description":"Rangefinder","bearing":800,"range":1200}}"#,
            60_000,
        );
        hmi
    }

    /// An HMI whose WPN and BMS areas are other programs' windows.
    fn with_external_areas() -> Hmi {
        let mut hmi = Hmi::new();
        for (area, window, overlay) in [(1, "sight", Some("data:,reticle")), (7, "map", None)] {
            let overlay = overlay.map(str::to_string);
            hmi.add_external(ExternalArea { area, window: window.to_string(), overlay, keyboard: false, require: Require::nothing(), socket: None, keys: Vec::new() }).unwrap();
        }
        hmi
    }

    #[test]
    fn areas_are_in_bezel_order() {
        let ids: Vec<_> = frame().areas.iter().map(|a| a.id).collect();
        assert_eq!(ids, ["SA", "WPN", "DEF", "SYS", "DRV", "STR", "COM", "BMS"]);
    }

    #[test]
    fn exactly_one_area_is_selected() {
        let selected: Vec<_> = frame()
            .areas
            .iter()
            .filter(|a| a.state == AreaState::Selected)
            .map(|a| a.id)
            .collect();
        assert_eq!(selected, ["SYS"]);
    }

    #[test]
    fn one_label_per_button_f1_to_f20() {
        let keys: Vec<_> = frame().labels.iter().map(|l| l.key.clone()).collect();
        let expected: Vec<_> = (1..=20).map(|n| format!("F{n}")).collect();
        assert_eq!(keys, expected);
    }

    #[test]
    fn to_142_common_function_labels() {
        let f = frame();
        let text = |key: &str| f.labels.iter().find(|l| l.key == key).unwrap().text.clone();
        assert_eq!(text("F13"), "Up");
        assert_eq!(text("F14"), "Alarms");
        assert_eq!(text("F15"), "Threats");
        assert_eq!(text("F16"), "Ack");
        // Not the 'Arrow Up' and 'Arrow Down' of See COMMON_LABELS.
        assert_eq!(text("F17"), "Up");
        assert_eq!(text("F18"), "Down");
        assert_eq!(text("F19"), "Labels");
        assert_eq!(text("F20"), "Enter");
    }

    #[test]
    fn twenty_eight_buttons_named_as_the_standard_names_them() {
        let names: Vec<_> = Button::all().map(|b| b.to_string()).collect();
        assert_eq!(names.len(), 28);
        assert_eq!(names[..8], ["SA", "WPN", "DEF", "SYS", "DRV", "STR", "COM", "BMS"]);
        assert_eq!(names[8], "F1");
        assert_eq!(names[27], "F20");
        assert_eq!(Button::area(8), None);
        assert_eq!(Button::f(0), None);
        assert_eq!(Button::f(21), None);
    }

    #[test]
    fn to_220_374_area_states_follow_the_programs_windows() {
        use AreaState::{Absent, Available, Selected, Unavailable};
        let mut hmi = with_external_areas();
        // No window yet: the areas exist on this platform but cannot be used.
        assert_eq!(
            states(&hmi),
            [Absent, Unavailable, Absent, Selected, Absent, Absent, Absent, Unavailable]
        );
        assert!(hmi.windows(&["sight", "something else"]));
        assert_eq!(states(&hmi)[1], Available);
        assert_eq!(states(&hmi)[7], Unavailable);
        assert!(!hmi.windows(&["sight"]), "the same windows again change nothing");
    }

    #[test]
    fn an_area_button_selects_an_area_that_has_functions() {
        let mut hmi = with_external_areas();
        hmi.windows(&["sight"]);

        press(&mut hmi, Button::Area(1));
        assert_eq!(states(&hmi)[1], AreaState::Selected);
        assert_eq!(states(&hmi)[SYS as usize], AreaState::Available);
        assert_eq!(hmi.window_to_show(), Some("sight"));
        let Page::External { overlay, .. } = hmi.frame(at(0), Vec::new()).page else {
            panic!("WPN is another program's page");
        };
        assert_eq!(overlay.as_deref(), Some("data:,reticle"));

        // BMS has no window and DEF no functions: WPN stays.
        press(&mut hmi, Button::Area(7));
        press(&mut hmi, Button::Area(2));
        assert_eq!(hmi.window_to_show(), Some("sight"));
        // Letting go of a button selects nothing.
        hmi.button(ButtonEvent { button: Button::Area(SYS), action: Action::Released }, at(0));
        assert_eq!(hmi.window_to_show(), Some("sight"));

        press(&mut hmi, Button::Area(SYS));
        assert_eq!(hmi.window_to_show(), None);
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Own { .. }));
    }

    #[test]
    fn sys_takes_over_when_the_selected_areas_window_goes() {
        let mut hmi = with_external_areas();
        hmi.windows(&["sight", "map"]);
        press(&mut hmi, Button::Area(7));
        assert_eq!(hmi.window_to_show(), Some("map"));

        assert!(hmi.windows(&["sight"]));
        assert_eq!(hmi.window_to_show(), None);
        assert_eq!(states(&hmi)[SYS as usize], AreaState::Selected);
        assert_eq!(states(&hmi)[7], AreaState::Unavailable);
        // Nor does it come back by itself: the crew chose nothing.
        hmi.windows(&["sight", "map"]);
        assert_eq!(hmi.window_to_show(), None);
    }

    #[test]
    fn an_area_is_given_away_once_and_sys_never() {
        let mut hmi = with_external_areas();
        let area = |area| ExternalArea { area, window: "w".to_string(), overlay: None, keyboard: false, require: Require::nothing(), socket: None, keys: Vec::new() };
        assert!(hmi.add_external(area(1)).unwrap_err().contains("WPN"));
        assert!(hmi.add_external(area(SYS)).unwrap_err().contains("SYS"));
        assert!(hmi.add_external(area(0)).is_ok());
    }

    #[test]
    fn a_button_event_reaches_the_frame() {
        let mut hmi = Hmi::new();
        let last = |hmi: &Hmi| match hmi.frame(at(0), Vec::new()).page {
            Page::Own { rows, .. } => rows.last().unwrap().value.clone(),
            other => panic!("SYS is the HMI's own page: {other:?}"),
        };
        assert_eq!(last(&hmi), "none");

        let event = ButtonEvent { button: Button::F(1), action: Action::Pressed };
        assert!(hmi.button(event, at(0)));
        assert_eq!(last(&hmi), "F1 pressed");
        assert!(!hmi.button(event, at(0)), "the same event again changes nothing");

        assert!(hmi.button(ButtonEvent { button: Button::Area(0), action: Action::Released }, at(0)));
        assert_eq!(last(&hmi), "SA released");
    }

    #[test]
    fn alarms_are_counted_on_the_status_bar() {
        let mut hmi = Hmi::new();
        assert_eq!(frame().status.alarms, AlarmCounts::default());
        raise(&mut hmi, "oil", "warning", 0);
        raise(&mut hmi, "fuel", "caution", 0);
        raise(&mut hmi, "tyre", "caution", 0);
        raise(&mut hmi, "service", "advisory", 0);
        let counts = hmi.frame(at(0), Vec::new()).status.alarms;
        assert_eq!((counts.warnings, counts.cautions, counts.advisories), (1, 2, 1));

        hmi.report(Report::Resolved("fuel".to_string()), at(0));
        assert_eq!(hmi.frame(at(0), Vec::new()).status.alarms.cautions, 1);
    }

    #[test]
    fn a_warning_has_a_text_message_and_f16_acknowledges_it() {
        let mut hmi = Hmi::new();
        assert_eq!(frame().alarm, None);
        raise(&mut hmi, "oil", "warning", 0);
        let message = hmi.frame(at(0), Vec::new()).alarm.unwrap();
        assert_eq!((message.category, message.text.as_str()), (Category::Warning, "W \u{2013} oil"));
        assert!(!message.acknowledged);

        press(&mut hmi, Button::F(16));
        assert!(hmi.frame(at(0), Vec::new()).alarm.unwrap().acknowledged, "It stays");
        assert_eq!(hmi.take_requests(), [Request::Acknowledged("oil".to_string())]);
        assert_eq!(hmi.take_requests(), [], "told once");
    }

    #[test]
    fn time_passing_takes_a_cautions_message_away() {
        let mut hmi = Hmi::new();
        assert_eq!(hmi.deadline(), None);
        raise(&mut hmi, "fuel", "caution", 500);
        assert_eq!(hmi.deadline(), Some(10_500));
        assert!(!hmi.tick(at(10_499)));
        assert!(hmi.tick(at(10_500)));
        assert_eq!(hmi.frame(at(10_500), Vec::new()).alarm, None);
        assert_eq!(hmi.deadline(), Some(10_500 + 2 * 60 * 60 * 1000), "To be repeated");
    }

    #[test]
    fn f14_shows_the_alarms_page_and_f13_goes_back_up() {
        let mut hmi = with_external_areas();
        hmi.windows(&["sight"]);
        press(&mut hmi, Button::Area(1));
        assert_eq!(label(&hmi, 14), ("Alarms".to_string(), LabelState::Available));

        press(&mut hmi, Button::F(14));
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Alarms { .. }));
        assert_eq!(label(&hmi, 14).1, LabelState::Selected, "The page on screen is F14's");
        assert_eq!(states(&hmi)[1], AreaState::Selected, "the area is still the one selected");
        assert_eq!(hmi.window_to_show(), None, "the page is the HMI's own: no window under it");
        press(&mut hmi, Button::F(14));
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Alarms { .. }), "and stays");

        // Up a level is the area's page again.
        press(&mut hmi, Button::F(13));
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::External { .. }));
        assert_eq!(hmi.window_to_show(), Some("sight"));
        assert!((1..=12).all(|n| label(&hmi, n).1 == LabelState::Blank));

        // An area button goes to its area from the Alarms page too.
        press(&mut hmi, Button::F(14));
        press(&mut hmi, Button::Area(SYS));
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Own { .. }));
    }

    #[test]
    fn the_alarms_page_lists_all_alarms_with_their_counts() {
        let mut hmi = Hmi::new();
        raise(&mut hmi, "fuel", "caution", 0);
        raise(&mut hmi, "oil", "warning", 1000);
        raise(&mut hmi, "service", "advisory", 2000);
        press(&mut hmi, Button::F(16));
        press(&mut hmi, Button::F(14));

        let (unacknowledged, acknowledged, page, pages, rows, detail) = alarms_page(&hmi);
        assert_eq!((unacknowledged.warnings, unacknowledged.cautions, unacknowledged.advisories), (0, 1, 1));
        assert_eq!((acknowledged.warnings, acknowledged.cautions, acknowledged.advisories), (1, 0, 0));
        assert_eq!((page, pages, detail), (1, 1, None));
        let texts: Vec<_> = rows.iter().map(|row| row.text.as_str()).collect();
        assert_eq!(texts, ["W \u{2013} oil", "C \u{2013} fuel", "A \u{2013} service"]);
        assert!(rows[0].acknowledged && !rows[1].acknowledged);
    }

    #[test]
    fn the_alarms_pages_buttons() {
        use alarms_page::*;
        let mut hmi = Hmi::new();
        raise(&mut hmi, "fuel", "caution", 0);
        raise(&mut hmi, "oil", "warning", 60_000);
        raise(&mut hmi, "service", "advisory", 120_000);
        press(&mut hmi, Button::F(14));
        let picked = |hmi: &Hmi| alarms_page(hmi).4.into_iter().find(|row| row.selected).map(|row| row.text);

        // a, b) the two orders.
        press(&mut hmi, Button::F(SORT_BY_TIME));
        assert_eq!(rows(&hmi), ["A \u{2013} service", "W \u{2013} oil", "C \u{2013} fuel"]);
        press(&mut hmi, Button::F(SORT_BY_CATEGORY));
        assert_eq!(rows(&hmi), ["W \u{2013} oil", "C \u{2013} fuel", "A \u{2013} service"]);

        // c) picking an alarm, with F17 and F18.
        assert_eq!(picked(&hmi), None);
        press(&mut hmi, Button::F(18));
        press(&mut hmi, Button::F(18));
        assert_eq!(picked(&hmi).as_deref(), Some("C \u{2013} fuel"));
        press(&mut hmi, Button::F(17));
        assert_eq!(picked(&hmi).as_deref(), Some("W \u{2013} oil"));

        // d) the picked alarm at length, by F4 or by F20; F13 or F20 puts it away.
        press(&mut hmi, Button::F(DETAIL));
        assert_eq!(alarms_page(&hmi).5.unwrap().text, "W \u{2013} oil");
        press(&mut hmi, Button::F(13));
        assert_eq!(alarms_page(&hmi).5, None);
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Alarms { .. }), "the page is still there");
        press(&mut hmi, Button::F(20));
        assert!(alarms_page(&hmi).5.is_some());
        press(&mut hmi, Button::F(CLEAR_ALL));
        assert_eq!(rows(&hmi).len(), 3, "nothing down the sides answers while it is open");
        press(&mut hmi, Button::F(18));
        assert_eq!(alarms_page(&hmi).5.unwrap().text, "C \u{2013} fuel", "F18 shows the next alarm at length");
        press(&mut hmi, Button::F(17));
        assert_eq!(alarms_page(&hmi).5.unwrap().text, "W \u{2013} oil");
        press(&mut hmi, Button::F(20));
        assert_eq!(alarms_page(&hmi).5, None);

        // f) acknowledging the picked alarm; c) leaving acknowledged alarms out.
        press(&mut hmi, Button::F(ACKNOWLEDGE));
        assert!(alarms_page(&hmi).4[0].acknowledged);
        press(&mut hmi, Button::F(ACKNOWLEDGED));
        assert_eq!(rows(&hmi), ["C \u{2013} fuel", "A \u{2013} service"]);
        press(&mut hmi, Button::F(ACKNOWLEDGED));
        assert_eq!(rows(&hmi).len(), 3);

        // j) overriding it: it stays in the table, marked.
        press(&mut hmi, Button::F(OVERRIDE));
        assert_eq!(alarms_page(&hmi).4[0].state, State::Overridden);
        assert_eq!(hmi.frame(at(0), Vec::new()).alarm.map(|m| m.category), Some(Category::Caution));

        // h) clearing it.
        press(&mut hmi, Button::F(CLEAR));
        assert_eq!(rows(&hmi), ["C \u{2013} fuel", "A \u{2013} service"]);

        // g) acknowledging all, i) clearing all.
        press(&mut hmi, Button::F(ACKNOWLEDGE_ALL));
        assert_eq!(alarms_page(&hmi).0.total(), 0);
        press(&mut hmi, Button::F(CLEAR_ALL));
        assert_eq!(rows(&hmi), [] as [&str; 0]);

        // And whatever raised them is told of each thing done.
        let told = |what: fn(String) -> Request, id: &str| what(id.to_string());
        assert_eq!(
            hmi.take_requests(),
            [
                told(Request::Acknowledged, "oil"),
                told(Request::Overridden, "oil"),
                told(Request::Cleared, "oil"),
                told(Request::Acknowledged, "fuel"),
                told(Request::Acknowledged, "service"),
                told(Request::Cleared, "fuel"),
                told(Request::Cleared, "service"),
            ]
        );
    }

    #[test]
    fn the_alarms_table_is_paged_with_f5_and_f6() {
        use alarms_page::*;
        let mut hmi = Hmi::new();
        for n in 0..15 {
            raise(&mut hmi, &format!("{n:02}"), "advisory", n * 1000);
        }
        press(&mut hmi, Button::F(14));
        assert_eq!((alarms_page(&hmi).2, alarms_page(&hmi).3, rows(&hmi).len()), (1, 2, 10));
        assert_eq!(label(&hmi, PAGE_UP).1, LabelState::Unavailable);
        assert_eq!(label(&hmi, PAGE_DOWN).1, LabelState::Available);

        press(&mut hmi, Button::F(PAGE_DOWN));
        assert_eq!((alarms_page(&hmi).2, rows(&hmi).len()), (2, 5));
        assert_eq!(label(&hmi, PAGE_UP).1, LabelState::Available);
        assert_eq!(label(&hmi, PAGE_DOWN).1, LabelState::Unavailable);
        press(&mut hmi, Button::F(PAGE_UP));
        assert_eq!(alarms_page(&hmi).2, 1);
    }

    #[test]
    fn the_alarms_pages_labels_say_what_can_be_done() {
        use LabelState::{Available, Selected, Unavailable};
        use alarms_page::*;
        let mut hmi = Hmi::new();
        press(&mut hmi, Button::F(14));
        let text = |hmi: &Hmi, n| label(hmi, n).0;
        let state = |hmi: &Hmi, n| label(hmi, n).1;

        // No alarms: the orders can be chosen, and nothing else done.
        assert_eq!(label(&hmi, SORT_BY_CATEGORY), ("Sort By\nCategory".to_string(), Selected));
        assert_eq!(label(&hmi, SORT_BY_TIME), ("Sort By\nTime".to_string(), Available));
        assert_eq!(label(&hmi, ACKNOWLEDGED), ("Hide\nAcked".to_string(), Available));
        for n in [DETAIL, PAGE_UP, PAGE_DOWN, ACKNOWLEDGE, ACKNOWLEDGE_ALL, CLEAR, CLEAR_ALL, OVERRIDE] {
            assert_eq!(state(&hmi, n), Unavailable, "F{n}");
        }
        assert_eq!(state(&hmi, 12), LabelState::Blank);
        assert_eq!(text(&hmi, 13), "Up");

        // One alarm, not picked: the functions for all of them.
        raise(&mut hmi, "oil", "warning", 0);
        assert_eq!((state(&hmi, ACKNOWLEDGE_ALL), state(&hmi, CLEAR_ALL)), (Available, Available));
        assert_eq!((state(&hmi, ACKNOWLEDGE), state(&hmi, CLEAR), state(&hmi, DETAIL)), (Unavailable, Unavailable, Unavailable));

        // Picked: the functions for the one.
        press(&mut hmi, Button::F(18));
        for n in [DETAIL, ACKNOWLEDGE, CLEAR, OVERRIDE] {
            assert_eq!(state(&hmi, n), Available, "F{n}");
        }
        assert_eq!(text(&hmi, ACKNOWLEDGE), "Ack\nSelected");
        press(&mut hmi, Button::F(ACKNOWLEDGE));
        assert_eq!((state(&hmi, ACKNOWLEDGE), state(&hmi, ACKNOWLEDGE_ALL)), (Unavailable, Unavailable));

        press(&mut hmi, Button::F(SORT_BY_TIME));
        assert_eq!((state(&hmi, SORT_BY_CATEGORY), state(&hmi, SORT_BY_TIME)), (Available, Selected));
        press(&mut hmi, Button::F(ACKNOWLEDGED));
        assert_eq!(text(&hmi, ACKNOWLEDGED), "Show\nAcked");

        // With an alarm shown at length F13 is 'Cancel',
        // and the page's own buttons wait.
        press(&mut hmi, Button::F(ACKNOWLEDGED));
        press(&mut hmi, Button::F(DETAIL));
        assert_eq!(text(&hmi, 13), "Cancel");
        assert!((1..=11).all(|n| state(&hmi, n) == Unavailable));
        press(&mut hmi, Button::F(13));
        assert_eq!(text(&hmi, 13), "Up");
    }

    #[test]
    fn f15_shows_the_threats_page_and_f13_goes_back_up() {
        let mut hmi = with_external_areas();
        hmi.windows(&["sight"]);
        press(&mut hmi, Button::Area(1));
        assert_eq!(label(&hmi, 15), ("Threats".to_string(), LabelState::Available));

        press(&mut hmi, Button::F(15));
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Threats { .. }));
        assert_eq!(label(&hmi, 15).1, LabelState::Selected, "The page on screen is F15's");
        assert_eq!(states(&hmi)[1], AreaState::Selected, "the area is still the one selected");
        assert_eq!(hmi.window_to_show(), None, "the page is the HMI's own: no window under it");

        // F14 from here is the Alarms page, and F15 from there this one again.
        press(&mut hmi, Button::F(14));
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Alarms { .. }));
        assert_eq!((label(&hmi, 14).1, label(&hmi, 15).1), (LabelState::Selected, LabelState::Available));
        press(&mut hmi, Button::F(15));
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Threats { .. }));
        assert_eq!((label(&hmi, 14).1, label(&hmi, 15).1), (LabelState::Available, LabelState::Selected));

        // Up a level is the area's page again, not the page before.
        press(&mut hmi, Button::F(13));
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::External { .. }));
        assert_eq!(hmi.window_to_show(), Some("sight"));
        assert!((1..=12).all(|n| label(&hmi, n).1 == LabelState::Blank));

        press(&mut hmi, Button::F(15));
        press(&mut hmi, Button::Area(SYS));
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Own { .. }));
    }

    #[test]
    fn the_threats_page_lists_the_threats_and_the_sensors() {
        let mut hmi = with_threats();
        feed(&mut hmi, r#"{"sensor":{"name":"Laser Warning","status":"ok"}}"#, 0);
        feed(&mut hmi, r#"{"sensor":{"name":"Shot Detection","status":"failed"}}"#, 0);
        feed(&mut hmi, r#"{"resolved":"shot"}"#, 90_000);
        press(&mut hmi, Button::F(15));

        let (counts, sensors, page, pages, rows, detail) = threats_page(&hmi);
        assert_eq!(counts, ThreatCounts { current: 1, past: 1 });
        assert_eq!(
            sensors,
            [
                Sensor { name: "Laser Warning".to_string(), status: SensorStatus::Ok },
                Sensor { name: "Shot Detection".to_string(), status: SensorStatus::Failed },
            ]
        );
        assert_eq!((page, pages, detail), (1, 1, None));
        assert_eq!(threat_rows(&hmi), ["T \u{2013} Laser Rangefinder", "T \u{2013} Gunshot"]);
        assert_eq!((rows[0].bearing_mils, rows[0].range_m, rows[0].state), (Some(800), Some(1200), ThreatState::Current));
        assert_eq!((rows[1].bearing_mils, rows[1].range_m, rows[1].state), (Some(5300), None, ThreatState::Past));
    }

    #[test]
    fn the_threats_pages_buttons() {
        use threats_page::*;
        let mut hmi = with_threats();
        press(&mut hmi, Button::F(15));
        let picked = |hmi: &Hmi| threats_page(hmi).4.into_iter().find(|row| row.selected).map(|row| row.text);

        // a, b) the two orders: the type that matters most first, or the latest.
        assert_eq!(threat_rows(&hmi), ["T \u{2013} Laser Rangefinder", "T \u{2013} Gunshot"]);
        feed(&mut hmi, r#"{"threat":{"id":"shot2","type":"Gunshot","priority":"low"}}"#, 120_000);
        press(&mut hmi, Button::F(SORT_BY_TIME));
        assert_eq!(threat_rows(&hmi), ["T \u{2013} Gunshot", "T \u{2013} Laser Rangefinder", "T \u{2013} Gunshot"]);
        press(&mut hmi, Button::F(SORT_BY_TYPE));
        assert_eq!(threat_rows(&hmi), ["T \u{2013} Laser Rangefinder", "T \u{2013} Gunshot", "T \u{2013} Gunshot"]);

        // d) picking a threat, with F17 and F18.
        assert_eq!(picked(&hmi), None);
        press(&mut hmi, Button::F(18));
        press(&mut hmi, Button::F(18));
        assert_eq!(picked(&hmi).as_deref(), Some("T \u{2013} Gunshot"));
        press(&mut hmi, Button::F(17));
        assert_eq!(picked(&hmi).as_deref(), Some("T \u{2013} Laser Rangefinder"));

        // c) the picked threat at length, by F4 or by F20; F13 or F20 puts it away.
        press(&mut hmi, Button::F(table_page::DETAIL));
        assert_eq!(threats_page(&hmi).5.unwrap().description, "Rangefinder");
        press(&mut hmi, Button::F(CANCEL_ALL));
        assert_eq!(threat_rows(&hmi).len(), 3, "nothing down the sides answers while it is open");
        press(&mut hmi, Button::F(18));
        assert_eq!(threats_page(&hmi).5.unwrap().kind, "Gunshot", "F18 shows the next threat at length");
        press(&mut hmi, Button::F(13));
        assert_eq!(threats_page(&hmi).5, None);
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Threats { .. }), "the page is still there");
        press(&mut hmi, Button::F(20));
        assert!(threats_page(&hmi).5.is_some());
        press(&mut hmi, Button::F(20));
        assert_eq!(threats_page(&hmi).5, None);

        // Leaving out the threats that have gone.
        feed(&mut hmi, r#"{"resolved":"shot"}"#, 130_000);
        press(&mut hmi, Button::F(PAST));
        assert_eq!(threat_rows(&hmi).len(), 2);
        press(&mut hmi, Button::F(PAST));
        assert_eq!(threat_rows(&hmi).len(), 3);

        // f) cancelling the picked threat; g) cancelling them all.
        assert_eq!(picked(&hmi).as_deref(), Some("T \u{2013} Gunshot"));
        press(&mut hmi, Button::F(CANCEL));
        assert_eq!(threat_rows(&hmi), ["T \u{2013} Laser Rangefinder", "T \u{2013} Gunshot"]);
        press(&mut hmi, Button::F(CANCEL_ALL));
        assert_eq!(threat_rows(&hmi), [] as [&str; 0]);
        assert_eq!(hmi.frame(at(0), Vec::new()).threat, None);

        // And whatever told of them is told of each one cancelled.
        let cancelled = |id: &str| Request::Cancelled(id.to_string());
        assert_eq!(hmi.take_requests(), [cancelled("shot2"), cancelled("shot"), cancelled("lws")]);
    }

    #[test]
    fn the_threats_page_is_a_table_or_a_picture() {
        use LabelState::{Available, Selected, Unavailable};
        use threats_page::*;
        let view = |hmi: &Hmi| match hmi.frame(at(0), Vec::new()).page {
            Page::Threats { view, .. } => view,
            other => panic!("not the Threats page: {other:?}"),
        };
        let mut hmi = with_threats();
        for n in 0..threats::ROWS_PER_PAGE {
            feed(&mut hmi, &format!(r#"{{"threat":{{"id":"{n}","type":"Radar","priority":"low"}}}}"#), 0);
        }
        press(&mut hmi, Button::F(15));
        assert!(matches!(view(&hmi), ThreatsView::Table { pages: 2, .. }), "it opens as a table");
        assert_eq!(label(&hmi, VIEW), ("View\nPPI".to_string(), Available));
        assert!(hmi.frame(at(0), Vec::new()).ppi.is_some());

        // F12: every listed threat at once, where it is - and the indicator in the corner gives way to it.
        press(&mut hmi, Button::F(VIEW));
        let ThreatsView::Plot(plot) = view(&hmi) else { panic!("not the picture") };
        assert_eq!((plot.range_m, plot.threats.len(), plot.elsewhere), (2000, 2, threats::ROWS_PER_PAGE as u32));
        assert_eq!(hmi.frame(at(0), Vec::new()).ppi, None);
        assert_eq!(label(&hmi, VIEW), ("View\nTable".to_string(), Available));

        // There is nothing to sort on it and no page to turn...
        for n in [SORT_BY_TYPE, SORT_BY_TIME, table_page::PAGE_UP, table_page::PAGE_DOWN] {
            assert_eq!(label(&hmi, n).1, Unavailable, "F{n}");
            press(&mut hmi, Button::F(n));
        }
        // ...but a threat is picked on it, looked closer at, left off it and cancelled as in the table.
        press(&mut hmi, Button::F(18));
        let ThreatsView::Plot(plot) = view(&hmi) else { panic!("not the picture") };
        assert_eq!((plot.threats[0].label.as_str(), plot.threats[0].picked), ("Laser", true));
        press(&mut hmi, Button::F(20));
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Threats { detail: Some(_), .. }));
        press(&mut hmi, Button::F(VIEW));
        assert!(matches!(view(&hmi), ThreatsView::Plot(_)), "nothing down the sides answers while it is open");
        press(&mut hmi, Button::F(13));
        feed(&mut hmi, r#"{"resolved":"shot"}"#, 0);
        press(&mut hmi, Button::F(PAST));
        let ThreatsView::Plot(plot) = view(&hmi) else { panic!("not the picture") };
        assert_eq!(plot.threats.len(), 1);
        press(&mut hmi, Button::F(CANCEL));
        let ThreatsView::Plot(plot) = view(&hmi) else { panic!("not the picture") };
        assert!(plot.threats.is_empty());

        // And back: the table is as it was left, at its first page, by type.
        press(&mut hmi, Button::F(VIEW));
        assert!(matches!(view(&hmi), ThreatsView::Table { page: 1, .. }));
        assert_eq!(label(&hmi, SORT_BY_TYPE).1, Selected);
    }

    #[test]
    fn the_threats_pages_labels_say_what_can_be_done() {
        use LabelState::{Available, Blank, Selected, Unavailable};
        let mut hmi = Hmi::new();
        press(&mut hmi, Button::F(15));
        let labels = |hmi: &Hmi| (1..=13).map(|n| label(hmi, n)).collect::<Vec<_>>();
        let said = |text: &str, state: LabelState| (text.to_string(), state);

        // With no threats there is nothing to look closer at, page through or cancel.
        assert_eq!(
            labels(&hmi),
            [
                said("Sort By\nType", Selected),
                said("Sort By\nTime", Available),
                said("Hide\nPast", Available),
                said("Detail", Unavailable),
                said("Page Up", Unavailable),
                said("Page Down", Unavailable),
                said("", Blank),
                said("", Blank),
                said("Cancel\nSelected", Unavailable),
                said("Cancel All", Unavailable),
                said("", Blank),
                said("View\nPPI", Available),
                said("Up", Available),
            ]
        );

        // What the Alarms page has too is on the same buttons there.
        press(&mut hmi, Button::F(14));
        for n in [2, 4, 5, 6] {
            assert_eq!(label(&hmi, n).0, labels(&with_threats_page())[usize::from(n) - 1].0);
        }
        press(&mut hmi, Button::F(15));

        for n in 0..threats::ROWS_PER_PAGE + 1 {
            feed(&mut hmi, &format!(r#"{{"threat":{{"id":"{n}","type":"Gunshot"}}}}"#), 0);
        }
        assert_eq!(label(&hmi, 9).1, Unavailable, "none is picked yet");
        assert_eq!((label(&hmi, 10).1, label(&hmi, 6).1), (Available, Available));
        press(&mut hmi, Button::F(18));
        assert_eq!((label(&hmi, 4).1, label(&hmi, 9).1), (Available, Available));
        press(&mut hmi, Button::F(2));
        assert_eq!((label(&hmi, 1).1, label(&hmi, 2).1), (Available, Selected));
        press(&mut hmi, Button::F(3));
        assert_eq!(label(&hmi, 3).0, "Show\nPast");

        // A threat shown at length is a dialog: F13 cancels it.
        press(&mut hmi, Button::F(20));
        assert!((1..=12).all(|n| matches!(label(&hmi, n).1, Unavailable | Blank)));
        assert_eq!(label(&hmi, 13), said("Cancel", Available));
        assert_eq!(label(&hmi, 15).1, Selected);
    }

    /// An HMI with no threats, showing the Threats page.
    fn with_threats_page() -> Hmi {
        let mut hmi = Hmi::new();
        press(&mut hmi, Button::F(15));
        hmi
    }

    #[test]
    fn f16_acknowledges_the_alarm_and_the_threat_whose_messages_show() {
        let mut hmi = with_threats();
        raise(&mut hmi, "oil", "warning", 0);
        // Each has its own place, so both are on the screen at once.
        let frame = hmi.frame(at(0), Vec::new());
        assert_eq!(frame.alarm.map(|m| (m.text, m.acknowledged)), Some(("W \u{2013} oil".to_string(), false)));
        assert_eq!(
            frame.threat,
            Some(ThreatMessage { text: "T \u{2013} Laser Rangefinder".to_string(), acknowledged: false })
        );

        press(&mut hmi, Button::F(16));
        let frame = hmi.frame(at(0), Vec::new());
        assert_eq!(frame.alarm.map(|m| m.acknowledged), Some(true));
        assert_eq!(frame.threat, Some(ThreatMessage { text: "T \u{2013} Gunshot".to_string(), acknowledged: false }));
        assert_eq!(
            hmi.take_requests(),
            [Request::Acknowledged("oil".to_string()), Request::Acknowledged("lws".to_string())]
        );

        // A threat that goes takes its message with it.
        press(&mut hmi, Button::F(16));
        feed(&mut hmi, r#"{"resolved":"lws"}"#, 0);
        feed(&mut hmi, r#"{"resolved":"shot"}"#, 0);
        assert_eq!(hmi.frame(at(0), Vec::new()).threat, None);
    }

    #[test]
    fn the_ppi_marks_the_current_threats_and_comes_up_for_them() {
        let ppi = |hmi: &Hmi| hmi.frame(at(0), Vec::new()).ppi.map(|ppi| ppi.threats);
        let mark = |bearing_mils: u16, picked: bool| ThreatMark { bearing_mils, picked };
        let mut hmi = with_external_areas();
        hmi.windows(&["sight"]);
        assert_eq!(ppi(&hmi), Some(vec![]), "it is on the HMI's own pages, threats or none");

        // Over another program's picture it is not - until there is a threat to mark.
        press(&mut hmi, Button::Area(1));
        assert_eq!(ppi(&hmi), None);
        feed(&mut hmi, r#"{"threat":{"id":"shot","type":"Gunshot","bearing":5300}}"#, 0);
        feed(&mut hmi, r#"{"threat":{"id":"somewhere","type":"Missile"}}"#, 0);
        assert_eq!(ppi(&hmi), Some(vec![mark(5300, false)]));

        // The Threats page has it, with the picked threat picked out on it.
        press(&mut hmi, Button::F(14));
        assert_eq!(ppi(&hmi), None, "the Alarms page is a table and nothing else");
        press(&mut hmi, Button::F(15));
        assert_eq!(ppi(&hmi), Some(vec![mark(5300, false)]));
        press(&mut hmi, Button::F(18));
        assert_eq!(ppi(&hmi), Some(vec![mark(5300, true)]));

        // A threat that has gone is not marked, and the picture is the program's again.
        feed(&mut hmi, r#"{"resolved":"shot"}"#, 0);
        assert_eq!(ppi(&hmi), Some(vec![]));
        press(&mut hmi, Button::F(13));
        assert_eq!(ppi(&hmi), None);
    }

    #[test]
    fn frame_serialises_in_the_shape_the_ui_reads() {
        let json = serde_json::to_value(frame()).unwrap();
        assert_eq!(json["areas"][3]["id"], "SYS");
        assert_eq!(json["areas"][3]["state"], "selected");
        assert_eq!(json["status"]["dtg"], "041230Z OCT 24");
        assert!(json["status"]["headingMils"].is_null());
        assert_eq!(json["status"]["alarms"]["warnings"], 0);
        assert_eq!(json["labels"][13]["state"], "available");
        assert_eq!(json["labels"][0]["state"], "blank");
        assert!(json["alarm"].is_null());
        assert!(json["threat"].is_null());
        assert_eq!(json["ppi"]["threats"], serde_json::json!([]));
        assert_eq!(json["page"]["kind"], "own");
        assert_eq!(json["page"]["rows"][0], serde_json::json!({"name": "Touch screen", "value": "On"}));
        assert_eq!(json["page"]["rows"][1]["name"], "Last button");
        assert_eq!(json["hidden"], serde_json::json!({"labels": false, "status": false}));
        assert_eq!(json["touch"], true);

        let mut hmi = with_external_areas();
        hmi.windows(&["sight"]);
        press(&mut hmi, Button::Area(1));
        let json = serde_json::to_value(hmi.frame(at(0), Vec::new())).unwrap();
        assert_eq!(json["areas"][7]["state"], "unavailable");
        assert_eq!(json["page"]["kind"], "external");
        assert_eq!(json["page"]["overlay"], "data:,reticle");
        assert!(json["ppi"].is_null());

        raise(&mut hmi, "oil", "warning", 0);
        press(&mut hmi, Button::F(14));
        press(&mut hmi, Button::F(18));
        press(&mut hmi, Button::F(20));
        let json = serde_json::to_value(hmi.frame(at(0), Vec::new())).unwrap();
        assert_eq!(json["status"]["alarms"]["warnings"], 1);
        assert_eq!(json["alarm"]["category"], "warning");
        assert_eq!(json["alarm"]["text"], "W \u{2013} oil");
        assert_eq!(json["alarm"]["acknowledged"], false);
        assert_eq!(json["labels"][0]["text"], "Sort By\nCategory");
        assert_eq!(json["page"]["kind"], "alarms");
        assert_eq!(json["page"]["unacknowledged"]["warnings"], 1);
        assert_eq!(json["page"]["acknowledged"]["warnings"], 0);
        assert_eq!((&json["page"]["page"], &json["page"]["pages"]), (&1.into(), &1.into()));
        let row = &json["page"]["rows"][0];
        assert_eq!(row["id"], "oil");
        assert_eq!(row["category"], "warning");
        assert_eq!(row["text"], "W \u{2013} oil");
        assert_eq!(row["time"], "041230Z");
        assert_eq!(row["acknowledged"], false);
        assert_eq!(row["state"], "active");
        assert_eq!(row["selected"], true);
        assert_eq!(json["page"]["detail"]["text"], "W \u{2013} oil");
        assert!(json["page"]["detail"]["detail"].is_null());

        let mut hmi = with_threats();
        feed(&mut hmi, r#"{"sensor":{"name":"Laser Warning","status":"degraded"}}"#, 0);
        press(&mut hmi, Button::F(15));
        press(&mut hmi, Button::F(18));
        press(&mut hmi, Button::F(20));
        let json = serde_json::to_value(hmi.frame(at(0), Vec::new())).unwrap();
        assert_eq!(json["threat"], serde_json::json!({"text": "T \u{2013} Laser Rangefinder", "acknowledged": false}));
        assert_eq!(json["ppi"]["threats"][0], serde_json::json!({"bearingMils": 5300, "picked": false}));
        assert_eq!(json["ppi"]["threats"][1], serde_json::json!({"bearingMils": 800, "picked": true}));
        assert_eq!(json["labels"][0]["text"], "Sort By\nType");
        assert_eq!(json["labels"][14]["state"], "selected");
        assert_eq!(json["page"]["kind"], "threats");
        assert_eq!(json["page"]["counts"], serde_json::json!({"current": 2, "past": 0}));
        assert_eq!(json["page"]["sensors"], serde_json::json!([{"name": "Laser Warning", "status": "degraded"}]));
        let view = &json["page"]["view"];
        assert_eq!((&view["kind"], &view["page"], &view["pages"]), (&"table".into(), &1.into(), &1.into()));
        let row = serde_json::json!({
            "id": "lws", "text": "T \u{2013} Laser Rangefinder", "time": "041231Z", "bearingMils": 800,
            "rangeM": 1200, "state": "current", "selected": true,
        });
        assert_eq!(view["rows"][0], row);
        assert!(view["rows"][1]["rangeM"].is_null());
        assert_eq!(
            json["page"]["detail"],
            serde_json::json!({
                "text": "T \u{2013} Laser Rangefinder", "type": "Laser", "description": "Rangefinder",
                "priority": "high", "time": "041231Z OCT 24", "bearingMils": 800, "rangeM": 1200,
                "state": "current", "acknowledged": false, "detail": null,
            })
        );

        // The same threats as a picture.
        press(&mut hmi, Button::F(20));
        press(&mut hmi, Button::F(threats_page::VIEW));
        let json = serde_json::to_value(hmi.frame(at(0), Vec::new())).unwrap();
        assert!(json["ppi"].is_null());
        assert_eq!(json["labels"][11]["text"], "View\nTable");
        let view = &json["page"]["view"];
        assert_eq!((&view["kind"], &view["rangeM"]), (&"plot".into(), &2000.into()));
        assert_eq!(
            view["threats"],
            serde_json::json!([
                {"id": "lws", "symbol": "SHGP-----------", "label": "Laser", "bearingMils": 800, "rangeM": 1200, "state": "current", "picked": true},
                {"id": "shot", "symbol": "SHGP-----------", "label": "Gunshot", "bearingMils": 5300, "rangeM": null, "state": "current", "picked": false},
            ])
        );
        assert_eq!(view["elsewhere"], 0);
    }

    /// What of the frame is on the screen: the labels, the status
    /// information bar and the Plan Position Indicator.
    fn seen(hmi: &Hmi) -> (bool, bool, bool) {
        let frame = hmi.frame(at(0), Vec::new());
        (!frame.hidden.labels, !frame.hidden.status, frame.ppi.is_some())
    }

    #[test]
    fn f19_hides_the_labels_then_the_status_bar_then_the_overlays() {
        let mut hmi = Hmi::new();
        assert_eq!(label(&hmi, 19).0, "Labels");
        assert_eq!(seen(&hmi), (true, true, true));
        press(&mut hmi, Button::F(19));
        assert_eq!(seen(&hmi), (false, true, true), "first the labels of the bezel buttons");
        press(&mut hmi, Button::F(19));
        assert_eq!(seen(&hmi), (false, false, true), "then the status information bar as well");
        press(&mut hmi, Button::F(19));
        assert_eq!(seen(&hmi), (false, false, false), "then the Plan Position Indicator, which is an overlay");
        press(&mut hmi, Button::F(19));
        assert_eq!(seen(&hmi), (true, true, true), "and then it is all back");
    }

    #[test]
    fn no_press_of_f19_does_nothing() {
        // Over another program's picture there is no overlay to hide: the
        // press after the status information bar's brings everything back.
        let mut hmi = with_external_areas();
        hmi.windows(&["sight"]);
        press(&mut hmi, Button::Area(1));
        assert_eq!(seen(&hmi), (true, true, false));
        press(&mut hmi, Button::F(19));
        press(&mut hmi, Button::F(19));
        assert_eq!(seen(&hmi), (false, false, false));
        press(&mut hmi, Button::F(19));
        assert_eq!(seen(&hmi), (true, true, false));

        // With a threat marked, the indicator is there to be hidden in its turn.
        feed(&mut hmi, r#"{"threat":{"id":"shot","type":"Gunshot","bearing":5300}}"#, 0);
        press(&mut hmi, Button::F(19));
        press(&mut hmi, Button::F(19));
        assert_eq!(seen(&hmi), (false, false, true));
        press(&mut hmi, Button::F(19));
        assert_eq!(seen(&hmi), (false, false, false));
        press(&mut hmi, Button::F(19));
        assert_eq!(seen(&hmi), (true, true, true));
    }

    #[test]
    fn what_is_hidden_stays_hidden_and_the_messages_never_are() {
        let mut hmi = Hmi::new();
        for _ in 0..3 {
            press(&mut hmi, Button::F(19));
        }
        assert_eq!(seen(&hmi), (false, false, false));

        // An alarm's message and a threat's are not overlays to do without.
        raise(&mut hmi, "oil", "warning", 0);
        feed(&mut hmi, r#"{"threat":{"id":"shot","type":"Gunshot","bearing":5300}}"#, 0);
        let frame = hmi.frame(at(0), Vec::new());
        assert_eq!(frame.alarm.map(|alarm| alarm.text), Some("W \u{2013} oil".to_string()));
        assert_eq!(frame.threat.map(|threat| threat.text), Some("T \u{2013} Gunshot".to_string()));

        // The buttons do what they did, labelled or not, and going from page
        // to page shows nothing again.
        press(&mut hmi, Button::F(14));
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Alarms { .. }));
        press(&mut hmi, Button::F(13));
        press(&mut hmi, Button::F(15));
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Threats { .. }));
        assert_eq!(seen(&hmi), (false, false, false));
    }

    #[test]
    fn a_touched_label_does_what_its_button_does() {
        let mut hmi = Hmi::new();
        assert!(hmi.touch(Touched::Label(Button::F(14)), at(0)));
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Alarms { .. }));
        assert_eq!(hmi.last_button, Some(ButtonEvent { button: Button::F(14), action: Action::Released }));
        assert!(hmi.touch(Touched::Label(Button::F(13)), at(0)));
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Own { .. }));

        // A label that F19 has hidden is not on the screen to be touched;
        // its button is still on the bezel.
        press(&mut hmi, Button::F(19));
        assert!(!hmi.touch(Touched::Label(Button::F(14)), at(0)));
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Own { .. }));
        press(&mut hmi, Button::F(14));
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Alarms { .. }));
    }

    #[test]
    fn a_touched_row_is_picked_and_touched_again_is_shown_at_length() {
        let mut hmi = Hmi::new();
        raise(&mut hmi, "oil", "warning", 0);
        raise(&mut hmi, "fuel", "caution", 0);
        let touch = |hmi: &mut Hmi, row: &str| hmi.touch(Touched::Row(row.to_string()), at(0));
        assert!(!touch(&mut hmi, "fuel"), "there is no table on the SYS page");

        press(&mut hmi, Button::F(14));
        let picked = |hmi: &Hmi| match hmi.frame(at(0), Vec::new()).page {
            Page::Alarms { rows, detail, .. } => {
                (rows.into_iter().find(|row| row.selected).map(|row| row.id), detail.map(|alarm| alarm.text))
            }
            other => panic!("the Alarms page is up: {other:?}"),
        };
        assert_eq!(picked(&hmi), (None, None));
        assert!(touch(&mut hmi, "fuel"));
        assert_eq!(picked(&hmi), (Some("fuel".to_string()), None));
        assert!(!touch(&mut hmi, "brakes"), "no such row");
        assert!(touch(&mut hmi, "oil"));
        assert_eq!(picked(&hmi), (Some("oil".to_string()), None));
        assert!(touch(&mut hmi, "oil"));
        assert_eq!(picked(&hmi), (Some("oil".to_string()), Some("W \u{2013} oil".to_string())));

        // What is shown at length is a dialog: the table under it is not
        // touched through it, and F13's label cancels it.
        assert!(!touch(&mut hmi, "fuel"));
        assert_eq!(label(&hmi, 13).0, "Cancel");
        assert!(hmi.touch(Touched::Label(Button::F(13)), at(0)));
        assert_eq!(picked(&hmi), (Some("oil".to_string()), None));
    }

    #[test]
    fn a_threat_is_picked_by_its_row_or_by_its_symbol() {
        let mut hmi = with_threats();
        press(&mut hmi, Button::F(15));
        assert!(hmi.touch(Touched::Row("shot".to_string()), at(0)));
        let (.., rows, _) = threats_page(&hmi);
        assert_eq!(rows.iter().find(|row| row.selected).map(|row| row.id.as_str()), Some("shot"));

        // On the picture, the symbols are what there is to touch.
        press(&mut hmi, Button::F(threats_page::VIEW));
        assert!(hmi.touch(Touched::Row("lws".to_string()), at(0)));
        match hmi.frame(at(0), Vec::new()).page {
            Page::Threats { view: ThreatsView::Plot(plot), .. } => {
                let picked: Vec<_> = plot.threats.iter().filter(|threat| threat.picked).map(|threat| &threat.id).collect();
                assert_eq!(picked, ["lws"]);
            }
            other => panic!("the threats' picture is up: {other:?}"),
        }
    }

    #[test]
    fn the_touch_screen_can_be_put_off_and_on_again() {
        let mut hmi = Hmi::new();
        let touch = |hmi: &Hmi| hmi.frame(at(0), Vec::new()).touch;
        assert!(touch(&hmi));
        assert_eq!(label(&hmi, 12), ("Touch\nOff".to_string(), LabelState::Available));

        // By its own label, as by the button.
        assert!(hmi.touch(Touched::Label(Button::F(12)), at(0)));
        assert!(!touch(&hmi));
        assert_eq!(label(&hmi, 12).0, "Touch\nOn");
        assert!(!hmi.touch(Touched::Label(Button::F(14)), at(0)), "nothing touched does anything");
        assert!(!hmi.touch(Touched::Label(Button::F(12)), at(0)), "not even this");
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Own { .. }));

        // The bezel is always there to put it on again.
        press(&mut hmi, Button::F(12));
        assert!(touch(&hmi));
        assert!(hmi.touch(Touched::Label(Button::F(14)), at(0)));

        // The button is the SYS page's: elsewhere it is something else's, or nothing's.
        press(&mut hmi, Button::F(12));
        assert!(touch(&hmi));
    }

    #[test]
    fn a_button_is_known_by_the_name_it_is_written_as() {
        for button in Button::all() {
            assert_eq!(button.to_string().parse(), Ok(button));
        }
        for name in ["", "F", "F0", "F21", "sa", "Labels"] {
            assert!(name.parse::<Button>().is_err(), "{name:?} is no button");
        }
        assert_eq!(Touched::Label(Button::F(14)).to_string(), "F14");
        assert_eq!(Touched::Row("oil".to_string()).to_string(), "row oil");
    }

    /// An HMI whose SA area is the windows "a" to "e", which the crew know
    /// as "A" to "E", shown three at a time: all of them open but "e".
    fn with_tiles() -> Hmi {
        let mut hmi = Hmi::new();
        let area = "area SA\ntiles 3\nwindow A a\nwindow B b\nwindow C c\nwindow D d\nwindow E e";
        hmi.add_tiled(TiledArea::parse(area).unwrap()).unwrap();
        hmi.windows(&["a", "b", "c", "d"]);
        hmi
    }

    /// The tiles of an HMI that is showing a page of them, by their labels.
    fn tiles(hmi: &Hmi) -> Vec<String> {
        match hmi.frame(at(0), Vec::new()).page {
            Page::Tiles { tiles } => tiles.into_iter().map(|tile| tile.label).collect(),
            other => panic!("not a page of tiles: {other:?}"),
        }
    }

    #[test]
    fn to_220_an_area_of_tiles_has_functions_while_any_of_its_windows_is_open() {
        use AreaState::{Available, Selected, Unavailable};
        let mut hmi = Hmi::new();
        hmi.add_tiled(TiledArea::parse("area SA\ntiles 2\nwindow A a\nwindow B b").unwrap()).unwrap();
        assert_eq!(states(&hmi)[0], Unavailable);
        press(&mut hmi, Button::Area(0));
        assert_eq!(states(&hmi)[SYS as usize], Selected, "with no window to show it stays unselected");

        assert!(hmi.windows(&["b", "something else"]));
        assert_eq!(states(&hmi)[0], Available);
        press(&mut hmi, Button::Area(0));
        assert_eq!(states(&hmi)[0], Selected);
        // None of its windows is shown until one is picked.
        assert!(tiles(&hmi).is_empty());
        assert!(hmi.windows_to_show().is_empty());
    }

    #[test]
    fn an_area_of_tiles_has_a_window_on_each_button_down_the_sides() {
        use LabelState::{Available, Blank, Selected, Unavailable};
        let labels = |hmi: &Hmi| (1..=6).map(|n| label(hmi, n)).collect::<Vec<_>>();
        let said = |text: &str, state: LabelState| (text.to_string(), state);
        let mut hmi = with_tiles();
        // The buttons are the windows' on the area's own page, and nowhere else.
        assert!((1..=11).all(|n| label(&hmi, n).1 == Blank));
        press(&mut hmi, Button::F(2));
        press(&mut hmi, Button::Area(0));
        assert!(tiles(&hmi).is_empty());
        assert_eq!(
            labels(&hmi),
            [
                said("A", Available),
                said("B", Available),
                said("C", Available),
                said("D", Available),
                said("E", Unavailable),
                said("", Blank),
            ]
        );
        assert!((7..=12).all(|n| label(&hmi, n).1 == Blank));
        assert_eq!(label(&hmi, 14), said("Alarms", Available), "the common functions are where they always are");

        // A button shows its window, in the order they are pressed.
        press(&mut hmi, Button::F(2));
        press(&mut hmi, Button::F(4));
        assert_eq!(tiles(&hmi), ["B", "D"]);
        assert_eq!(
            labels(&hmi)[..4],
            [said("A", Available), said("B", Selected), said("C", Available), said("D", Selected)]
        );
        // Three are as many as are shown: the fourth waits for one of them to go.
        press(&mut hmi, Button::F(3));
        assert_eq!(label(&hmi, 1), said("A", Unavailable));
        press(&mut hmi, Button::F(1));
        assert_eq!(tiles(&hmi), ["B", "D", "C"]);
        // And pressed again, the button stops showing it.
        press(&mut hmi, Button::F(4));
        assert_eq!(tiles(&hmi), ["B", "C"]);
        assert_eq!(label(&hmi, 1), said("A", Available));

        // A window that is not open is not shown, and letting go of a button does nothing.
        press(&mut hmi, Button::F(5));
        hmi.button(ButtonEvent { button: Button::F(1), action: Action::Released }, at(0));
        assert_eq!(tiles(&hmi), ["B", "C"]);
    }

    #[test]
    fn the_windows_to_show_are_those_with_tiles_each_with_its_part_of_the_display() {
        let mut hmi = with_tiles();
        press(&mut hmi, Button::Area(0));
        press(&mut hmi, Button::F(3));
        press(&mut hmi, Button::F(1));
        let shown = hmi.windows_to_show();
        assert_eq!(shown.iter().map(|shown| shown.window).collect::<Vec<_>>(), ["c", "a"]);
        let Page::Tiles { tiles: on_the_page } = hmi.frame(at(0), Vec::new()).page else {
            panic!("SA is a page of tiles");
        };
        assert_eq!(
            shown.iter().map(|shown| shown.at).collect::<Vec<_>>(),
            on_the_page.iter().map(|tile| Some(tile.at)).collect::<Vec<_>>(),
            "each where the frame has its tile"
        );
        assert_eq!(hmi.window_to_show(), None, "none of them has the page to itself");

        // The area is as it was left when it is come back to.
        press(&mut hmi, Button::Area(SYS));
        assert!(hmi.windows_to_show().is_empty());
        press(&mut hmi, Button::Area(0));
        assert_eq!(tiles(&hmi), ["C", "A"]);
    }

    #[test]
    fn a_window_can_have_a_page_to_itself_in_one_area_and_a_tile_in_another() {
        let mut hmi = with_external_areas();
        hmi.add_tiled(TiledArea::parse("area SA\ntiles 2\nwindow Sight sight\nwindow Rear rear").unwrap()).unwrap();
        hmi.windows(&["sight", "rear"]);

        press(&mut hmi, Button::Area(1));
        assert_eq!(hmi.windows_to_show(), [Shown { window: "sight", at: None }]);
        assert_eq!(hmi.window_to_show(), Some("sight"));

        press(&mut hmi, Button::Area(0));
        press(&mut hmi, Button::F(2));
        press(&mut hmi, Button::F(1));
        let shown = hmi.windows_to_show();
        assert_eq!(shown.iter().map(|shown| shown.window).collect::<Vec<_>>(), ["rear", "sight"]);
        assert!(shown.iter().all(|shown| shown.at.is_some()));
    }

    #[test]
    fn a_window_that_goes_loses_its_tile_and_sys_takes_over_when_none_is_left() {
        let mut hmi = with_tiles();
        press(&mut hmi, Button::Area(0));
        press(&mut hmi, Button::F(1));
        press(&mut hmi, Button::F(2));

        assert!(hmi.windows(&["b", "c", "d"]));
        assert_eq!(tiles(&hmi), ["B"]);
        assert_eq!(label(&hmi, 1), ("A".to_string(), LabelState::Unavailable));
        // It can be picked again when it is back, and is not until then.
        assert!(hmi.windows(&["a", "b", "c", "d"]));
        assert_eq!(tiles(&hmi), ["B"]);
        assert_eq!(label(&hmi, 1).1, LabelState::Available);

        assert!(hmi.windows(&["something else"]));
        assert_eq!(states(&hmi)[SYS as usize], AreaState::Selected);
        assert_eq!(states(&hmi)[0], AreaState::Unavailable);
        assert!(hmi.windows_to_show().is_empty());
        // Nor is the area on screen again by itself, or as it was.
        hmi.windows(&["a", "b"]);
        assert_eq!(states(&hmi)[0], AreaState::Available);
        press(&mut hmi, Button::Area(0));
        assert!(tiles(&hmi).is_empty());
    }

    #[test]
    fn the_alarms_and_threats_pages_come_up_over_tiles_and_f13_goes_back() {
        let mut hmi = with_tiles();
        press(&mut hmi, Button::Area(0));
        press(&mut hmi, Button::F(1));

        for (page, text) in [(14, "Sort By\nCategory"), (15, "Sort By\nType")] {
            press(&mut hmi, Button::F(page));
            assert!(!matches!(hmi.frame(at(0), Vec::new()).page, Page::Tiles { .. }));
            assert!(hmi.windows_to_show().is_empty(), "the page is the HMI's own: no window under it");
            assert_eq!(label(&hmi, 1).0, text, "and the buttons down the sides are that page's");
            press(&mut hmi, Button::F(2));

            // Up a level is the area's page, as it was.
            press(&mut hmi, Button::F(13));
            assert_eq!(tiles(&hmi), ["A"]);
            assert_eq!(hmi.windows_to_show().len(), 1);
        }
    }

    #[test]
    fn the_ppi_is_on_a_page_of_tiles() {
        let ppi = |hmi: &Hmi| hmi.frame(at(0), Vec::new()).ppi.map(|ppi| ppi.threats);
        let mut hmi = with_tiles();
        press(&mut hmi, Button::Area(0));
        assert_eq!(ppi(&hmi), Some(vec![]), "with nothing picked");
        press(&mut hmi, Button::F(1));
        assert_eq!(ppi(&hmi), Some(vec![]), "and with a picture on the page");

        feed(&mut hmi, r#"{"threat":{"id":"shot","type":"Gunshot","bearing":5300}}"#, 0);
        assert_eq!(ppi(&hmi), Some(vec![ThreatMark { bearing_mils: 5300, picked: false }]));
    }

    #[test]
    fn a_tile_grows_into_the_room_of_what_f19_hides() {
        let mut hmi = with_tiles();
        press(&mut hmi, Button::Area(0));
        press(&mut hmi, Button::F(1));
        let tile = |hmi: &Hmi| match hmi.frame(at(0), Vec::new()).page {
            Page::Tiles { tiles } => tiles[0].at,
            other => panic!("not a page of tiles: {other:?}"),
        };
        let seen = tile(&hmi);
        assert_eq!((seen.width, seen.height), (776, 582));

        // The labels go, and the picture has their room; then the status
        // information bar, and it has that too.
        press(&mut hmi, Button::F(19));
        assert_eq!((tile(&hmi).width, tile(&hmi).height), (899, 674));
        assert_eq!(hmi.windows_to_show()[0].at, Some(tile(&hmi)), "its window goes where its tile does");
        press(&mut hmi, Button::F(19));
        let most = Rect { x: 40, y: 30, width: 944, height: 708 };
        assert_eq!(tile(&hmi), most);
        // The Plan Position Indicator goes next, and gives up no room: it was over the picture.
        press(&mut hmi, Button::F(19));
        assert_eq!(tile(&hmi), most);
        assert_eq!(hmi.frame(at(0), Vec::new()).ppi, None);

        // The buttons go on doing what they did with their labels out of
        // sight, and a label that is out of sight is not there to be touched.
        press(&mut hmi, Button::F(2));
        assert_eq!(tiles(&hmi), ["A", "B"]);
        assert!(!hmi.touch(Touched::Label(Button::F(3)), at(0)));
        assert_eq!(tiles(&hmi), ["A", "B"]);

        press(&mut hmi, Button::F(19));
        press(&mut hmi, Button::F(2));
        assert_eq!(tile(&hmi), seen, "and it is all as it was");
    }

    #[test]
    fn a_windows_label_touched_shows_it_and_touched_again_stops_showing_it() {
        let mut hmi = with_tiles();
        press(&mut hmi, Button::Area(0));
        assert!(hmi.touch(Touched::Label(Button::F(2)), at(0)));
        assert_eq!(tiles(&hmi), ["B"]);
        assert_eq!(label(&hmi, 2), ("B".to_string(), LabelState::Selected));
        assert!(hmi.touch(Touched::Label(Button::F(4)), at(0)));
        assert_eq!(tiles(&hmi), ["B", "D"]);
        assert!(hmi.touch(Touched::Label(Button::F(2)), at(0)));
        assert_eq!(tiles(&hmi), ["D"]);
        // One that cannot be shown is not, by a touch either.
        hmi.touch(Touched::Label(Button::F(5)), at(0));
        assert_eq!(tiles(&hmi), ["D"]);
    }

    #[test]
    fn an_area_is_given_to_tiles_once_and_sys_never() {
        let mut hmi = with_external_areas();
        let tiled = |area: &str| TiledArea::parse(&format!("area {area}\ntiles 2\nwindow A a")).unwrap();
        assert!(hmi.add_tiled(tiled("WPN")).unwrap_err().contains("WPN"));
        assert!(hmi.add_tiled(tiled("SYS")).unwrap_err().contains("SYS"));
        // One that cannot be right is not taken either.
        let mut without_windows = tiled("SA");
        without_windows.windows.clear();
        assert!(hmi.add_tiled(without_windows).unwrap_err().contains("no 'window'"));

        assert!(hmi.add_tiled(tiled("SA")).is_ok());
        let window = ExternalArea { area: 0, window: "w".to_string(), overlay: None, keyboard: false, require: Require::nothing(), socket: None, keys: Vec::new() };
        assert!(hmi.add_external(window).unwrap_err().contains("SA"));
    }

    #[test]
    fn a_page_of_tiles_serialises_in_the_shape_the_ui_reads() {
        let mut hmi = with_tiles();
        press(&mut hmi, Button::Area(0));
        let json = serde_json::to_value(hmi.frame(at(0), Vec::new())).unwrap();
        assert_eq!(json["page"], serde_json::json!({"kind": "tiles", "tiles": []}));

        press(&mut hmi, Button::F(2));
        press(&mut hmi, Button::F(1));
        let json = serde_json::to_value(hmi.frame(at(0), Vec::new())).unwrap();
        assert_eq!(json["areas"][0]["state"], "selected");
        assert_eq!(
            json["page"],
            serde_json::json!({"kind": "tiles", "tiles": [
                {"label": "B", "x": 124, "y": 236, "width": 384, "height": 288},
                {"label": "A", "x": 516, "y": 236, "width": 384, "height": 288},
            ]})
        );
        assert_eq!(json["labels"][0], serde_json::json!({"key": "F1", "text": "A", "state": "selected"}));
        assert_eq!(json["labels"][2]["state"], "available");
        assert_eq!(json["labels"][4]["state"], "unavailable");
        assert_eq!(json["labels"][5]["state"], "blank");
        assert_eq!(json["ppi"]["threats"], serde_json::json!([]));
    }

    /// The STR area, an index into [`AREAS`]: where the tests' form is.
    const STR: u8 = 5;

    /// An HMI whose STR area is a form of two fields: free text, and a
    /// number.
    fn with_form() -> Hmi {
        let mut hmi = Hmi::new();
        let form = "area STR\nform load Load Record\n\
                    field description text 12 Description\nfield quantity number 4 Quantity";
        hmi.add_form(FormArea::parse(form).unwrap()).unwrap();
        hmi
    }

    /// The form's page of an HMI that is showing it.
    fn form_page(hmi: &Hmi) -> FormPage {
        match hmi.frame(at(0), Vec::new()).page {
            Page::Form(page) => page,
            other => panic!("not a form's page: {other:?}"),
        }
    }

    /// What the fields on the form's page hold.
    fn held(hmi: &Hmi) -> Vec<String> {
        form_page(hmi).rows.into_iter().map(|row| row.value).collect()
    }

    /// What is written on the keys of the on-screen keyboard that are lit.
    fn lit(hmi: &Hmi) -> Vec<String> {
        let keyboard = form_page(hmi).keyboard.expect("the keyboard is up");
        keyboard.keys.into_iter().filter(|key| key.pressed).map(|key| key.text).collect()
    }

    /// Press these of F1 to F20 one after another, letting each go.
    #[test]
    fn a_programs_buttons_are_its_own_labelled_as_it_says() {
        let mut hmi = Hmi::new();
        let area = ExternalArea::parse("area WPN\nwindow sight\nsocket /s\nkey F1 zoom Zoom\nkey F2 laser Laser").unwrap();
        hmi.add_external(area).unwrap();
        assert_eq!(hmi.app_sockets(), [(1, "/s".to_string())]);
        hmi.windows(&["sight"]);
        press(&mut hmi, Button::Area(1));
        assert_eq!(label(&hmi, 1), ("Zoom".to_string(), LabelState::Available));

        // Pressed and let go, it is told of both.
        buttons(&mut hmi, &[1]);
        let told = |hmi: &mut Hmi| hmi.take_to_apps().into_iter().map(|(a, t)| (a, t.line())).collect::<Vec<_>>();
        assert_eq!(told(&mut hmi), [(1, r#"{"pressed":"zoom"}"#.into()), (1, r#"{"released":"zoom"}"#.into())]);
        // A button it has not got, nothing.
        buttons(&mut hmi, &[3]);
        assert_eq!(told(&mut hmi), []);

        // It says what its buttons are now: the labels follow, and one it
        // has made unavailable is not told of.
        let line = r#"{"keys":[{"key":"F2","id":"laser","label":"Laser","state":"selected"},{"key":"F1","id":"zoom","label":"Zoom","state":"unavailable"}]}"#;
        assert_eq!(hmi.from_app(1, line), Ok(true));
        assert_eq!(label(&hmi, 2), ("Laser".to_string(), LabelState::Selected));
        buttons(&mut hmi, &[1]);
        assert_eq!(told(&mut hmi), []);
        assert!(hmi.from_app(1, r#"{"keys":[{"key":"F13","id":"x","label":"X"}]}"#).is_err());
        assert!(hmi.from_app(3, r#"{"keys":[]}"#).is_err(), "SYS is nobody's");

        // Down on WPN and let go on the Alarms page: it is still told it was let go.
        press(&mut hmi, Button::F(2));
        press(&mut hmi, Button::F(14));
        hmi.button(ButtonEvent { button: Button::F(2), action: Action::Released }, at(0));
        assert_eq!(told(&mut hmi), [(1, r#"{"pressed":"laser"}"#.into()), (1, r#"{"released":"laser"}"#.into())]);
        // The Alarms page has the buttons, not the program.
        buttons(&mut hmi, &[2]);
        assert_eq!(told(&mut hmi), []);
    }

    fn buttons(hmi: &mut Hmi, numbers: &[u8]) {
        for &n in numbers {
            for action in [Action::Pressed, Action::Released] {
                hmi.button(ButtonEvent { button: Button::F(n), action }, at(0));
            }
        }
    }

    /// Touch the keys of the on-screen keyboard that say these things, one
    /// after another, the finger going from each.
    fn fingers(hmi: &mut Hmi, texts: &[&str]) {
        for text in texts {
            let keyboard = form_page(hmi).keyboard.expect("the keyboard is up");
            let key = keyboard.keys.iter().position(|key| key.text == *text);
            hmi.touch(Touched::Key(key.unwrap_or_else(|| panic!("no key says {text}"))), at(0));
            hmi.lift();
        }
    }

    #[test]
    fn an_area_can_be_a_form_which_is_there_from_the_start() {
        use AreaState::{Absent, Available, Selected};
        let mut hmi = with_form();
        // There is no program to wait for: the area has functions as soon as
        // there is an HMI, and windows coming and going are nothing to it.
        assert_eq!(states(&hmi), [Absent, Absent, Absent, Selected, Absent, Available, Absent, Absent]);
        assert!(!hmi.windows(&["something"]));
        assert_eq!(states(&hmi)[usize::from(STR)], Available);

        press(&mut hmi, Button::Area(STR));
        assert_eq!(states(&hmi)[usize::from(STR)], Selected);
        let page = form_page(&hmi);
        assert_eq!((page.title.as_str(), page.field, page.fields), ("Load Record", 1, 2));
        let titles: Vec<_> = page.rows.iter().map(|row| row.title.as_str()).collect();
        assert_eq!(titles, ["Description", "Quantity"]);
        assert_eq!(page.keyboard, None, "nothing is being entered yet");
        // The page is the HMI's own: no window is under it, and the Plan
        // Position Indicator is on it.
        assert!(hmi.windows_to_show().is_empty());
        assert_eq!(hmi.frame(at(0), Vec::new()).ppi, Some(Ppi { threats: vec![] }));
    }

    #[test]
    fn an_area_is_given_a_form_once_and_sys_never() {
        let mut hmi = with_form();
        let form = |area: &str| FormArea::parse(&format!("area {area}\nform f F\nfield a text 4 A")).unwrap();
        assert!(hmi.add_form(form("STR")).unwrap_err().contains("STR"));
        assert!(hmi.add_form(form("SYS")).unwrap_err().contains("SYS"));
        assert!(hmi.add_form(form("COM")).is_ok());
        // Nor is an area that is a form a program's as well.
        let window = ExternalArea { area: STR, window: "w".to_string(), overlay: None, keyboard: false, require: Require::nothing(), socket: None, keys: Vec::new() };
        assert!(hmi.add_external(window).unwrap_err().contains("STR"));
        // A form that cannot be one is not taken.
        let mut without_fields = form("DRV");
        without_fields.form.fields.clear();
        assert!(hmi.add_form(without_fields).unwrap_err().contains("no 'field'"));
    }

    #[test]
    fn a_forms_page_has_the_buttons_its_labels_say() {
        use LabelState::{Available, Blank, Unavailable};
        let said = |text: &str, state: LabelState| (text.to_string(), state);
        let mut hmi = with_form();
        // The buttons are the form's on its own page, and nowhere else.
        buttons(&mut hmi, &[18, 20]);
        press(&mut hmi, Button::Area(STR));
        assert_eq!((form_page(&hmi).field, form_page(&hmi).keyboard), (1, None));
        assert_eq!(
            (1..=13).map(|n| label(&hmi, n)).collect::<Vec<_>>(),
            [
                said("Previous\nField", Unavailable),
                said("", Blank),
                said("", Blank),
                said("", Blank),
                said("", Blank),
                said("Left", Unavailable),
                said("Next\nField", Available),
                said("Save", Unavailable),
                said("", Blank),
                said("Space", Unavailable),
                said("Backspace", Unavailable),
                said("Right", Unavailable),
                said("Up", Available),
            ]
        );

        // F18 takes the cursor to the next field, and F17 back.
        buttons(&mut hmi, &[18]);
        assert_eq!(form_page(&hmi).field, 2);
        buttons(&mut hmi, &[17]);
        assert_eq!(form_page(&hmi).field, 1);

        // F20 enters data into the field. The keyboard comes
        // up, and F20 is then the key its highlight is on: G, and with F12
        // H; F11 takes that away again, F10 is a space, and F18 goes down to
        // the B.
        buttons(&mut hmi, &[20]);
        assert_eq!(form_page(&hmi).keyboard.map(|keyboard| keyboard.layout), Some(hmi_entry::Layout::Qwerty));
        buttons(&mut hmi, &[20, 12, 20, 11, 10, 18, 20]);
        assert_eq!(held(&hmi), ["G B", ""]);

        // What can be done now is available, and F13
        // cancels. The common functions are where they always are.
        assert_eq!(label(&hmi, 13), said("Cancel", Available));
        for n in [6, 7, 8, 10, 11, 12] {
            assert_eq!(label(&hmi, n).1, Available, "F{n}");
        }
        assert_eq!(label(&hmi, 1), said("Previous\nField", Unavailable));
        let common: Vec<_> = (14..=20).map(|n| label(&hmi, n).0).collect();
        assert_eq!(common, COMMON_LABELS[1..]);
    }

    #[test]
    fn what_is_saved_is_shown_and_goes_down_the_feed() {
        let mut hmi = with_form();
        press(&mut hmi, Button::Area(STR));
        buttons(&mut hmi, &[20]);
        fingers(&mut hmi, &["W", "A", "T", "E", "R"]);
        // a) Next Field takes what was entered and goes on; b) Previous
        // Field goes back.
        buttons(&mut hmi, &[7]);
        assert_eq!(form_page(&hmi).field, 2);
        fingers(&mut hmi, &["0", "4", "8"]);
        buttons(&mut hmi, &[1]);
        assert_eq!((form_page(&hmi).field, held(&hmi)), (1, vec!["WATER".to_string(), "048".to_string()]));
        assert_eq!(hmi.take_requests(), [], "nothing is anyone's until it is saved");

        // c) Save: data entry is over, the page shows what was saved, and
        // whatever the form is for is told - a number as it was entered.
        buttons(&mut hmi, &[8]);
        let page = form_page(&hmi);
        assert_eq!((page.keyboard, page.standing), (None, hmi_entry::Standing::Saved));
        assert_eq!(held(&hmi), ["WATER", "048"]);
        let told: Vec<String> = hmi.take_requests().iter().map(Request::line).collect();
        assert_eq!(told, [r#"{"entered":{"form":"load","fields":{"description":"WATER","quantity":"048"}}}"#]);
        assert_eq!(hmi.take_requests(), [], "told once");
    }

    #[test]
    fn f13_leaves_data_entry_and_asks_before_what_was_changed_is_lost() {
        use LabelState::{Available, Unavailable};
        let mut hmi = with_form();
        press(&mut hmi, Button::Area(STR));
        // d) With nothing changed, F13 leaves at once - and is 'Up' again.
        buttons(&mut hmi, &[20, 13]);
        assert_eq!((form_page(&hmi).keyboard, label(&hmi, 13).0.as_str()), (None, "Up"));

        // With something changed it asks: a dialog, which F13 cancels as it
        // does any while nothing down the sides answers...
        buttons(&mut hmi, &[20, 20, 13]);
        let page = form_page(&hmi);
        assert!(page.discard && page.keyboard.is_none());
        assert_eq!(label(&hmi, 13), ("Cancel".to_string(), Available));
        assert!([1, 6, 7, 8, 10, 11, 12].into_iter().all(|n| label(&hmi, n).1 == Unavailable));
        buttons(&mut hmi, &[8, 13]);
        assert!(!form_page(&hmi).discard && form_page(&hmi).keyboard.is_some());
        assert_eq!(held(&hmi), ["G", ""]);
        assert_eq!(hmi.take_requests(), [], "F8 did not save through the dialog");

        // ...and Enter accepts: the fields are as they were.
        buttons(&mut hmi, &[13, 20]);
        let page = form_page(&hmi);
        assert!(!page.discard && page.keyboard.is_none());
        assert_eq!(held(&hmi), ["", ""]);
        assert_eq!(hmi.take_requests(), []);
    }

    #[test]
    fn a_touched_field_is_entered_into_and_a_touched_key_lit_until_the_finger_goes() {
        let mut hmi = with_form();
        // What is not on the screen is not there to be touched.
        assert!(!hmi.touch(Touched::Field(2), at(0)));
        press(&mut hmi, Button::Area(STR));

        // A field touched is entered into, as F18 and F20 would have done.
        assert!(hmi.touch(Touched::Field(2), at(0)));
        assert!(!hmi.lift(), "nothing is lit: the field was entered into as the finger landed");
        let page = form_page(&hmi);
        assert_eq!((page.field, page.keyboard.map(|keyboard| keyboard.layout)), (2, Some(hmi_entry::Layout::Phone)));

        // A key touched is a key selected: it types, and it is lit while the
        // finger is on it.
        assert!(hmi.touch(Touched::Key(3), at(0)));
        assert_eq!((held(&hmi)[1].as_str(), lit(&hmi)), ("4", vec!["4".to_string()]));
        assert!(hmi.lift());
        assert!(lit(&hmi).is_empty());
        assert!(!hmi.lift(), "it is let go once");
        // So is the key a button selects, for as long as the button is down.
        hmi.button(ButtonEvent { button: Button::F(20), action: Action::Pressed }, at(0));
        assert_eq!((held(&hmi)[1].as_str(), lit(&hmi)), ("44", vec!["4".to_string()]));
        hmi.button(ButtonEvent { button: Button::F(20), action: Action::Released }, at(0));
        assert!(lit(&hmi).is_empty());

        // A label touched is its button, here as anywhere: Save.
        assert!(hmi.touch(Touched::Label(Button::F(8)), at(0)));
        assert_eq!(form_page(&hmi).standing, hmi_entry::Standing::Saved);
        // Under the Alarms page the form is not there to be touched.
        press(&mut hmi, Button::F(14));
        assert!(!hmi.touch(Touched::Field(1), at(0)));
        assert!(!hmi.touch(Touched::Key(0), at(0)));
        press(&mut hmi, Button::F(13));
        assert_eq!(form_page(&hmi).keyboard, None);
    }

    #[test]
    fn with_the_touch_screen_off_a_form_is_filled_in_from_the_bezel_and_no_other_way() {
        let mut hmi = with_form();
        press(&mut hmi, Button::F(sys_page::TOUCH));
        assert!(!hmi.frame(at(0), Vec::new()).touch);
        press(&mut hmi, Button::Area(STR));

        // A touch does nothing: not on a field, and not on a key.
        assert!(!hmi.touch(Touched::Field(1), at(0)));
        assert_eq!(form_page(&hmi).keyboard, None);
        buttons(&mut hmi, &[20]);
        assert!(!hmi.touch(Touched::Key(0), at(0)));
        assert!(!hmi.lift());
        assert_eq!(held(&hmi), ["", ""]);

        // The buttons do all that a finger would have: G into the first
        // field, 5 into the second, and Save.
        buttons(&mut hmi, &[20, 7, 20, 8]);
        assert_eq!(held(&hmi), ["G", "5"]);
        assert_eq!(hmi.take_requests().len(), 1);

        // A key that a button is holding is the button's to let go.
        buttons(&mut hmi, &[20]);
        hmi.button(ButtonEvent { button: Button::F(20), action: Action::Pressed }, at(0));
        assert!(!hmi.lift());
        assert_eq!(lit(&hmi), ["5"]);
    }

    #[test]
    fn the_common_functions_work_while_data_is_entered_and_the_form_is_as_it_was_left() {
        let mut hmi = with_form();
        press(&mut hmi, Button::Area(STR));
        buttons(&mut hmi, &[20, 20, 12]);
        let left = form_page(&hmi);
        assert_eq!(left.rows[0].value, "G");

        // F14 and F15 bring their pages up over the form. The buttons are
        // those pages' then - F20 types nothing - and F13 there is 'Up':
        // back to the form, with the keyboard as it was.
        for page in [14, 15] {
            press(&mut hmi, Button::F(page));
            assert!(!matches!(hmi.frame(at(0), Vec::new()).page, Page::Form(_)));
            assert_eq!(label(&hmi, 13).0, "Up");
            buttons(&mut hmi, &[20, 13]);
            assert_eq!(form_page(&hmi), left);
        }

        // F16 acknowledges on a form's page as on any.
        raise(&mut hmi, "oil", "warning", 0);
        press(&mut hmi, Button::F(16));
        assert!(hmi.frame(at(0), Vec::new()).alarm.unwrap().acknowledged);
        assert_eq!(form_page(&hmi), left);

        // Another area, and back - the form is as it was left,
        // and what is typed next goes on the end.
        press(&mut hmi, Button::Area(SYS));
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Own { .. }));
        press(&mut hmi, Button::Area(STR));
        assert_eq!(form_page(&hmi), left);
        buttons(&mut hmi, &[20]);
        assert_eq!(held(&hmi)[0], "GH");
    }

    #[test]
    fn f19_hides_what_it_hides_anywhere_and_the_forms_buttons_go_on_working() {
        let shown = |hmi: &Hmi| {
            let frame = hmi.frame(at(0), Vec::new());
            (!frame.hidden.labels, !frame.hidden.status, frame.ppi.is_some())
        };
        let mut hmi = with_form();
        press(&mut hmi, Button::Area(STR));
        buttons(&mut hmi, &[20]);
        assert_eq!(shown(&hmi), (true, true, true), "a form's page has the Plan Position Indicator");

        // The labels, then the status information bar, then the indicator;
        // then all of them back.
        let hidden: Vec<_> = (0..4).map(|_| (press(&mut hmi, Button::F(19)), shown(&hmi)).1).collect();
        assert_eq!(hidden, [(false, true, true), (false, false, true), (false, false, false), (true, true, true)]);

        // With the labels out of sight the buttons do what they did. A label
        // is not there to be touched, and the keys are.
        press(&mut hmi, Button::F(19));
        buttons(&mut hmi, &[20]);
        assert_eq!(held(&hmi)[0], "G");
        assert!(!hmi.touch(Touched::Label(Button::F(8)), at(0)));
        fingers(&mut hmi, &["H"]);
        buttons(&mut hmi, &[8]);
        assert_eq!((form_page(&hmi).standing, held(&hmi)[0].as_str()), (hmi_entry::Standing::Saved, "GH"));
    }

    #[test]
    fn a_forms_page_serialises_in_the_shape_the_ui_reads() {
        let mut hmi = with_form();
        press(&mut hmi, Button::Area(STR));
        let json = serde_json::to_value(hmi.frame(at(0), Vec::new())).unwrap();
        assert_eq!(json["areas"][5]["state"], "selected");
        assert_eq!(
            json["page"],
            serde_json::json!({
                "kind": "form", "title": "Load Record", "standing": "empty", "field": 1, "fields": 2,
                "rows": [
                    {"number": 1, "title": "Description", "kind": "text", "length": 12, "value": "", "cursor": true},
                    {"number": 2, "title": "Quantity", "kind": "number", "length": 4, "value": "", "cursor": false},
                ],
                "keyboard": null, "discard": false,
            })
        );
        assert_eq!(json["labels"][6], serde_json::json!({"key": "F7", "text": "Next\nField", "state": "available"}));
        assert_eq!(json["labels"][7], serde_json::json!({"key": "F8", "text": "Save", "state": "unavailable"}));
        assert_eq!(json["labels"][12]["text"], "Up");
        assert_eq!(json["ppi"]["threats"], serde_json::json!([]));

        // The number being entered, and the key that types 8 under a finger.
        buttons(&mut hmi, &[18, 20]);
        hmi.touch(Touched::Key(7), at(0));
        let json = serde_json::to_value(hmi.frame(at(0), Vec::new())).unwrap();
        let page = &json["page"];
        assert_eq!((&page["kind"], &page["standing"], &page["field"]), (&"form".into(), &"changed".into(), &2.into()));
        assert_eq!(page["rows"][1]["value"], "8");
        let keyboard = &page["keyboard"];
        assert_eq!((&keyboard["layout"], &keyboard["columns"]), (&"phone".into(), &3.into()));
        assert_eq!((&keyboard["rows"], keyboard["keys"].as_array().map(Vec::len)), (&4.into(), Some(12)));
        assert_eq!(
            keyboard["keys"][7],
            serde_json::json!({
                "text": "8", "kind": "character", "row": 2, "column": 1, "span": 1,
                "available": true, "highlighted": true, "pressed": true, "on": false,
            })
        );
        assert_eq!(json["labels"][12]["text"], "Cancel");
        assert_eq!(json["labels"][7]["state"], "available");

        // The question asked before that is discarded.
        hmi.lift();
        buttons(&mut hmi, &[13]);
        let json = serde_json::to_value(hmi.frame(at(0), Vec::new())).unwrap();
        assert_eq!((&json["page"]["discard"], &json["page"]["keyboard"]), (&true.into(), &serde_json::Value::Null));
        assert_eq!(json["labels"][7]["state"], "unavailable");
    }

    /// The BMS area, an index into [`AREAS`].
    const BMS: u8 = 7;

    /// An HMI whose WPN is a sight's window, which takes no typing, whose
    /// DRV is a program's that does, and whose BMS is two views: a map that
    /// takes none and pages that do. All four windows are open, and DRV is
    /// on screen.
    fn with_pages_to_type_into() -> Hmi {
        let mut hmi = Hmi::new();
        hmi.add_external(ExternalArea::parse("area WPN\nwindow sight").unwrap()).unwrap();
        hmi.add_external(ExternalArea::parse("area DRV\nwindow log\nkeyboard yes").unwrap()).unwrap();
        let views = "area BMS\nview Map\nwindow map\nview Pages\nwindow pages\nkeyboard yes";
        hmi.add_viewed(ViewedArea::parse(views).unwrap()).unwrap();
        hmi.windows(&["sight", "log", "map", "pages"]);
        press(&mut hmi, Button::Area(4));
        hmi
    }

    /// The on-screen keyboard over the program's page that is on screen, if
    /// it is up.
    fn keyboard(hmi: &Hmi) -> Option<KeyboardView> {
        match hmi.frame(at(0), Vec::new()).page {
            Page::External { keyboard, .. } => keyboard,
            _ => None,
        }
    }

    /// What is written on the keys of it that are lit.
    fn lit_over(hmi: &Hmi) -> Vec<String> {
        keyboard(hmi).into_iter().flat_map(|keyboard| keyboard.keys).filter(|key| key.pressed).map(|key| key.text).collect()
    }

    /// The labels down the sides that say anything, and F13's.
    fn said(hmi: &Hmi) -> Vec<(u8, String, LabelState)> {
        let labels = (1..=13).map(|n| (n, label(hmi, n)));
        labels.filter(|(_, (_, state))| *state != LabelState::Blank).map(|(n, (text, state))| (n, text, state)).collect()
    }

    #[test]
    fn a_page_that_can_be_typed_into_has_the_keyboard_on_the_same_button_whichever_it_is() {
        use LabelState::{Available, Selected};
        let mut hmi = with_pages_to_type_into();
        let down = vec![(5, "Keyboard".to_string(), Available), (13, "Up".to_string(), Available)];
        assert_eq!(said(&hmi), down);
        assert_eq!((keyboard(&hmi), hmi.typing_into_window()), (None, false));

        // F5 brings it up, and says that it is up; the buttons of data entry
        // are where a form has them, with what they do for a program.
        buttons(&mut hmi, &[5]);
        assert!(hmi.typing_into_window());
        let keys = keyboard(&hmi).expect("the keyboard is up");
        assert_eq!((keys.layout, keys.keys.len()), (hmi_entry::Layout::Qwerty, 44));
        let texts = |labels: Vec<(u8, String, LabelState)>| -> Vec<(u8, String)> {
            labels.into_iter().map(|(n, text, _)| (n, text.replace('\n', " "))).collect()
        };
        assert_eq!(
            texts(said(&hmi)),
            [(1, "Previous Field"), (5, "Keyboard"), (6, "Left"), (7, "Next Field"), (8, "Return"), (10, "Space")]
                .into_iter()
                .chain([(11, "Backspace"), (12, "Right"), (13, "Cancel")])
                .map(|(n, text)| (n, text.to_string()))
                .collect::<Vec<_>>()
        );
        assert!(said(&hmi).iter().all(|(n, _, state)| *state == if *n == 5 { Selected } else { Available }));
        // The same button puts it away, and so does F13.
        buttons(&mut hmi, &[5]);
        assert_eq!((keyboard(&hmi), said(&hmi)), (None, down.clone()));
        buttons(&mut hmi, &[5, 13]);
        assert_eq!((keyboard(&hmi), said(&hmi)), (None, down.clone()));

        // A view that can be typed into has it on the same button; the view
        // beside it, which cannot, has not, and nor has the area's menu.
        press(&mut hmi, Button::Area(BMS));
        assert_eq!(hmi.window_to_show(), Some("map"));
        assert_eq!(said(&hmi), [(13, "Up".to_string(), Available)]);
        buttons(&mut hmi, &[5]);
        assert_eq!(keyboard(&hmi), None);
        buttons(&mut hmi, &[13]);
        assert!(label(&hmi, 5).1 == LabelState::Blank && hmi.window_to_show().is_none());
        buttons(&mut hmi, &[2]);
        assert_eq!((hmi.window_to_show(), said(&hmi)), (Some("pages"), down));
        buttons(&mut hmi, &[5]);
        assert!(keyboard(&hmi).is_some());

        // Nor has a page whose program takes no typing, nor one of the
        // HMI's own.
        for area in [1, SYS] {
            press(&mut hmi, Button::Area(area));
            buttons(&mut hmi, &[5]);
            assert!(keyboard(&hmi).is_none() && !hmi.typing_into_window());
            assert!(said(&hmi).iter().all(|(_, text, _)| text != "Keyboard"));
        }
    }

    #[test]
    fn what_is_typed_from_the_bezel_or_by_touch_is_handed_on_for_the_program() {
        use Stroke::{BackTab, Backspace, Character, Enter, Tab};
        let mut hmi = with_pages_to_type_into();
        // Nothing is typed while the keyboard is down: the buttons are
        // nobody's, and there is no key to touch.
        buttons(&mut hmi, &[20, 10, 7]);
        assert!(!hmi.touch(Touched::Key(3), at(0)));
        assert_eq!(hmi.take_strokes(), []);

        // The label is a soft key too, and brings it up.
        assert!(hmi.touch(Touched::Label(Button::F(5)), at(0)));
        assert!(hmi.typing_into_window());

        // From the bezel alone: the highlight, Enter on the
        // key it is on - lit until the button is let go - and
        // the buttons that stand for keys.
        press(&mut hmi, Button::F(20));
        assert_eq!(lit_over(&hmi), ["g"]);
        hmi.button(ButtonEvent { button: Button::F(20), action: Action::Released }, at(0));
        assert_eq!(lit_over(&hmi), [] as [&str; 0]);
        buttons(&mut hmi, &[12, 17, 20, 10, 11, 7, 1, 8]);
        assert_eq!(hmi.take_strokes(), [Character('g'), Character('y'), Character(' '), Backspace, Tab, BackTab, Enter]);
        assert_eq!(hmi.take_strokes(), [], "handed on once");

        // By touch: a key is lit until the finger goes, and what it typed
        // is told of at once.
        let keys = keyboard(&hmi).unwrap().keys;
        let key = |text: &str| keys.iter().position(|key| key.text == text).unwrap();
        assert!(hmi.touch(Touched::Key(key("k")), at(0)));
        assert_eq!(lit_over(&hmi), ["k"]);
        assert!(hmi.lift());
        assert_eq!(lit_over(&hmi), [] as [&str; 0]);
        assert!(hmi.touch(Touched::Key(key("Caps")), at(0)) && hmi.lift());
        assert!(hmi.touch(Touched::Key(key("k")), at(0)) && hmi.lift());
        assert_eq!(hmi.take_strokes(), [Character('k'), Character('K')]);

        // A press with nothing between it and the one before types again,
        // and is not passed over as no change.
        press(&mut hmi, Button::F(10));
        assert!(hmi.button(ButtonEvent { button: Button::F(10), action: Action::Pressed }, at(0)));
        assert_eq!(hmi.take_strokes(), [Character(' '), Character(' ')]);
    }

    #[test]
    fn with_the_touch_screen_off_a_program_is_typed_into_from_the_bezel_and_no_other_way() {
        let mut hmi = with_pages_to_type_into();
        press(&mut hmi, Button::Area(SYS));
        buttons(&mut hmi, &[sys_page::TOUCH]);
        press(&mut hmi, Button::Area(4));
        assert!(!hmi.frame(at(0), Vec::new()).touch);

        assert!(!hmi.touch(Touched::Label(Button::F(5)), at(0)));
        assert_eq!(keyboard(&hmi), None);
        buttons(&mut hmi, &[5]);
        assert!(!hmi.touch(Touched::Key(0), at(0)) && !hmi.lift());
        assert_eq!((lit_over(&hmi), hmi.take_strokes()), (Vec::new(), Vec::new()));
        buttons(&mut hmi, &[20, 8]);
        assert_eq!(hmi.take_strokes(), [Stroke::Character('g'), Stroke::Enter]);
    }

    #[test]
    fn f13_puts_the_keyboard_away_before_it_goes_up_and_a_page_is_as_it_was_left() {
        let mut hmi = with_pages_to_type_into();
        press(&mut hmi, Button::Area(BMS));
        buttons(&mut hmi, &[13, 2, 5, 12]);
        assert_eq!(hmi.window_to_show(), Some("pages"));
        let left = keyboard(&hmi).expect("the keyboard is up over the view");
        assert!(left.keys.iter().any(|key| key.text == "h" && key.highlighted));

        // The common functions work while it is up: the
        // Alarms page comes up over it, where the buttons are that page's
        // and nothing is typed; F13 goes back up to the view, as it was.
        buttons(&mut hmi, &[14]);
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Alarms { .. }));
        assert!(!hmi.typing_into_window());
        buttons(&mut hmi, &[20, 10, 13]);
        assert_eq!((keyboard(&hmi).as_ref(), hmi.take_strokes()), (Some(&left), Vec::new()));
        // F19 hides what it hides anywhere, and the buttons go on typing.
        buttons(&mut hmi, &[19, 20]);
        assert!(hmi.frame(at(0), Vec::new()).hidden.labels);
        assert_eq!(hmi.take_strokes(), [Stroke::Character('h')]);
        buttons(&mut hmi, &[19, 19]);

        // Another area and back, and the area's own menu and back.
        press(&mut hmi, Button::Area(4));
        assert_eq!(keyboard(&hmi), None, "DRV's keyboard is DRV's to bring up");
        press(&mut hmi, Button::Area(BMS));
        assert_eq!(keyboard(&hmi).as_ref(), Some(&left));
        press(&mut hmi, Button::Area(BMS));
        assert!(hmi.window_to_show().is_none() && keyboard(&hmi).is_none());
        buttons(&mut hmi, &[2]);
        assert_eq!(keyboard(&hmi).as_ref(), Some(&left));

        // F13 cancels data entry first, and only then goes up to the menu.
        buttons(&mut hmi, &[13]);
        assert_eq!((keyboard(&hmi), hmi.window_to_show()), (None, Some("pages")));
        buttons(&mut hmi, &[13]);
        assert_eq!(hmi.window_to_show(), None);

        // What was typed for a page that has gone from the screen since is
        // nobody's; and a window that goes takes its keyboard with it.
        buttons(&mut hmi, &[2, 5]);
        press(&mut hmi, Button::F(20));
        press(&mut hmi, Button::Area(SYS));
        assert_eq!(hmi.take_strokes(), []);
        hmi.windows(&["sight", "log", "map"]);
        hmi.windows(&["sight", "log", "map", "pages"]);
        press(&mut hmi, Button::Area(BMS));
        buttons(&mut hmi, &[2]);
        assert_eq!((hmi.window_to_show(), keyboard(&hmi)), (Some("pages"), None));
    }

    #[test]
    fn a_program_s_page_with_the_keyboard_up_serialises_in_the_shape_the_ui_reads() {
        let mut hmi = with_pages_to_type_into();
        let json = serde_json::to_value(hmi.frame(at(0), Vec::new())).unwrap();
        assert_eq!(json["page"], serde_json::json!({"kind": "external", "overlay": null, "keyboard": null}));
        assert_eq!(json["labels"][4], serde_json::json!({"key": "F5", "text": "Keyboard", "state": "available"}));

        buttons(&mut hmi, &[5]);
        hmi.touch(Touched::Key(10), at(0));
        let json = serde_json::to_value(hmi.frame(at(0), Vec::new())).unwrap();
        let keyboard = &json["page"]["keyboard"];
        assert_eq!((&json["page"]["kind"], &keyboard["layout"]), (&"external".into(), &"qwerty".into()));
        assert_eq!((&keyboard["columns"], &keyboard["rows"]), (&20.into(), &5.into()));
        assert_eq!(
            keyboard["keys"][10],
            serde_json::json!({
                "text": "q", "kind": "character", "row": 1, "column": 0, "span": 2,
                "available": true, "highlighted": true, "pressed": true, "on": false,
            })
        );
        assert_eq!(json["labels"][4]["state"], "selected");
        assert_eq!(json["labels"][7]["text"], "Return");
    }

    #[test]
    fn a_field_that_takes_the_focus_has_its_keyboard_come_up_by_itself_and_go_when_it_lets_go() {
        use LabelState::{Available, Unavailable};
        let mut hmi = with_pages_to_type_into();
        // A program need not be one the crew can bring a keyboard up over:
        // the sight's page has no such button, and a field of it that took
        // the focus would have the keyboard all the same.
        press(&mut hmi, Button::Area(1));
        assert_eq!((hmi.window_to_show(), said(&hmi).len()), (Some("sight"), 1));
        assert!(hmi.asked(Some(Kind::Text)));
        let keys = keyboard(&hmi).expect("the keyboard has come up");
        assert_eq!(keys.layout, hmi_entry::Layout::Qwerty);
        assert!(keys.keys.iter().any(|key| key.text == "g" && key.highlighted));
        assert!(said(&hmi).iter().all(|(n, text, _)| *n != 5 && text != "Keyboard"), "no button of its own");
        assert!(label(&hmi, 13).0 == "Cancel" && label(&hmi, 10).1 == Available);
        buttons(&mut hmi, &[5, 20]);
        assert_eq!(hmi.take_strokes(), [Stroke::Character('g')]);
        assert!(!hmi.asked(Some(Kind::Text)), "said again of the same field, with the keyboard up: nothing new");

        // On to a number: the pad, without its point for a whole one, and
        // no Space. And when the field lets go, the keyboard goes.
        assert!(hmi.asked(Some(Kind::Number)));
        let pad = keyboard(&hmi).unwrap();
        assert_eq!(pad.layout, hmi_entry::Layout::Phone);
        assert!(pad.keys.iter().all(|key| key.available == (key.text != ".")));
        assert_eq!(label(&hmi, 10), ("Space".to_string(), Unavailable));
        assert!(hmi.asked(Some(Kind::Decimal)));
        assert!(keyboard(&hmi).unwrap().keys.iter().all(|key| key.available));
        buttons(&mut hmi, &[20, 10]);
        assert_eq!(hmi.take_strokes(), [Stroke::Character('5')]);
        assert!(hmi.asked(None));
        assert_eq!((keyboard(&hmi), said(&hmi).len(), hmi.typing_into_window()), (None, 1, false));
        assert!(!hmi.asked(None));
    }

    #[test]
    fn a_keyboard_that_came_up_by_itself_is_put_away_at_once_and_stays_away_until_asked_for_again() {
        let mut hmi = with_pages_to_type_into();
        assert!(hmi.asked(Some(Kind::Text)));
        // F13 cancels data entry, as on a form; the field has the focus
        // still, and the keyboard does not come back for that.
        buttons(&mut hmi, &[13]);
        assert_eq!((keyboard(&hmi), hmi.window_to_show()), (None, Some("log")));
        buttons(&mut hmi, &[20, 10, 7]);
        assert_eq!(hmi.take_strokes(), []);
        // The field is touched again: the program asks again.
        assert!(hmi.asked(Some(Kind::Text)));
        assert!(keyboard(&hmi).is_some());
        // And on a page that has the button, the button puts it away too.
        buttons(&mut hmi, &[5]);
        assert_eq!(keyboard(&hmi), None);

        // A keyboard the crew brought up themselves is theirs: a field does
        // not change it for another, nor take it away by letting go.
        buttons(&mut hmi, &[5]);
        assert!(!hmi.asked(Some(Kind::Number)) && !hmi.asked(None));
        assert_eq!(keyboard(&hmi).map(|keys| keys.layout), Some(hmi_entry::Layout::Qwerty));
    }

    #[test]
    fn no_keyboard_comes_up_by_itself_over_a_page_of_the_hmis_own() {
        let mut hmi = with_pages_to_type_into();
        // The Alarms page over a program's: the field under it has let go
        // by then, and what is said for a page that is not to be seen
        // brings nothing up. Back on the page there is no keyboard until
        // its program asks again, which it does if a field still has the
        // focus.
        assert!(hmi.asked(Some(Kind::Text)));
        buttons(&mut hmi, &[14]);
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Alarms { .. }));
        assert!(!hmi.typing_into_window());
        hmi.asked(None);
        assert!(!hmi.asked(Some(Kind::Text)));
        buttons(&mut hmi, &[13]);
        assert_eq!((keyboard(&hmi), hmi.window_to_show()), (None, Some("log")));
        assert!(hmi.asked(Some(Kind::Text)));

        // Nor over SYS, an area's menu, or a page of tiles.
        hmi.asked(None);
        press(&mut hmi, Button::Area(SYS));
        assert!(!hmi.asked(Some(Kind::Text)));
        press(&mut hmi, Button::Area(BMS));
        press(&mut hmi, Button::Area(BMS));
        assert!(hmi.window_to_show().is_none() && !hmi.asked(Some(Kind::Text)));
        let mut tiled = with_tiles();
        press(&mut tiled, Button::Area(0));
        assert!(!tiled.asked(Some(Kind::Text)));
        assert!(matches!(tiled.frame(at(0), Vec::new()).page, Page::Tiles { .. }));
    }

    /// What a program says for itself of the window it names, as the line
    /// it writes. Returns whether the frame changed.
    fn says(hmi: &mut Hmi, line: &str) -> bool {
        hmi.asked_for(&Asking::parse(line).unwrap())
    }

    #[test]
    fn what_a_program_says_of_its_own_window_is_a_line_that_names_it_and_what_its_field_takes() {
        let asking = |line: &str| Asking::parse(line).map(|asking| (asking.window, asking.kind));
        assert_eq!(asking("map text"), Ok(("map".to_string(), Some(Kind::Text))));
        assert_eq!(asking("map number"), Ok(("map".to_string(), Some(Kind::Number))));
        assert_eq!(asking("map decimal"), Ok(("map".to_string(), Some(Kind::Decimal))));
        assert_eq!(asking("map nothing"), Ok(("map".to_string(), None)));
        // A window is named as a configuration names it, in as many words
        // as that takes; and a line ends as lines do.
        assert_eq!(asking("  Fire Plan \t text\r\n"), Ok(("Fire Plan".to_string(), Some(Kind::Text))));
        let refused = [
            ("map", "'map' is not"),
            ("map keyboard", "'keyboard' is not"),
            ("map Text", "'Text' is not"),
            ("", "'' is not"),
            ("text", "'text' is said of no window"),
            (" nothing ", "'nothing' is said of no window"),
        ];
        for (line, why) in refused {
            assert!(asking(line).is_err_and(|error| error.contains(why)), "{line:?}: {:?}", asking(line));
        }
    }

    #[test]
    fn a_program_that_says_for_itself_that_a_field_has_the_focus_has_the_keyboard_come_up_and_go() {
        use LabelState::{Available, Unavailable};
        let mut hmi = with_pages_to_type_into();
        // As with what the display server says: the sight's page has no
        // button for the keyboard, and has the keyboard all the same.
        press(&mut hmi, Button::Area(1));
        assert_eq!((hmi.window_to_show(), said(&hmi).len()), (Some("sight"), 1));
        assert!(says(&mut hmi, "sight text"));
        let keys = keyboard(&hmi).expect("the keyboard has come up");
        assert_eq!(keys.layout, hmi_entry::Layout::Qwerty);
        assert!(said(&hmi).iter().all(|(n, text, _)| *n != 5 && text != "Keyboard"), "no button of its own");
        assert!(label(&hmi, 13).0 == "Cancel" && label(&hmi, 10).1 == Available);
        buttons(&mut hmi, &[5, 20]);
        assert_eq!(hmi.take_strokes(), [Stroke::Character('g')]);
        assert!(!says(&mut hmi, "sight text"), "said again of the same field, with the keyboard up: nothing new");

        // On to a number: the pad, without its point for a whole one. And
        // when the field lets go, the keyboard goes.
        assert!(says(&mut hmi, "sight number"));
        let pad = keyboard(&hmi).unwrap();
        assert_eq!(pad.layout, hmi_entry::Layout::Phone);
        assert!(pad.keys.iter().all(|key| key.available == (key.text != ".")));
        assert_eq!(label(&hmi, 10), ("Space".to_string(), Unavailable));
        assert!(says(&mut hmi, "sight decimal"));
        assert!(keyboard(&hmi).unwrap().keys.iter().all(|key| key.available));
        buttons(&mut hmi, &[20]);
        assert_eq!(hmi.take_strokes(), [Stroke::Character('5')]);
        assert!(says(&mut hmi, "sight nothing"));
        assert_eq!((keyboard(&hmi), said(&hmi).len(), hmi.typing_into_window()), (None, 1, false));
        assert!(!says(&mut hmi, "sight nothing"));
    }

    #[test]
    fn a_program_is_heard_only_while_its_window_has_the_page_to_itself() {
        let mut hmi = with_pages_to_type_into();
        // DRV's page is on screen. Said of a window that is another area's,
        // or nobody's: nothing comes up over the page that is on screen,
        // and nothing over the window's own when that is next seen.
        assert!(!says(&mut hmi, "sight number") && !says(&mut hmi, "pages text") && !says(&mut hmi, "elsewhere text"));
        assert_eq!(keyboard(&hmi), None);
        press(&mut hmi, Button::Area(1));
        assert_eq!((hmi.window_to_show(), keyboard(&hmi)), (Some("sight"), None));
        // Until its field asks again, which it does when it is touched.
        assert!(says(&mut hmi, "sight number"));

        // The Alarms page over it is the HMI's own: nothing said brings a
        // keyboard up there, and back on the program's page the keyboard is
        // as it was left - the pad - whatever was said meanwhile.
        buttons(&mut hmi, &[14]);
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Alarms { .. }));
        assert!(!says(&mut hmi, "sight text") && !hmi.typing_into_window());
        buttons(&mut hmi, &[13]);
        assert_eq!(keyboard(&hmi).map(|keys| keys.layout), Some(hmi_entry::Layout::Phone));

        // That the field has let go is heard wherever the crew are: nothing
        // on the screen changes for it, and the keyboard is not there when
        // the page is come back to.
        press(&mut hmi, Button::Area(SYS));
        assert!(!says(&mut hmi, "sight text"));
        assert!(!says(&mut hmi, "sight nothing"));
        press(&mut hmi, Button::Area(1));
        assert_eq!((hmi.window_to_show(), keyboard(&hmi)), (Some("sight"), None));

        // A view is heard while it is the one viewed; not the view beside
        // it, and neither while the area's menu has the page.
        press(&mut hmi, Button::Area(BMS));
        assert_eq!(hmi.window_to_show(), Some("map"));
        assert!(!says(&mut hmi, "pages text") && keyboard(&hmi).is_none());
        assert!(says(&mut hmi, "map text") && keyboard(&hmi).is_some());
        press(&mut hmi, Button::Area(BMS));
        assert!(hmi.window_to_show().is_none() && !says(&mut hmi, "map number") && !says(&mut hmi, "pages text"));
        buttons(&mut hmi, &[1]);
        assert_eq!(keyboard(&hmi).map(|keys| keys.layout), Some(hmi_entry::Layout::Qwerty));

        // And not a window in a tile: there is no saying, there, which of
        // several a key would be for.
        let mut tiled = with_tiles();
        press(&mut tiled, Button::Area(0));
        buttons(&mut tiled, &[1]);
        assert_eq!(tiles(&tiled), ["A"]);
        assert!(!says(&mut tiled, "a text"));
        assert!(matches!(tiled.frame(at(0), Vec::new()).page, Page::Tiles { .. }));
    }

    #[test]
    fn a_keyboard_a_program_asked_for_is_put_away_by_f13_and_by_nobody_who_did_not_bring_it_up() {
        let mut hmi = with_pages_to_type_into();
        assert!(says(&mut hmi, "log text"));
        // F13 cancels data entry, as on a form. The field has the focus
        // still, and its program says nothing new: the keyboard stays away.
        buttons(&mut hmi, &[13]);
        assert_eq!((keyboard(&hmi), hmi.window_to_show()), (None, Some("log")));
        buttons(&mut hmi, &[20, 10, 7]);
        assert_eq!(hmi.take_strokes(), []);
        // The field is touched again, and its program asks again.
        assert!(says(&mut hmi, "log text"));
        assert!(keyboard(&hmi).is_some());

        // Each puts away what it brought up. The display server's word that
        // no field has the focus is of the fields it spoke for, and takes
        // away no keyboard a program asked for - nor the other way about.
        assert!(!hmi.asked(None) && keyboard(&hmi).is_some());
        assert!(says(&mut hmi, "log nothing"));
        assert!(hmi.asked(Some(Kind::Number)));
        assert!(!says(&mut hmi, "log nothing"));
        assert_eq!(keyboard(&hmi).map(|keys| keys.layout), Some(hmi_entry::Layout::Phone));
        assert!(hmi.asked(None) && keyboard(&hmi).is_none());

        // A keyboard the crew brought up themselves is theirs: a program
        // does not change it for another, nor take it away.
        buttons(&mut hmi, &[5]);
        assert!(!says(&mut hmi, "log number") && !says(&mut hmi, "log nothing"));
        assert_eq!(keyboard(&hmi).map(|keys| keys.layout), Some(hmi_entry::Layout::Qwerty));
        buttons(&mut hmi, &[5]);

        // And a window that goes takes its keyboard with it: its program,
        // started again, is not where it was.
        assert!(says(&mut hmi, "log text"));
        hmi.windows(&["sight", "map", "pages"]);
        hmi.windows(&["sight", "log", "map", "pages"]);
        press(&mut hmi, Button::Area(4));
        assert_eq!((hmi.window_to_show(), keyboard(&hmi)), (Some("log"), None));
    }

    #[test]
    fn no_keyboard_comes_up_for_a_program_while_a_pin_is_asked_for() {
        // The sight is anybody's, and the log the commander's.
        let members = |role: &str| if role == "commander" { vec!["cmdr".to_string()] } else { Vec::new() };
        let mut hmi = Hmi::new();
        hmi.set_access(AccessConfig::read("role commander", &FUNCTIONS, members).unwrap());
        hmi.add_external(ExternalArea::parse("area WPN\nwindow sight").unwrap()).unwrap();
        hmi.add_external(ExternalArea::parse("area DRV\nwindow log\nrequire commander").unwrap()).unwrap();
        hmi.windows(&["sight", "log"]);
        press(&mut hmi, Button::Area(1));
        assert!(says(&mut hmi, "sight text"));

        // The crew go to what needs a role, and the page is the PIN's:
        // nothing a program says brings a keyboard up over it - not the
        // program whose page it came up over, nor the one that was wanted.
        press(&mut hmi, Button::Area(4));
        assert!(hmi.pin_asked() && !hmi.typing_into_window());
        assert!(!says(&mut hmi, "sight number") && !says(&mut hmi, "log text"));
        assert!(matches!(hmi.frame(at(0), Vec::new()).page, Page::Pin(_)) && !hmi.typing_into_window());
        assert_eq!(hmi.take_strokes(), []);

        // Cancelled, the crew are at the sight's page with its keyboard as
        // they left it: what was said meanwhile was not heard.
        buttons(&mut hmi, &[13]);
        assert_eq!((hmi.window_to_show(), hmi.pin_asked()), (Some("sight"), false));
        assert_eq!(keyboard(&hmi).map(|keys| keys.layout), Some(hmi_entry::Layout::Qwerty));
        // But that its field had let go would have been.
        press(&mut hmi, Button::Area(4));
        assert!(hmi.pin_asked() && !says(&mut hmi, "sight nothing"));
        buttons(&mut hmi, &[13]);
        assert_eq!((hmi.window_to_show(), keyboard(&hmi)), (Some("sight"), None));
    }
}
