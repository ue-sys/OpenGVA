//! Tiles: several programs' windows on one page, side by side.
//!
//! An area can be given more windows than are to be shown at once
//! ([`TiledArea`]). The crew picks which, with the buttons down the sides,
//! and each window picked has a tile: a rectangle of the page, where the
//! display server puts the window and round which the UI draws. This module
//! keeps what is picked and says where the tiles are. What the windows show
//! is no more its business than it is anywhere else in this crate.

use crate::external::TiledArea;
use crate::{Hidden, LabelState};
use serde::Serialize;

/// The display at its reference size, and what is round the page on it: the
/// bars under the functional-area buttons and the status information bar
/// along the top, and the soft-key labels down the sides and along the
/// bottom. `hmi.css` lays the display out, and has the same figures.
const DISPLAY: (u32, u32) = (1024, 768);
const AREA_BARS: u32 = 14;
const STATUS_BAR: u32 = 34;
const SIDE_LABELS: u32 = 124;
const BOTTOM_LABELS: u32 = 56;
/// The height of a text message of one line. An alarm's goes along the top
/// of the page and a threat's along the bottom, and the tiles keep clear of
/// both, so that a message arriving covers none of a picture.
const MESSAGE: u32 = 30;
/// What is left between one tile and the next.
const GAP: u32 = 8;

/// The page: what the bars and the labels leave of the display. What F19
/// has hidden gives up its room, so with the labels gone the
/// page is the width of the display and goes down to its bottom edge, and
/// with the status information bar gone as well it is the display.
fn page(hidden: Hidden) -> Rect {
    let bars = if hidden.labels { 0 } else { AREA_BARS } + if hidden.status { 0 } else { STATUS_BAR };
    let (sides, bottom) = if hidden.labels { (0, 0) } else { (SIDE_LABELS, BOTTOM_LABELS) };
    Rect { x: sides, y: bars, width: DISPLAY.0 - 2 * sides, height: DISPLAY.1 - bars - bottom }
}

/// A rectangle of the display, in the pixels of its reference size, 1024 by
/// 768: the size the UI draws the display at, whatever the screen's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    /// This rectangle on a screen that shows the display `scale` times its
    /// reference size, with the display's top left corner at (`left`,
    /// `top`): its left, top, width and height in whole pixels of the
    /// screen. It is the edges that are rounded, so rectangles that meet on
    /// the display meet on the screen.
    pub fn on_screen(&self, left: f64, top: f64, scale: f64) -> [i32; 4] {
        let at = |origin: f64, along: u32| (origin + f64::from(along) * scale).round() as i32;
        let (x, y) = (at(left, self.x), at(top, self.y));
        [x, y, at(left, self.x + self.width) - x, at(top, self.y + self.height) - y]
    }
}

/// The shape of a picture: its width to its height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aspect {
    pub width: u32,
    pub height: u32,
}

/// 4:3, the shape of the display itself.
impl Default for Aspect {
    fn default() -> Self {
        Aspect { width: 4, height: 3 }
    }
}

impl Aspect {
    /// Read a shape as a configuration writes it: "4:3".
    pub fn parse(text: &str) -> Option<Self> {
        let (width, height) = text.split_once(':')?;
        let (width, height) = (width.trim().parse().ok()?, height.trim().parse().ok()?);
        (width > 0 && height > 0).then_some(Aspect { width, height })
    }

    /// The largest picture of this shape that goes in `width` by `height`,
    /// to the nearest pixel.
    fn fit(self, width: u32, height: u32) -> (u32, u32) {
        let (across, down) = (u64::from(self.width), u64::from(self.height));
        let (width, height) = (u64::from(width), u64::from(height));
        let nearest = |n: u64, d: u64| ((2 * n + d) / (2 * d)).max(1);
        let (width, height) = if width * down <= height * across {
            (width, nearest(width * down, across))
        } else {
            (nearest(height * across, down), height)
        };
        (width as u32, height as u32)
    }
}

/// Where `count` pictures of one shape go in `within`: in rows, every
/// picture the same size, with `gap` between one and the next. Of the ways
/// of making rows of them it is the one that makes the pictures largest -
/// the wider, of two that do as well. Two pictures are the exception: they
/// are side by side whatever the room, which is how they were asked for,
/// though on a page taller than it is wide one above the other would have
/// them a little larger. The rows are filled in order, and a last row that
/// is short is centred.
///
/// Nothing but the number of pictures decides it: taking one away leaves
/// what there was before it was added.
fn arrange(within: Rect, count: usize, shape: Aspect, gap: u32) -> Vec<Rect> {
    let count = count as u32;
    // What each of `along` pictures can have of a length, the gaps taken out.
    let share = |of: u32, along: u32| of.saturating_sub(gap * (along - 1)) / along;
    let ways = if count == 2 { 2..=2 } else { 1..=count };
    let Some((columns, (width, height))) = ways
        .map(|columns| {
            let rows = count.div_ceil(columns);
            (columns, shape.fit(share(within.width, columns), share(within.height, rows)))
        })
        .max_by_key(|&(columns, size)| (size, columns))
    else {
        return Vec::new();
    };

    // What `along` pictures leave of a length, halved: they go in the middle.
    let margin = |of: u32, along: u32, each: u32| of.saturating_sub(along * each + (along - 1) * gap) / 2;
    let top = within.y + margin(within.height, count.div_ceil(columns), height);
    (0..count)
        .map(|i| {
            let (row, column) = (i / columns, i % columns);
            let in_row = columns.min(count - row * columns);
            Rect {
                x: within.x + margin(within.width, in_row, width) + column * (width + gap),
                y: top + row * (height + gap),
                width,
                height,
            }
        })
        .collect()
}

/// A tile: the part of the display that one window's picture has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Tile {
    /// What the crew know the window as: the label of the button that put
    /// it there, to be written on it.
    pub label: String,
    #[serde(flatten)]
    pub at: Rect,
}

/// A tiled area's windows, and which of them the crew has picked.
#[derive(Debug)]
pub(crate) struct Tiles {
    area: TiledArea,
    /// Whether each of the area's windows is open, in the area's order.
    open: Vec<bool>,
    /// The windows shown, in the order they were picked: each by its place
    /// in the area's list.
    shown: Vec<usize>,
}

impl Tiles {
    pub fn new(area: TiledArea) -> Self {
        let open = vec![false; area.windows.len()];
        Tiles { area, open, shown: Vec::new() }
    }

    /// The functional area: an index into [`AREAS`](crate::AREAS).
    pub fn area(&self) -> u8 {
        self.area.area
    }

    /// The roles any one of which will do to have the area.
    pub fn require(&self) -> &hmi_access::Require {
        &self.area.require
    }

    /// Whether any of the windows is open: there is something to pick.
    pub fn present(&self) -> bool {
        self.open.contains(&true)
    }

    /// Take the windows now open, by the names their programs give them.
    /// Returns whether anything changed. A window that has gone has no tile
    /// any more, and is not given one again by itself when it comes back.
    pub fn windows<S: AsRef<str>>(&mut self, open: &[S]) -> bool {
        let now: Vec<bool> =
            self.area.windows.iter().map(|w| open.iter().any(|name| name.as_ref() == w.window)).collect();
        let changed = now != self.open;
        self.open = now;
        self.shown.retain(|&window| self.open[window]);
        changed
    }

    /// F`n` is pressed. Each of the buttons down the sides has a window, F1
    /// the first: it shows the window if that is open and there is a tile
    /// to spare, and stops showing it if it is shown.
    pub fn press(&mut self, n: u8) {
        let Some(window) = usize::from(n).checked_sub(1).filter(|&window| window < self.open.len()) else {
            return;
        };
        if let Some(at) = self.shown.iter().position(|&shown| shown == window) {
            self.shown.remove(at);
        } else if self.open[window] && self.shown.len() < self.area.tiles {
            self.shown.push(window);
        }
    }

    /// The labels of those buttons, each with the number of its button: a
    /// window's name for the crew, selected while it is shown, and
    /// unavailable while it cannot be - it is not open, or there is no tile
    /// to spare for it.
    pub fn labels(&self) -> impl Iterator<Item = (u8, &str, LabelState)> {
        let full = self.shown.len() >= self.area.tiles;
        (1u8..).zip(&self.area.windows).zip(&self.open).map(move |((n, window), &open)| {
            let state = if self.shown.contains(&(usize::from(n) - 1)) {
                LabelState::Selected
            } else if open && !full {
                LabelState::Available
            } else {
                LabelState::Unavailable
            };
            (n, window.label.as_str(), state)
        })
    }

    /// The windows shown, in the order they were picked: each by the name
    /// its program gives it, with what the crew know it as and its tile.
    /// The tiles have the page, as it is with what is `hidden` off the
    /// screen, less a line at its top and one at its bottom for the text
    /// messages.
    pub fn shown(&self, hidden: Hidden) -> impl Iterator<Item = (&str, Tile)> {
        let page = page(hidden);
        let within = Rect { y: page.y + MESSAGE, height: page.height - 2 * MESSAGE, ..page };
        let tiles = arrange(within, self.shown.len(), self.area.aspect, GAP);
        self.shown.iter().zip(tiles).map(|(&window, at)| {
            let window = &self.area.windows[window];
            (window.window.as_str(), Tile { label: window.label.clone(), at })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external::TileWindow;

    const FOUR_BY_THREE: Aspect = Aspect { width: 4, height: 3 };
    /// Where the tiles go on the display: the page, less a line at the top
    /// and one at the bottom for text messages.
    const WITHIN: Rect = Rect { x: 124, y: 78, width: 776, height: 604 };

    fn rect(x: u32, y: u32, width: u32, height: u32) -> Rect {
        Rect { x, y, width, height }
    }

    /// An area of six windows, "a" to "f" with the labels "A" to "F", of
    /// which `tiles` are shown at once: all six open.
    fn six(tiles: usize) -> Tiles {
        let windows = ["a", "b", "c", "d", "e", "f"]
            .map(|name| TileWindow { label: name.to_uppercase(), window: name.to_string() })
            .to_vec();
        let mut tiles = Tiles::new(TiledArea { area: 0, tiles, aspect: FOUR_BY_THREE, windows, require: hmi_access::Require::nothing() });
        tiles.windows(&["a", "b", "c", "d", "e", "f"]);
        tiles
    }

    /// With nothing hidden.
    const SEEN: Hidden = Hidden { labels: false, status: false };

    fn places(tiles: &Tiles) -> Vec<(String, Rect)> {
        tiles.shown(SEEN).map(|(_, tile)| (tile.label, tile.at)).collect()
    }

    #[test]
    fn one_picture_has_the_page_and_more_share_it_as_large_as_they_go() {
        let arranged = |count| arrange(WITHIN, count, FOUR_BY_THREE, 8);
        assert!(arranged(0).is_empty());
        // One: the width of the page.
        assert_eq!(arranged(1), [rect(124, 89, 776, 582)]);
        // Two: side by side, in the middle of the page's height.
        assert_eq!(arranged(2), [rect(124, 236, 384, 288), rect(516, 236, 384, 288)]);
        // Three: two and one, the one in the middle.
        assert_eq!(arranged(3), [rect(124, 88, 384, 288), rect(516, 88, 384, 288), rect(320, 384, 384, 288)]);
        // Four: two and two.
        assert_eq!(
            arranged(4),
            [rect(124, 88, 384, 288), rect(516, 88, 384, 288), rect(124, 384, 384, 288), rect(516, 384, 384, 288)]
        );
    }

    #[test]
    fn pictures_keep_their_shape_and_their_distance_however_many_there_are() {
        for shape in [FOUR_BY_THREE, Aspect { width: 16, height: 9 }, Aspect { width: 1, height: 1 }] {
            for count in 1..=12 {
                let tiles = arrange(WITHIN, count, shape, 8);
                assert_eq!(tiles.len(), count);
                for (i, tile) in tiles.iter().enumerate() {
                    // The shape, to within the pixel that sizes are rounded to.
                    let (across, down) = (u64::from(shape.width), u64::from(shape.height));
                    let (width, height) = (u64::from(tile.width), u64::from(tile.height));
                    let out = (width * down).abs_diff(height * across);
                    assert!(out <= across.max(down), "{shape:?} {count}: {tile:?}");
                    // All one size, and on the page.
                    assert_eq!((tile.width, tile.height), (tiles[0].width, tiles[0].height));
                    assert!(tile.x >= WITHIN.x && tile.x + tile.width <= WITHIN.x + WITHIN.width);
                    assert!(tile.y >= WITHIN.y && tile.y + tile.height <= WITHIN.y + WITHIN.height);
                    // And none nearer another than the gap.
                    for other in &tiles[..i] {
                        let beside = other.x + other.width + 8 <= tile.x || tile.x + tile.width + 8 <= other.x;
                        let below = other.y + other.height + 8 <= tile.y;
                        assert!(beside || below, "{shape:?} {count}: {other:?} and {tile:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_shape_of_the_pictures_and_of_the_room_decide_how_they_are_arranged() {
        // Wide pictures are put one above the other sooner than narrow ones...
        let wide = arrange(WITHIN, 3, Aspect { width: 3, height: 1 }, 8);
        assert!(wide.iter().all(|tile| tile.x == wide[0].x), "{wide:?}");
        // ...and room that is wide has them side by side.
        let room = rect(0, 0, 1600, 400);
        assert_eq!(arrange(room, 2, FOUR_BY_THREE, 0), [rect(267, 0, 533, 400), rect(800, 0, 533, 400)]);
        // Of two ways that do as well, it is the wider.
        assert_eq!(arrange(rect(0, 0, 800, 800), 2, Aspect { width: 1, height: 1 }, 0), [
            rect(0, 200, 400, 400),
            rect(400, 200, 400, 400)
        ]);
    }

    #[test]
    fn a_shape_is_written_as_a_width_to_a_height() {
        assert_eq!(Aspect::parse("4:3"), Some(FOUR_BY_THREE));
        assert_eq!(Aspect::parse(" 16 : 9 "), Some(Aspect { width: 16, height: 9 }));
        assert_eq!(Aspect::default(), FOUR_BY_THREE);
        for not_one in ["", "4", "4:", ":3", "4:0", "0:3", "4/3", "wide", "-4:3", "1.5:1"] {
            assert_eq!(Aspect::parse(not_one), None, "{not_one}");
        }
        // A shape too thin to be seen still has a size.
        assert_eq!(Aspect { width: 1, height: 5000 }.fit(100, 100), (1, 100));
    }

    #[test]
    fn windows_have_tiles_in_the_order_they_are_picked() {
        let mut tiles = six(4);
        assert!(places(&tiles).is_empty());

        // The one picked has the page; another shares it.
        tiles.press(3);
        assert_eq!(places(&tiles), [("C".to_string(), rect(124, 89, 776, 582))]);
        tiles.press(1);
        assert_eq!(
            places(&tiles),
            [("C".to_string(), rect(124, 236, 384, 288)), ("A".to_string(), rect(516, 236, 384, 288))]
        );
        let names: Vec<_> = tiles.shown(SEEN).map(|(window, _)| window.to_string()).collect();
        assert_eq!(names, ["c", "a"], "the display server is told of them by their windows' names");

        tiles.press(6);
        tiles.press(2);
        let labels = |tiles: &Tiles| places(tiles).into_iter().map(|(label, _)| label).collect::<Vec<_>>();
        assert_eq!(labels(&tiles), ["C", "A", "F", "B"]);
        // The one taken away leaves those after it to close up.
        tiles.press(1);
        assert_eq!(labels(&tiles), ["C", "F", "B"]);
        assert_eq!(places(&tiles)[1].1, rect(516, 88, 384, 288));
    }

    #[test]
    fn taking_windows_away_goes_back_the_way_adding_them_came() {
        let mut tiles = six(4);
        let mut on_the_way = Vec::new();
        for n in 1..=4 {
            tiles.press(n);
            on_the_way.push(places(&tiles));
        }
        for n in (2..=4).rev() {
            tiles.press(n);
            assert_eq!(places(&tiles), on_the_way[usize::from(n) - 2], "with {} left", n - 1);
        }
        tiles.press(1);
        assert!(places(&tiles).is_empty());
    }

    #[test]
    fn a_windows_label_says_whether_it_is_shown_or_can_be() {
        use LabelState::{Available, Selected, Unavailable};
        let mut tiles = six(2);
        let states = |tiles: &Tiles| tiles.labels().map(|(_, _, state)| state).collect::<Vec<_>>();
        let said: Vec<_> = tiles.labels().map(|(n, text, _)| (n, text.to_string())).collect();
        assert_eq!(said[0], (1, "A".to_string()));
        assert_eq!(said[5], (6, "F".to_string()));
        assert_eq!(states(&tiles), [Available; 6]);

        tiles.press(2);
        assert_eq!(states(&tiles), [Available, Selected, Available, Available, Available, Available]);
        // With as many shown as there are tiles, no other can be.
        tiles.press(5);
        assert_eq!(states(&tiles), [Unavailable, Selected, Unavailable, Unavailable, Selected, Unavailable]);
        tiles.press(1);
        assert_eq!(places(&tiles).len(), 2, "pressing its button shows nothing more");
        // Until one of those is taken away.
        tiles.press(2);
        assert_eq!(states(&tiles), [Available, Available, Available, Available, Selected, Available]);

        // A button that has no window does nothing.
        for n in [0, 7, 12, 20] {
            tiles.press(n);
        }
        assert_eq!(places(&tiles).len(), 1);
    }

    #[test]
    fn a_window_that_is_not_open_cannot_be_shown_and_one_that_goes_loses_its_tile() {
        use LabelState::{Available, Selected, Unavailable};
        let mut tiles = six(4);
        let states = |tiles: &Tiles| tiles.labels().map(|(_, _, state)| state).collect::<Vec<_>>();
        assert!(tiles.windows(&["a", "c", "something else"]));
        assert!(!tiles.windows(&["c", "a"]), "the same windows again change nothing");
        assert_eq!(states(&tiles), [Available, Unavailable, Available, Unavailable, Unavailable, Unavailable]);
        tiles.press(2);
        assert!(places(&tiles).is_empty());

        tiles.press(1);
        tiles.press(3);
        assert!(tiles.windows(&["c"]));
        assert_eq!(places(&tiles), [("C".to_string(), rect(124, 89, 776, 582))]);
        // It is not shown again by itself when it is back: the crew chose nothing.
        assert!(tiles.windows(&["a", "c"]));
        assert_eq!(states(&tiles), [Available, Unavailable, Selected, Unavailable, Unavailable, Unavailable]);
        assert_eq!(places(&tiles).len(), 1);

        assert!(tiles.present());
        tiles.windows(&[] as &[&str]);
        assert!(!tiles.present());
        assert!(places(&tiles).is_empty());
    }

    #[test]
    fn tiles_have_the_room_of_what_is_hidden() {
        let labels = Hidden { labels: true, status: false };
        let all = Hidden { labels: true, status: true };
        // The page is what the bars and labels leave, and all of the display
        // when there are none.
        assert_eq!(page(SEEN), rect(124, 48, 776, 664));
        assert_eq!(page(labels), rect(0, 34, 1024, 734));
        assert_eq!(page(all), rect(0, 0, 1024, 768));

        let mut tiles = six(4);
        let at = |tiles: &Tiles, hidden| tiles.shown(hidden).map(|(_, tile)| tile.at).collect::<Vec<_>>();
        tiles.press(1);
        assert_eq!(at(&tiles, SEEN), [rect(124, 89, 776, 582)]);
        assert_eq!(at(&tiles, labels), [rect(62, 64, 899, 674)]);
        assert_eq!(at(&tiles, all), [rect(40, 30, 944, 708)]);
        // Two are side by side, and larger in the wider room.
        tiles.press(2);
        assert_eq!(at(&tiles, SEEN), [rect(124, 236, 384, 288), rect(516, 236, 384, 288)]);
        assert_eq!(at(&tiles, labels), [rect(0, 210, 508, 381), rect(516, 210, 508, 381)]);
        assert_eq!(at(&tiles, all), [rect(0, 193, 508, 381), rect(516, 193, 508, 381)]);
        // Three and four are two rows, as they were, of larger pictures.
        tiles.press(3);
        assert_eq!(at(&tiles, labels), [rect(64, 64, 444, 333), rect(516, 64, 444, 333), rect(290, 405, 444, 333)]);
        assert_eq!(at(&tiles, all), [rect(41, 30, 467, 350), rect(516, 30, 467, 350), rect(278, 388, 467, 350)]);
        tiles.press(4);
        assert_eq!(at(&tiles, SEEN)[3], rect(516, 384, 384, 288));
        assert_eq!(
            at(&tiles, labels),
            [rect(64, 64, 444, 333), rect(516, 64, 444, 333), rect(64, 405, 444, 333), rect(516, 405, 444, 333)]
        );
        assert_eq!(
            at(&tiles, all),
            [rect(41, 30, 467, 350), rect(516, 30, 467, 350), rect(41, 388, 467, 350), rect(516, 388, 467, 350)]
        );

        // Whatever is hidden, the pictures keep their shape, and keep clear
        // of where the text messages go: a line at the top of the page and
        // one at its bottom.
        for hidden in [SEEN, labels, all] {
            let page = page(hidden);
            for tile in at(&tiles, hidden) {
                assert!((u64::from(tile.width) * 3).abs_diff(u64::from(tile.height) * 4) <= 4, "{tile:?}");
                assert!(tile.x >= page.x && tile.x + tile.width <= page.x + page.width, "{tile:?}");
                assert!(tile.y >= page.y + 30 && tile.y + tile.height <= page.y + page.height - 30, "{tile:?}");
            }
        }
    }

    #[test]
    fn a_rectangle_of_the_display_is_found_on_a_screen_of_any_size() {
        // The display at its own size is the screen.
        assert_eq!(rect(124, 88, 384, 288).on_screen(0.0, 0.0, 1.0), [124, 88, 384, 288]);
        // On one of 1280 by 800 it is 800 high, in the middle.
        let (left, scale) = ((1280.0 - 1024.0 * 800.0 / 768.0) / 2.0, 800.0 / 768.0);
        let [x, y, width, height] = rect(124, 88, 384, 288).on_screen(left, 0.0, scale);
        assert_eq!([x, y, width, height], [236, 92, 400, 300]);
        // What meets on the display meets on the screen.
        let [beside, ..] = rect(124 + 384, 88, 8, 288).on_screen(left, 0.0, scale);
        assert_eq!(beside, x + width);
    }
}
