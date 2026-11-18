//! The way from the page to the server.
//!
//! noVNC, in the page, speaks down a WebSocket; a VNC server listens on a TCP
//! port. This listens for the page - on the loopback address, where nothing
//! off the machine can reach it, and on a port the system picks - and for
//! each connection the page makes, makes one to the server and passes on
//! what each says to the other as it comes. None of it is looked at.
//!
//! Anything on the machine can find a port. The page is the only thing meant
//! to use this one, so it is given a key - the path it is to ask for - and a
//! connection that asks for anything else is turned away.
//!
//! When the server cannot be reached, or goes, the page's WebSocket is closed
//! with the reason, in the operating system's words for it. Saying so on the
//! screen, and trying again, are the page's to do: every connection it makes
//! is a new one to the server.
//!
//! The WebSocket is tungstenite's, on tokio.

use crate::Server;
use futures_util::{SinkExt, StreamExt};
use socket2::{SockRef, TcpKeepalive};
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::thread;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{sleep, timeout};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::{WebSocketStream, accept_hdr_async};

/// How long a server is given to answer when it is called.
const ANSWERED_WITHIN: Duration = Duration::from_secs(5);

/// A server that is switched off, or cut off, says nothing more: not even
/// that it has gone. One that has said nothing for this long is asked
/// whether it is there, and again every second...
const ASKED_AFTER: Duration = Duration::from_secs(5);
/// ...and one that has not answered for this long has gone.
const GONE_AFTER: Duration = Duration::from_secs(10);

/// How much of what the server says is passed on at a time.
const AT_A_TIME: usize = 64 * 1024;

/// How long the page is given to hear why its connection is being closed.
const HEARD_WITHIN: Duration = Duration::from_secs(2);

/// The longest reason a WebSocket can be closed with, in bytes.
const LONGEST_REASON: usize = 123;

/// The bridge, listening. It goes on listening for as long as the program
/// runs.
pub struct Bridge {
    address: SocketAddr,
    key: String,
}

impl Bridge {
    /// Start listening for the page, to join it to `server`.
    pub fn open(server: Server) -> io::Result<Bridge> {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let listener = {
            let _on_the_runtime = runtime.enter();
            TcpListener::from_std(listener)?
        };
        let key = format!("{:032x}", rand::random::<u128>());
        let path = format!("/{key}");
        thread::spawn(move || runtime.block_on(listen(listener, server, path)));
        Ok(Bridge { address, key })
    }

    /// Where the page is to connect: the port, and the key to it.
    pub fn url(&self) -> String {
        format!("ws://{}/{}", self.address, self.key)
    }
}

async fn listen(listener: TcpListener, server: Server, path: String) {
    loop {
        match listener.accept().await {
            Ok((page, _)) => {
                tokio::spawn(carry(page, server.clone(), path.clone()));
            }
            // The machine is out of something, which it will still be out
            // of if it is asked again at once.
            Err(_) => sleep(Duration::from_millis(100)).await,
        }
    }
}

/// One connection of the page's: to the server and back, until either ends.
async fn carry(page: TcpStream, server: Server, path: String) {
    // What is said is wanted now, a touch above all: none of it is held
    // back to be sent with more.
    let _ = page.set_nodelay(true);
    let keyed = |request: &Request, response: Response| {
        if request.uri().path() == path {
            return Ok(response);
        }
        let mut refusal = ErrorResponse::new(None);
        *refusal.status_mut() = StatusCode::FORBIDDEN;
        Err(refusal)
    };
    let Ok(page) = accept_hdr_async(page, keyed).await else { return };

    let called = timeout(ANSWERED_WITHIN, TcpStream::connect((server.host.as_str(), server.port))).await;
    let mut server = match called {
        Ok(Ok(server)) => server,
        Ok(Err(why)) => return close(page, &why.to_string()).await,
        Err(_) => return close(page, "no answer").await,
    };
    let _ = server.set_nodelay(true);
    let _ = miss(&server);

    // Each way on its own, so that neither waits on the other, until one
    // ends: with a reason, if it is the server that has gone, and with none
    // if it is the page, when there is nobody to give one to.
    let (mut to_page, mut from_page) = page.split();
    let (mut from_server, mut to_server) = server.split();
    let up = async {
        while let Some(Ok(said)) = from_page.next().await {
            match said {
                Message::Binary(bytes) => {
                    if let Err(why) = to_server.write_all(&bytes).await {
                        return Some(why.to_string());
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        None
    };
    let down = async {
        let mut some = vec![0; AT_A_TIME];
        loop {
            match from_server.read(&mut some).await {
                Ok(0) => break Some("it closed the connection".to_string()),
                Ok(read) if to_page.send(Message::binary(some[..read].to_vec())).await.is_ok() => {}
                Ok(_) => break None,
                Err(why) => break Some(why.to_string()),
            }
        }
    };
    let why = tokio::select! {
        why = up => why,
        why = down => why,
    };
    if let (Some(why), Ok(page)) = (why, to_page.reunite(from_page)) {
        close(page, &why).await;
    }
}

/// Have the kernel miss a server that has gone without saying so.
fn miss(server: &TcpStream) -> io::Result<()> {
    let server = SockRef::from(server);
    let asked = (GONE_AFTER - ASKED_AFTER).as_secs() as u32;
    server.set_tcp_keepalive(
        &TcpKeepalive::new().with_time(ASKED_AFTER).with_interval(Duration::from_secs(1)).with_retries(asked),
    )?;
    // And one that does not acknowledge what it is sent.
    server.set_tcp_user_timeout(Some(GONE_AFTER))
}

/// Close the page's WebSocket and say why.
async fn close(mut page: WebSocketStream<TcpStream>, why: &str) {
    let mut end = why.len().min(LONGEST_REASON);
    while !why.is_char_boundary(end) {
        end -= 1;
    }
    let frame = CloseFrame { code: CloseCode::Error, reason: why[..end].into() };
    // The page is sure to have heard once it has answered: a connection
    // dropped with something of the page's still unread is one the page may
    // see as broken, with the reason lost.
    if page.close(Some(frame)).await.is_ok() {
        let _ = timeout(HEARD_WITHIN, async { while let Some(Ok(_)) = page.next().await {} }).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{ErrorKind, Read, Write};
    use std::net::{TcpListener as Listener, TcpStream as Stream, UdpSocket};
    use std::process::{Child, Command, Stdio};
    use std::time::Instant;
    use tungstenite::{HandshakeError, WebSocket};

    /// How long anything in a test is waited for.
    const SOON: Duration = Duration::from_secs(5);

    /// A stand-in for a VNC server: something listening on a TCP port.
    fn listening() -> (Listener, Server) {
        let listener = Listener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        (listener, Server { host: "127.0.0.1".to_string(), port })
    }

    fn accepted(listener: &Listener) -> Stream {
        let (stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(SOON)).unwrap();
        stream
    }

    /// A stand-in for the page: the other end of a WebSocket, and what has
    /// come down it that has not been looked at yet.
    struct Page {
        socket: WebSocket<Stream>,
        unread: Vec<u8>,
    }

    impl Page {
        fn open(url: &str) -> tungstenite::Result<Page> {
            let address = url.trim_start_matches("ws://").split('/').next().unwrap();
            let stream = Stream::connect(address)?;
            stream.set_read_timeout(Some(SOON))?;
            match tungstenite::client(url, stream) {
                Ok((socket, _)) => Ok(Page { socket, unread: Vec::new() }),
                Err(HandshakeError::Failure(why)) => Err(why),
                Err(HandshakeError::Interrupted(_)) => unreachable!("the stream waits"),
            }
        }

        fn says(&mut self, bytes: &[u8]) {
            self.socket.send(Message::binary(bytes.to_vec())).unwrap();
        }

        /// The next `bytes` bytes that reach it, however they are parcelled.
        fn hears(&mut self, bytes: usize) -> Vec<u8> {
            while self.unread.len() < bytes {
                match self.socket.read().expect("something for the page") {
                    Message::Binary(some) => self.unread.extend_from_slice(&some),
                    other => panic!("the page was sent {other:?}"),
                }
            }
            self.unread.drain(..bytes).collect()
        }

        /// Why its WebSocket was closed, which it hears after whatever was
        /// said before that.
        fn closed(&mut self) -> (CloseCode, String) {
            loop {
                match self.socket.read().expect("a reason for the page") {
                    Message::Binary(_) => {}
                    Message::Close(Some(frame)) => return (frame.code, frame.reason.to_string()),
                    other => panic!("the page was sent {other:?}"),
                }
            }
        }
    }

    #[test]
    fn what_each_says_reaches_the_other() {
        let (listener, server) = listening();
        let bridge = Bridge::open(server).unwrap();
        let mut page = Page::open(&bridge.url()).unwrap();
        let mut server = accepted(&listener);

        server.write_all(b"RFB 003.008\n").unwrap();
        assert_eq!(page.hears(12), b"RFB 003.008\n");
        page.says(b"RFB 003.008\n");
        let mut said = [0; 12];
        server.read_exact(&mut said).unwrap();
        assert_eq!(&said, b"RFB 003.008\n");

        // A screenful comes whole and in order, whatever it is parcelled as,
        // while the page goes on saying things of its own.
        let screen: Vec<u8> = (0..3_000_000u32).map(|n| (n % 251) as u8).collect();
        let sent = screen.clone();
        let mut from_server = server.try_clone().unwrap();
        let sending = thread::spawn(move || from_server.write_all(&sent).unwrap());
        for touch in 0..100 {
            page.says(&[5, 1, 0, touch, 0, touch]);
        }
        assert!(page.hears(screen.len()) == screen);
        sending.join().unwrap();
        let mut touches = [0; 600];
        server.read_exact(&mut touches).unwrap();
        assert!(touches.chunks(6).zip(0..).all(|(touch, n)| touch == [5, 1, 0, n, 0, n]));
    }

    #[test]
    fn a_connection_without_the_key_is_turned_away() {
        let (listener, server) = listening();
        listener.set_nonblocking(true).unwrap();
        let bridge = Bridge::open(server).unwrap();
        let port = bridge.address.port();

        for wrong in ["", "0", "websockify", &format!("{}0", bridge.key), &format!("x/{}", bridge.key)] {
            match Page::open(&format!("ws://127.0.0.1:{port}/{wrong}")) {
                Err(tungstenite::Error::Http(refusal)) => {
                    assert_eq!(refusal.status(), StatusCode::FORBIDDEN, "{wrong}")
                }
                Err(other) => panic!("'{wrong}' was not turned away as it should be: {other}"),
                Ok(_) => panic!("'{wrong}' was not turned away"),
            }
        }
        // And the server never hears of it.
        thread::sleep(Duration::from_millis(200));
        assert_eq!(listener.accept().unwrap_err().kind(), ErrorKind::WouldBlock);

        // The key is different every time, and too long to be guessed.
        let (_listener, server) = listening();
        let other = Bridge::open(server).unwrap();
        assert_ne!(bridge.key, other.key);
        assert_eq!(bridge.key.len(), 32);
    }

    #[test]
    fn it_listens_to_this_machine_only() {
        let (_listener, server) = listening();
        let bridge = Bridge::open(server).unwrap();
        assert!(bridge.address.ip().is_loopback());
        assert!(bridge.url().starts_with("ws://127.0.0.1:"));

        // By an address of this machine's that others can reach, if it has
        // one, there is nothing at the port. (Which address that is, is
        // found by asking the way to another machine: nothing is sent.)
        let outward = UdpSocket::bind("0.0.0.0:0").and_then(|socket| {
            socket.connect("192.0.2.1:9")?;
            socket.local_addr()
        });
        if let Some(outward) = outward.ok().map(|address| address.ip()).filter(|ip| !ip.is_loopback()) {
            let reached = Stream::connect_timeout(&SocketAddr::new(outward, bridge.address.port()), SOON);
            assert_eq!(reached.unwrap_err().kind(), ErrorKind::ConnectionRefused, "by {outward}");
        }
    }

    #[test]
    fn a_server_that_is_not_there_is_said_not_to_be() {
        // A port that nothing listens on: one that something just did.
        let (listener, server) = listening();
        drop(listener);
        let bridge = Bridge::open(server).unwrap();

        let (code, why) = Page::open(&bridge.url()).unwrap().closed();
        assert_eq!(code, CloseCode::Error);
        assert!(why.contains("refused"), "{why}");
        // However often it is asked for.
        let (_, why) = Page::open(&bridge.url()).unwrap().closed();
        assert!(why.contains("refused"), "{why}");
    }

    #[test]
    fn a_server_that_goes_is_said_to_have_gone_and_is_found_again_when_it_is_back() {
        let (listener, server) = listening();
        let bridge = Bridge::open(server).unwrap();
        let mut first = Page::open(&bridge.url()).unwrap();
        let mut server = accepted(&listener);
        server.write_all(b"here").unwrap();
        assert_eq!(first.hears(4), b"here");

        drop(server);
        assert_eq!(first.closed(), (CloseCode::Error, "it closed the connection".to_string()));

        // The page's next connection is a new one to the server.
        let mut second = Page::open(&bridge.url()).unwrap();
        accepted(&listener).write_all(b"back").unwrap();
        assert_eq!(second.hears(4), b"back");
    }

    #[test]
    fn when_the_page_goes_so_does_its_connection_to_the_server() {
        let (listener, server) = listening();
        let bridge = Bridge::open(server).unwrap();
        let mut page = Page::open(&bridge.url()).unwrap();
        let mut server = accepted(&listener);
        page.says(b"last");

        // Gone without a word, as a page that is closed goes: the server
        // hears what was said, and then that there is no more.
        drop(page);
        let mut said = Vec::new();
        server.read_to_end(&mut said).unwrap();
        assert_eq!(said, b"last");
    }

    #[test]
    fn a_reason_is_no_longer_than_a_websocket_can_carry() {
        // A reason too long to be said whole is cut where a character ends:
        // 'é' is two bytes, and the last that there is room for is its first.
        let long = format!("{}{}", "x".repeat(122), "é".repeat(10));
        let listener = Listener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("ws://{}/", listener.local_addr().unwrap());
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let closing = thread::spawn(move || {
            runtime.block_on(async {
                let (page, _) = TcpListener::from_std(listener).unwrap().accept().await.unwrap();
                close(tokio_tungstenite::accept_async(page).await.unwrap(), &long).await;
            })
        });
        let (_, why) = Page::open(&url).unwrap().closed();
        assert_eq!(why, "x".repeat(122));
        closing.join().unwrap();
    }

    /// QEMU with nothing to start from: a machine that shows its firmware's
    /// screen, and serves it by VNC. It is stopped when this is dropped.
    struct Qemu {
        process: Child,
        server: Server,
    }

    impl Qemu {
        /// Start one, on the first of QEMU's displays from 70 up whose port
        /// is free: a display's port is 5900 and its number.
        fn start() -> Qemu {
            let free = |display: &u16| Listener::bind(("127.0.0.1", 5900 + display)).is_ok();
            Qemu::start_on((70..100).find(free).expect("a free port"))
        }

        fn start_on(display: u16) -> Qemu {
            let process = Command::new("qemu-system-x86_64")
                .args(["-display", "none", "-m", "64", "-nodefaults", "-vga", "std"])
                .args(["-vnc", &format!("127.0.0.1:{display}")])
                .stdin(Stdio::null())
                .spawn()
                .expect("qemu-system-x86_64, which this test needs");
            let qemu = Qemu { process, server: Server { host: "127.0.0.1".to_string(), port: 5900 + display } };
            let started = Instant::now();
            while Stream::connect(("127.0.0.1", qemu.server.port)).is_err() {
                assert!(started.elapsed() < 2 * SOON, "QEMU is not serving its display {display}");
                thread::sleep(Duration::from_millis(50));
            }
            qemu
        }
    }

    impl Drop for Qemu {
        fn drop(&mut self) {
            let _ = self.process.kill();
            let _ = self.process.wait();
        }
    }

    /// As much of RFB as says that a VNC server is at the other end: the two
    /// agree which version of it to speak and that no password is wanted,
    /// and the server says what its screen is. Returns the screen's width,
    /// its height and its name.
    fn greeted(page: &mut Page) -> (u16, u16, String) {
        assert_eq!(page.hears(12), b"RFB 003.008\n");
        page.says(b"RFB 003.008\n");
        let offered = page.hears(1)[0] as usize;
        assert!(page.hears(offered).contains(&1), "the server wants a password");
        page.says(&[1]);
        assert_eq!(page.hears(4), [0, 0, 0, 0], "the server did not let the page in");
        // To share the screen with whoever else is looking at it.
        page.says(&[1]);
        let screen = page.hears(24);
        let name = page.hears(u32::from_be_bytes([screen[20], screen[21], screen[22], screen[23]]) as usize);
        (
            u16::from_be_bytes([screen[0], screen[1]]),
            u16::from_be_bytes([screen[2], screen[3]]),
            String::from_utf8_lossy(&name).to_string(),
        )
    }

    #[test]
    #[ignore = "needs qemu-system-x86_64"]
    fn a_vnc_server_is_spoken_to_missed_when_it_goes_and_found_when_it_is_back() {
        let qemu = Qemu::start();
        let server = qemu.server.clone();
        let bridge = Bridge::open(server.clone()).unwrap();

        let mut page = Page::open(&bridge.url()).unwrap();
        let (width, height, name) = greeted(&mut page);
        assert!(width >= 320 && height >= 200, "{width} x {height}");
        assert!(name.starts_with("QEMU"), "{name}");

        // The page asks for the whole screen and is sent it: one rectangle
        // or more, of which this reads only how many.
        let (wide, high) = (width.to_be_bytes(), height.to_be_bytes());
        page.says(&[3, 0, 0, 0, 0, 0, wide[0], wide[1], high[0], high[1]]);
        let update = page.hears(4);
        assert_eq!(update[0], 0, "a framebuffer update");
        assert!(u16::from_be_bytes([update[2], update[3]]) >= 1);

        // The server is switched off...
        drop(qemu);
        let (code, _) = page.closed();
        assert_eq!(code, CloseCode::Error);
        // ...is not there while it is off...
        let (_, why) = Page::open(&bridge.url()).unwrap().closed();
        assert!(why.contains("refused"), "{why}");
        // ...and is there again when it is back on.
        let _qemu = Qemu::start_on(server.port - 5900);
        let (again, _, _) = greeted(&mut Page::open(&bridge.url()).unwrap());
        assert_eq!(again, width);
    }
}
