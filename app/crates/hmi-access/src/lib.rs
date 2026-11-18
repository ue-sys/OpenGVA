//! Access: who may have what at the crew station.
//!
//! The crew station comes up and is used with nobody logged in, and nothing
//! asked of anyone. A PIN is asked for only when the crew go to something
//! that needs a role: a functional area, a view of one, or a function of the
//! HMI's own, that its configuration marks with the roles any one of which
//! will do ([`Require`]). The right PIN shows what was asked for, and whoever
//! gave it is logged in until they log out or have left the display alone
//! for the idle time; Cancel goes back to where the crew were. Whatever is
//! not marked - which is everything, until a configuration says otherwise -
//! is never asked about.
//!
//! An [`Access`] is what the HMI's core keeps: who is logged in, and the PIN
//! being asked for, if one is. It is told what the crew want, the buttons
//! and touches that are a PIN page's, the time, and what the verifier said
//! of a PIN; it says what there is to draw ([`PinPage`]), what there is for
//! the verifier to check ([`Check`]) and what there is to write down
//! ([`Audit`]). It checks no PIN itself and keeps none: whether a PIN is an
//! account's is for something with the privilege to know, outside the HMI.
//! Buttons come in as the numbers of the F keys and touches as what on the
//! page was touched, as they do to a form; what is wanted is whatever the
//! core says it is, and is handed back when it is given.
//!
//! A role is a word. So is an account. What either means outside the HMI -
//! a Unix group and its members, where the HMI is on the image - is for
//! whatever reads the configuration to say ([`Config::read`]).
//!
//! Some things are never asked about, and a configuration that tries is
//! refused ([`NEVER`]): what the display owes to whoever is looking at it.
//! And four buttons are never the PIN page's ([`Pressed::Passed`]): an alert
//! is seen and acknowledged, and the labels hidden and shown, whatever is
//! being asked.
//!
//! The PIN is typed on the phone pad of `hmi-entry`, by touch or with the
//! buttons down the sides, which have its digits on them. The page carries
//! how many digits have been entered and never which; nor does anything
//! this crate prints.
//!
//! This crate has no windowing or I/O dependencies so that it can be tested
//! on any machine with `cargo test -p hmi-access`.

mod access;
mod config;

pub use access::{
    ASKING_MS, Access, AccountRow, Answer, Asked, Audit, Check, Event, FEWEST_DIGITS, MOST_DIGITS, Pin, PinPage,
    Pressed, Session, Standing, Touched, button,
};
pub use config::{Config, IDLE_MS, NEVER, Require};
