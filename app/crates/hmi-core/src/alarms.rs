//! Alarms: what the vehicle says is wrong, and what the crew has done about
//! it.
//!
//! An alarm is raised and resolved by its source - whatever knows the
//! vehicle. The crew acknowledges, overrides and clears. This module keeps
//! the list of alarms, decides which one has its text message on the screen,
//! and keeps the table behind the Alarms page. It has no clock of its own:
//! every call is told the time ([`Time`]).

use crate::dtg::{date_time_group, day_time_group};
use crate::feed::Request;
use crate::table::{Paged, View};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::HashSet;

/// How long a caution's text message has before the HMI acknowledges it for
/// the crew.
pub const CAUTION_PRESENTATION_MS: u64 = 10_000;
/// A caution that was acknowledged and is still the case is presented again
/// after this long.
pub const CAUTION_REPEAT_MS: u64 = 2 * 60 * 60 * 1000;
/// An advisory likewise.
pub const ADVISORY_REPEAT_MS: u64 = 24 * 60 * 60 * 1000;
/// The rows on one page of the alarms table: as many as fit when every one of
/// them needs a second line for its text.
pub const ROWS_PER_PAGE: usize = 10;
/// Alarms that are over are kept in the table until the crew clears them, but
/// not without limit: beyond this many alarms the oldest of those go.
const MOST_ALARMS: usize = 200;

/// The time, twice: by the calendar, for what is shown, and by a clock that
/// only runs forwards, for how long things last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Time {
    /// Seconds since 1970, UTC.
    pub unix_s: u64,
    /// Milliseconds since some fixed moment, such as the HMI starting.
    pub uptime_ms: u64,
}

/// The three categories of alarm, most important first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Warning,
    Caution,
    Advisory,
}

impl Category {
    /// The letter a category goes by: in a text message, in the table and on
    /// the status information bar.
    pub fn letter(self) -> char {
        match self {
            Category::Warning => 'W',
            Category::Caution => 'C',
            Category::Advisory => 'A',
        }
    }
}

/// An alarm as its source describes it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Raised {
    /// The source's name for the alarm: what it is resolved by, and what the
    /// source is told the crew has done to.
    pub id: String,
    pub category: Category,
    // What the alarm is about, in the order a text message gives it.
    // Any of them can be left out, but not all.
    #[serde(default)]
    pub subsystem: String,
    #[serde(default)]
    pub component: String,
    #[serde(default)]
    pub nature: String,
    #[serde(default, rename = "type")]
    pub kind: String,
    /// More about it, for the crew to call up.
    #[serde(default)]
    pub detail: Option<String>,
    /// When it happened, in seconds since 1970, if that was not just now.
    #[serde(default)]
    pub time: Option<u64>,
}

/// Where an alarm stands with its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// The condition is there.
    Active,
    /// The crew has overridden it: its source is not listened to on this
    /// alarm again until the HMI next starts (10.2.2.1 c).
    Overridden,
    /// The condition has gone. The alarm stays in the table until cleared.
    Resolved,
}

/// The alarm text message under the status information bar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    pub category: Category,
    /// "W – Engine Oil Low Pressure".
    pub text: String,
    pub acknowledged: bool,
}

/// How many alarms there are of each category.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct AlarmCounts {
    pub warnings: u32,
    pub cautions: u32,
    pub advisories: u32,
}

impl AlarmCounts {
    pub fn total(self) -> u32 {
        self.warnings + self.cautions + self.advisories
    }

    fn add(&mut self, category: Category) {
        match category {
            Category::Warning => self.warnings += 1,
            Category::Caution => self.cautions += 1,
            Category::Advisory => self.advisories += 1,
        }
    }
}

/// A row of the alarms table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AlarmRow {
    /// Which alarm it is, as its source names it: what a touch on the row
    /// picks it by.
    pub id: String,
    pub category: Category,
    /// "W – Engine Oil Low Pressure".
    pub text: String,
    /// When it was raised: the day and time of its date-time group. The
    /// month and year are in [`AlarmDetail`].
    pub time: String,
    pub acknowledged: bool,
    pub state: State,
    pub selected: bool,
}

/// One alarm at length.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AlarmDetail {
    pub category: Category,
    pub text: String,
    pub time: String,
    pub acknowledged: bool,
    pub state: State,
    /// What the source had to add, if anything.
    pub detail: Option<String>,
}

#[derive(Debug, Clone)]
struct Alarm {
    id: String,
    category: Category,
    /// "Engine Oil Low Pressure".
    title: String,
    detail: Option<String>,
    /// When it was raised, seconds since 1970.
    at: u64,
    /// The order alarms were raised in, to tell apart those of one second.
    order: u64,
    state: State,
    acknowledged: bool,
    /// Since when it has been left alone, by the forward-running clock: since
    /// it was acknowledged, or its message's time ran out without that. For
    /// presenting it again.
    quiet_since: Option<u64>,
    /// A caution that has had its turn at the text message, or lost it.
    presented: bool,
}

impl Alarm {
    fn text(&self) -> String {
        format!("{} \u{2013} {}", self.category.letter(), self.title)
    }

    /// Whether it is still the crew's concern: there, and not overridden.
    fn active(&self) -> bool {
        self.state == State::Active
    }
}

/// The caution whose turn at the text message it is.
#[derive(Debug, Clone)]
struct Turn {
    id: String,
    /// When its time is up, by the forward-running clock.
    until: u64,
    /// Whether a warning's message has been in its place for any of that
    /// time.
    covered: bool,
}

/// The alarms the HMI knows of.
#[derive(Debug, Default)]
pub struct Alarms {
    list: Vec<Alarm>,
    raised: u64,
    /// Overridden alarms, by the names their sources give them: kept apart
    /// from the list because an alarm cleared from the list stays overridden.
    overridden: HashSet<String>,
    turn: Option<Turn>,
    requests: Vec<Request>,
}

impl Alarms {
    /// The source says an alarm is the case.
    pub fn raise(&mut self, raised: Raised, now: Time) {
        let title: Vec<&str> = [&raised.subsystem, &raised.component, &raised.nature, &raised.kind]
            .into_iter()
            .map(|part| part.trim())
            .filter(|part| !part.is_empty())
            .collect();
        let title = if title.is_empty() { raised.id.clone() } else { title.join(" ") };

        match self.list.iter_mut().find(|alarm| alarm.id == raised.id) {
            Some(alarm) => {
                // Said again of an alarm that is there: at most its wording
                // has changed. Of one that had gone: it is a new alarm.
                let anew = alarm.state == State::Resolved || alarm.category != raised.category;
                alarm.category = raised.category;
                alarm.title = title;
                alarm.detail = raised.detail;
                if anew {
                    if alarm.state == State::Resolved {
                        let overridden = self.overridden.contains(&alarm.id);
                        alarm.state = if overridden { State::Overridden } else { State::Active };
                    }
                    alarm.at = raised.time.unwrap_or(now.unix_s);
                    alarm.order = self.raised;
                    self.raised += 1;
                    alarm.acknowledged = alarm.state == State::Overridden;
                    alarm.quiet_since = None;
                    alarm.presented = false;
                }
            }
            // The crew has silenced this one, and cleared it away since.
            None if self.overridden.contains(&raised.id) => {}
            None => {
                self.list.push(Alarm {
                    id: raised.id,
                    category: raised.category,
                    title,
                    detail: raised.detail,
                    at: raised.time.unwrap_or(now.unix_s),
                    order: self.raised,
                    state: State::Active,
                    acknowledged: false,
                    quiet_since: None,
                    presented: false,
                });
                self.raised += 1;
                self.forget_the_oldest();
            }
        }
        self.settle(now);
    }

    /// The source says an alarm is no longer the case.
    pub fn resolve(&mut self, id: &str, now: Time) {
        if let Some(alarm) = self.list.iter_mut().find(|alarm| alarm.id == id) {
            alarm.state = State::Resolved;
        }
        self.settle(now);
    }

    /// F16: acknowledge the alarm whose text message is on the screen.
    /// A warning's message stays; a caution's
    /// goes.
    pub fn acknowledge_presented(&mut self, now: Time) {
        if let Some(id) = self.presented().filter(|alarm| !alarm.acknowledged).map(|alarm| alarm.id.clone()) {
            self.acknowledge(&id, now);
        }
    }

    pub fn acknowledge(&mut self, id: &str, now: Time) {
        if let Some(alarm) = self.list.iter_mut().find(|alarm| alarm.id == id && !alarm.acknowledged) {
            alarm.acknowledged = true;
            alarm.quiet_since = Some(now.uptime_ms);
            self.requests.push(Request::Acknowledged(alarm.id.clone()));
        }
        self.settle(now);
    }

    pub fn acknowledge_all(&mut self, now: Time) {
        let ids: Vec<String> = self.list.iter().filter(|a| !a.acknowledged).map(|a| a.id.clone()).collect();
        for id in ids {
            self.acknowledge(&id, now);
        }
    }

    /// Silence an alarm at its source: it stays in the table, marked, and no
    /// longer has a text message or counts as active (10.2.2.1 c).
    pub fn override_alarm(&mut self, id: &str, now: Time) {
        if let Some(alarm) = self.list.iter_mut().find(|alarm| alarm.id == id && alarm.active()) {
            alarm.state = State::Overridden;
            alarm.acknowledged = true;
            self.overridden.insert(alarm.id.clone());
            self.requests.push(Request::Overridden(alarm.id.clone()));
        }
        self.settle(now);
    }

    /// Take an alarm out of the table (10.2.2.1 d). If it is still the case,
    /// its source is expected to raise it again.
    pub fn clear(&mut self, id: &str, now: Time) {
        if let Some(index) = self.list.iter().position(|alarm| alarm.id == id) {
            self.requests.push(Request::Cleared(self.list.remove(index).id));
        }
        self.settle(now);
    }

    pub fn clear_all(&mut self, now: Time) {
        for alarm in self.list.drain(..) {
            self.requests.push(Request::Cleared(alarm.id));
        }
        self.settle(now);
    }

    /// Time has passed. Returns whether anything changed with it.
    pub fn tick(&mut self, now: Time) -> bool {
        let mut changed = false;

        // A caution's turn at the text message is over. One that had the
        // screen for its ten seconds is acknowledged for the crew.
        // One that did not - a warning's message was in its
        // place for some or all of them - is not shown once the warning has
        // gone, and nobody has had the time to read it that
        // acknowledging it for them would claim: it stays unacknowledged in
        // the table, to be presented again as an acknowledged one is.
        if let Some(turn) = self.turn.take_if(|turn| turn.until <= now.uptime_ms) {
            if !turn.covered {
                self.acknowledge(&turn.id, now);
            } else if let Some(alarm) = self.list.iter_mut().find(|alarm| alarm.id == turn.id) {
                alarm.quiet_since = Some(now.uptime_ms);
            }
            changed = true;
        }

        // What has been left alone for long enough, and is still the case,
        // is put to the crew again.
        for alarm in self.list.iter_mut().filter(|alarm| alarm.active()) {
            let after = match alarm.category {
                Category::Warning => continue,
                Category::Caution => CAUTION_REPEAT_MS,
                Category::Advisory => ADVISORY_REPEAT_MS,
            };
            if alarm.quiet_since.is_some_and(|at| at + after <= now.uptime_ms) {
                alarm.acknowledged = false;
                alarm.quiet_since = None;
                alarm.presented = false;
                changed = true;
            }
        }

        self.settle(now);
        changed
    }

    /// When [`Alarms::tick`] next has something to do, by the forward-running
    /// clock.
    pub fn deadline(&self) -> Option<u64> {
        let repeats = self.list.iter().filter(|alarm| alarm.active()).filter_map(|alarm| {
            let after = match alarm.category {
                Category::Warning => return None,
                Category::Caution => CAUTION_REPEAT_MS,
                Category::Advisory => ADVISORY_REPEAT_MS,
            };
            alarm.quiet_since.map(|at| at + after)
        });
        self.turn.iter().map(|turn| turn.until).chain(repeats).min()
    }

    /// What the crew has done since this was last asked, for the source.
    pub fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }

    /// The text message to show, if any: a warning's before a caution's,
    /// and never an advisory's.
    pub fn message(&self) -> Option<Message> {
        self.presented().map(|alarm| Message {
            category: alarm.category,
            text: alarm.text(),
            acknowledged: alarm.acknowledged,
        })
    }

    /// The active alarms of each category, for the status information bar.
    pub fn active(&self) -> AlarmCounts {
        self.count(|alarm| alarm.active())
    }

    /// The alarms in the table that have, or have not, been acknowledged.
    pub fn acknowledged(&self, acknowledged: bool) -> AlarmCounts {
        self.count(|alarm| alarm.acknowledged == acknowledged)
    }

    /// Whether there are no alarms at all, of any standing.
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    fn count(&self, which: impl Fn(&Alarm) -> bool) -> AlarmCounts {
        let mut counts = AlarmCounts::default();
        self.list.iter().filter(|alarm| which(alarm)).for_each(|alarm| counts.add(alarm.category));
        counts
    }

    /// The alarm whose text message is on the screen.
    fn presented(&self) -> Option<&Alarm> {
        self.warning().or_else(|| {
            let turn = self.turn.as_ref()?;
            self.list.iter().find(|alarm| alarm.id == turn.id)
        })
    }

    /// The warning whose text message is on the screen: the latest that has
    /// not been acknowledged, or with none of those the latest that has - an
    /// acknowledged warning's message stays while the warning does.
    fn warning(&self) -> Option<&Alarm> {
        self.list
            .iter()
            .filter(|alarm| alarm.category == Category::Warning && alarm.active())
            .max_by_key(|alarm| (!alarm.acknowledged, alarm.at, alarm.order))
    }

    /// Put the turn at the text message right after a change: cautions take
    /// it one after another, in the order they were raised.
    /// Their turns run whether or not a warning's message is over theirs: a
    /// caution raised under a warning is shown if the warning goes while the
    /// caution still has time, and for what is left of it.
    fn settle(&mut self, now: Time) {
        fn waiting(alarm: &Alarm) -> bool {
            alarm.category == Category::Caution && alarm.active() && !alarm.acknowledged
        }
        if let Some(turn) = &self.turn {
            if !self.list.iter().any(|alarm| alarm.id == turn.id && waiting(alarm)) {
                self.turn = None;
            }
        }
        if self.turn.is_none() {
            let next =
                self.list.iter_mut().filter(|a| waiting(a) && !a.presented).min_by_key(|a| a.order);
            if let Some(alarm) = next {
                alarm.presented = true;
                self.turn = Some(Turn {
                    id: alarm.id.clone(),
                    until: now.uptime_ms + CAUTION_PRESENTATION_MS,
                    covered: false,
                });
            }
        }
        let covered = self.warning().is_some();
        if let Some(turn) = &mut self.turn {
            turn.covered |= covered;
        }
    }

    fn forget_the_oldest(&mut self) {
        while self.list.len() > MOST_ALARMS {
            let oldest = self
                .list
                .iter()
                .enumerate()
                .filter(|(_, alarm)| alarm.state == State::Resolved)
                .min_by_key(|(_, alarm)| (!alarm.acknowledged, alarm.order))
                .map(|(index, _)| index);
            match oldest {
                Some(index) => drop(self.list.remove(index)),
                None => break,
            }
        }
    }
}

/// The order of the rows of the alarms table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    /// By category, most important first, then by time, latest first.
    Category,
    /// By time, latest first, then by category, most important first.
    Time,
}

/// The alarms table as the crew has it set: its order, which alarms it
/// leaves out, which row is picked and which page is showing.
#[derive(Debug, Clone)]
pub struct Table {
    pub sort: Sort,
    /// Whether acknowledged alarms are listed.
    pub acknowledged: bool,
    view: View,
}

impl Default for Table {
    /// As the Alarms page opens: everything, by category.
    fn default() -> Self {
        Table { sort: Sort::Category, acknowledged: true, view: View::default() }
    }
}

impl Paged for Table {
    type From = Alarms;
    const ROWS: usize = ROWS_PER_PAGE;

    fn view(&self) -> &View {
        &self.view
    }

    fn view_mut(&mut self) -> &mut View {
        &mut self.view
    }

    fn names<'a>(&self, alarms: &'a Alarms) -> Vec<&'a str> {
        self.listed(alarms).into_iter().map(|alarm| alarm.id.as_str()).collect()
    }

    fn has(alarms: &Alarms, name: &str) -> bool {
        alarms.list.iter().any(|alarm| alarm.id == name)
    }
}

impl Table {
    fn listed<'a>(&self, alarms: &'a Alarms) -> Vec<&'a Alarm> {
        let mut listed: Vec<&Alarm> =
            alarms.list.iter().filter(|alarm| self.acknowledged || !alarm.acknowledged).collect();
        match self.sort {
            Sort::Category => listed.sort_by_key(|a| (a.category, Reverse(a.at), Reverse(a.order))),
            Sort::Time => listed.sort_by_key(|a| (Reverse(a.at), a.category, Reverse(a.order))),
        }
        listed
    }

    /// Whether the picked alarm can still be acknowledged, and overridden.
    pub fn can(&self, alarms: &Alarms) -> (bool, bool) {
        let listed = self.listed(alarms);
        let picked = self.on_screen(alarms).picked;
        picked.map_or((false, false), |at| (!listed[at].acknowledged, listed[at].active()))
    }

    /// The page showing (from 1), how many there are, and its rows.
    pub fn rows(&self, alarms: &Alarms) -> (usize, usize, Vec<AlarmRow>) {
        let on_screen = self.on_screen(alarms);
        let rows = self
            .listed(alarms)
            .iter()
            .enumerate()
            .skip(on_screen.first)
            .take(ROWS_PER_PAGE)
            .map(|(at, alarm)| AlarmRow {
                id: alarm.id.clone(),
                category: alarm.category,
                text: alarm.text(),
                time: day_time_group(alarm.at),
                acknowledged: alarm.acknowledged,
                state: alarm.state,
                selected: on_screen.picked == Some(at),
            })
            .collect();
        (on_screen.page, on_screen.pages, rows)
    }

    /// The alarm shown at length, if one is.
    pub fn detail(&self, alarms: &Alarms) -> Option<AlarmDetail> {
        let shown = self.shown()?;
        let alarm = alarms.list.iter().find(|alarm| alarm.id == shown)?;
        Some(AlarmDetail {
            category: alarm.category,
            text: alarm.text(),
            time: date_time_group(alarm.at),
            acknowledged: alarm.acknowledged,
            state: alarm.state,
            detail: alarm.detail.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: u64 = 1000;
    const HOUR: u64 = 60 * 60 * SECOND;

    /// A moment `ms` after the tests' beginning of time.
    fn at(ms: u64) -> Time {
        Time { unix_s: 1_728_045_000 + ms / 1000, uptime_ms: ms }
    }

    fn raised(id: &str, category: Category, about: &str) -> Raised {
        Raised {
            id: id.to_string(),
            category,
            subsystem: about.to_string(),
            component: String::new(),
            nature: String::new(),
            kind: String::new(),
            detail: None,
            time: None,
        }
    }

    fn warning(alarms: &mut Alarms, id: &str, ms: u64) {
        alarms.raise(raised(id, Category::Warning, id), at(ms));
    }

    fn caution(alarms: &mut Alarms, id: &str, ms: u64) {
        alarms.raise(raised(id, Category::Caution, id), at(ms));
    }

    fn text(alarms: &Alarms) -> Option<String> {
        alarms.message().map(|message| message.text)
    }

    fn counts(warnings: u32, cautions: u32, advisories: u32) -> AlarmCounts {
        AlarmCounts { warnings, cautions, advisories }
    }

    #[test]
    fn a_text_message_is_the_category_then_what_it_is_about() {
        let mut alarms = Alarms::default();
        alarms.raise(
            Raised {
                subsystem: "Engine".into(),
                component: "Oil".into(),
                nature: "Low Pressure".into(),
                ..raised("oil", Category::Warning, "")
            },
            at(0),
        );
        assert_eq!(text(&alarms).as_deref(), Some("W \u{2013} Engine Oil Low Pressure"));
    }

    #[test]
    fn the_status_bar_counts_active_alarms_by_category() {
        let mut alarms = Alarms::default();
        warning(&mut alarms, "w1", 0);
        caution(&mut alarms, "c1", 0);
        caution(&mut alarms, "c2", 0);
        alarms.raise(raised("a1", Category::Advisory, "a1"), at(0));
        assert_eq!(alarms.active(), counts(1, 2, 1));

        // Acknowledged alarms are still active; resolved and overridden ones are not.
        alarms.acknowledge("c1", at(1));
        assert_eq!(alarms.active(), counts(1, 2, 1));
        alarms.resolve("c1", at(2));
        alarms.override_alarm("w1", at(3));
        assert_eq!(alarms.active(), counts(0, 1, 1));
    }

    #[test]
    fn a_warnings_message_stays_when_acknowledged() {
        let mut alarms = Alarms::default();
        assert_eq!(alarms.message(), None);
        warning(&mut alarms, "fire", 0);
        assert!(!alarms.message().unwrap().acknowledged);

        alarms.acknowledge_presented(at(SECOND));
        let message = alarms.message().expect("the message of an acknowledged warning stays");
        assert!(message.acknowledged);
        assert_eq!(alarms.take_requests(), [Request::Acknowledged("fire".to_string())]);

        // Not for ever, though time alone does not take it away...
        assert!(!alarms.tick(at(HOUR)));
        assert!(alarms.message().is_some());
        // ...being overridden or cleared in the table does, and so does being resolved.
        let mut overridden = Alarms::default();
        warning(&mut overridden, "fire", 0);
        overridden.override_alarm("fire", at(1));
        assert_eq!(overridden.message(), None);
        alarms.clear("fire", at(HOUR));
        assert_eq!(alarms.message(), None);
        warning(&mut alarms, "fire", HOUR);
        alarms.resolve("fire", at(HOUR));
        assert_eq!(alarms.message(), None);
    }

    #[test]
    fn a_new_warning_is_shown_before_one_already_acknowledged() {
        let mut alarms = Alarms::default();
        warning(&mut alarms, "first", 0);
        alarms.acknowledge_presented(at(1));
        warning(&mut alarms, "second", 2 * SECOND);
        assert_eq!(text(&alarms).as_deref(), Some("W \u{2013} second"));
        assert!(!alarms.message().unwrap().acknowledged);

        // With both acknowledged the later one has the line.
        alarms.acknowledge_presented(at(3 * SECOND));
        assert_eq!(text(&alarms).as_deref(), Some("W \u{2013} second"));
        alarms.resolve("second", at(4 * SECOND));
        assert_eq!(text(&alarms).as_deref(), Some("W \u{2013} first"));
    }

    #[test]
    fn a_warnings_message_is_not_replaced_by_a_lesser_alarms() {
        let mut alarms = Alarms::default();
        warning(&mut alarms, "fire", 0);
        caution(&mut alarms, "fuel", SECOND);
        alarms.raise(raised("service", Category::Advisory, "service"), at(SECOND));
        assert_eq!(text(&alarms).as_deref(), Some("W \u{2013} fire"));
        // F16 is for the message on the screen: the warning's.
        alarms.acknowledge_presented(at(2 * SECOND));
        assert_eq!(alarms.acknowledged(true), counts(1, 0, 0));
        assert_eq!(text(&alarms).as_deref(), Some("W \u{2013} fire"));
    }

    #[test]
    fn a_cautions_message_goes_when_acknowledged() {
        let mut alarms = Alarms::default();
        caution(&mut alarms, "fuel", 0);
        assert_eq!(text(&alarms).as_deref(), Some("C \u{2013} fuel"));
        alarms.acknowledge_presented(at(SECOND));
        assert_eq!(alarms.message(), None);
        assert_eq!(alarms.active(), counts(0, 1, 0), "it is still active");
    }

    #[test]
    fn a_cautions_message_is_acknowledged_for_the_crew_after_ten_seconds() {
        let mut alarms = Alarms::default();
        caution(&mut alarms, "fuel", 0);
        assert_eq!(alarms.deadline(), Some(10 * SECOND));
        assert!(!alarms.tick(at(10 * SECOND - 1)));
        assert!(alarms.message().is_some());

        assert!(alarms.tick(at(10 * SECOND)));
        assert_eq!(alarms.message(), None);
        assert_eq!(alarms.acknowledged(true), counts(0, 1, 0));
        assert_eq!(alarms.take_requests(), [Request::Acknowledged("fuel".to_string())]);
    }

    #[test]
    fn cautions_are_presented_one_after_another() {
        let mut alarms = Alarms::default();
        caution(&mut alarms, "one", 0);
        caution(&mut alarms, "two", SECOND);
        caution(&mut alarms, "three", 2 * SECOND);
        assert_eq!(text(&alarms).as_deref(), Some("C \u{2013} one"));

        // Acknowledged by the crew: the next has its own ten seconds from then.
        alarms.acknowledge_presented(at(3 * SECOND));
        assert_eq!(text(&alarms).as_deref(), Some("C \u{2013} two"));
        assert_eq!(alarms.deadline(), Some(13 * SECOND));
        // Left alone: acknowledged for the crew, and the last has its turn.
        alarms.tick(at(13 * SECOND));
        assert_eq!(text(&alarms).as_deref(), Some("C \u{2013} three"));
        alarms.tick(at(23 * SECOND));
        assert_eq!(alarms.message(), None);
        assert_eq!(alarms.acknowledged(true), counts(0, 3, 0));
    }

    #[test]
    fn a_caution_under_a_warning_is_shown_only_if_the_warning_goes_in_its_time() {
        // The warning goes while the caution still has time: it is shown for the rest.
        let mut alarms = Alarms::default();
        warning(&mut alarms, "fire", 0);
        caution(&mut alarms, "fuel", SECOND);
        assert_eq!(text(&alarms).as_deref(), Some("W \u{2013} fire"));
        alarms.resolve("fire", at(5 * SECOND));
        assert_eq!(text(&alarms).as_deref(), Some("C \u{2013} fuel"));
        assert!(!alarms.tick(at(11 * SECOND - 1)));
        assert!(alarms.tick(at(11 * SECOND)));
        assert_eq!(alarms.message(), None);
        assert_eq!(alarms.acknowledged(false).cautions, 1, "it has not had ten seconds on the screen");
        assert_eq!(alarms.take_requests(), []);

        // The warning outlasts it: it is not shown then or afterwards.
        let mut alarms = Alarms::default();
        warning(&mut alarms, "fire", 0);
        caution(&mut alarms, "fuel", SECOND);
        alarms.tick(at(11 * SECOND));
        alarms.override_alarm("fire", at(12 * SECOND));
        assert_eq!(alarms.message(), None);
        assert_eq!(alarms.acknowledged(false), counts(0, 1, 0));
        assert_eq!(alarms.active(), counts(0, 1, 0));

        // Until it has been left as long as an acknowledged one is:
        // then it has its ten seconds.
        assert_eq!(alarms.deadline(), Some(11 * SECOND + 2 * HOUR));
        assert!(alarms.tick(at(11 * SECOND + 2 * HOUR)));
        assert_eq!(text(&alarms).as_deref(), Some("C \u{2013} fuel"));
        assert!(alarms.tick(at(21 * SECOND + 2 * HOUR)));
        assert_eq!(alarms.acknowledged(true).cautions, 1);
    }

    #[test]
    fn a_caution_that_a_warning_takes_the_screen_from_is_not_acknowledged_for_the_crew() {
        let mut alarms = Alarms::default();
        caution(&mut alarms, "fuel", 0);
        assert_eq!(text(&alarms).as_deref(), Some("C \u{2013} fuel"));
        warning(&mut alarms, "fire", 3 * SECOND);
        assert_eq!(text(&alarms).as_deref(), Some("W \u{2013} fire"));
        assert!(alarms.tick(at(10 * SECOND)));
        assert_eq!(alarms.acknowledged(false), counts(1, 1, 0));
        assert_eq!(alarms.take_requests(), []);
    }

    #[test]
    fn an_acknowledged_caution_still_the_case_is_repeated_after_two_hours() {
        let mut alarms = Alarms::default();
        caution(&mut alarms, "fuel", 0);
        alarms.acknowledge_presented(at(SECOND));
        assert_eq!(alarms.deadline(), Some(SECOND + 2 * HOUR));
        assert!(!alarms.tick(at(2 * HOUR)));
        assert_eq!(alarms.message(), None);

        assert!(alarms.tick(at(SECOND + 2 * HOUR)));
        assert_eq!(text(&alarms).as_deref(), Some("C \u{2013} fuel"));
        assert!(!alarms.message().unwrap().acknowledged);

        // One that has been resolved in the meantime is not.
        let mut alarms = Alarms::default();
        caution(&mut alarms, "fuel", 0);
        alarms.acknowledge_presented(at(SECOND));
        alarms.resolve("fuel", at(HOUR));
        assert!(!alarms.tick(at(3 * HOUR)));
        assert_eq!(alarms.message(), None);
    }

    #[test]
    fn an_advisory_has_no_message_and_is_repeated_after_a_day() {
        let mut alarms = Alarms::default();
        alarms.raise(raised("service", Category::Advisory, "service"), at(0));
        assert_eq!(alarms.message(), None);
        assert_eq!(alarms.deadline(), None);
        // F16 is for what is on the screen, and an advisory is not.
        alarms.acknowledge_presented(at(1));
        assert_eq!(alarms.acknowledged(false), counts(0, 0, 1));

        alarms.acknowledge("service", at(SECOND));
        assert!(!alarms.tick(at(24 * HOUR)));
        assert!(alarms.tick(at(SECOND + 24 * HOUR)));
        assert_eq!(alarms.acknowledged(false), counts(0, 0, 1));
        assert_eq!(alarms.message(), None);
    }

    #[test]
    fn an_overridden_alarm_is_not_heard_of_again() {
        let mut alarms = Alarms::default();
        warning(&mut alarms, "fire", 0);
        alarms.override_alarm("fire", at(SECOND));
        assert_eq!(alarms.take_requests(), [Request::Overridden("fire".to_string())]);
        // It stays in the table (10.2.2.1 c), as overridden and dealt with.
        let (_, _, rows) = Table::default().rows(&alarms);
        assert_eq!((rows[0].state, rows[0].acknowledged), (State::Overridden, true));

        // Raised again, resolved and raised again, cleared and raised again: nothing.
        warning(&mut alarms, "fire", 2 * SECOND);
        alarms.resolve("fire", at(3 * SECOND));
        warning(&mut alarms, "fire", 4 * SECOND);
        assert_eq!((alarms.message(), alarms.active()), (None, counts(0, 0, 0)));
        alarms.clear("fire", at(5 * SECOND));
        warning(&mut alarms, "fire", 6 * SECOND);
        assert_eq!((alarms.message(), alarms.active()), (None, counts(0, 0, 0)));

        // Overriding lasts until the HMI next starts, and that
        // is a new `Alarms`.
        let mut restarted = Alarms::default();
        warning(&mut restarted, "fire", 0);
        assert!(restarted.message().is_some());
    }

    #[test]
    fn a_resolved_alarm_stays_in_the_table_until_cleared_and_is_new_if_raised_again() {
        let mut alarms = Alarms::default();
        caution(&mut alarms, "fuel", 0);
        alarms.acknowledge_presented(at(SECOND));
        alarms.resolve("fuel", at(2 * SECOND));
        let (_, _, rows) = Table::default().rows(&alarms);
        assert_eq!((rows.len(), rows[0].state), (1, State::Resolved));

        caution(&mut alarms, "fuel", 60 * SECOND);
        assert_eq!(text(&alarms).as_deref(), Some("C \u{2013} fuel"), "it is presented afresh");
        let (_, _, rows) = Table::default().rows(&alarms);
        assert_eq!((rows.len(), rows[0].state, rows[0].acknowledged), (1, State::Active, false));

        alarms.take_requests();
        alarms.clear("fuel", at(61 * SECOND));
        assert_eq!(alarms.take_requests(), [Request::Cleared("fuel".to_string())]);
        assert_eq!(Table::default().rows(&alarms).2, []);
        assert_eq!(alarms.message(), None);
    }

    /// Alarms for the table tests: raised a minute apart in the order given.
    fn several(what: &[(&str, Category)]) -> Alarms {
        let mut alarms = Alarms::default();
        for (minute, (id, category)) in (0..).zip(what) {
            alarms.raise(raised(id, *category, id), at(minute * 60 * SECOND));
        }
        alarms
    }

    fn texts(table: &Table, alarms: &Alarms) -> Vec<String> {
        table.rows(alarms).2.into_iter().map(|row| row.text).collect()
    }

    #[test]
    fn the_table_is_by_category_then_time_or_by_time_then_category() {
        use Category::{Advisory, Caution, Warning};
        let alarms = several(&[("a", Caution), ("b", Advisory), ("c", Warning), ("d", Caution), ("e", Warning)]);
        let mut table = Table::default();
        assert_eq!(
            texts(&table, &alarms),
            ["W \u{2013} e", "W \u{2013} c", "C \u{2013} d", "C \u{2013} a", "A \u{2013} b"]
        );
        table.sort = Sort::Time;
        assert_eq!(
            texts(&table, &alarms),
            ["W \u{2013} e", "C \u{2013} d", "W \u{2013} c", "A \u{2013} b", "C \u{2013} a"]
        );

        // Of one second, the most important is first, whichever was raised first.
        let mut alarms = Alarms::default();
        alarms.raise(raised("a", Advisory, "a"), at(0));
        alarms.raise(raised("w", Warning, "w"), at(0));
        alarms.raise(raised("c", Caution, "c"), at(0));
        assert_eq!(texts(&table, &alarms), ["W \u{2013} w", "C \u{2013} c", "A \u{2013} a"]);
    }

    #[test]
    fn a_row_has_the_alarm_its_time_its_category_and_whether_acknowledged() {
        let mut alarms = several(&[("fuel", Category::Caution)]);
        alarms.acknowledge("fuel", at(SECOND));
        let (_, _, rows) = Table::default().rows(&alarms);
        assert_eq!(
            rows,
            [AlarmRow {
                id: "fuel".to_string(),
                category: Category::Caution,
                text: "C \u{2013} fuel".to_string(),
                time: "041230Z".to_string(),
                acknowledged: true,
                state: State::Active,
                selected: false,
            }]
        );
    }

    #[test]
    fn acknowledged_alarms_are_counted_and_can_be_left_out() {
        use Category::{Advisory, Caution, Warning};
        let mut alarms = several(&[("a", Caution), ("b", Advisory), ("c", Warning), ("d", Caution)]);
        alarms.acknowledge("a", at(0));
        alarms.acknowledge("c", at(0));
        assert_eq!(alarms.acknowledged(true), counts(1, 1, 0));
        assert_eq!(alarms.acknowledged(false), counts(0, 1, 1));

        let mut table = Table::default();
        assert_eq!(texts(&table, &alarms).len(), 4);
        table.acknowledged = false;
        assert_eq!(texts(&table, &alarms), ["C \u{2013} d", "A \u{2013} b"]);

        alarms.acknowledge_all(at(SECOND));
        assert_eq!(alarms.acknowledged(false), counts(0, 0, 0));
        assert_eq!(texts(&table, &alarms), [] as [&str; 0]);
    }

    #[test]
    fn the_table_is_paged_and_the_pick_moves_through_it() {
        let ids: Vec<String> = (0..ROWS_PER_PAGE + 3).map(|n| format!("{n:02}")).collect();
        let what: Vec<(&str, Category)> = ids.iter().map(|id| (id.as_str(), Category::Advisory)).collect();
        let alarms = several(&what);
        let mut table = Table::default();
        let (page, pages, rows) = table.rows(&alarms);
        assert_eq!((page, pages, rows.len()), (1, 2, ROWS_PER_PAGE));
        assert!(rows.iter().all(|row| !row.selected));
        assert_eq!(table.more_pages(&alarms), (false, true));

        // Down picks the first row, then the next; up stops at the top.
        table.step(true, &alarms);
        assert_eq!(table.rows(&alarms).2[0].selected, true);
        table.step(true, &alarms);
        assert_eq!(table.rows(&alarms).2[1].selected, true);
        table.step(false, &alarms);
        table.step(false, &alarms);
        assert_eq!(table.rows(&alarms).2[0].selected, true);

        // The next page takes the pick with it, and the pick takes the page.
        table.turn_page(true, &alarms);
        let (page, _, rows) = table.rows(&alarms);
        assert_eq!((page, rows.len(), rows[0].selected), (2, 3, true));
        assert_eq!(table.more_pages(&alarms), (true, false));
        table.step(false, &alarms);
        let (page, _, rows) = table.rows(&alarms);
        assert_eq!((page, rows[ROWS_PER_PAGE - 1].selected), (1, true));
        for _ in 0..ROWS_PER_PAGE + 10 {
            table.step(true, &alarms);
        }
        let (page, _, rows) = table.rows(&alarms);
        assert_eq!((page, rows[2].selected), (2, true), "down stops at the last row");
    }

    #[test]
    fn the_picked_alarm_is_acknowledged_overridden_cleared_and_shown_at_length() {
        let mut alarms = Alarms::default();
        alarms.raise(Raised { detail: Some("0.8 bar".into()), ..raised("oil", Category::Warning, "oil") }, at(0));
        caution(&mut alarms, "fuel", SECOND);
        let mut table = Table::default();
        assert_eq!(table.selected(&alarms), None);
        assert_eq!(table.can(&alarms), (false, false));

        table.step(true, &alarms);
        assert_eq!(table.selected(&alarms), Some("oil"));
        assert_eq!(table.can(&alarms), (true, true));
        assert_eq!(table.detail(&alarms), None);
        table.show(&alarms);
        let detail = table.detail(&alarms).unwrap();
        assert_eq!((detail.text.as_str(), detail.detail.as_deref()), ("W \u{2013} oil", Some("0.8 bar")));

        alarms.acknowledge("oil", at(2 * SECOND));
        assert_eq!(table.can(&alarms), (false, true));
        alarms.override_alarm("oil", at(3 * SECOND));
        assert_eq!(table.can(&alarms), (false, false));
        alarms.clear("oil", at(4 * SECOND));
        table.settle(&alarms);
        assert_eq!(table.selected(&alarms), None, "the pick does not outlast its alarm");
        assert!(!table.showing(), "nor does its being shown at length");

        alarms.clear_all(at(5 * SECOND));
        assert_eq!(table.rows(&alarms), (1, 1, vec![]));
    }

    #[test]
    fn an_alarm_shown_at_length_stays_shown_when_it_leaves_the_table() {
        let mut alarms = Alarms::default();
        caution(&mut alarms, "fuel", 0);
        caution(&mut alarms, "coolant", SECOND);
        let mut table = Table { acknowledged: false, ..Table::default() };
        table.step(true, &alarms);
        table.step(true, &alarms);
        table.show(&alarms);
        assert_eq!(table.detail(&alarms).unwrap().text, "C \u{2013} fuel");

        // Its ten seconds are up while the crew is reading about it: it is
        // acknowledged, and the table is of what is not.
        assert!(alarms.tick(at(10 * SECOND)));
        table.settle(&alarms);
        assert_eq!(table.rows(&alarms).2.len(), 1);
        let detail = table.detail(&alarms).expect("still shown");
        assert_eq!((detail.text.as_str(), detail.acknowledged), ("C \u{2013} fuel", true));

        // F17 and F18 go on to the alarms that are in the table.
        table.step(true, &alarms);
        assert_eq!(table.detail(&alarms).unwrap().text, "C \u{2013} coolant");
        table.close();
        assert_eq!((table.showing(), table.selected(&alarms)), (false, Some("coolant")));
    }

    #[test]
    fn the_table_does_not_grow_without_limit() {
        let mut alarms = Alarms::default();
        for n in 0..MOST_ALARMS as u64 + 50 {
            alarms.raise(raised(&n.to_string(), Category::Advisory, "x"), at(n));
            alarms.resolve(&n.to_string(), at(n));
        }
        warning(&mut alarms, "fire", 0);
        assert_eq!(alarms.list.len(), MOST_ALARMS);
        assert!(alarms.message().is_some(), "and it is what is over that goes");
    }
}
