//! Tests of what needs a role (`hmi-access`, and `require` in an area's
//! configuration): that where nothing is marked nothing is asked, and that
//! what is marked asks for a PIN, is given for the right one and to nobody
//! else, and never keeps an alert waiting.

use crate::*;

const WPN: u8 = 1;
const DEF: u8 = 2;
const SYS: u8 = 3;
const DRV: u8 = 4;
const COM: u8 = 6;
const BMS: u8 = 7;

fn at(ms: u64) -> Time {
    Time { unix_s: 1_728_045_000 + ms / 1000, uptime_ms: ms }
}

/// Press a button and let it go.
fn press(hmi: &mut Hmi, button: Button, ms: u64) {
    hmi.button(ButtonEvent { button, action: Action::Pressed }, at(ms));
    hmi.button(ButtonEvent { button, action: Action::Released }, at(ms));
}

fn f(hmi: &mut Hmi, n: u8, ms: u64) {
    press(hmi, Button::F(n), ms);
}

/// Give an area away as a configuration file would.
fn give(hmi: &mut Hmi, text: &str) -> Result<(), String> {
    match External::parse(text)? {
        External::Window(area) => hmi.add_external(area),
        External::Tiles(area) => hmi.add_tiled(area),
        External::Views(area) => hmi.add_viewed(area),
        External::Form(area) => hmi.add_form(area),
    }
}

/// A crew station with a commander - the account "cmdr" - and a maintainer's
/// role that nobody at it holds; idle after five minutes; and overriding an
/// alarm the commander's. WPN is a sight's window, anybody's, and can be
/// typed into; COM is a form, and the commander's; BMS is two views, of
/// which the second is the commander's. Every window is open.
fn hmi() -> Hmi {
    let members = |role: &str| if role == "commander" { vec!["cmdr".to_string()] } else { Vec::new() };
    let text = "role commander\nrole maintainer\nidle 5m\nfunction alarms.override commander";
    let mut hmi = Hmi::new();
    hmi.set_access(AccessConfig::read(text, &FUNCTIONS, members).unwrap());
    give(&mut hmi, "area WPN\nwindow sight\nkeyboard yes").unwrap();
    give(&mut hmi, "area COM\nrequire commander\nform net Net Settings\nfield callsign text 8 Call Sign").unwrap();
    give(&mut hmi, "area BMS\nview Map\nwindow map\nview Orders\nwindow orders\nrequire commander").unwrap();
    hmi.windows(&["sight", "map", "orders"]);
    hmi
}

/// The page on the screen, in a word or two.
fn page(hmi: &Hmi, ms: u64) -> String {
    match hmi.frame(at(ms), Vec::new()).page {
        Page::Own { title, .. } => format!("own {title}"),
        Page::External { keyboard, .. } => format!("external{}", if keyboard.is_some() { " keyboard" } else { "" }),
        Page::Tiles { .. } => "tiles".to_string(),
        Page::Form(_) => "form".to_string(),
        Page::Pin(pin) => format!("pin {}", pin.wanted),
        Page::Alarms { .. } => "alarms".to_string(),
        Page::Threats { .. } => "threats".to_string(),
    }
}

fn pin(hmi: &Hmi, ms: u64) -> PinPage {
    match hmi.frame(at(ms), Vec::new()).page {
        Page::Pin(pin) => pin,
        other => panic!("not a PIN page: {other:?}"),
    }
}

fn state(hmi: &Hmi, area: u8) -> AreaState {
    hmi.frame(at(0), Vec::new()).areas[usize::from(area)].state
}

fn label(hmi: &Hmi, n: u8, ms: u64) -> (String, LabelState) {
    let label = hmi.frame(at(ms), Vec::new()).labels.swap_remove(usize::from(n) - 1);
    (label.text, label.state)
}

/// The rows of the SYS page, which is the page on screen.
fn sys_rows(hmi: &Hmi) -> Vec<(String, String)> {
    match hmi.frame(at(0), Vec::new()).page {
        Page::Own { rows, .. } => rows.into_iter().map(|row| (row.name, row.value)).collect(),
        other => panic!("not the SYS page: {other:?}"),
    }
}

/// Type digits on the buttons that have them.
fn digits(hmi: &mut Hmi, pin: &str, ms: u64) {
    for digit in pin.bytes() {
        f(hmi, if digit == b'0' { 10 } else { digit - b'0' }, ms);
    }
}

/// Type a PIN and enter it: what there is to check.
fn enter(hmi: &mut Hmi, pin: &str, ms: u64) -> Check {
    digits(hmi, pin, ms);
    f(hmi, 20, ms);
    hmi.take_check().expect("a PIN to check")
}

/// Go to COM and give the commander's PIN.
fn log_in(hmi: &mut Hmi, ms: u64) {
    press(hmi, Button::Area(COM), ms);
    let check = enter(hmi, "246810", ms);
    assert!(hmi.verified(check.serial, Answer::Accepted, at(ms)));
    assert_eq!(hmi.session().account(), Some("cmdr"));
}

fn refused() -> Answer {
    Answer::Refused { why: "not the account's PIN".to_string(), wait_ms: None }
}

#[test]
fn where_nothing_needs_a_role_nothing_is_asked_and_sys_says_nothing_of_logging_in() {
    let mut hmi = Hmi::new();
    give(&mut hmi, "area WPN\nwindow sight").unwrap();
    hmi.windows(&["sight"]);
    press(&mut hmi, Button::Area(WPN), 0);
    assert_eq!(page(&hmi, 0), "external");
    assert!(hmi.take_check().is_none() && hmi.take_audit().is_empty() && hmi.deadline().is_none());
    assert_eq!(hmi.session(), &Session::Nobody);

    press(&mut hmi, Button::Area(SYS), 0);
    assert!(sys_rows(&hmi).iter().all(|(name, _)| name != "Logged in"));
    assert_eq!(label(&hmi, 11, 0).1, LabelState::Blank, "nor is there anything to log out of");

    // An area that asks for a role, where there are none, is a mistake: it
    // is refused, not given to all.
    let why = give(&mut hmi, "area COM\nrequire commander\nform net Net\nfield a text 4 A").unwrap_err();
    assert!(why.contains("there is no role 'commander'"), "{why}");
    assert_eq!(state(&hmi, COM), AreaState::Absent);
}

#[test]
fn an_area_that_needs_a_role_asks_for_a_pin_and_the_right_one_shows_it() {
    use LabelState::{Available, Blank, Unavailable};
    let mut hmi = hmi();
    assert_eq!(state(&hmi, COM), AreaState::Available, "it is to be had, for the asking");
    press(&mut hmi, Button::Area(COM), 1000);
    assert_eq!(page(&hmi, 1000), "pin COM");
    assert_eq!((state(&hmi, SYS), state(&hmi, COM)), (AreaState::Selected, AreaState::Available));
    assert_eq!(pin(&hmi, 1000).account, "cmdr");

    // The buttons down the sides have the pad's digits, and
    // F13 and F20 are a dialog's.
    assert_eq!((label(&hmi, 1, 1000), label(&hmi, 10, 1000)), (("1".into(), Available), ("0".into(), Available)));
    assert_eq!((label(&hmi, 11, 1000), label(&hmi, 12, 1000).1), (("Backspace".into(), Unavailable), Blank));
    assert_eq!((label(&hmi, 13, 1000), label(&hmi, 20, 1000)), (("Cancel".into(), Available), ("Enter".into(), Unavailable)));
    assert_eq!(label(&hmi, 14, 1000), ("Alarms".into(), Available), "the others are what they are everywhere");

    let check = enter(&mut hmi, "246810", 2000);
    assert_eq!((check.account.as_str(), check.pin.reveal()), ("cmdr", "246810"));
    assert_eq!(pin(&hmi, 2000).standing, hmi_access::Standing::Checking);
    assert!(hmi.verified(check.serial, Answer::Accepted, at(2500)));
    assert_eq!(page(&hmi, 2500), "form");
    assert_eq!(state(&hmi, COM), AreaState::Selected);

    // SYS says who is logged in, and has the button that logs out.
    press(&mut hmi, Button::Area(SYS), 3000);
    assert!(sys_rows(&hmi).contains(&("Logged in".to_string(), "cmdr".to_string())));
    assert_eq!(label(&hmi, 11, 3000), ("Log Out".into(), Available));
    // Whoever is logged in is not asked again.
    press(&mut hmi, Button::Area(COM), 4000);
    assert_eq!(page(&hmi, 4000), "form");
}

#[test]
fn cancel_returns_the_crew_to_where_they_were_and_a_wrong_pin_shows_nothing() {
    let mut hmi = hmi();
    press(&mut hmi, Button::Area(WPN), 0);
    assert_eq!((page(&hmi, 0).as_str(), hmi.window_to_show()), ("external", Some("sight")));

    press(&mut hmi, Button::Area(COM), 100);
    assert_eq!(page(&hmi, 100), "pin COM");
    assert_eq!(hmi.window_to_show(), None, "the PIN page is the HMI's own: no program's picture is under it");
    digits(&mut hmi, "24", 200);
    f(&mut hmi, 13, 300);
    assert_eq!((page(&hmi, 300).as_str(), hmi.window_to_show()), ("external", Some("sight")));
    assert_eq!((state(&hmi, WPN), hmi.session()), (AreaState::Selected, &Session::Nobody));

    press(&mut hmi, Button::Area(COM), 400);
    let check = enter(&mut hmi, "111111", 500);
    assert!(hmi.verified(check.serial, refused(), at(600)));
    assert_eq!(page(&hmi, 600), "pin COM");
    assert_eq!(pin(&hmi, 600).standing, hmi_access::Standing::Refused);
    assert_eq!((state(&hmi, COM), hmi.session()), (AreaState::Available, &Session::Nobody));

    // Another area's button is the crew going elsewhere, a PIN half entered
    // or not: an answer that comes after is nobody's.
    let check = enter(&mut hmi, "246810", 700);
    press(&mut hmi, Button::Area(WPN), 800);
    assert_eq!(page(&hmi, 800), "external");
    assert!(!hmi.verified(check.serial, Answer::Accepted, at(900)));
    assert_eq!(hmi.session(), &Session::Nobody);
}

#[test]
fn a_view_that_needs_a_role_asks_when_it_is_chosen_from_its_areas_menu() {
    let mut hmi = hmi();
    // The area is anybody's, and so is its first view.
    press(&mut hmi, Button::Area(BMS), 0);
    assert_eq!((page(&hmi, 0).as_str(), hmi.window_to_show()), ("external", Some("map")));
    press(&mut hmi, Button::Area(BMS), 100);
    assert_eq!(page(&hmi, 100), "own BMS");

    f(&mut hmi, 2, 200);
    assert_eq!(page(&hmi, 200), "pin BMS: Orders");
    f(&mut hmi, 13, 300);
    assert_eq!(page(&hmi, 300), "own BMS", "cancelled, the crew are at the menu they chose from");

    f(&mut hmi, 2, 400);
    let check = enter(&mut hmi, "246810", 500);
    hmi.verified(check.serial, Answer::Accepted, at(600));
    assert_eq!((page(&hmi, 600).as_str(), hmi.window_to_show()), ("external", Some("orders")));

    // With the commander gone, the view is left for the menu - and is not
    // come back to as it was left, from another area.
    assert!(hmi.tick(at(600 + 300_000)));
    assert_eq!((page(&hmi, 400_000).as_str(), hmi.session()), ("own BMS", &Session::Nobody));
    f(&mut hmi, 1, 400_000);
    assert_eq!(hmi.window_to_show(), Some("map"), "what needs nothing is there as ever");
}

#[test]
fn an_area_whose_first_view_needs_a_role_is_at_its_menu_until_the_crew_choose() {
    let mut hmi = hmi();
    give(&mut hmi, "area DEF\nview Plan\nwindow plan\nrequire commander\nview Status\nwindow status").unwrap();
    hmi.windows(&["sight", "map", "orders", "status"]);
    press(&mut hmi, Button::Area(DEF), 0);
    assert_eq!(page(&hmi, 0), "own DEF");
    // Nor does the first view take the page when its window opens.
    assert!(hmi.windows(&["sight", "map", "orders", "status", "plan"]));
    assert_eq!((page(&hmi, 0).as_str(), hmi.window_to_show()), ("own DEF", None));
    f(&mut hmi, 2, 100);
    assert_eq!(hmi.window_to_show(), Some("status"));
}

#[test]
fn a_function_kept_for_a_role_asks_and_is_done_for_the_right_pin() {
    let mut hmi = hmi();
    hmi.report(Report::parse(r#"{"alarm":{"id":"oil","category":"warning","subsystem":"Engine"}}"#).unwrap(), at(0));
    f(&mut hmi, 14, 100);
    f(&mut hmi, 18, 100);
    assert_eq!(label(&hmi, 11, 100), ("Override\nSelected".into(), LabelState::Available));

    f(&mut hmi, 11, 200);
    assert_eq!(page(&hmi, 200), "pin Override Alarm");
    assert!(hmi.take_requests().is_empty(), "nothing is done for the asking");
    f(&mut hmi, 13, 300);
    assert_eq!(page(&hmi, 300), "alarms");

    f(&mut hmi, 11, 400);
    let check = enter(&mut hmi, "246810", 500);
    hmi.verified(check.serial, Answer::Accepted, at(600));
    assert_eq!(page(&hmi, 600), "alarms");
    assert_eq!(hmi.take_requests(), [Request::Overridden("oil".to_string())]);

    // Acknowledging is anybody's, and no configuration can make it otherwise.
    let mut hmi = self::hmi();
    hmi.report(Report::parse(r#"{"alarm":{"id":"oil","category":"warning","subsystem":"Engine"}}"#).unwrap(), at(0));
    f(&mut hmi, 16, 100);
    assert_eq!(hmi.take_requests(), [Request::Acknowledged("oil".to_string())]);
    assert!(hmi.take_check().is_none() && hmi.session() == &Session::Nobody);
}

#[test]
fn alerts_are_shown_and_acknowledged_and_labels_hidden_over_a_pin_page() {
    let mut hmi = hmi();
    press(&mut hmi, Button::Area(COM), 0);
    digits(&mut hmi, "24", 100);

    hmi.report(Report::parse(r#"{"alarm":{"id":"oil","category":"warning","subsystem":"Engine"}}"#).unwrap(), at(200));
    hmi.report(Report::parse(r#"{"threat":{"id":"lws","type":"Laser","bearing":800}}"#).unwrap(), at(200));
    let frame = hmi.frame(at(300), Vec::new());
    assert!(frame.alarm.is_some_and(|message| !message.acknowledged));
    assert!(frame.threat.is_some_and(|message| !message.acknowledged));
    assert_eq!(frame.status.alarms.warnings, 1);

    f(&mut hmi, 16, 400);
    let frame = hmi.frame(at(400), Vec::new());
    assert!(frame.alarm.is_some_and(|message| message.acknowledged));
    assert!(frame.threat.is_some_and(|message| message.acknowledged));
    assert_eq!(pin(&hmi, 400).entered, 2, "and the PIN is where it was");

    f(&mut hmi, 19, 500);
    assert!(hmi.frame(at(500), Vec::new()).hidden.labels);
    assert_eq!(page(&hmi, 500), "pin COM");
    // The buttons go on doing what they did while their labels are hidden.
    digits(&mut hmi, "68", 600);
    assert_eq!(pin(&hmi, 600).entered, 4);

    // The Alarms page, and the Threats page, come up in the PIN page's place.
    f(&mut hmi, 14, 700);
    assert_eq!(page(&hmi, 700), "alarms");
    assert!(hmi.take_check().is_none());
    press(&mut hmi, Button::Area(COM), 800);
    f(&mut hmi, 15, 900);
    assert_eq!(page(&hmi, 900), "threats");
}

#[test]
fn a_pin_is_typed_by_touch_and_with_the_touch_screen_off_from_the_bezel_alone() {
    let mut hmi = hmi();
    press(&mut hmi, Button::Area(COM), 0);
    let key = |hmi: &Hmi, text: &str| pin(hmi, 0).pad.keys.iter().position(|key| key.text == text).unwrap();
    let five = key(&hmi, "5");
    assert!(hmi.touch(Touched::Key(five), at(100)));
    assert!(pin(&hmi, 100).pad.keys[five].pressed, "lit while the finger is on it");
    assert!(hmi.lift());
    assert_eq!((pin(&hmi, 100).entered, pin(&hmi, 100).pad.keys[five].pressed), (1, false));
    // A label is a soft key here as anywhere.
    assert!(hmi.touch(Touched::Label(Button::F(7)), at(200)));
    assert_eq!(pin(&hmi, 200).entered, 2);
    f(&mut hmi, 13, 300);

    press(&mut hmi, Button::Area(SYS), 400);
    f(&mut hmi, 12, 400);
    assert!(!hmi.frame(at(400), Vec::new()).touch);
    press(&mut hmi, Button::Area(COM), 500);
    assert!(!hmi.touch(Touched::Key(five), at(600)));
    assert_eq!(pin(&hmi, 600).entered, 0);
    let check = enter(&mut hmi, "246810", 700);
    hmi.verified(check.serial, Answer::Accepted, at(800));
    assert_eq!(page(&hmi, 800), "form");
}

#[test]
fn while_a_pin_is_asked_for_nothing_is_typed_into_a_program() {
    let mut hmi = hmi();
    press(&mut hmi, Button::Area(WPN), 0);
    f(&mut hmi, 5, 100);
    assert!(hmi.typing_into_window());
    assert_eq!(page(&hmi, 100), "external keyboard");
    hmi.take_strokes();

    // The PIN page takes the program's page, and its keyboard with it.
    press(&mut hmi, Button::Area(COM), 200);
    assert_eq!(page(&hmi, 200), "pin COM");
    assert!(!hmi.typing_into_window());
    digits(&mut hmi, "2468", 300);
    let six = pin(&hmi, 300).pad.keys.iter().position(|key| key.text == "6").unwrap();
    hmi.touch(Touched::Key(six), at(300));
    hmi.lift();
    f(&mut hmi, 11, 300);
    f(&mut hmi, 20, 300);
    assert!(hmi.take_strokes().is_empty(), "not a digit, not Backspace, not Enter");
    assert_eq!(hmi.take_check().unwrap().pin.reveal(), "2468");

    // Cancelled, the crew are at the program's page with its keyboard up,
    // as they left it - and still nothing has been typed there.
    f(&mut hmi, 13, 400);
    assert_eq!(page(&hmi, 400), "external keyboard");
    assert!(hmi.typing_into_window() && hmi.take_strokes().is_empty());
}

#[test]
fn ism_2012_logging_out_and_the_idle_time_leave_what_needed_the_role() {
    let mut hmi = hmi();
    log_in(&mut hmi, 1000);
    assert_eq!(page(&hmi, 1000), "form");
    press(&mut hmi, Button::Area(SYS), 2000);
    f(&mut hmi, 11, 2000);
    assert_eq!(hmi.session(), &Session::Nobody);
    assert_eq!(label(&hmi, 11, 2000), ("Log Out".into(), LabelState::Unavailable));
    assert!(sys_rows(&hmi).contains(&("Logged in".to_string(), "nobody".to_string())));
    press(&mut hmi, Button::Area(COM), 3000);
    assert_eq!(page(&hmi, 3000), "pin COM");
    f(&mut hmi, 13, 3000);

    // Left alone at the form for the idle time, the display goes back to
    // SYS and to nobody; a button in the meantime puts that off.
    log_in(&mut hmi, 10_000);
    assert_eq!(hmi.deadline(), Some(310_000));
    f(&mut hmi, 19, 100_000);
    assert_eq!(hmi.deadline(), Some(400_000));
    assert!(!hmi.tick(at(399_999)));
    assert!(hmi.tick(at(400_000)));
    assert_eq!((page(&hmi, 400_000).as_str(), hmi.session()), ("own System", &Session::Nobody));
    assert_eq!(state(&hmi, SYS), AreaState::Selected);
}

#[test]
fn what_needs_a_role_nobody_here_holds_is_unavailable() {
    let mut hmi = hmi();
    give(&mut hmi, "area DRV\nrequire maintainer\nwindow workshop").unwrap();
    assert!(hmi.windows(&["sight", "map", "orders", "workshop"]));
    assert_eq!(state(&hmi, DRV), AreaState::Unavailable);
    press(&mut hmi, Button::Area(DRV), 0);
    assert_eq!(page(&hmi, 0), "own System");
    assert!(hmi.take_audit().is_empty());
}

#[test]
fn the_frame_has_the_pin_page_as_a_page_of_its_own_kind_with_a_count_and_no_digit() {
    let mut hmi = hmi();
    press(&mut hmi, Button::Area(COM), 0);
    digits(&mut hmi, "975", 100);
    let frame = serde_json::to_value(hmi.frame(at(100), Vec::new())).unwrap();
    assert_eq!(frame["page"]["kind"], "pin");
    assert_eq!((&frame["page"]["wanted"], &frame["page"]["entered"]), (&serde_json::json!("COM"), &serde_json::json!(3)));
    assert_eq!(frame["page"]["pad"]["layout"], "phone");
    assert!(!frame.to_string().contains("975"));
    assert!(!format!("{hmi:?}").contains("975"));
}

#[test]
fn what_happens_about_access_is_for_the_record_and_no_pin_with_it() {
    let mut hmi = hmi();
    press(&mut hmi, Button::Area(COM), 0);
    let check = enter(&mut hmi, "111111", 0);
    hmi.verified(check.serial, refused(), at(0));
    let check = enter(&mut hmi, "246810", 0);
    hmi.verified(check.serial, Answer::Accepted, at(0));
    press(&mut hmi, Button::Area(SYS), 0);
    f(&mut hmi, 11, 0);
    let lines: Vec<String> = hmi.take_audit().iter().map(Audit::to_string).collect();
    assert_eq!(
        lines,
        [
            "asked roles=commander for=\"COM\"",
            "refused account=cmdr for=\"COM\" said=\"not the account's PIN\"",
            "accepted account=cmdr roles=commander for=\"COM\"",
            "logged-out account=cmdr roles=commander",
        ]
    );
}

#[test]
fn a_require_is_the_areas_before_any_view_and_a_views_after_it_and_says_why_if_it_cannot_be_right() {
    use hmi_access::Require;
    let roles = |require: &Require| require.roles().join(" ");
    let External::Views(area) =
        External::parse("area BMS\nrequire crew\nview Map\nwindow map\nview Orders\nrequire commander\nwindow orders\nrequire gunner")
            .unwrap()
    else {
        panic!("an area of views");
    };
    assert_eq!(roles(&area.require), "crew");
    assert_eq!((roles(&area.views[0].require), roles(&area.views[1].require)), (String::new(), "commander gunner".to_string()));

    // Any kind of area can need a role, on as many lines as it takes to say.
    let External::Window(area) = External::parse("require commander\narea WPN\nwindow sight\nrequire gunner crew").unwrap() else {
        panic!("an area of one window");
    };
    assert_eq!(roles(&area.require), "commander gunner crew");
    let External::Tiles(area) = External::parse("area SA\ntiles 2\nwindow Front front\nrequire crew").unwrap() else {
        panic!("an area of tiles");
    };
    assert_eq!(roles(&area.require), "crew");
    assert!(ExternalArea::parse("area WPN\nwindow sight").unwrap().require.is_nothing(), "none, unless it says");

    let why = |text| External::parse(text).unwrap_err();
    assert!(why("area WPN\nwindow sight\nrequire").contains("line 3: 'require' has no value"));
    assert!(why("area WPN\nwindow sight\nrequire The Commander").contains("line 3: 'The' cannot be a role"));
}

#[test]
fn ism_1597_a_digit_of_a_pin_is_not_to_be_named_in_anything_written_down_nor_remembered() {
    let mut hmi = hmi();
    let digit = Button::F(6);
    assert!(!hmi.unspoken(digit) && !hmi.untold(&Touched::Key(5)), "with no PIN asked for a button is a button");
    press(&mut hmi, Button::Area(COM), 0);
    for n in 1..=11 {
        assert!(hmi.unspoken(Button::F(n)) && hmi.untold(&Touched::Label(Button::F(n))), "F{n}");
    }
    assert!(hmi.untold(&Touched::Key(5)));
    // What is not a digit can be named: an area's button, Cancel, Enter,
    // and the four that are never the page's.
    for button in [Button::Area(WPN), Button::F(12), Button::F(13), Button::F(14), Button::F(16), Button::F(19), Button::F(20)] {
        assert!(!hmi.unspoken(button), "{button}");
    }
    assert!(!hmi.untold(&Touched::Field(1)));

    // The SYS page says which button was last pressed: not a digit's.
    digits(&mut hmi, "2468", 100);
    f(&mut hmi, 13, 200);
    assert!(sys_rows(&hmi).contains(&("Last button".to_string(), "F13 released".to_string())));
    press(&mut hmi, Button::Area(COM), 300);
    digits(&mut hmi, "24", 400);
    assert!(hmi.tick(at(400 + hmi_access::ASKING_MS)), "the page goes away, nobody having answered it");
    assert!(sys_rows(&hmi).contains(&("Last button".to_string(), "COM released".to_string())));
}

#[test]
fn whether_a_pin_is_asked_for_can_be_asked() {
    let mut hmi = hmi();
    assert!(!hmi.pin_asked());
    press(&mut hmi, Button::Area(COM), 0);
    assert!(hmi.pin_asked() && matches!(hmi.frame(at(0), Vec::new()).page, Page::Pin(_)));
    f(&mut hmi, 13, 0);
    assert!(!hmi.pin_asked());
}
