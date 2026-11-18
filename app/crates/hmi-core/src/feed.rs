//! The feed: what the HMI is told about the vehicle, and what it says back.
//!
//! Each is a line of JSON, in the HMI's own terms. Where the lines come from
//! is not this crate's business: something that knows the vehicle writes
//! them, and reads the HMI's.
//!
//! To the HMI:
//!
//! ```text
//! {"alarm": {"id": "oil", "category": "warning", "subsystem": "Engine",
//!            "component": "Oil", "nature": "Low Pressure",
//!            "detail": "0.8 bar; minimum 1.5 bar"}}
//! {"threat": {"id": "lws-3", "type": "Laser", "description": "Rangefinder",
//!             "priority": "high", "bearing": 800, "range": 1200,
//!             "symbol": "SHGPEXL", "detail": "1064 nm, pulsed"}}
//! {"resolved": "oil"}
//! {"sensor": {"name": "Laser Warning", "status": "ok"}}
//! ```
//!
//! An `id` is the source's name for an alarm or a threat, and no two of
//! them share one: `resolved` says that the alarm or the threat of that name
//! is no longer there.
//!
//! An alarm's `category` is `warning`, `caution` or `advisory`. `subsystem`,
//! `component`, `nature` and `type` are what it is about, in the order a
//! text message gives them; `detail` is more for the crew to
//! call up, and `time` when it happened, in seconds since 1970, if that was
//! not just now.
//!
//! A threat's `type` is what kind of threat it is, and `description` more
//! about that, if there is more. `priority` is `high`,
//! `medium` or `low`: how much it matters beside the others, and `high` if
//! it is not said. `bearing` is where it is from the vehicle, in mils
//! clockwise from the vehicle's front, 0 to 6399, and `range` how far, in
//! metres; either can be left out where it is not known. `symbol` is what
//! to draw it as where threats are drawn: a MIL-STD-2525C symbol
//! identification code, of which the dashes at the end can be left off.
//! Without one it is drawn as a hostile track on the ground, of no stated
//! kind. `detail` and `time` are as an alarm's. A threat said again while
//! it is there is the same threat, moved or better known.
//!
//! A sensor is one that looks for threats: its `name` is what the crew know
//! it as, and its `status` is `ok`, `degraded`, `failed` or `off`.
//! Said again, it is how the sensor of that name now is.
//!
//! From the HMI, when the crew has done something about an alarm or a
//! threat - acknowledged either, overridden or cleared an alarm, cancelled
//! a threat:
//!
//! ```text
//! {"acknowledged": "oil"}
//! {"overridden": "oil"}
//! {"cleared": "oil"}
//! {"cancelled": "lws-3"}
//! ```
//!
//! When the crew has filled in a form and saved it:
//!
//! ```text
//! {"entered": {"form": "load", "fields": {"description": "WATER 20L",
//!                                         "quantity": "48", "weight": null}}}
//! ```
//!
//! `form` is the name the form's configuration gives it, and `fields` what
//! each of its fields holds, by the field's name: what was entered, as it
//! was entered, or `null` where nothing was. A number is its digits - "007"
//! is not 7 - for what a field holds is for whatever the form is for to
//! make sense of, and not the HMI.
//!
//! And whenever it finds someone at the other end of the feed ([`READY`]) -
//! as it starts, or as they do:
//!
//! ```text
//! {"ready": true}
//! ```
//!
//! An HMI that has just started knows of no alarms, threats or sensors, and
//! remembers nothing from before - not what had been acknowledged, nor what
//! overridden. One that has been running knows what
//! it was last told. Either way, what is the case is for its source to say
//! again: an alarm said again is the alarm it was. So is what no longer is
//! the case. The HMI does not take silence for the end of anything.

use crate::alarms::Raised;
use crate::threats::{Detected, Sensor};
use hmi_entry::Saved;
use serde::{Deserialize, Serialize};

/// The line the HMI sends when it finds someone at the other end of the feed.
pub const READY: &str = r#"{"ready":true}"#;

/// What the crew has done about an alarm or a threat, for its source to be
/// told - or has entered into a form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Request {
    Acknowledged(String),
    Overridden(String),
    Cleared(String),
    Cancelled(String),
    /// The crew has filled in a form and saved it: for whatever the form is
    /// for to be told.
    Entered(Saved),
}

/// A line to the HMI.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Report {
    /// An alarm is the case.
    Alarm(Raised),
    /// There is a threat.
    Threat(Detected),
    /// The alarm or the threat of this name no longer is.
    Resolved(String),
    /// A sensor that looks for threats says how it is.
    Sensor(Sensor),
}

impl Report {
    pub fn parse(line: &str) -> Result<Report, String> {
        let report: Report = serde_json::from_str(line).map_err(|e| e.to_string())?;
        let fault = match &report {
            Report::Alarm(alarm) if alarm.id.is_empty() => Some("an alarm needs an id"),
            Report::Threat(threat) => threat.fault(),
            Report::Sensor(sensor) if sensor.name.trim().is_empty() => Some("a sensor needs a name"),
            _ => None,
        };
        fault.map_or(Ok(report), |fault| Err(fault.to_string()))
    }
}

impl Request {
    /// The line from the HMI that says this.
    pub fn line(&self) -> String {
        serde_json::to_string(self).expect("a request is a name, and a string or what a form held")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alarms::Category;
    use crate::threats::{Priority, SensorStatus};

    #[test]
    fn an_alarm_is_read_from_a_line() {
        let line = r#"{"alarm": {"id": "oil", "category": "warning", "subsystem": "Engine", "component": "Oil", "nature": "Low Pressure", "detail": "0.8 bar", "time": 1728045000}}"#;
        let Ok(Report::Alarm(alarm)) = Report::parse(line) else { panic!("{:?}", Report::parse(line)) };
        assert_eq!((alarm.id.as_str(), alarm.category), ("oil", Category::Warning));
        assert_eq!((alarm.subsystem.as_str(), alarm.component.as_str()), ("Engine", "Oil"));
        assert_eq!((alarm.nature.as_str(), alarm.kind.as_str()), ("Low Pressure", ""));
        assert_eq!((alarm.detail.as_deref(), alarm.time), (Some("0.8 bar"), Some(1_728_045_000)));

        let least = Report::parse(r#"{"alarm":{"id":"x","category":"advisory","type":"Service Due"}}"#);
        let Ok(Report::Alarm(alarm)) = least else { panic!("{least:?}") };
        assert_eq!((alarm.category, alarm.kind.as_str(), alarm.detail), (Category::Advisory, "Service Due", None));
    }

    #[test]
    fn a_threat_is_read_from_a_line() {
        let line = r#"{"threat": {"id": "lws-3", "type": "Laser", "description": "Rangefinder", "priority": "medium", "bearing": 800, "range": 1200, "symbol": "SHGPEXL", "detail": "1064 nm", "time": 1728045000}}"#;
        let Ok(Report::Threat(threat)) = Report::parse(line) else { panic!("{:?}", Report::parse(line)) };
        assert_eq!((threat.id.as_str(), threat.kind.as_str(), threat.description.as_str()), ("lws-3", "Laser", "Rangefinder"));
        assert_eq!((threat.priority, threat.bearing, threat.range), (Priority::Medium, Some(800), Some(1200)));
        assert_eq!((threat.symbol.as_deref(), threat.detail.as_deref()), (Some("SHGPEXL"), Some("1064 nm")));
        assert_eq!(threat.time, Some(1_728_045_000));

        let least = Report::parse(r#"{"threat":{"id":"x","type":"Gunshot"}}"#);
        let Ok(Report::Threat(threat)) = least else { panic!("{least:?}") };
        assert_eq!((threat.priority, threat.bearing, threat.range, threat.symbol), (Priority::High, None, None, None));
    }

    #[test]
    fn a_sensor_is_read_from_a_line() {
        assert_eq!(
            Report::parse(r#"{"sensor":{"name":"Laser Warning","status":"degraded"}}"#),
            Ok(Report::Sensor(Sensor { name: "Laser Warning".to_string(), status: SensorStatus::Degraded }))
        );
    }

    #[test]
    fn a_resolution_is_read_from_a_line() {
        assert_eq!(Report::parse(r#"{"resolved":"oil"}"#), Ok(Report::Resolved("oil".to_string())));
    }

    #[test]
    fn a_line_that_is_not_one_of_these_is_refused() {
        for line in [
            "",
            "oil",
            r#"{"alarm":{"id":"x"}}"#,
            r#"{"alarm":{"id":"x","category":"dire"}}"#,
            r#"{"alarm":{"id":"","category":"warning"}}"#,
            r#"{"alarm":{"id":"x","category":"warning","colour":"red"}}"#,
            r#"{"threat":{"id":"x"}}"#,
            r#"{"threat":{"id":"","type":"Laser"}}"#,
            r#"{"threat":{"id":"x","type":""}}"#,
            r#"{"threat":{"id":"x","type":"Laser","priority":"dire"}}"#,
            r#"{"threat":{"id":"x","type":"Laser","bearing":6400}}"#,
            r#"{"threat":{"id":"x","type":"Laser","bearing":-1}}"#,
            r#"{"threat":{"id":"x","type":"Laser","symbol":"a laser"}}"#,
            r#"{"threat":{"id":"x","type":"Laser","elevation":3}}"#,
            r#"{"sensor":{"name":"","status":"ok"}}"#,
            r#"{"sensor":{"name":"Laser Warning","status":"fine"}}"#,
            r#"{"position":"30U XC 123 456"}"#,
        ] {
            assert!(Report::parse(line).is_err(), "{line}");
        }
    }

    #[test]
    fn what_the_crew_did_is_written_as_a_line() {
        assert_eq!(Request::Acknowledged("oil".to_string()).line(), r#"{"acknowledged":"oil"}"#);
        assert_eq!(Request::Overridden("oil".to_string()).line(), r#"{"overridden":"oil"}"#);
        assert_eq!(Request::Cleared("a \"b\"".to_string()).line(), r#"{"cleared":"a \"b\""}"#);
        assert_eq!(Request::Cancelled("lws-3".to_string()).line(), r#"{"cancelled":"lws-3"}"#);
    }

    #[test]
    fn what_the_crew_entered_is_written_as_a_line() {
        let field = |name: &str, held: Option<&str>| (name.to_string(), held.map(str::to_string));
        let saved = Saved {
            form: "load".to_string(),
            fields: vec![field("description", Some("WATER 20L")), field("serial", Some("007")), field("weight", None)],
        };
        // The fields in the form's order, each as it was entered - a number
        // is its digits - and one that nothing was entered into as null.
        assert_eq!(
            Request::Entered(saved).line(),
            r#"{"entered":{"form":"load","fields":{"description":"WATER 20L","serial":"007","weight":null}}}"#
        );
    }
}
