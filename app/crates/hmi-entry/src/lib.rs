//! Data entry: forms for the crew to fill in, and the on-screen keyboard
//! they are filled in with.
//!
//! A [`Form`] is a title and some fields, each of free text or a number,
//! whole or not. An [`Entry`] is a form being filled in:
//! what its fields hold, the cursor that goes from one to the next and, while
//! one of them is being entered into, the [`Keyboard`] that types into it.
//! What it has to show is a [`FormPage`], for a UI to draw as it is given;
//! what the crew has saved is a [`Saved`], for whatever the form is for to be
//! told. A [`Typing`] is the same keyboard up over something that is not a
//! form of the HMI's - another program's page - with what it types handed
//! on, a [`Stroke`] at a time.
//!
//! What a form is for is not known here, and nor is where a button press or
//! a touch came from: buttons come in as the numbers of the F keys, touches
//! as what on the page was touched. The keyboard knows nothing of forms: it
//! is told what moves its highlight and what selects a key, and says what
//! was typed.
//!
//! This crate has no windowing or I/O dependencies so that it can be tested
//! on any machine with `cargo test -p hmi-entry`.

mod entry;
mod form;
mod keyboard;
mod typing;

pub use entry::{Entry, FIELDS_BESIDE_KEYBOARD, FieldRow, FormPage, Saved, Standing, Touched, button};
pub use form::{Field, Form, Kind, LONGEST, MOST_FIELDS};
pub use keyboard::{Direction, Key, KeyKind, Keyboard, KeyboardView, Layout, Stroke, Takes};
pub use typing::Typing;
