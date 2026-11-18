//! A view of a VNC server's screen, less its window.
//!
//! - [`View`] is what a view is told to be: its configuration.
//! - [`Bridge`] is the way from the page that shows the screen to the server
//!   whose screen it is.
//!
//! The VNC itself is neither's. It is spoken by noVNC, in the page, and by
//! the server, and nothing here looks at what they say to each other.

mod bridge;
mod view;

pub use bridge::Bridge;
pub use view::{Scale, Server, View};
