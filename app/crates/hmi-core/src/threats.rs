//! Threats: what is out there, by the account of the vehicle's sensors, and
//! what the crew has done about it.
//!
//! A threat is detected, and in time found to be gone, by its source -
//! whatever knows the vehicle's sensors. The crew acknowledges and cancels.
//! This module keeps the list of threats, decides which one has its text
//! message on the screen, says where the current ones are for the Plan
//! Position Indicator, and keeps the table behind the Threats page - which
//! can also be had as a picture, the threats drawn where they are by the
//! symbols MIL-STD-2525C gives them. The sensors' own health is kept here
//! too: a threat display says what its picture is made from.

use crate::alarms::Time;
use crate::dtg::{date_time_group, day_time_group};
use crate::feed::Request;
use crate::table::{Paged, View};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;

/// The rows on one page of the threats table: as many as fit under the Plan
/// Position Indicator when every one of them needs a second line for its
/// text.
pub const ROWS_PER_PAGE: usize = 8;
/// Threats that are past are kept in the table until the crew cancels them,
/// but not without limit: beyond this many threats the oldest of those go.
const MOST_THREATS: usize = 200;
/// Mils to the circle.
const CIRCLE: u16 = 6400;

/// The symbol of a threat whose source gives it none, as MIL-STD-2525C
/// identifies symbols: hostile, on the ground, there now, and no more known
/// of what it is.
pub const USUAL_SYMBOL: &str = "SHGP-----------";
/// The characters of a MIL-STD-2525C symbol identification code.
const SYMBOL_LENGTH: usize = 15;

/// How far out the edge of the threats' picture can be, in metres: it is the
/// nearest of these that takes in the farthest threat...
const SCALES: [u32; 9] = [100, 200, 500, 1000, 2000, 5000, 10_000, 20_000, 50_000];
/// ...and this where no threat's range is known.
const USUAL_SCALE: u32 = 1000;

/// A symbol identification code as it is kept: `said`, in capitals and made
/// up to its full length with the dashes that stand for what is not said.
/// `None` if it cannot be one. Whether it identifies a symbol there is, is
/// for whatever draws it to find.
fn symbol(said: &str) -> Option<String> {
    let said = said.trim().to_ascii_uppercase();
    let of_a_code = |c: char| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-';
    ((4..=SYMBOL_LENGTH).contains(&said.len()) && said.chars().all(of_a_code))
        .then(|| format!("{said:-<SYMBOL_LENGTH$}"))
}

/// How much a threat matters, most first. What makes one matter more than
/// another is for whatever detects them to say.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    /// What a threat is taken to be when its source does not say.
    #[default]
    High,
    Medium,
    Low,
}

/// A threat as its source describes it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detected {
    /// The source's name for the threat: what it is ended by, and what the
    /// source is told the crew has done to.
    pub id: String,
    /// What type of threat it is: "Laser", "Gunshot".
    #[serde(rename = "type")]
    pub kind: String,
    /// More about what it is: "Rangefinder", "Small Arms".
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub priority: Priority,
    /// Where it is from the vehicle, if that is known: mils clockwise from
    /// the vehicle's front, 0 to 6399...
    #[serde(default)]
    pub bearing: Option<u16>,
    /// ...and how far, in metres.
    #[serde(default)]
    pub range: Option<u32>,
    /// What to draw it as on the threats' picture: a MIL-STD-2525C symbol
    /// identification code, whose trailing dashes can be left off. Without
    /// one it is drawn as [`USUAL_SYMBOL`].
    #[serde(default)]
    pub symbol: Option<String>,
    /// More about it, for the crew to call up.
    #[serde(default)]
    pub detail: Option<String>,
    /// When it was detected, in seconds since 1970, if that was not just now.
    #[serde(default)]
    pub time: Option<u64>,
}

impl Detected {
    /// Why this cannot be a threat, if it cannot.
    pub fn fault(&self) -> Option<&'static str> {
        if self.id.is_empty() {
            Some("a threat needs an id")
        } else if self.kind.trim().is_empty() {
            Some("a threat needs a type")
        } else if self.bearing.is_some_and(|bearing| bearing >= CIRCLE) {
            Some("a bearing is 0 to 6399 mils")
        } else if self.symbol.as_deref().is_some_and(|said| symbol(said).is_none()) {
            Some("a symbol is a MIL-STD-2525C symbol identification code: 4 to 15 letters, digits and dashes")
        } else {
            None
        }
    }
}

/// Where a threat stands with its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThreatState {
    /// It is there.
    Current,
    /// It has gone. It stays in the table until cancelled.
    Past,
}

/// The threat text message, which goes along the bottom of the page: not
/// where an alarm's does, so that both can be read at once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThreatMessage {
    /// "T – Laser Rangefinder": as an alarm's is written.
    pub text: String,
    pub acknowledged: bool,
}

/// A current threat's place on the Plan Position Indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreatMark {
    /// Mils clockwise from the vehicle's front.
    pub bearing_mils: u16,
    /// Whether it is the threat picked out in the threats table.
    pub picked: bool,
}

/// How many threats there are, there and gone.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ThreatCounts {
    pub current: u32,
    pub past: u32,
}

/// A row of the threats table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreatRow {
    /// Which threat it is, as its source names it: what a touch on the row
    /// picks it by.
    pub id: String,
    /// "T – Laser Rangefinder": its type and description, written as an
    /// alarm in the alarms table is.
    pub text: String,
    /// When it was detected: the day and time of its date-time group. The
    /// month and year are in [`ThreatDetail`].
    pub time: String,
    /// Where it is from the vehicle, as far as is known: mils clockwise
    /// from the vehicle's front...
    pub bearing_mils: Option<u16>,
    /// ...and metres.
    pub range_m: Option<u32>,
    pub state: ThreatState,
    pub selected: bool,
}

/// The threats as a picture: the Plan Position Indicator at the size of the
/// page, with each threat on it where it is from the vehicle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreatPlot {
    /// How far out the outer range ring is, in metres.
    pub range_m: u32,
    /// The listed threats that are known to be somewhere.
    pub threats: Vec<Plotted>,
    /// How many of them are not, and so are not on the picture: there is no
    /// saying which way they are. The table has them.
    pub elsewhere: u32,
}

/// A threat on the picture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plotted {
    /// Which threat it is, as its source names it: what a touch on it picks
    /// it by.
    pub id: String,
    /// What it is drawn as: a MIL-STD-2525C symbol identification code.
    pub symbol: String,
    /// What type of threat it is, to be written by its symbol.
    pub label: String,
    /// Mils clockwise from the vehicle's front.
    pub bearing_mils: u16,
    /// Metres from the vehicle. `None` for a threat known only by its
    /// direction, which goes at the edge.
    pub range_m: Option<u32>,
    pub state: ThreatState,
    pub picked: bool,
}

/// One threat at length.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreatDetail {
    pub text: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub description: String,
    pub priority: Priority,
    pub time: String,
    pub bearing_mils: Option<u16>,
    pub range_m: Option<u32>,
    pub state: ThreatState,
    pub acknowledged: bool,
    /// What the source had to add, if anything.
    pub detail: Option<String>,
}

/// What a sensor that looks for threats says of itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SensorStatus {
    Ok,
    /// Working, with less than all of itself.
    Degraded,
    Failed,
    Off,
}

/// A sensor that looks for threats, and how it is: the crew
/// are to know what the picture they are shown is made from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sensor {
    /// What the crew know it as: "Laser Warning". A sensor said again by
    /// the same name is the same sensor.
    pub name: String,
    pub status: SensorStatus,
}

#[derive(Debug, Clone)]
struct Threat {
    id: String,
    kind: String,
    description: String,
    priority: Priority,
    bearing: Option<u16>,
    range: Option<u32>,
    /// A MIL-STD-2525C symbol identification code, at its full length.
    symbol: String,
    detail: Option<String>,
    /// When it was detected, seconds since 1970.
    at: u64,
    /// The order threats were detected in, to tell apart those of one second.
    order: u64,
    state: ThreatState,
    acknowledged: bool,
}

impl Threat {
    fn text(&self) -> String {
        format!("T \u{2013} {} {}", self.kind, self.description).trim_end().to_string()
    }

    fn current(&self) -> bool {
        self.state == ThreatState::Current
    }

    fn row(&self, selected: bool) -> ThreatRow {
        ThreatRow {
            id: self.id.clone(),
            text: self.text(),
            time: day_time_group(self.at),
            bearing_mils: self.bearing,
            range_m: self.range,
            state: self.state,
            selected,
        }
    }
}

/// The threats the HMI knows of, and the sensors that look for them.
#[derive(Debug, Default)]
pub struct Threats {
    list: Vec<Threat>,
    detected: u64,
    sensors: Vec<Sensor>,
    requests: Vec<Request>,
}

impl Threats {
    /// The source says there is a threat.
    pub fn detect(&mut self, detected: Detected, now: Time) {
        let kind = detected.kind.trim().to_string();
        let description = detected.description.trim().to_string();
        let symbol = detected.symbol.as_deref().and_then(symbol).unwrap_or_else(|| USUAL_SYMBOL.to_string());
        match self.list.iter_mut().find(|threat| threat.id == detected.id) {
            Some(threat) => {
                // Said again of a threat that is there: it has moved, or
                // more is known of it. Of one that had gone, or that
                // matters more than it did: it is a new threat.
                let anew = !threat.current() || detected.priority < threat.priority;
                threat.kind = kind;
                threat.description = description;
                threat.priority = detected.priority;
                threat.bearing = detected.bearing;
                threat.range = detected.range;
                threat.symbol = symbol;
                threat.detail = detected.detail;
                if anew {
                    threat.state = ThreatState::Current;
                    threat.at = detected.time.unwrap_or(now.unix_s);
                    threat.order = self.detected;
                    self.detected += 1;
                    threat.acknowledged = false;
                }
            }
            None => {
                self.list.push(Threat {
                    id: detected.id,
                    kind,
                    description,
                    priority: detected.priority,
                    bearing: detected.bearing,
                    range: detected.range,
                    symbol,
                    detail: detected.detail,
                    at: detected.time.unwrap_or(now.unix_s),
                    order: self.detected,
                    state: ThreatState::Current,
                    acknowledged: false,
                });
                self.detected += 1;
                self.forget_the_oldest();
            }
        }
    }

    /// The source says a threat is no longer there.
    pub fn end(&mut self, id: &str) {
        if let Some(threat) = self.list.iter_mut().find(|threat| threat.id == id) {
            threat.state = ThreatState::Past;
        }
    }

    /// F16: acknowledge the threat whose text message is on the screen.
    /// The message stays, as an acknowledged warning's does,
    /// while the threat is there.
    pub fn acknowledge_presented(&mut self) {
        if let Some(id) = self.presented().filter(|threat| !threat.acknowledged).map(|threat| threat.id.clone()) {
            if let Some(threat) = self.list.iter_mut().find(|threat| threat.id == id) {
                threat.acknowledged = true;
            }
            self.requests.push(Request::Acknowledged(id));
        }
    }

    /// Take a threat off the display, there or gone. If its
    /// source says it again, that is a new threat.
    pub fn cancel(&mut self, id: &str) {
        if let Some(index) = self.list.iter().position(|threat| threat.id == id) {
            self.requests.push(Request::Cancelled(self.list.remove(index).id));
        }
    }

    pub fn cancel_all(&mut self) {
        for threat in self.list.drain(..) {
            self.requests.push(Request::Cancelled(threat.id));
        }
    }

    /// What the crew has done since this was last asked, for the source.
    pub fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }

    /// The text message to show, if any.
    pub fn message(&self) -> Option<ThreatMessage> {
        self.presented().map(|threat| ThreatMessage { text: threat.text(), acknowledged: threat.acknowledged })
    }

    /// Where the current threats are, for the Plan Position Indicator: those
    /// of them that are known to be anywhere. `picked` is the threat picked
    /// out in the table, if one is.
    pub fn marks(&self, picked: Option<&str>) -> Vec<ThreatMark> {
        self.list
            .iter()
            .filter(|threat| threat.current())
            .filter_map(|threat| {
                Some(ThreatMark { bearing_mils: threat.bearing?, picked: picked == Some(threat.id.as_str()) })
            })
            .collect()
    }

    pub fn counts(&self) -> ThreatCounts {
        let current = self.list.iter().filter(|threat| threat.current()).count() as u32;
        ThreatCounts { current, past: self.list.len() as u32 - current }
    }

    /// Whether there are no threats at all, there or gone.
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// A sensor says how it is.
    pub fn sensor(&mut self, sensor: Sensor) {
        match self.sensors.iter_mut().find(|known| known.name == sensor.name) {
            Some(known) => known.status = sensor.status,
            None => self.sensors.push(sensor),
        }
    }

    /// The sensors that have said how they are, in the order they first did.
    pub fn sensors(&self) -> &[Sensor] {
        &self.sensors
    }

    /// The threat whose text message is on the screen: of those that are
    /// there, one not yet acknowledged before one that has been, then the
    /// one that matters most, then the latest.
    fn presented(&self) -> Option<&Threat> {
        self.list
            .iter()
            .filter(|threat| threat.current())
            .max_by_key(|threat| (!threat.acknowledged, Reverse(threat.priority), threat.at, threat.order))
    }

    fn forget_the_oldest(&mut self) {
        while self.list.len() > MOST_THREATS {
            let oldest = self
                .list
                .iter()
                .enumerate()
                .filter(|(_, threat)| !threat.current())
                .min_by_key(|(_, threat)| threat.order)
                .map(|(index, _)| index);
            match oldest {
                Some(index) => drop(self.list.remove(index)),
                None => break,
            }
        }
    }
}

/// The order of the rows of the threats table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    /// By type, the types that matter most first, then by time, latest
    /// first.
    Type,
    /// By time, latest first, then by type, the types that matter most
    /// first.
    Time,
}

/// The threats table as the crew has it set: its order, whether it lists
/// threats that are past, which row is picked and which page is showing -
/// and whether it is being looked at as a table at all, or as a picture.
#[derive(Debug, Clone)]
pub struct Table {
    pub sort: Sort,
    /// Whether threats that have gone are listed: leaving them out is how
    /// the table, or the picture, is cleared of what is out of date without
    /// losing it.
    pub past: bool,
    /// Whether the listed threats are shown as a picture ([`Table::plot`])
    /// in place of the table.
    pub plotted: bool,
    view: View,
}

impl Default for Table {
    /// As the Threats page opens: a table of everything, by type - as the
    /// alarms table opens by category.
    fn default() -> Self {
        Table { sort: Sort::Type, past: true, plotted: false, view: View::default() }
    }
}

impl Paged for Table {
    type From = Threats;
    const ROWS: usize = ROWS_PER_PAGE;

    fn view(&self) -> &View {
        &self.view
    }

    fn view_mut(&mut self) -> &mut View {
        &mut self.view
    }

    fn names<'a>(&self, threats: &'a Threats) -> Vec<&'a str> {
        self.listed(threats).into_iter().map(|threat| threat.id.as_str()).collect()
    }

    fn has(threats: &Threats, name: &str) -> bool {
        threats.list.iter().any(|threat| threat.id == name)
    }
}

impl Table {
    fn listed<'a>(&self, threats: &'a Threats) -> Vec<&'a Threat> {
        let mut listed: Vec<&Threat> =
            threats.list.iter().filter(|threat| self.past || threat.current()).collect();
        // A type matters as much as the threat of it that matters most.
        let of_type = |threat: &Threat| {
            let most = listed.iter().filter(|other| other.kind == threat.kind).map(|other| other.priority).min();
            (most.unwrap_or(threat.priority), threat.kind.clone())
        };
        let keys: Vec<_> = listed.iter().map(|threat| of_type(threat)).collect();
        let mut keyed: Vec<_> = listed.drain(..).zip(keys).collect();
        match self.sort {
            Sort::Type => keyed.sort_by_key(|(t, of_type)| (of_type.clone(), Reverse(t.at), Reverse(t.order))),
            Sort::Time => keyed.sort_by_key(|(t, of_type)| (Reverse(t.at), of_type.clone(), Reverse(t.order))),
        }
        keyed.into_iter().map(|(threat, _)| threat).collect()
    }

    /// The threat picked out, for the Plan Position Indicator to mark.
    pub fn picked<'a>(&'a self, threats: &Threats) -> Option<&'a str> {
        self.selected(threats)
    }

    /// The page showing (from 1), how many there are, and its rows.
    pub fn rows(&self, threats: &Threats) -> (usize, usize, Vec<ThreatRow>) {
        let on_screen = self.on_screen(threats);
        let rows = self
            .listed(threats)
            .iter()
            .enumerate()
            .skip(on_screen.first)
            .take(ROWS_PER_PAGE)
            .map(|(at, threat)| threat.row(on_screen.picked == Some(at)))
            .collect();
        (on_screen.page, on_screen.pages, rows)
    }

    /// The listed threats as a picture: every one of them at once, where it
    /// is, out to a range that takes in the farthest.
    pub fn plot(&self, threats: &Threats) -> ThreatPlot {
        let listed = self.listed(threats);
        let picked = self.on_screen(threats).picked;
        let farthest = listed.iter().filter(|threat| threat.bearing.is_some()).filter_map(|threat| threat.range).max();
        let range_m = match farthest {
            Some(farthest) => SCALES.into_iter().find(|&scale| farthest <= scale).unwrap_or(SCALES[SCALES.len() - 1]),
            None => USUAL_SCALE,
        };
        let mut plot = ThreatPlot { range_m, threats: Vec::new(), elsewhere: 0 };
        for (at, threat) in listed.iter().enumerate() {
            match threat.bearing {
                Some(bearing_mils) => plot.threats.push(Plotted {
                    id: threat.id.clone(),
                    symbol: threat.symbol.clone(),
                    label: threat.kind.clone(),
                    bearing_mils,
                    range_m: threat.range,
                    state: threat.state,
                    picked: picked == Some(at),
                }),
                None => plot.elsewhere += 1,
            }
        }
        plot
    }

    /// The threat shown at length, if one is.
    pub fn detail(&self, threats: &Threats) -> Option<ThreatDetail> {
        let shown = self.shown()?;
        let threat = threats.list.iter().find(|threat| threat.id == shown)?;
        Some(ThreatDetail {
            text: threat.text(),
            kind: threat.kind.clone(),
            description: threat.description.clone(),
            priority: threat.priority,
            time: date_time_group(threat.at),
            bearing_mils: threat.bearing,
            range_m: threat.range,
            state: threat.state,
            acknowledged: threat.acknowledged,
            detail: threat.detail.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A moment `s` seconds after the tests' beginning of time.
    fn at(s: u64) -> Time {
        Time { unix_s: 1_728_045_000 + s, uptime_ms: s * 1000 }
    }

    fn threat(id: &str, kind: &str, description: &str) -> Detected {
        Detected {
            id: id.to_string(),
            kind: kind.to_string(),
            description: description.to_string(),
            priority: Priority::High,
            bearing: None,
            range: None,
            symbol: None,
            detail: None,
            time: None,
        }
    }

    fn several(threats: &[(&str, &str, Priority)]) -> Threats {
        let mut all = Threats::default();
        for (s, (id, kind, priority)) in threats.iter().enumerate() {
            all.detect(Detected { priority: *priority, ..threat(id, kind, "") }, at(60 * s as u64));
        }
        all
    }

    fn text(threats: &Threats) -> Option<String> {
        threats.message().map(|message| message.text)
    }

    fn texts(table: &Table, threats: &Threats) -> Vec<String> {
        table.rows(threats).2.into_iter().map(|row| row.text).collect()
    }

    fn ids(table: &Table, threats: &Threats) -> Vec<String> {
        table.names(threats).into_iter().map(str::to_string).collect()
    }

    #[test]
    fn a_threats_text_message_is_written_as_an_alarms_is() {
        let mut threats = Threats::default();
        assert_eq!(threats.message(), None);
        threats.detect(threat("lws", "Laser", "Rangefinder"), at(0));
        assert_eq!(
            threats.message(),
            Some(ThreatMessage { text: "T \u{2013} Laser Rangefinder".to_string(), acknowledged: false })
        );
        // With nothing more said of it than its type.
        threats.cancel_all();
        threats.detect(threat("shot", " Gunshot ", ""), at(1));
        assert_eq!(text(&threats).as_deref(), Some("T \u{2013} Gunshot"));
    }

    #[test]
    fn f16_acknowledges_the_threat_whose_message_is_showing_and_it_stays() {
        let mut threats = Threats::default();
        threats.detect(threat("lws", "Laser", "Rangefinder"), at(0));
        threats.acknowledge_presented();
        assert_eq!(threats.message().map(|m| m.acknowledged), Some(true), "it is still there");
        assert_eq!(threats.take_requests(), [Request::Acknowledged("lws".to_string())]);
        threats.acknowledge_presented();
        assert_eq!(threats.take_requests(), [], "once is enough");

        // One not yet acknowledged is shown before one that has been.
        threats.detect(Detected { priority: Priority::Low, ..threat("shot", "Gunshot", "") }, at(1));
        assert_eq!(text(&threats).as_deref(), Some("T \u{2013} Gunshot"));
        threats.acknowledge_presented();
        assert_eq!(text(&threats).as_deref(), Some("T \u{2013} Laser Rangefinder"), "then the one that matters most");
    }

    #[test]
    fn the_message_is_of_the_threat_that_matters_most_then_the_latest() {
        let mut threats = several(&[
            ("a", "Gunshot", Priority::Low),
            ("b", "Missile", Priority::High),
            ("c", "Laser", Priority::Medium),
        ]);
        assert_eq!(text(&threats).as_deref(), Some("T \u{2013} Missile"));
        threats.detect(threat("d", "Missile", "Second"), at(600));
        assert_eq!(text(&threats).as_deref(), Some("T \u{2013} Missile Second"));
    }

    #[test]
    fn a_threat_that_has_gone_has_no_message_and_stays_in_the_table() {
        let mut threats = Threats::default();
        threats.detect(threat("lws", "Laser", ""), at(0));
        threats.end("lws");
        threats.end("never heard of it");
        assert_eq!(threats.message(), None);
        assert_eq!(threats.counts(), ThreatCounts { current: 0, past: 1 });
        let (_, _, rows) = Table::default().rows(&threats);
        assert_eq!(rows[0].state, ThreatState::Past);

        // Detected again, it is a new threat: its message is back, unacknowledged.
        threats.acknowledge_presented();
        threats.detect(threat("lws", "Laser", ""), at(60));
        assert_eq!(threats.message().map(|m| m.acknowledged), Some(false));
        assert_eq!(threats.counts(), ThreatCounts { current: 1, past: 0 });
        assert_eq!(Table::default().rows(&threats).2[0].time, "041231Z");
    }

    #[test]
    fn a_threat_said_again_while_it_is_there_is_the_same_threat_moved() {
        let mut threats = Threats::default();
        threats.detect(Detected { bearing: Some(800), ..threat("lws", "Laser", "") }, at(0));
        threats.acknowledge_presented();
        threats.detect(Detected { bearing: Some(1600), range: Some(900), ..threat("lws", "Laser", "Designator") }, at(60));
        let (_, _, rows) = Table::default().rows(&threats);
        assert_eq!((rows.len(), rows[0].bearing_mils, rows[0].range_m), (1, Some(1600), Some(900)));
        assert_eq!(rows[0].time, "041230Z", "it was detected when it first was");
        assert_eq!(
            threats.message(),
            Some(ThreatMessage { text: "T \u{2013} Laser Designator".to_string(), acknowledged: true })
        );

        // Unless it now matters more than it did.
        let mut threats = Threats::default();
        threats.detect(Detected { priority: Priority::Low, ..threat("lws", "Laser", "") }, at(0));
        threats.acknowledge_presented();
        threats.detect(threat("lws", "Laser", ""), at(60));
        assert_eq!(threats.message().map(|m| m.acknowledged), Some(false));
    }

    #[test]
    fn the_current_threats_that_are_known_to_be_somewhere_are_marked() {
        let mut threats = Threats::default();
        threats.detect(Detected { bearing: Some(5300), ..threat("shot", "Gunshot", "") }, at(0));
        threats.detect(Detected { bearing: Some(0), ..threat("lws", "Laser", "") }, at(1));
        threats.detect(threat("somewhere", "Missile", ""), at(2));
        threats.detect(Detected { bearing: Some(3200), ..threat("gone", "Gunshot", "") }, at(3));
        threats.end("gone");
        assert_eq!(
            threats.marks(Some("lws")),
            [ThreatMark { bearing_mils: 5300, picked: false }, ThreatMark { bearing_mils: 0, picked: true }]
        );
        // Acknowledged, it is still there.
        threats.acknowledge_presented();
        assert_eq!(threats.marks(None).len(), 2);
    }

    #[test]
    fn a_row_has_the_threat_where_it_is_when_it_was_detected_and_whether_it_is_there() {
        let mut threats = Threats::default();
        threats.detect(
            Detected { bearing: Some(5300), range: Some(250), ..threat("shot", "Gunshot", "Small Arms") },
            at(0),
        );
        let (page, pages, rows) = Table::default().rows(&threats);
        assert_eq!((page, pages), (1, 1));
        assert_eq!(
            rows,
            [ThreatRow {
                id: "shot".to_string(),
                text: "T \u{2013} Gunshot Small Arms".to_string(),
                time: "041230Z".to_string(),
                bearing_mils: Some(5300),
                range_m: Some(250),
                state: ThreatState::Current,
                selected: false,
            }]
        );
    }

    #[test]
    fn the_table_is_sorted_by_type_or_by_time() {
        let threats = several(&[
            ("a", "Gunshot", Priority::Low),
            ("b", "Laser", Priority::Medium),
            ("c", "Gunshot", Priority::Low),
            ("d", "Missile", Priority::High),
            ("e", "Laser", Priority::Medium),
        ]);
        let mut table = Table::default();
        // a) By type, the types that matter most first, and the latest first within one.
        assert_eq!(ids(&table, &threats), ["d", "e", "b", "c", "a"]);
        // b) By time, the latest first.
        table.sort = Sort::Time;
        assert_eq!(ids(&table, &threats), ["e", "d", "c", "b", "a"]);

        // Of one second, the type that matters most is first, whichever was
        // detected first.
        let mut threats = Threats::default();
        threats.detect(threat("b", "Missile", ""), at(0));
        threats.detect(Detected { priority: Priority::Low, ..threat("a", "Gunshot", "") }, at(0));
        assert_eq!(ids(&table, &threats), ["b", "a"]);

        // Types that matter alike are in the order of their names, and a
        // type's threats are together though they are not graded alike.
        let threats = several(&[
            ("a", "Radar", Priority::High),
            ("b", "Laser", Priority::Low),
            ("c", "Laser", Priority::High),
        ]);
        assert_eq!(ids(&Table::default(), &threats), ["c", "b", "a"]);
    }

    #[test]
    fn a_threat_is_picked_shown_at_length_and_cancelled() {
        let mut threats = Threats::default();
        threats.detect(
            Detected { bearing: Some(800), detail: Some("1064 nm".into()), ..threat("lws", "Laser", "Rangefinder") },
            at(0),
        );
        threats.detect(Detected { priority: Priority::Low, ..threat("shot", "Gunshot", "") }, at(60));
        let mut table = Table::default();
        assert_eq!(table.picked(&threats), None);

        // d) picking one; c) more about it.
        table.step(true, &threats);
        assert_eq!(table.picked(&threats), Some("lws"));
        assert_eq!(table.detail(&threats), None);
        table.show(&threats);
        let detail = table.detail(&threats).unwrap();
        assert_eq!((detail.kind.as_str(), detail.description.as_str()), ("Laser", "Rangefinder"));
        assert_eq!((detail.time.as_str(), detail.bearing_mils, detail.range_m), ("041230Z OCT 24", Some(800), None));
        assert_eq!((detail.priority, detail.state, detail.detail.as_deref()), (Priority::High, ThreatState::Current, Some("1064 nm")));
        table.close();

        // f) cancelling it: it is gone from the table, and its source is told.
        threats.cancel("lws");
        table.settle(&threats);
        assert_eq!(texts(&table, &threats), ["T \u{2013} Gunshot"]);
        assert_eq!(table.picked(&threats), None, "the pick does not outlast its threat");
        assert_eq!(threats.take_requests(), [Request::Cancelled("lws".to_string())]);

        // g) cancelling them all.
        threats.detect(threat("lws", "Laser", ""), at(120));
        threats.cancel_all();
        assert!(threats.is_empty());
        assert_eq!(table.rows(&threats), (1, 1, vec![]));
        assert_eq!(
            threats.take_requests(),
            [Request::Cancelled("shot".to_string()), Request::Cancelled("lws".to_string())]
        );
    }

    #[test]
    fn the_table_is_paged() {
        let mut threats = Threats::default();
        for n in 0..ROWS_PER_PAGE as u64 + 3 {
            threats.detect(threat(&format!("{n:02}"), "Gunshot", ""), at(n));
        }
        let mut table = Table::default();
        let (page, pages, rows) = table.rows(&threats);
        assert_eq!((page, pages, rows.len()), (1, 2, ROWS_PER_PAGE));
        assert_eq!(table.more_pages(&threats), (false, true));
        table.turn_page(true, &threats);
        let (page, _, rows) = table.rows(&threats);
        assert_eq!((page, rows.len()), (2, 3));
        assert_eq!(table.more_pages(&threats), (true, false));
    }

    #[test]
    fn threats_that_are_past_can_be_left_out_of_the_table() {
        let mut threats = several(&[("a", "Gunshot", Priority::Low), ("b", "Laser", Priority::High)]);
        threats.end("b");
        let mut table = Table::default();
        assert_eq!(ids(&table, &threats), ["b", "a"]);
        table.past = false;
        assert_eq!(ids(&table, &threats), ["a"]);

        // One shown at length stays shown when it goes, and so leaves the table.
        table.step(true, &threats);
        table.show(&threats);
        threats.end("a");
        table.settle(&threats);
        assert!(table.rows(&threats).2.is_empty());
        assert_eq!(table.detail(&threats).map(|detail| detail.state), Some(ThreatState::Past));
    }

    #[test]
    fn the_listed_threats_are_plotted_where_they_are() {
        let mut threats = Threats::default();
        let plot = Table::default().plot(&threats);
        assert_eq!(plot, ThreatPlot { range_m: 1000, threats: vec![], elsewhere: 0 }, "nothing yet");

        let somewhere = |bearing: u16, range: Option<u32>, id: &str, kind: &str| Detected {
            bearing: Some(bearing),
            range,
            ..threat(id, kind, "")
        };
        threats.detect(Detected { symbol: Some("shgpewrr".into()), ..somewhere(5300, Some(250), "shot", "Gunshot") }, at(0));
        threats.detect(somewhere(800, None, "lws", "Laser"), at(60));
        threats.detect(threat("somewhere", "Missile", "Launch"), at(120));
        let mut table = Table::default();
        let plot = table.plot(&threats);
        assert_eq!(plot.range_m, 500, "the nearest scale that takes in the farthest");
        assert_eq!(
            plot.threats,
            [
                Plotted {
                    id: "shot".to_string(),
                    symbol: "SHGPEWRR-------".to_string(),
                    label: "Gunshot".to_string(),
                    bearing_mils: 5300,
                    range_m: Some(250),
                    state: ThreatState::Current,
                    picked: false,
                },
                Plotted {
                    id: "lws".to_string(),
                    symbol: USUAL_SYMBOL.to_string(),
                    label: "Laser".to_string(),
                    bearing_mils: 800,
                    range_m: None,
                    state: ThreatState::Current,
                    picked: false,
                },
            ]
        );
        assert_eq!(plot.elsewhere, 1, "there is no saying which way the missile is");

        // The picked threat is picked out.
        table.step(true, &threats);
        table.step(true, &threats);
        let plot = table.plot(&threats);
        assert_eq!(plot.threats.iter().map(|threat| threat.picked).collect::<Vec<_>>(), [false, true]);

        // What is out of date can be taken off it.
        threats.end("lws");
        assert_eq!(table.plot(&threats).threats[1].state, ThreatState::Past);
        table.past = false;
        assert_eq!(table.plot(&threats).threats.len(), 1);

        // The scales go as far as they go; a threat farther off than that is still plotted.
        threats.detect(somewhere(0, Some(1001), "far", "Radar"), at(180));
        assert_eq!(table.plot(&threats).range_m, 2000);
        threats.detect(somewhere(0, Some(400_000), "farther", "Radar"), at(180));
        assert_eq!((table.plot(&threats).range_m, table.plot(&threats).threats.len()), (50_000, 3));
    }

    #[test]
    fn a_symbol_is_a_symbol_identification_code_made_up_to_its_length() {
        assert_eq!(symbol("SHGPEWRR-------").as_deref(), Some("SHGPEWRR-------"));
        assert_eq!(symbol(" shapwmss ").as_deref(), Some("SHAPWMSS-------"));
        assert_eq!(symbol("SHGP").as_deref(), Some(USUAL_SYMBOL));
        for said in ["", "SHG", "SHGPEWRR--------", "SHGP EWRR", "SHGP*WRR", "laser!"] {
            assert_eq!(symbol(said), None, "{said}");
        }
        assert!(Detected { symbol: Some("laser!".into()), ..threat("lws", "Laser", "") }.fault().is_some());
        assert_eq!(Detected { symbol: Some("SHGPEXL".into()), ..threat("lws", "Laser", "") }.fault(), None);
    }

    #[test]
    fn the_sensors_say_how_they_are() {
        let mut threats = Threats::default();
        assert!(threats.sensors().is_empty());
        threats.sensor(Sensor { name: "Laser Warning".to_string(), status: SensorStatus::Ok });
        threats.sensor(Sensor { name: "Shot Detection".to_string(), status: SensorStatus::Off });
        threats.sensor(Sensor { name: "Laser Warning".to_string(), status: SensorStatus::Failed });
        assert_eq!(
            threats.sensors(),
            [
                Sensor { name: "Laser Warning".to_string(), status: SensorStatus::Failed },
                Sensor { name: "Shot Detection".to_string(), status: SensorStatus::Off },
            ]
        );
    }

    #[test]
    fn a_threat_that_cannot_be_one_is_found_out() {
        assert_eq!(threat("lws", "Laser", "").fault(), None);
        assert!(threat("", "Laser", "").fault().is_some());
        assert!(threat("lws", " ", "").fault().is_some());
        assert!(Detected { bearing: Some(6400), ..threat("lws", "Laser", "") }.fault().is_some());
        assert_eq!(Detected { bearing: Some(6399), ..threat("lws", "Laser", "") }.fault(), None);
    }

    #[test]
    fn the_table_does_not_grow_without_limit() {
        let mut threats = Threats::default();
        for n in 0..MOST_THREATS as u64 + 10 {
            threats.detect(threat(&format!("{n}"), "Gunshot", ""), at(n));
            threats.end(&format!("{n}"));
        }
        let counts = threats.counts();
        assert_eq!(counts.current + counts.past, MOST_THREATS as u32);
        // The ones still there are never the ones to go.
        let mut threats = Threats::default();
        for n in 0..MOST_THREATS as u64 + 10 {
            threats.detect(threat(&format!("{n}"), "Gunshot", ""), at(n));
        }
        assert_eq!(threats.counts().current, MOST_THREATS as u32 + 10);
    }
}
