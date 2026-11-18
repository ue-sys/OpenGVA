//! The bridge on its own, for working on the page in a browser, where there
//! is no shell to start one:
//!
//!     cargo run -p vnc-view --example bridge -- 192.168.1.20:5900
//!
//! It prints where the page is to connect - which the page is given in its
//! address, as `?url=` (src/bridge.ts) - and joins the page to that VNC
//! server until it is interrupted.

use std::{env, process, thread};
use vnc_view::{Bridge, View};

fn main() {
    let server = env::args().nth(1).unwrap_or_default();
    let view = View::parse(&format!("window bridge\nserver {server}")).unwrap_or_else(|why| {
        eprintln!("usage: bridge HOST[:PORT]    ({why})");
        process::exit(2);
    });
    let bridge = Bridge::open(view.server).expect("a port to listen on");
    println!("{}", bridge.url());
    loop {
        thread::park();
    }
}
