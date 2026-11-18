//! The verifier: what is asked whether a PIN is an account's.
//!
//! The HMI checks no PIN itself, and can read nothing that a PIN could be
//! checked against: it runs as a user that owns the display and little else.
//! greetd can - a small daemon that runs as root, puts the question to PAM
//! and says what PAM said - and it is asked over its socket, in its own
//! protocol (greetd-ipc(7)): a length and a line of JSON each way. Only the
//! asking is used: `create_session` for the account, the PIN in answer to
//! PAM's one question, and `cancel_session` once it is known whether PAM was
//! satisfied. Nothing is started for anybody.
//!
//! greetd expects whoever talks to it to be a program it started and will
//! outlive: its "greeter". The HMI is neither, so the greeter greetd starts
//! is a script that says where the socket is and then waits
//! (os/package/greetd/greeter), and the HMI - which runs as the same user,
//! and so may use the socket - reads that and asks.
//!
//! Wrong PINs are counted by PAM (pam_faillock), not here: after too many
//! it will hear no more for the account for a while, and says so in words.
//! Those words are passed on, with how long it said, if it said.
//!
//! This is written from greetd's manual and is none of greetd's code, which
//! is under another licence than the HMI's.

use hmi_access::Answer;
use std::ffi::{CStr, CString};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{env, fs};

/// The file in the HMI's runtime directory that says where greetd is
/// listening: written by the greeter each time greetd starts it.
const WHERE: &str = "opengva-verifier";

/// How long greetd is given to say anything. PAM takes a moment over a PIN
/// on purpose, and two after a wrong one.
const PATIENCE: Duration = Duration::from_secs(10);

/// The most a message from greetd can be: what it says is a line or two.
const LONGEST: usize = 64 * 1024;

/// Ask whether `pin` is `account`'s. Whatever goes wrong on the way, the
/// answer is that it could not be checked: nothing is ever given for want
/// of a verifier.
pub fn check(account: &str, pin: &str) -> Answer {
    let Some(socket) = socket() else {
        return Answer::Unavailable("no verifier is running".to_string());
    };
    ask(&socket, account, pin).unwrap_or_else(Answer::Unavailable)
}

/// Where greetd is listening, if its greeter has said.
fn socket() -> Option<PathBuf> {
    let said = fs::read_to_string(Path::new(&env::var_os("XDG_RUNTIME_DIR")?).join(WHERE)).ok()?;
    Some(PathBuf::from(said.trim())).filter(|path| !path.as_os_str().is_empty())
}

/// Ask the greetd at `socket`. `Err` says why it could not be asked.
fn ask(socket: &Path, account: &str, pin: &str) -> Result<Answer, String> {
    let io = |e: std::io::Error| format!("the verifier does not answer: {e}");
    let mut greetd = UnixStream::connect(socket).map_err(io)?;
    greetd.set_read_timeout(Some(PATIENCE)).map_err(io)?;
    greetd.set_write_timeout(Some(PATIENCE)).map_err(io)?;

    send(&mut greetd, &format!(r#"{{"type":"create_session","username":{}}}"#, quoted(account)?))?;
    // What PAM has said along the way, that was not a question.
    let mut said: Vec<String> = Vec::new();
    let mut given = false;
    let answer = loop {
        let reply = receive(&mut greetd)?;
        let text = |name: &str| reply[name].as_str().unwrap_or_default().trim().to_string();
        match reply["type"].as_str() {
            Some("success") => break Answer::Accepted,
            Some("error") if reply["error_type"] == "auth_error" => {
                // How long PAM says to wait is passed on as a time, which
                // the PIN page counts down; the rest of what it said, as
                // it said it.
                let wait_ms = said.iter().find_map(|line| wait_in(line));
                said.retain(|line| wait_in(line).is_none());
                let why = if said.is_empty() { in_words(&text("description")) } else { said.join(" ") };
                break Answer::Refused { why, wait_ms };
            }
            Some("error") => return Err(format!("the verifier failed: {}", text("description"))),
            Some("auth_message") => match reply["auth_message_type"].as_str() {
                // PAM's question, which is for the PIN.
                Some("secret") if !given => {
                    given = true;
                    let mut answered =
                        format!(r#"{{"type":"post_auth_message_response","response":{}}}"#, quoted(pin)?);
                    let sent = send(&mut greetd, &answered);
                    wipe(&mut answered);
                    sent?;
                }
                // A second question is for more than a PIN - a new one,
                // most likely, the old having run out - and a PIN page
                // has nothing to answer it with.
                Some("secret" | "visible") => {
                    break Answer::Refused {
                        why: format!("more than a PIN is asked for: {}", text("auth_message")),
                        wait_ms: None,
                    };
                }
                // Something PAM has to say: kept, and acknowledged.
                _ => {
                    said.extend(Some(text("auth_message")).filter(|line| !line.is_empty()));
                    send(&mut greetd, r#"{"type":"post_auth_message_response"}"#)?;
                }
            },
            _ => return Err(format!("the verifier said what is not understood: {reply}")),
        }
    };
    // Checked, and that is all that was wanted. After a refusal, too, greetd
    // is to be told that the attempt is over.
    send(&mut greetd, r#"{"type":"cancel_session"}"#)?;
    let _ = receive(&mut greetd);
    Ok(answer)
}

/// A string as JSON writes one.
fn quoted(text: &str) -> Result<String, String> {
    serde_json::to_string(text).map_err(|e| e.to_string())
}

/// Overwrite what held a PIN, for whoever reads the memory later.
fn wipe(held: &mut String) {
    let mut bytes = std::mem::take(held).into_bytes();
    bytes.fill(0);
    std::hint::black_box(&bytes);
}

/// A message to greetd: its length, as this machine writes a number, and
/// then the message.
fn send(greetd: &mut UnixStream, message: &str) -> Result<(), String> {
    let length = u32::try_from(message.len()).map_err(|e| e.to_string())?;
    greetd
        .write_all(&length.to_ne_bytes())
        .and_then(|()| greetd.write_all(message.as_bytes()))
        .map_err(|e| format!("the verifier does not answer: {e}"))
}

/// A message from greetd.
fn receive(greetd: &mut UnixStream) -> Result<serde_json::Value, String> {
    let io = |e: std::io::Error| format!("the verifier does not answer: {e}");
    let mut length = [0u8; 4];
    greetd.read_exact(&mut length).map_err(io)?;
    let length = u32::from_ne_bytes(length) as usize;
    if length > LONGEST {
        return Err(format!("the verifier said what is not understood: {length} bytes of it"));
    }
    let mut message = vec![0u8; length];
    greetd.read_exact(&mut message).map_err(io)?;
    serde_json::from_slice(&message).map_err(|e| format!("the verifier said what is not understood: {e}"))
}

/// Why PAM was not satisfied, where it said nothing itself: greetd gives the
/// name of PAM's return code, which is not a thing to show a crew.
fn in_words(description: &str) -> String {
    let code = description.rsplit(": ").next().unwrap_or_default();
    match code {
        "AUTH_ERR" => "not the account's PIN",
        "MAXTRIES" => "too many tries",
        "USER_UNKNOWN" => "there is no such account",
        "ACCT_EXPIRED" | "CRED_EXPIRED" => "the account has run out",
        "PERM_DENIED" => "the account may not be used here",
        _ => description,
    }
    .to_string()
}

/// How long pam_faillock says it will be before the account can be tried
/// again, in milliseconds: it says "(2 minutes left to unlock)", in whole
/// minutes, rounded up. `None` if the line is not that.
fn wait_in(line: &str) -> Option<u64> {
    let (count, rest) = line.strip_prefix('(')?.split_once(' ')?;
    let minutes: u64 = count.parse().ok()?;
    rest.starts_with("minute").then(|| minutes.saturating_mul(60_000))
}

/// The accounts that hold a role: the members of the Unix group of that
/// name, as the system knows them - its files, or wherever else it has
/// been told to look - and the users whose own group it is. None, where
/// there is no such group. A directory that will not list its users gives
/// the first of these and not the second.
///
/// To be called as the HMI starts, before anything else in the process asks
/// about users or groups: the C library answers these from one place.
pub fn members(role: &str) -> Vec<String> {
    let Ok(name) = CString::new(role) else { return Vec::new() };
    let mut members: Vec<String> = Vec::new();
    // SAFETY: the C library's own records are read while nothing else is
    // asking for them (see above), each pointer is checked before it is
    // followed, and nothing is kept that points into them.
    unsafe {
        let group = libc::getgrnam(name.as_ptr());
        if group.is_null() {
            return members;
        }
        let gid = (*group).gr_gid;
        let mut member = (*group).gr_mem;
        while !member.is_null() && !(*member).is_null() {
            members.push(CStr::from_ptr(*member).to_string_lossy().into_owned());
            member = member.add(1);
        }
        libc::setpwent();
        loop {
            let user = libc::getpwent();
            if user.is_null() {
                break;
            }
            let account = CStr::from_ptr((*user).pw_name).to_string_lossy().into_owned();
            if (*user).pw_gid == gid && !members.contains(&account) {
                members.push(account);
            }
        }
        libc::endpwent();
    }
    members
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::thread;

    /// A stand-in for greetd: it listens where it says, takes one caller,
    /// and for each message it is sent gives the next of `replies`. What it
    /// was sent is what the thread ends with.
    fn greetd(replies: &[&str]) -> (PathBuf, thread::JoinHandle<Vec<serde_json::Value>>) {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let path = env::temp_dir().join(format!(
            "opengva-verifier-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();
        let replies: Vec<String> = replies.iter().map(|reply| reply.to_string()).collect();
        let heard = thread::spawn(move || {
            let (mut caller, _) = listener.accept().unwrap();
            let mut heard = Vec::new();
            for reply in replies {
                let Ok(message) = receive(&mut caller) else { break };
                heard.push(message);
                send(&mut caller, &reply).unwrap();
            }
            // Whatever else it is sent, it hears and does not answer.
            caller.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
            while let Ok(message) = receive(&mut caller) {
                heard.push(message);
            }
            heard
        });
        (path, heard)
    }

    const QUESTION: &str = r#"{"type":"auth_message","auth_message_type":"secret","auth_message":"Password: "}"#;
    const SUCCESS: &str = r#"{"type":"success"}"#;
    const WRONG: &str = r#"{"type":"error","error_type":"auth_error","description":"pam_authenticate: AUTH_ERR"}"#;

    fn types(heard: &[serde_json::Value]) -> Vec<&str> {
        heard.iter().map(|message| message["type"].as_str().unwrap()).collect()
    }

    #[test]
    fn the_right_pin_is_accepted_and_nothing_is_started_for_the_account() {
        let (socket, heard) = greetd(&[QUESTION, SUCCESS, SUCCESS]);
        assert_eq!(ask(&socket, "commander", "246810"), Ok(Answer::Accepted));
        let heard = heard.join().unwrap();
        assert_eq!(types(&heard), ["create_session", "post_auth_message_response", "cancel_session"]);
        assert_eq!(heard[0]["username"], "commander");
        assert_eq!(heard[1]["response"], "246810");
        let _ = fs::remove_file(socket);
    }

    #[test]
    fn a_wrong_pin_is_refused_in_words_and_the_attempt_is_ended() {
        let (socket, heard) = greetd(&[QUESTION, WRONG, SUCCESS]);
        let refused = Answer::Refused { why: "not the account's PIN".to_string(), wait_ms: None };
        assert_eq!(ask(&socket, "commander", "111111"), Ok(refused));
        assert_eq!(types(&heard.join().unwrap()).last(), Some(&"cancel_session"));
        let _ = fs::remove_file(socket);
    }

    #[test]
    fn an_account_being_waited_for_is_not_sent_a_pin_and_says_how_long() {
        let locked = r#"{"type":"auth_message","auth_message_type":"info","auth_message":"The account is locked due to 5 failed logins."}"#;
        let left = r#"{"type":"auth_message","auth_message_type":"info","auth_message":"(1 minute left to unlock)"}"#;
        let (socket, heard) = greetd(&[locked, left, WRONG, SUCCESS]);
        let refused = Answer::Refused {
            why: "The account is locked due to 5 failed logins.".to_string(),
            wait_ms: Some(60_000),
        };
        assert_eq!(ask(&socket, "commander", "246810"), Ok(refused));
        let heard = heard.join().unwrap();
        // PAM's words are acknowledged with nothing in answer to them, and
        // the PIN, never having been asked for, has gone nowhere.
        assert!(heard.iter().all(|message| message.get("response").is_none()));
        assert!(!format!("{heard:?}").contains("246810"));
        let _ = fs::remove_file(socket);
    }

    #[test]
    fn more_than_a_pin_asked_for_is_a_refusal() {
        let new = r#"{"type":"auth_message","auth_message_type":"secret","auth_message":"New password: "}"#;
        let (socket, heard) = greetd(&[QUESTION, new, SUCCESS]);
        let Ok(Answer::Refused { why, wait_ms: None }) = ask(&socket, "commander", "246810") else {
            panic!("a second question is not one a PIN page can answer");
        };
        assert!(why.contains("New password"), "{why}");
        let heard = heard.join().unwrap();
        assert_eq!(heard.iter().filter(|message| message.get("response").is_some()).count(), 1, "the PIN is sent once");
        let _ = fs::remove_file(socket);
    }

    #[test]
    fn with_no_verifier_or_one_that_fails_the_pin_is_not_checked() {
        let nowhere = env::temp_dir().join("opengva-verifier-test-nowhere");
        assert!(ask(&nowhere, "commander", "246810").unwrap_err().contains("does not answer"));

        let failed = r#"{"type":"error","error_type":"error","description":"session not active"}"#;
        let (socket, heard) = greetd(&[failed]);
        assert!(ask(&socket, "commander", "246810").unwrap_err().contains("session not active"));
        heard.join().unwrap();
        let _ = fs::remove_file(&socket);

        let (socket, heard) = greetd(&[r#"{"type":"surprise"}"#]);
        assert!(ask(&socket, "commander", "246810").unwrap_err().contains("not understood"));
        heard.join().unwrap();
        let _ = fs::remove_file(socket);
    }

    #[test]
    fn the_wait_is_read_from_what_pam_says_of_it() {
        assert_eq!(wait_in("(1 minute left to unlock)"), Some(60_000));
        assert_eq!(wait_in("(12 minutes left to unlock)"), Some(720_000));
        assert_eq!(wait_in("The account is locked due to 5 failed logins."), None);
        assert_eq!(wait_in("(soon)"), None);
        assert_eq!(in_words("pam_authenticate: MAXTRIES"), "too many tries");
        assert_eq!(in_words("something else"), "something else");
    }

    #[test]
    fn a_roles_accounts_are_its_groups_members_and_those_whose_own_group_it_is() {
        // Every system has a root whose own group is root's.
        assert!(members("root").contains(&"root".to_string()));
        assert!(members("opengva-no-such-group").is_empty());
        assert!(members("no\0such").is_empty());
    }
}
