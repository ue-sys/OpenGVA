//! A view of a web page, less its window.
//!
//! - [`View`] is what a view is told to be: its configuration.
//! - [`Places`] is where it may go: the servers its page and whatever that
//!   leads to may be fetched from.
//! - [`Backoff`] is how long it waits before trying again what did not work.
//!
//! The page is nobody's here. WebKit fetches it and draws it, in the viewer,
//! and nothing in this crate looks at what a server sends.
//!
//! This crate has no windowing dependencies so that it can be tested on any
//! machine with `cargo test -p web-view`.

mod backoff;
mod places;
mod view;

pub use backoff::Backoff;
pub use places::{Places, server};
pub use view::{Identity, View};
