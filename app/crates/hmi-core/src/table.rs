//! What the Alarms and Threats pages' tables have in common:
//! a row can be picked, the table is shown a page at a time, and the picked
//! row can be shown at length. Which rows there are, and in what order, is
//! each page's own business; this works on their names, as listed.

/// Which row is picked, which page is showing and whether a row is shown at
/// length: what the crew has done with a table, apart from how it is ordered.
#[derive(Debug, Clone, Default)]
pub(crate) struct View {
    /// The row picked out, by name.
    selected: Option<String>,
    /// The page showing, counted from 0, while no row is picked.
    page: usize,
    /// The row shown at length, if one is: a dialog over the table.
    shown: Option<String>,
}

/// The part of a table that is on the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OnScreen {
    /// The page showing, counted from 1...
    pub page: usize,
    /// ...of this many.
    pub pages: usize,
    /// Where in the listing the page's first row is.
    pub first: usize,
    /// Where in the listing the picked row is, if it is in it.
    pub picked: Option<usize>,
}

impl View {
    fn position(&self, listed: &[&str]) -> Option<usize> {
        let selected = self.selected.as_ref()?;
        listed.iter().position(|name| name == selected)
    }

    /// The page showing (from 0) of how many: the one with the picked row on
    /// it, if one is picked.
    fn pages(&self, listed: &[&str], rows: usize) -> (usize, usize) {
        let pages = listed.len().div_ceil(rows).max(1);
        let page = self.position(listed).map_or(self.page, |at| at / rows);
        (page.min(pages - 1), pages)
    }

    fn selected(&self, listed: &[&str]) -> Option<&str> {
        self.position(listed).and(self.selected.as_deref())
    }

    fn step(&mut self, down: bool, listed: &[&str], rows: usize) {
        let (page, _) = self.pages(listed, rows);
        let to = match self.position(listed) {
            Some(at) if down => (at + 1).min(listed.len().saturating_sub(1)),
            Some(at) => at.saturating_sub(1),
            None => page * rows,
        };
        self.selected = listed.get(to).map(|name| name.to_string());
        // With a row shown at length, the next one is shown in its place.
        if self.shown.is_some() && self.selected.is_some() {
            self.shown = self.selected.clone();
        }
    }

    /// Pick the row of this name; picked already, it is shown at length.
    /// Returns whether there was such a row.
    fn pick(&mut self, name: &str, listed: &[&str]) -> bool {
        if !listed.contains(&name) {
            return false;
        }
        if self.selected.as_deref() == Some(name) {
            self.shown = self.selected.clone();
        } else {
            self.selected = Some(name.to_string());
        }
        true
    }

    fn turn_page(&mut self, down: bool, listed: &[&str], rows: usize) {
        let (page, pages) = self.pages(listed, rows);
        let to = if down { (page + 1).min(pages - 1) } else { page.saturating_sub(1) };
        if to != page && self.position(listed).is_some() {
            self.selected = listed.get(to * rows).map(|name| name.to_string());
        }
        self.page = to;
    }

    fn on_screen(&self, listed: &[&str], rows: usize) -> OnScreen {
        let (page, pages) = self.pages(listed, rows);
        OnScreen { page: page + 1, pages, first: page * rows, picked: self.position(listed) }
    }
}

/// A table of the things one of the pages lists: the page says what is in
/// it and in what order, and has the rest from here.
pub(crate) trait Paged {
    /// Where the things come from.
    type From;
    /// The rows on one page of the table: as many as fit when every one of
    /// them needs a second line for its text.
    const ROWS: usize;

    fn view(&self) -> &View;
    fn view_mut(&mut self) -> &mut View;
    /// The rows' names, in the order the table lists them.
    fn names<'a>(&self, from: &'a Self::From) -> Vec<&'a str>;
    /// Whether there is still a thing of this name at all, in the table or
    /// left out of it.
    fn has(from: &Self::From, name: &str) -> bool;

    /// The row picked out, if it is in the table as it now is.
    fn selected<'a>(&'a self, from: &Self::From) -> Option<&'a str> {
        self.view().selected(&self.names(from))
    }

    /// Move the pick a row up or down; with none, pick the first row on
    /// the page.
    fn step(&mut self, down: bool, from: &Self::From) {
        let names = self.names(from);
        self.view_mut().step(down, &names, Self::ROWS);
    }

    /// Pick the row of this name, as a touch on it does; a
    /// touch on the row that is picked already shows it at length. Returns
    /// whether the table lists such a row.
    fn pick(&mut self, name: &str, from: &Self::From) -> bool {
        let names = self.names(from);
        self.view_mut().pick(name, &names)
    }

    /// Show the page before or after, and move the pick to it if there is
    /// one.
    fn turn_page(&mut self, down: bool, from: &Self::From) {
        let names = self.names(from);
        self.view_mut().turn_page(down, &names, Self::ROWS);
    }

    /// Whether there is a page before, and one after, the one showing.
    fn more_pages(&self, from: &Self::From) -> (bool, bool) {
        let on_screen = self.on_screen(from);
        (on_screen.page > 1, on_screen.page < on_screen.pages)
    }

    /// Which part of the table is on the screen.
    fn on_screen(&self, from: &Self::From) -> OnScreen {
        self.view().on_screen(&self.names(from), Self::ROWS)
    }

    /// Show the picked row at length.
    fn show(&mut self, from: &Self::From) {
        let picked = self.selected(from).map(str::to_string);
        self.view_mut().shown = picked;
    }

    /// Stop showing a row at length.
    fn close(&mut self) {
        self.view_mut().shown = None;
    }

    /// Whether a row is shown at length.
    fn showing(&self) -> bool {
        self.view().shown.is_some()
    }

    /// The name of the row shown at length, if one is.
    fn shown(&self) -> Option<&str> {
        self.view().shown.as_deref()
    }

    /// Keep up with the things after they have changed. One shown at length
    /// stays shown while it is there at all - in the table or left out of
    /// it, as it may be by the table's own choice of what to list - and no
    /// longer.
    fn settle(&mut self, from: &Self::From) {
        if self.shown().is_some_and(|name| !Self::has(from, name)) {
            self.close();
        }
    }
}
