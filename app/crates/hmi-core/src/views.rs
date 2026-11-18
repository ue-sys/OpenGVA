//! Views: several programs' windows in one area, one at a time.
//!
//! An area can be given to more than one program ([`ViewedArea`]). Each
//! program's window is then a page of the area - a view - and has the page to
//! itself while it is the one viewed. Which that is, the crew chooses from
//! the area's top level menu: a page of the HMI's own, with a view on each of
//! the buttons down the sides. This module keeps which view was last viewed
//! and whether the menu is up, and says what the menu has on it. What the
//! windows show is no more its business than it is anywhere else in this
//! crate.

use crate::external::{AreaView, ViewedArea};
use hmi_access::Require;
use crate::{LabelState, Row};

/// How many characters go on a line of a soft key: as many as the longest
/// line of the HMI's own labels has.
const LINE: usize = 10;

/// A viewed area's windows, and which of them the crew is viewing.
#[derive(Debug)]
pub(crate) struct Views {
    area: ViewedArea,
    /// Whether each view's window is open, in the area's order.
    open: Vec<bool>,
    /// The view last viewed: the first, until the crew chooses another.
    viewed: usize,
    /// Whether the area is at its menu: the crew asked for it, or the view
    /// they had has lost its window.
    menu: bool,
}

impl Views {
    pub fn new(area: ViewedArea) -> Self {
        let open = vec![false; area.views.len()];
        Views { area, open, viewed: 0, menu: false }
    }

    /// The functional area: an index into [`AREAS`](crate::AREAS).
    pub fn area(&self) -> u8 {
        self.area.area
    }

    /// Whether any of the windows is open: there is something to view.
    pub fn present(&self) -> bool {
        self.open.contains(&true)
    }

    /// Take the windows now open, by the names their programs give them.
    /// Returns whether anything changed.
    pub fn windows<S: AsRef<str>>(&mut self, open: &[S]) -> bool {
        let now: Vec<bool> =
            self.area.views.iter().map(|view| open.iter().any(|name| name.as_ref() == view.window)).collect();
        let changed = now != self.open;
        self.open = now;
        changed
    }

    /// Put the area at its menu, to stay there until the crew chooses a
    /// view. The area's own button does, pressed while the area is selected:
    /// that is its top level menu. And so does a view going
    /// from under the crew's eyes: it is not to have the page again by
    /// itself when its window is back.
    pub fn to_menu(&mut self) {
        self.menu = true;
    }

    /// F`n` is pressed. On the menu each of the buttons down the sides has a
    /// view, F1 the first, and shows it if its window is open. On a view F13
    /// goes up a level, which is to the menu, and the buttons
    /// down the sides are nobody's.
    pub fn press(&mut self, n: u8) {
        if self.showing().is_some() {
            if n == 13 {
                self.menu = true;
            }
            return;
        }
        if let Some(view) = self.chooses(n) {
            self.choose(view);
        }
    }

    /// The view F`n` would show, by its place among the area's: the one its
    /// button has, while the menu is the page and the view's window is open.
    /// Whether the crew may have it is for whoever asks this to see to,
    /// before [`Views::choose`].
    pub fn chooses(&self, n: u8) -> Option<usize> {
        if self.showing().is_some() {
            return None;
        }
        usize::from(n).checked_sub(1).filter(|&view| self.open.get(view) == Some(&true))
    }

    /// Show a view: it is the one viewed, and the menu is left for it. Not
    /// one whose window is not open.
    pub fn choose(&mut self, view: usize) {
        if self.open.get(view) == Some(&true) {
            (self.viewed, self.menu) = (view, false);
        }
    }

    /// The view that has the page, by its place among the area's.
    pub fn viewed(&self) -> Option<usize> {
        self.showing().map(|_| self.viewed)
    }

    /// What the crew know a view as.
    pub fn label(&self, view: usize) -> &str {
        &self.area.views[view].label
    }

    /// The roles any one of which will do to have the area at all...
    pub fn area_requires(&self) -> &Require {
        &self.area.require
    }

    /// ...and to be shown one of its views, over and above that.
    pub fn requires(&self, view: usize) -> &Require {
        &self.area.views[view].require
    }

    /// The view that has the page: the one last viewed, unless the area is
    /// at its menu or that view's window is not open - when the page is the
    /// menu. So the first view has the page as soon as its window opens, if
    /// the crew has chosen nothing; and an area left at a view is at that
    /// view when it is come back to, if the view can be shown then.
    pub fn showing(&self) -> Option<&AreaView> {
        (!self.menu && self.open[self.viewed]).then(|| &self.area.views[self.viewed])
    }

    /// How each view stands, with the number of its button on the menu:
    /// selected if it is the one last viewed, and unavailable while its
    /// window is not open.
    fn states(&self) -> impl Iterator<Item = (u8, &AreaView, LabelState)> {
        (1u8..).zip(&self.area.views).zip(&self.open).map(|((n, view), &open)| {
            let state = if !open {
                LabelState::Unavailable
            } else if usize::from(n) - 1 == self.viewed {
                LabelState::Selected
            } else {
                LabelState::Available
            };
            (n, view, state)
        })
    }

    /// The labels of the menu's buttons, each with the number of its button:
    /// a view's name for the crew. None while a view has the
    /// page.
    pub fn labels(&self) -> Vec<(u8, String, LabelState)> {
        if self.showing().is_some() {
            return Vec::new();
        }
        self.states().map(|(n, view, state)| (n, on_a_key(&view.label), state)).collect()
    }

    /// What the menu's page says: each view, the button that shows it, and
    /// how it stands - which the page goes on saying when F19 has taken the
    /// labels away.
    pub fn rows(&self) -> Vec<Row> {
        self.states()
            .map(|(n, view, state)| {
                let how = match state {
                    LabelState::Selected => " - last viewed",
                    LabelState::Unavailable => " - not available",
                    _ => "",
                };
                Row::new(&view.label, format!("F{n}{how}"))
            })
            .collect()
    }
}

/// A label as it goes on a soft key: broken at its spaces into lines no
/// longer than a key has room for, where it can be.
fn on_a_key(label: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for word in label.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.chars().count() + 1 + word.chars().count() <= LINE => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use LabelState::{Available, Selected, Unavailable};

    /// An area of three views, "Map", "Server Pages" and "Log", whose
    /// windows are "map", "pages" and "log": none open yet.
    fn three() -> Views {
        let views = [("Map", "map"), ("Server Pages", "pages"), ("Log", "log")]
            .map(|(label, window)| AreaView { label: label.to_string(), window: window.to_string(), overlay: None, keyboard: false, require: Require::nothing() })
            .to_vec();
        Views::new(ViewedArea { area: 7, views, require: Require::nothing() })
    }

    fn showing(views: &Views) -> Option<&str> {
        views.showing().map(|view| view.window.as_str())
    }

    fn states(views: &Views) -> Vec<LabelState> {
        views.labels().into_iter().map(|(_, _, state)| state).collect()
    }

    #[test]
    fn the_first_view_is_the_areas_until_the_crew_chooses_another() {
        let mut views = three();
        assert!(!views.present());
        assert!(views.windows(&["map", "pages", "something else"]));
        assert!(!views.windows(&["pages", "map"]), "the same windows again change nothing");
        assert!(views.present());
        assert_eq!(showing(&views), Some("map"));
        assert!(views.labels().is_empty(), "a view has the page: the buttons down the sides are nobody's");
    }

    #[test]
    fn the_menu_has_a_view_on_each_button_and_marks_the_one_last_viewed() {
        let mut views = three();
        views.windows(&["map", "pages"]);
        views.to_menu();
        assert_eq!(showing(&views), None);
        assert_eq!(
            views.labels(),
            [
                (1, "Map".to_string(), Selected),
                (2, "Server\nPages".to_string(), Available),
                (3, "Log".to_string(), Unavailable),
            ]
        );

        // A button shows its view, and that is then the one last viewed.
        views.press(2);
        assert_eq!(showing(&views), Some("pages"));
        views.to_menu();
        assert_eq!(states(&views), [Available, Selected, Unavailable]);
        // One whose window is not open cannot be shown, and a button that has no view does nothing.
        for n in [3, 4, 12, 0, 13, 20] {
            views.press(n);
            assert_eq!(showing(&views), None, "F{n}");
        }
        assert_eq!(states(&views), [Available, Selected, Unavailable]);
    }

    #[test]
    fn f13_goes_up_from_a_view_to_the_menu_and_no_other_button_does_anything_there() {
        let mut views = three();
        views.windows(&["map", "pages"]);
        for n in (1..=12).chain(14..=20) {
            views.press(n);
            assert_eq!(showing(&views), Some("map"), "F{n}");
        }
        views.press(13);
        assert_eq!(showing(&views), None);
        // The menu is the top: there is no level above it to go up to.
        views.press(13);
        assert_eq!(showing(&views), None);
        views.press(1);
        assert_eq!(showing(&views), Some("map"));
    }

    #[test]
    fn a_view_whose_window_is_not_open_cannot_have_the_page_and_the_menu_has_it() {
        let mut views = three();
        views.windows(&["map", "pages"]);
        views.press(13);
        views.press(2);
        assert_eq!(showing(&views), Some("pages"));

        assert!(views.windows(&["map"]));
        assert_eq!(showing(&views), None);
        assert_eq!(states(&views), [Available, Unavailable, Unavailable]);
        // Its window back, it is the view last viewed still.
        assert!(views.windows(&["map", "pages"]));
        assert_eq!(showing(&views), Some("pages"));

        // Unless the area was put at its menu meanwhile: then it stays there
        // until the crew chooses.
        views.windows(&["map"]);
        views.to_menu();
        views.windows(&["map", "pages"]);
        assert_eq!(showing(&views), None);
        assert_eq!(states(&views), [Available, Selected, Unavailable]);
        views.press(2);
        assert_eq!(showing(&views), Some("pages"));

        // A view that is not the one viewed can come and go as it likes.
        assert!(views.windows(&["pages", "log"]));
        assert_eq!(showing(&views), Some("pages"));
    }

    #[test]
    fn the_first_view_has_the_page_as_soon_as_its_window_opens_if_the_crew_has_chosen_nothing() {
        let mut views = three();
        // Another view's window is open first: the area has something to
        // view, and its page is the menu, with the first view grey on it.
        views.windows(&["pages"]);
        assert!(views.present());
        assert_eq!(showing(&views), None);
        assert_eq!(states(&views), [Unavailable, Available, Unavailable]);
        views.windows(&["pages", "map"]);
        assert_eq!(showing(&views), Some("map"));

        // But a choice is kept to: the first does not take the page from it.
        let mut views = three();
        views.windows(&["pages"]);
        views.press(2);
        views.windows(&["pages", "map"]);
        assert_eq!(showing(&views), Some("pages"));
    }

    #[test]
    fn the_menus_page_says_each_view_its_button_and_how_it_stands() {
        let mut views = three();
        views.windows(&["map", "pages"]);
        let said: Vec<(String, String)> = views.rows().into_iter().map(|row| (row.name, row.value)).collect();
        assert_eq!(
            said,
            [
                ("Map".to_string(), "F1 - last viewed".to_string()),
                ("Server Pages".to_string(), "F2".to_string()),
                ("Log".to_string(), "F3 - not available".to_string()),
            ]
        );
    }

    #[test]
    fn a_label_is_broken_at_its_spaces_into_lines_a_key_has_room_for() {
        assert_eq!(on_a_key("Map"), "Map");
        assert_eq!(on_a_key("TAK Server"), "TAK Server");
        assert_eq!(on_a_key("Server Pages"), "Server\nPages");
        assert_eq!(on_a_key("One Two Three Four"), "One Two\nThree Four");
        assert_eq!(on_a_key("  spaced   out  "), "spaced out");
        // A word longer than a line is a line: there is nowhere to break it.
        assert_eq!(on_a_key("Reconnaissance view"), "Reconnaissance\nview");
    }

    // What follows is the HMI with such an area in it: the area's button, the
    // menu it brings up, and what else is on the screen with them.

    use crate::{
        Action, AreaState, Button, ButtonEvent, ExternalArea, FormArea, Hmi, Page, Report, TiledArea, Time, Touched,
    };

    const SA: u8 = 0;
    const WPN: u8 = 1;
    const SYS: u8 = 3;
    const STR: u8 = 5;
    const BMS: u8 = 7;

    fn at(ms: u64) -> Time {
        Time { unix_s: 1_728_045_000 + ms / 1000, uptime_ms: ms }
    }

    fn press(hmi: &mut Hmi, button: Button) {
        hmi.button(ButtonEvent { button, action: Action::Pressed }, at(0));
    }

    /// An HMI whose BMS is two views - "Map", in the window "map", and
    /// "Server Pages", in "pages" - and whose WPN is a sight's window: all
    /// three windows open.
    fn hmi() -> Hmi {
        let mut hmi = Hmi::new();
        let view = |label: &str, window: &str, overlay: Option<&str>| AreaView {
            label: label.to_string(),
            window: window.to_string(),
            overlay: overlay.map(str::to_string),
            keyboard: false,
            require: Require::nothing(),
        };
        let views = vec![view("Map", "map", None), view("Server Pages", "pages", Some("data:,grid"))];
        hmi.add_viewed(ViewedArea { area: BMS, views, require: Require::nothing() }).unwrap();
        hmi.add_external(ExternalArea { area: WPN, window: "sight".to_string(), overlay: None, keyboard: false, require: Require::nothing(), socket: None, keys: Vec::new() }).unwrap();
        hmi.windows(&["map", "pages", "sight"]);
        hmi
    }

    /// The page on the screen, in a word or two.
    fn page(hmi: &Hmi) -> String {
        match hmi.frame(at(0), Vec::new()).page {
            Page::Own { title, .. } => format!("own {title}"),
            Page::External { .. } => format!("external {}", hmi.window_to_show().unwrap_or("?")),
            Page::Tiles { .. } => "tiles".to_string(),
            Page::Form(_) => "form".to_string(),
            Page::Pin(pin) => format!("pin {}", pin.wanted),
            Page::Alarms { .. } => "alarms".to_string(),
            Page::Threats { .. } => "threats".to_string(),
        }
    }

    fn area_state(hmi: &Hmi, area: u8) -> AreaState {
        hmi.frame(at(0), Vec::new()).areas[usize::from(area)].state
    }

    /// The labels down the sides that are not blank, each with its button's number.
    fn side_labels(hmi: &Hmi) -> Vec<(usize, String, LabelState)> {
        let labels = hmi.frame(at(0), Vec::new()).labels;
        (1..=12)
            .zip(labels)
            .filter(|(_, label)| label.state != LabelState::Blank)
            .map(|(n, label)| (n, label.text, label.state))
            .collect()
    }

    #[test]
    fn another_areas_button_shows_the_page_last_viewed_for_it() {
        let mut hmi = hmi();
        // Never viewed, the area is at its first view.
        press(&mut hmi, Button::Area(BMS));
        assert_eq!(page(&hmi), "external map");
        assert_eq!(area_state(&hmi, BMS), AreaState::Selected);
        assert!(side_labels(&hmi).is_empty(), "a view is its program's: the buttons down the sides are nobody's");

        // The other view chosen, the area comes back to that one...
        press(&mut hmi, Button::Area(BMS));
        press(&mut hmi, Button::F(2));
        assert_eq!(page(&hmi), "external pages");
        press(&mut hmi, Button::Area(SYS));
        assert_eq!(page(&hmi), "own System");
        assert_eq!(hmi.window_to_show(), None);
        press(&mut hmi, Button::Area(BMS));
        assert_eq!(page(&hmi), "external pages");

        // ...and left at its menu, to its menu: that was the page last viewed.
        press(&mut hmi, Button::Area(BMS));
        press(&mut hmi, Button::Area(WPN));
        assert_eq!(page(&hmi), "external sight");
        press(&mut hmi, Button::Area(BMS));
        assert_eq!(page(&hmi), "own BMS");
    }

    #[test]
    fn the_selected_areas_button_shows_its_top_level_menu() {
        let mut hmi = hmi();
        press(&mut hmi, Button::Area(BMS));
        press(&mut hmi, Button::Area(BMS));

        // A page of the HMI's own, that says whose it is and what it has,
        // with no program's window under it...
        let Page::Own { title, rows } = hmi.frame(at(0), Vec::new()).page else { panic!("not the HMI's own page") };
        assert_eq!(title, "BMS");
        let said: Vec<_> = rows.iter().map(|row| (row.name.as_str(), row.value.as_str())).collect();
        assert_eq!(said, [("Map", "F1 - last viewed"), ("Server Pages", "F2")]);
        assert_eq!(hmi.window_to_show(), None);
        assert!(hmi.windows_to_show().is_empty());
        assert_eq!(area_state(&hmi, BMS), AreaState::Selected, "the area is still the one selected");

        // ...and a view on each of the buttons down the sides, from F1: the
        // one last viewed marked.
        assert_eq!(
            side_labels(&hmi),
            [(1, "Map".to_string(), Selected), (2, "Server\nPages".to_string(), Available)]
        );
        let frame = hmi.frame(at(0), Vec::new());
        assert_eq!((frame.labels[12].text.as_str(), frame.labels[13].text.as_str()), ("Up", "Alarms"));

        // Pressed again it is the menu still; a view's button shows the view.
        press(&mut hmi, Button::Area(BMS));
        assert_eq!(page(&hmi), "own BMS");
        press(&mut hmi, Button::F(2));
        assert_eq!(page(&hmi), "external pages");
        let Page::External { overlay, .. } = hmi.frame(at(0), Vec::new()).page else { panic!("not a program's page") };
        assert_eq!(overlay.as_deref(), Some("data:,grid"), "a view has its own image over its picture");
        // And the menu then marks that one.
        press(&mut hmi, Button::Area(BMS));
        assert_eq!(side_labels(&hmi).iter().map(|label| label.2).collect::<Vec<_>>(), [Available, Selected]);
    }

    #[test]
    fn another_areas_button_pressed_twice_shows_its_top_level_menu() {
        let mut hmi = hmi();
        assert_eq!(page(&hmi), "own System");
        press(&mut hmi, Button::Area(BMS));
        press(&mut hmi, Button::Area(BMS));
        assert_eq!(page(&hmi), "own BMS");
        // From another program's area as from the HMI's own.
        press(&mut hmi, Button::F(1));
        press(&mut hmi, Button::Area(WPN));
        press(&mut hmi, Button::Area(BMS));
        press(&mut hmi, Button::Area(BMS));
        assert_eq!(page(&hmi), "own BMS");
    }

    #[test]
    fn f13_goes_up_a_level_from_a_view_to_the_areas_menu() {
        let mut hmi = hmi();
        press(&mut hmi, Button::Area(BMS));
        // A view's buttons down the sides do nothing: they are not the program's either.
        for n in 1..=12 {
            press(&mut hmi, Button::F(n));
        }
        assert_eq!(page(&hmi), "external map");
        press(&mut hmi, Button::F(13));
        assert_eq!(page(&hmi), "own BMS");
        // The menu is the top level: there is nothing above it.
        press(&mut hmi, Button::F(13));
        assert_eq!(page(&hmi), "own BMS");
        // Letting go of a button does nothing, here as anywhere.
        hmi.button(ButtonEvent { button: Button::F(2), action: Action::Released }, at(0));
        assert_eq!(page(&hmi), "own BMS");
    }

    #[test]
    fn an_area_with_one_page_has_that_page_for_its_top_level_menu() {
        let mut hmi = hmi();
        hmi.add_tiled(TiledArea::parse("area SA\ntiles 2\nwindow Front front\nwindow Rear rear").unwrap()).unwrap();
        hmi.add_form(FormArea::parse("area STR\nform load Load Record\nfield what text 12 What").unwrap()).unwrap();
        hmi.windows(&["map", "pages", "sight", "front", "rear"]);

        // SYS, a program's one window, tiles and a form: the selected area's
        // own button leaves each as it is...
        press(&mut hmi, Button::Area(SYS));
        assert_eq!(page(&hmi), "own System");
        for (area, its_page) in [(WPN, "external sight"), (SA, "tiles"), (STR, "form")] {
            press(&mut hmi, Button::Area(area));
            if area == SA {
                press(&mut hmi, Button::F(2));
            }
            let before = format!("{:?}", hmi.frame(at(0), Vec::new()).page);
            press(&mut hmi, Button::Area(area));
            assert_eq!(page(&hmi), its_page);
            assert_eq!(format!("{:?}", hmi.frame(at(0), Vec::new()).page), before, "as it was");
        }

        // ...and brings it back from the Alarms page or the Threats page,
        // as it always did.
        for area in [SYS, WPN, SA, STR] {
            press(&mut hmi, Button::Area(area));
            let its_page = page(&hmi);
            for over in [14, 15] {
                press(&mut hmi, Button::F(over));
                assert!(matches!(page(&hmi).as_str(), "alarms" | "threats"));
                press(&mut hmi, Button::Area(area));
                assert_eq!(page(&hmi), its_page);
            }
        }
    }

    #[test]
    fn the_alarms_and_threats_pages_come_up_over_a_view_and_over_the_menu() {
        let mut hmi = hmi();
        press(&mut hmi, Button::Area(BMS));
        press(&mut hmi, Button::Area(BMS));
        press(&mut hmi, Button::F(2));

        for (over, its_page) in [(14, "alarms"), (15, "threats")] {
            // Over a view: no window under it, and F13 goes back to the view.
            press(&mut hmi, Button::F(over));
            assert_eq!(page(&hmi), its_page);
            assert_eq!(hmi.window_to_show(), None);
            assert_eq!(area_state(&hmi, BMS), AreaState::Selected);
            // A button that is a view's on the menu is that page's here.
            press(&mut hmi, Button::F(1));
            press(&mut hmi, Button::F(13));
            assert_eq!(page(&hmi), "external pages");

            // Over the menu, and back to the menu.
            press(&mut hmi, Button::Area(BMS));
            press(&mut hmi, Button::F(over));
            assert_eq!(page(&hmi), its_page);
            press(&mut hmi, Button::F(13));
            assert_eq!(page(&hmi), "own BMS");
            press(&mut hmi, Button::F(2));

            // And from over a view the area's own button is its menu.
            press(&mut hmi, Button::F(over));
            press(&mut hmi, Button::Area(BMS));
            assert_eq!(page(&hmi), "own BMS");
            press(&mut hmi, Button::F(2));
        }
    }

    #[test]
    fn to_220_an_area_of_views_has_functions_while_any_of_its_windows_is_open() {
        use AreaState::{Available as There, Selected as On, Unavailable as Grey};
        let mut hmi = hmi();
        hmi.windows(&[] as &[&str]);
        assert_eq!(area_state(&hmi, BMS), Grey);
        press(&mut hmi, Button::Area(BMS));
        assert_eq!(page(&hmi), "own System", "with no window to show it stays unselected");

        // The second view's window is open first: the area can be selected,
        // and its page is its menu, with the first view grey on it...
        assert!(hmi.windows(&["pages"]));
        assert_eq!(area_state(&hmi, BMS), There);
        press(&mut hmi, Button::Area(BMS));
        assert_eq!(area_state(&hmi, BMS), On);
        assert_eq!(page(&hmi), "own BMS");
        assert_eq!(
            side_labels(&hmi),
            [(1, "Map".to_string(), Unavailable), (2, "Server\nPages".to_string(), Available)]
        );
        press(&mut hmi, Button::F(1));
        assert_eq!(page(&hmi), "own BMS", "a view whose window is not open cannot be chosen");
        // ...until the first view's window opens, when it has the page: the
        // crew has chosen nothing else.
        assert!(hmi.windows(&["pages", "map"]));
        assert_eq!(page(&hmi), "external map");
    }

    #[test]
    fn the_menu_takes_the_place_of_a_view_whose_window_goes_and_sys_of_an_area_with_none_left() {
        let mut hmi = hmi();
        press(&mut hmi, Button::Area(BMS));
        press(&mut hmi, Button::F(13));
        press(&mut hmi, Button::F(2));
        assert_eq!(page(&hmi), "external pages");

        assert!(hmi.windows(&["map", "sight"]));
        assert_eq!(page(&hmi), "own BMS");
        assert_eq!(area_state(&hmi, BMS), AreaState::Selected);
        assert_eq!(side_labels(&hmi).iter().map(|label| label.2).collect::<Vec<_>>(), [Available, Unavailable]);
        // It does not come back by itself: the crew chose nothing.
        assert!(hmi.windows(&["map", "pages", "sight"]));
        assert_eq!(page(&hmi), "own BMS");
        // Nor from behind the Alarms page, where it went unseen.
        press(&mut hmi, Button::F(2));
        press(&mut hmi, Button::F(14));
        hmi.windows(&["map", "sight"]);
        hmi.windows(&["map", "pages", "sight"]);
        press(&mut hmi, Button::F(13));
        assert_eq!(page(&hmi), "own BMS");

        assert!(hmi.windows(&["sight"]));
        assert_eq!(page(&hmi), "own System");
        assert_eq!(area_state(&hmi, BMS), AreaState::Unavailable);
        hmi.windows(&["map", "pages", "sight"]);
        assert_eq!(page(&hmi), "own System", "nor is the area on screen again by itself");
        assert_eq!(area_state(&hmi, BMS), AreaState::Available);
    }

    #[test]
    fn a_view_that_came_and_went_while_its_area_was_not_selected_is_the_page_last_viewed_still() {
        let mut hmi = hmi();
        // Before the area has ever been looked at...
        hmi.windows(&["pages", "sight"]);
        hmi.windows(&["map", "pages", "sight"]);
        press(&mut hmi, Button::Area(BMS));
        assert_eq!(page(&hmi), "external map");

        // ...and after the crew has chosen, and gone elsewhere.
        press(&mut hmi, Button::Area(BMS));
        press(&mut hmi, Button::F(2));
        press(&mut hmi, Button::Area(SYS));
        hmi.windows(&["map", "sight"]);
        hmi.windows(&["map", "pages", "sight"]);
        press(&mut hmi, Button::Area(BMS));
        assert_eq!(page(&hmi), "external pages");

        // Come back to while its window is away, the area is at its menu,
        // and at the view again when the view can be shown.
        press(&mut hmi, Button::Area(SYS));
        hmi.windows(&["map", "sight"]);
        press(&mut hmi, Button::Area(BMS));
        assert_eq!(page(&hmi), "own BMS");
        assert_eq!(side_labels(&hmi).iter().map(|label| label.2).collect::<Vec<_>>(), [Available, Unavailable]);
        hmi.windows(&["map", "pages", "sight"]);
        assert_eq!(page(&hmi), "external pages");
    }

    #[test]
    fn the_ppi_is_on_the_menu_and_over_a_view_only_for_a_threat() {
        let ppi = |hmi: &Hmi| hmi.frame(at(0), Vec::new()).ppi.map(|ppi| ppi.threats.len());
        let mut hmi = hmi();
        press(&mut hmi, Button::Area(BMS));
        assert_eq!(ppi(&hmi), None, "a view is its program's picture");
        press(&mut hmi, Button::Area(BMS));
        assert_eq!(ppi(&hmi), Some(0), "the menu is the HMI's own page");

        let threat = r#"{"threat":{"id":"shot","type":"Gunshot","bearing":5300}}"#;
        hmi.report(Report::parse(threat).unwrap(), at(0));
        assert_eq!(ppi(&hmi), Some(1));
        press(&mut hmi, Button::F(1));
        assert_eq!(ppi(&hmi), Some(1), "and comes up over a view while there is a threat to mark");
    }

    #[test]
    fn a_touched_label_of_the_menu_chooses_its_view() {
        let mut hmi = hmi();
        press(&mut hmi, Button::Area(BMS));
        press(&mut hmi, Button::Area(BMS));
        assert!(hmi.touch(Touched::Label(Button::F(2)), at(0)));
        assert_eq!(page(&hmi), "external pages");
    }

    #[test]
    fn an_area_is_given_views_once_and_sys_never_and_not_views_that_cannot_be() {
        let mut hmi = hmi();
        let view = |label: &str, window: &str| AreaView {
            label: label.to_string(),
            window: window.to_string(),
            overlay: None,
            keyboard: false,
            require: Require::nothing(),
        };
        let viewed = |area, views: Vec<AreaView>| ViewedArea { area, views, require: Require::nothing() };
        assert!(hmi.add_viewed(viewed(BMS, vec![view("A", "a")])).unwrap_err().contains("BMS"));
        assert!(hmi.add_viewed(viewed(WPN, vec![view("A", "a")])).unwrap_err().contains("WPN"));
        assert!(hmi.add_viewed(viewed(SYS, vec![view("A", "a")])).unwrap_err().contains("SYS"));
        assert!(hmi.add_viewed(viewed(SA, vec![])).unwrap_err().contains("no 'view'"));
        assert!(hmi.add_viewed(viewed(SA, vec![view("A", "a"), view("A", "b")])).unwrap_err().contains("'A' is there twice"));
        assert!(hmi.add_viewed(viewed(SA, vec![view("A", "a"), view("B", "a")])).unwrap_err().contains("'a' is there twice"));
        assert!(hmi.add_viewed(viewed(SA, vec![view("", "a")])).unwrap_err().contains("a view is a label"));
        let many = |count| (1..=count).map(|n| view(&format!("V{n}"), &format!("v{n}"))).collect::<Vec<_>>();
        assert!(hmi.add_viewed(viewed(SA, many(13))).unwrap_err().contains("13 views"));
        // One view is an area of views, and twelve are as many as there are buttons for.
        assert!(hmi.add_viewed(viewed(SA, many(12))).is_ok());
        assert!(hmi.add_viewed(viewed(STR, many(1))).is_ok());
        // And an area that has views has no other program.
        let window = ExternalArea { area: SA, window: "w".to_string(), overlay: None, keyboard: false, require: Require::nothing(), socket: None, keys: Vec::new() };
        assert!(hmi.add_external(window).unwrap_err().contains("SA"));
    }

    // And how a configuration says all this: the reading is `external`'s,
    // with that of every other kind of area.

    use crate::External;

    #[test]
    fn a_configuration_of_views_names_each_with_its_window_and_perhaps_an_overlay() {
        let text = "# BMS: a map, and the server's own pages\narea BMS\n\n\
                    view    Map\nwindow  map-viewer\n\n\
                    view    Server Pages     # a name can have spaces\n\
                    overlay /usr/share/pages/frame.svg\n\
                    window\tServer Pages 2   # and so can a window's\n";
        let view = |label: &str, window: &str, overlay: Option<&str>| AreaView {
            label: label.to_string(),
            window: window.to_string(),
            overlay: overlay.map(str::to_string),
            keyboard: false,
            require: Require::nothing(),
        };
        assert_eq!(
            ViewedArea::parse(text),
            Ok(ViewedArea {
                area: BMS,
                views: vec![
                    view("Map", "map-viewer", None),
                    view("Server Pages", "Server Pages 2", Some("/usr/share/pages/frame.svg")),
                ],
                require: Require::nothing(),
            })
        );
        // The area can be named anywhere, and one view is an area of views.
        let area = ViewedArea::parse("view Only\nwindow w\narea DRV").unwrap();
        assert_eq!((area.area, area.views.len()), (4, 1));
    }

    #[test]
    fn a_configuration_is_of_views_if_it_names_one_and_the_other_kinds_are_read_as_they_were() {
        assert!(matches!(External::parse("area BMS\nview Map\nwindow map"), Ok(External::Views(_))));
        assert!(matches!(External::parse("area WPN\nwindow sight\noverlay /x.svg"), Ok(External::Window(_))));
        assert!(matches!(External::parse("area SA\ntiles 2\nwindow Front front"), Ok(External::Tiles(_))));
        assert!(matches!(External::parse("area STR\nform load Load\nfield what text 4 What"), Ok(External::Form(_))));
        // Each kind's own reading turns the others away.
        let views = "area BMS\nview Map\nwindow map";
        assert!(ExternalArea::parse(views).unwrap_err().contains("'view'"));
        assert!(TiledArea::parse(views).unwrap_err().contains("'tiles'"));
        assert!(FormArea::parse(views).unwrap_err().contains("'form'"));
        for other in ["area WPN\nwindow sight", "area SA\ntiles 2\nwindow Front front", "area STR\nform load Load\nfield what text 4 What"] {
            assert!(ViewedArea::parse(other).unwrap_err().contains("no 'view'"), "{other}");
        }
    }

    #[test]
    fn a_configuration_of_views_that_cannot_be_right_says_why() {
        let error = |text: &str| External::parse(text).unwrap_err();
        assert!(error("view Map\nwindow map").contains("no 'area'"));
        assert!(error("area BMS\nview\nwindow map").contains("line 2: 'view' has no value"));
        assert!(error("area BMS\nview Map").contains("line 2: the view 'Map' has no 'window'"));
        assert!(error("area BMS\nview Map\nwindow a\nview Log").contains("line 4: the view 'Log' has no 'window'"));
        // A view has one window, and one image over it at the most...
        assert!(error("area BMS\nview Map\nwindow a\nwindow b").contains("line 4: 'window' is given twice for the view 'Map'"));
        assert!(error("area BMS\nview Map\nwindow a\noverlay /x.svg\noverlay /y.svg").contains("line 5: 'overlay' is given twice"));
        // ...which come after it, not before the first.
        assert!(error("area BMS\nwindow a\nview Map\nwindow b").contains("line 2: 'window' comes before the first 'view', on line 3"));
        assert!(error("area BMS\noverlay /x.svg\nview Map\nwindow b").contains("line 2: 'overlay' comes before the first 'view'"));
        // What another kind of area has, an area of views has not.
        assert!(error("area BMS\ntiles 2\nview Map\nwindow b").contains("line 2: 'tiles' is not for an area with a 'view'"));
        assert!(error("area BMS\nview Map\nwindow b\naspect 4:3").contains("line 4: 'aspect' is not for an area with a 'view'"));
        assert!(error("area BMS\nform load Load\nview Map\nwindow b").contains("line 2: 'form' is not for an area with a 'view'"));
        assert!(error("area BMS\nview Map\nwindow b\nfield what text 4 What").contains("line 4: 'field' is not for an area with a 'view'"));
        // Two views are two things, by what the crew call them and by their windows.
        assert!(error("area BMS\nview Map\nwindow a\nview Map\nwindow b").contains("'Map' is there twice"));
        assert!(error("area BMS\nview Map\nwindow a\nview Log\nwindow a").contains("'a' is there twice"));

        // A button to a view, and there are twelve down the sides.
        let views = |count| (1..=count).map(|n| format!("view V{n}\nwindow v{n}\n")).collect::<String>();
        assert!(External::parse(&format!("area BMS\n{}", views(12))).is_ok());
        assert!(error(&format!("area BMS\n{}", views(13))).contains("13 views"));
    }

    #[test]
    fn a_menu_serialises_in_the_shape_the_ui_reads() {
        let mut hmi = hmi();
        press(&mut hmi, Button::Area(BMS));
        press(&mut hmi, Button::Area(BMS));
        let json = serde_json::to_value(hmi.frame(at(0), Vec::new())).unwrap();
        assert_eq!(json["areas"][7], serde_json::json!({"id": "BMS", "state": "selected"}));
        assert_eq!(
            json["page"],
            serde_json::json!({"kind": "own", "title": "BMS", "rows": [
                {"name": "Map", "value": "F1 - last viewed"},
                {"name": "Server Pages", "value": "F2"},
            ]})
        );
        assert_eq!(json["labels"][0], serde_json::json!({"key": "F1", "text": "Map", "state": "selected"}));
        assert_eq!(json["labels"][1], serde_json::json!({"key": "F2", "text": "Server\nPages", "state": "available"}));
        assert_eq!(json["labels"][2]["state"], "blank");
        assert_eq!(json["ppi"]["threats"], serde_json::json!([]));

        // A view is another program's page, as an area of one window is.
        press(&mut hmi, Button::F(2));
        let json = serde_json::to_value(hmi.frame(at(0), Vec::new())).unwrap();
        assert_eq!(json["page"], serde_json::json!({"kind": "external", "overlay": "data:,grid", "keyboard": null}));
        assert_eq!(json["labels"][0]["state"], "blank");
        assert!(json["ppi"].is_null());
    }
}
