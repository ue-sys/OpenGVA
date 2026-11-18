//! Functional areas given away: to other programs, or to a form for the crew
//! to fill in.

use crate::tiles::Aspect;
use crate::{AREAS, Button, LabelState};
use hmi_access::Require;
use hmi_entry::{Field, Form};
use serde::{Deserialize, Serialize};

/// A functional area whose page is another program's window - a sight's
/// video, a map - shown under the HMI's own elements, which stay on top of it.
/// The area can be selected while the program has its window open. That, and
/// the name of the window, is all the HMI knows of the program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalArea {
    /// The functional area: an index into [`AREAS`].
    pub area: u8,
    /// The name the program gives its window; on Wayland, its application id.
    pub window: String,
    /// An image to draw over the program's picture, such as a reticle. A
    /// configuration names a file; what the UI is to be handed is a URL it
    /// can load, so the file's contents as a `data:` URL go here before the
    /// area is given to the [`Hmi`](crate::Hmi).
    pub overlay: Option<String>,
    /// Whether the program can be typed into. The page then has a button
    /// that brings the on-screen keyboard up over it, and what the crew
    /// types on that is the program's. One that takes no typing - a sight's
    /// picture - is not offered a keyboard it has no use for.
    pub keyboard: bool,
    /// The roles any one of which will do to have the area: none, unless its
    /// configuration says, and then whoever is there may have it. Otherwise
    /// choosing the area asks for the PIN of someone who holds one.
    pub require: Require,
    /// Where the program listens for the HMI ([`ToApp`], [`FromApp`]): a
    /// Unix socket, which the HMI connects to. Without one the program is
    /// told nothing, and has no buttons.
    pub socket: Option<String>,
    /// The program's buttons down the sides, F1 to F12: the HMI draws their
    /// labels and tells the program when one is pressed. These are what the
    /// configuration gives it; the program can say others over the socket.
    pub keys: Vec<AppKey>,
}

/// One of a program's buttons ([`ExternalArea::keys`]): as a configuration
/// gives it (`key F1 zoom-in Zoom In`), and as the program says it.
///
/// ```text
/// {"key": "F1", "id": "zoom-in", "label": "Zoom In", "state": "available"}
/// ```
///
/// `id` is what the program is told when it is pressed; `label` what the
/// crew read, a line break in it one on the screen; `state` how it is drawn,
/// `available` if it is not said. An `unavailable` or `blank` one is not
/// told of when pressed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppKey {
    pub key: String,
    pub id: String,
    pub label: String,
    #[serde(default = "available")]
    pub state: LabelState,
}

fn available() -> LabelState {
    LabelState::Available
}

impl AppKey {
    /// Its button's number: 1 to 12, once [`keys_fault`] has passed it.
    pub fn n(&self) -> u8 {
        match self.key.parse() {
            Ok(Button::F(n)) => n,
            _ => 0,
        }
    }
}

/// Why these cannot be a program's buttons, if they cannot. `keyboard` is
/// whether the program can be typed into, which takes a button of its own.
pub fn keys_fault(keys: &[AppKey], keyboard: bool) -> Option<String> {
    use crate::typed_page::KEYBOARD;
    let (mut buttons, mut ids) = (Vec::new(), Vec::new());
    for key in keys {
        let n = key.n();
        if !(1..=12).contains(&n) {
            return Some(format!("'{}' is not a button down the sides: F1 to F12", key.key));
        }
        if keyboard && n == KEYBOARD {
            return Some(format!("F{KEYBOARD} brings up the keyboard, and is not the program's"));
        }
        if key.id.is_empty() {
            return Some(format!("F{n} has no id"));
        }
        if buttons.contains(&n) || ids.contains(&&key.id) {
            return Some(format!("'{}' is there twice", if buttons.contains(&n) { &key.key } else { &key.id }));
        }
        buttons.push(n);
        ids.push(&key.id);
    }
    None
}

/// A line from a program to the HMI, over its socket.
///
/// ```text
/// {"keys": [{"key": "F1", "id": "laser", "label": "Laser", "state": "selected"}]}
/// ```
///
/// `keys` are all of its buttons from now on, in place of what it had.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FromApp {
    Keys(Vec<AppKey>),
}

/// A line from the HMI to a program: one of its buttons pressed, and let go.
/// And `{"ready":true}` ([`crate::feed::READY`]) whenever the HMI connects,
/// to which the program says its buttons if they are not what the
/// configuration gives.
///
/// ```text
/// {"pressed": "zoom-in"}
/// {"released": "zoom-in"}
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToApp {
    Pressed(String),
    Released(String),
}

impl FromApp {
    pub fn parse(line: &str) -> Result<Self, String> {
        serde_json::from_str(line).map_err(|e| e.to_string())
    }
}

impl ToApp {
    pub fn line(&self) -> String {
        serde_json::to_string(self).expect("a string serializes")
    }
}

/// A functional area whose page is several programs' windows, side by side,
/// each in a tile of the page. There can be more windows than are shown at
/// once: every one has a button down the sides of the screen, which shows it
/// and stops showing it. The area can be selected while any of the windows
/// is open. Their names, what the crew know each as, and the shape of their
/// pictures, is all the HMI knows of the programs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TiledArea {
    /// The functional area: an index into [`AREAS`].
    pub area: u8,
    /// How many of the windows are shown at once, at the most.
    pub tiles: usize,
    /// The shape of every window's picture, which its tile has too.
    pub aspect: Aspect,
    /// The windows, in the order of their buttons: F1 has the first.
    pub windows: Vec<TileWindow>,
    /// The roles any one of which will do to have the area, as an
    /// [`ExternalArea`]'s.
    pub require: Require,
}

/// One of the windows of a [`TiledArea`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileWindow {
    /// What the crew know it as: its button's label, and its tile's.
    pub label: String,
    /// The name its program gives it; on Wayland, its application id.
    pub window: String,
}

/// A functional area whose page is a form for the crew to fill in, with the
/// on-screen keyboard: a page of the HMI's own, with nothing of another
/// program's in it. What the form is for is as little the HMI's business as
/// what a program's picture is of: what the crew enters and saves is told to
/// whatever knows, by the names the configuration gives the form and its
/// fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormArea {
    /// The functional area: an index into [`AREAS`].
    pub area: u8,
    pub form: Form,
    /// The roles any one of which will do to have the area, as an
    /// [`ExternalArea`]'s.
    pub require: Require,
}

/// A functional area whose page is one of several programs' windows at a
/// time: each a view of the area, with a name the crew know it by. The first
/// is what the area shows until the crew chooses another, which they do from
/// the area's top level menu - the page the area's own button brings up while
/// the area is selected. The area can be selected while any of the windows
/// is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewedArea {
    /// The functional area: an index into [`AREAS`].
    pub area: u8,
    /// The views, in the order of their buttons on the menu: F1 has the first.
    pub views: Vec<AreaView>,
    /// The roles any one of which will do to have the area at all, as an
    /// [`ExternalArea`]'s: what the configuration requires before it names a
    /// view.
    pub require: Require,
}

/// One of the views of a [`ViewedArea`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaView {
    /// What the crew know it as: its button's label on the area's menu.
    pub label: String,
    /// The name its program gives its window; on Wayland, its application id.
    pub window: String,
    /// An image to draw over the program's picture, as an [`ExternalArea`]
    /// can have: a URL the UI can load, by the time the area is given to the
    /// [`Hmi`](crate::Hmi).
    pub overlay: Option<String>,
    /// Whether the view's program can be typed into, as an
    /// [`ExternalArea`]'s can be.
    pub keyboard: bool,
    /// The roles any one of which will do to be shown this view, over and
    /// above what the area needs: what the configuration requires after
    /// naming it. Choosing the view from the area's menu asks for the PIN of
    /// someone who holds one, if nobody logged in does.
    pub require: Require,
}

impl ViewedArea {
    /// Read the configuration of an area of views, which is one that names
    /// a view. `view` is what the crew know the view as, in as many words as
    /// it takes; the `window` after it is that view's, and so is an
    /// `overlay`, if it has one, and whether its program can be typed into
    /// with the on-screen `keyboard`. The views are on the area's menu in the
    /// order they are given, and the first is what the area shows until the
    /// crew chooses another.
    ///
    /// ```text
    /// area    BMS
    /// view    Map
    /// window  map-viewer
    /// view    Server Pages
    /// window  server-pages
    /// overlay /usr/share/pages/frame.svg      # optional
    /// keyboard yes                            # optional: 'no' unless said
    /// require commander                       # optional: this view's; before any view, the area's
    /// ```
    pub fn parse(text: &str) -> Result<Self, String> {
        match External::parse(text)? {
            External::Views(area) => Ok(area),
            External::Window(_) | External::Tiles(_) | External::Form(_) => Err("no 'view'".to_string()),
        }
    }

    /// Why this cannot be an area of views, if it cannot.
    pub fn fault(&self) -> Option<String> {
        if self.views.is_empty() {
            return Some("no 'view'".to_string());
        }
        if self.views.len() > BUTTONS {
            return Some(format!("{} views, and a button for {BUTTONS}: F1 to F{BUTTONS}", self.views.len()));
        }
        let (mut labels, mut names) = (Vec::new(), Vec::new());
        for view in &self.views {
            if view.label.is_empty() || view.window.is_empty() {
                return Some("a view is a label, and a window by its name".to_string());
            }
            for (said, what) in [(&mut labels, &view.label), (&mut names, &view.window)] {
                if said.contains(&what) {
                    return Some(format!("'{what}' is there twice"));
                }
                said.push(what);
            }
        }
        None
    }
}

/// A functional area as a configuration gives it away: to one program's
/// window, to several of which some are shown at a time, to several of which
/// one is, or to a form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum External {
    Window(ExternalArea),
    Tiles(TiledArea),
    Views(ViewedArea),
    Form(FormArea),
}

/// How many windows a [`TiledArea`] can have: one to each button down the
/// sides, F1 to F12.
const BUTTONS: usize = 12;

/// What a configuration's lines say, each with the number of its line,
/// before it is known which kind of area they describe.
#[derive(Default)]
struct Said<'a> {
    area: Option<(usize, &'a str)>,
    windows: Vec<(usize, &'a str)>,
    overlay: Option<(usize, &'a str)>,
    keyboard: Option<(usize, &'a str)>,
    require: Vec<(usize, &'a str)>,
    tiles: Option<(usize, &'a str)>,
    aspect: Option<(usize, &'a str)>,
    form: Option<(usize, &'a str)>,
    fields: Vec<(usize, &'a str)>,
    views: Vec<SaidView<'a>>,
    socket: Option<(usize, &'a str)>,
    keys: Vec<(usize, &'a str)>,
}

/// What a configuration says of one view: the line that names it, and of the
/// lines after that one - up to the next view - those that are a view's.
struct SaidView<'a> {
    label: (usize, &'a str),
    window: Option<(usize, &'a str)>,
    overlay: Option<(usize, &'a str)>,
    keyboard: Option<(usize, &'a str)>,
    require: Vec<(usize, &'a str)>,
}

/// Whether a program can be typed into, as a configuration says it: `yes` or
/// `no`, and `no` if it does not say.
fn keyboard(said: Option<(usize, &str)>) -> Result<bool, String> {
    match said {
        None | Some((_, "no")) => Ok(false),
        Some((_, "yes")) => Ok(true),
        Some((line, other)) => Err(format!("line {line}: 'keyboard' is 'yes' or 'no', not '{other}'")),
    }
}

/// The roles any one of which will do, as a configuration's `require` lines
/// say them: those of every such line, together. With no such line nothing
/// is required.
fn require(said: &[(usize, &str)]) -> Result<Require, String> {
    let mut require = Require::nothing();
    for (line, roles) in said {
        require.or(Require::parse(roles).map_err(|why| format!("line {line}: {why}"))?);
    }
    Ok(require)
}

impl<'a> Said<'a> {
    /// Read a configuration: a name and a value to a line, `#` starting a
    /// comment.
    fn read(text: &'a str) -> Result<Self, String> {
        let mut said = Said::default();
        for (number, line) in (1..).zip(text.lines()) {
            let line = line.split('#').next().unwrap_or_default().trim();
            if line.is_empty() {
                continue;
            }
            let (name, value) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            let value = value.trim();
            if value.is_empty() {
                return Err(format!("line {number}: '{name}' has no value"));
            }
            // What is required can take more than one line to say. Once a
            // view has been named it is that view which requires it, and
            // before that the area.
            if name == "require" {
                match said.views.last_mut() {
                    Some(view) => view.require.push((number, value)),
                    None => said.require.push((number, value)),
                }
                continue;
            }
            // Once a view has been named, a window, an overlay and whether
            // there is a keyboard are that view's, and each view has one of
            // each at the most.
            if let ("window" | "overlay" | "keyboard", Some(view)) = (name, said.views.last_mut()) {
                let slot = match name {
                    "window" => &mut view.window,
                    "overlay" => &mut view.overlay,
                    _ => &mut view.keyboard,
                };
                if slot.replace((number, value)).is_some() {
                    return Err(format!("line {number}: '{name}' is given twice for the view '{}'", view.label.1));
                }
                continue;
            }
            let slot = match name {
                "area" => &mut said.area,
                "overlay" => &mut said.overlay,
                "keyboard" => &mut said.keyboard,
                "tiles" => &mut said.tiles,
                "aspect" => &mut said.aspect,
                "form" => &mut said.form,
                "socket" => &mut said.socket,
                // A program has as many buttons as it has.
                "key" => {
                    said.keys.push((number, value));
                    continue;
                }
                // Whether an area can have more than one depends on its kind.
                "window" => {
                    said.windows.push((number, value));
                    continue;
                }
                // A form has as many as it has.
                "field" => {
                    said.fields.push((number, value));
                    continue;
                }
                // And so has an area of views.
                "view" => {
                    said.views.push(SaidView {
                        label: (number, value),
                        window: None,
                        overlay: None,
                        keyboard: None,
                        require: Vec::new(),
                    });
                    continue;
                }
                _ => return Err(format!("line {number}: '{name}' is not something an area has")),
            };
            if slot.replace((number, value)).is_some() {
                return Err(format!("line {number}: '{name}' is given twice"));
            }
        }
        Ok(said)
    }
}

impl External {
    /// Read a configuration of any kind. It is of views if it names one, of
    /// a form if it names one, and of tiles if it says how many: see
    /// [`ExternalArea::parse`], [`TiledArea::parse`], [`ViewedArea::parse`]
    /// and [`FormArea::parse`].
    pub fn parse(text: &str) -> Result<Self, String> {
        let said = Said::read(text)?;
        let (_, area) = said.area.ok_or("no 'area'")?;
        let area = (0u8..)
            .zip(AREAS)
            .find_map(|(i, id)| (id == area).then_some(i))
            .ok_or_else(|| format!("there is no functional area '{area}'"))?;
        // Whatever kind of area it is, it can need a role.
        let needs = require(&said.require)?;

        // Only a program with the page to itself has the buttons down the
        // sides: the other kinds of area have them already.
        let of_one_window = [("socket", said.socket), ("key", said.keys.first().copied())];
        if let Some((name, (line, _))) = of_one_window.into_iter().find_map(|(name, said)| Some((name, said?))) {
            let kind = [("view", said.views.first().map(|v| v.label)), ("form", said.form), ("tiles", said.tiles)];
            if let Some((kind, _)) = kind.into_iter().find_map(|(kind, said)| Some((kind, said?))) {
                return Err(format!("line {line}: '{name}' is not for an area with a '{kind}'"));
            }
        }

        // An area with views is several programs' windows, one at a time:
        // it has its views, each with a window of its own, and nothing of
        // any other kind of area.
        if let Some(first) = said.views.first() {
            let before =
                [("window", said.windows.first().copied()), ("overlay", said.overlay), ("keyboard", said.keyboard)];
            if let Some((name, (line, _))) = before.into_iter().find_map(|(name, said)| Some((name, said?))) {
                let (at, _) = first.label;
                return Err(format!(
                    "line {line}: '{name}' comes before the first 'view', on line {at}: each view has its own"
                ));
            }
            let of_another_kind = [
                ("tiles", said.tiles),
                ("aspect", said.aspect),
                ("form", said.form),
                ("field", said.fields.first().copied()),
            ];
            if let Some((name, (line, _))) = of_another_kind.into_iter().find_map(|(name, said)| Some((name, said?))) {
                return Err(format!("line {line}: '{name}' is not for an area with a 'view'"));
            }
            let views = said
                .views
                .iter()
                .map(|view| {
                    let (line, label) = view.label;
                    let (_, window) = view.window.ok_or_else(|| format!("line {line}: the view '{label}' has no 'window'"))?;
                    Ok(AreaView {
                        label: label.to_string(),
                        window: window.to_string(),
                        overlay: view.overlay.map(|(_, image)| image.to_string()),
                        keyboard: keyboard(view.keyboard)?,
                        require: require(&view.require)?,
                    })
                })
                .collect::<Result<_, String>>()?;
            let area = ViewedArea { area, views, require: needs };
            return match area.fault() {
                Some(fault) => Err(fault),
                None => Ok(External::Views(area)),
            };
        }

        // An area with a form is a page of the HMI's own: it has fields, and
        // nothing that an area given to a program has.
        if let Some((line, form)) = said.form {
            let of_a_program = [
                ("window", said.windows.first().copied()),
                ("overlay", said.overlay),
                ("keyboard", said.keyboard),
                ("tiles", said.tiles),
                ("aspect", said.aspect),
            ];
            if let Some((name, (line, _))) = of_a_program.into_iter().find_map(|(name, said)| Some((name, said?))) {
                return Err(format!("line {line}: '{name}' is not for an area with a 'form'"));
            }
            let fields = said
                .fields
                .iter()
                .map(|(line, field)| Field::parse(field).map_err(|why| format!("line {line}: {why}")))
                .collect::<Result<_, String>>()?;
            let form = Form::parse(form, fields).map_err(|why| format!("line {line}: {why}"))?;
            return Ok(External::Form(FormArea { area, form, require: needs }));
        }
        if let Some((line, _)) = said.fields.first() {
            return Err(format!("line {line}: 'field' is for an area with a 'form'"));
        }

        let Some((line, tiles)) = said.tiles else {
            if let Some((line, _)) = said.aspect {
                return Err(format!("line {line}: 'aspect' is for an area with 'tiles'"));
            }
            if let Some((line, _)) = said.windows.get(1) {
                return Err(format!("line {line}: 'window' is given twice, in an area without 'tiles'"));
            }
            let (_, window) = said.windows.first().ok_or("no 'window'")?;
            let keys = said
                .keys
                .iter()
                .map(|(line, key)| {
                    let (button, rest) = key.split_once(char::is_whitespace).unwrap_or((key, ""));
                    let (id, label) = rest.trim().split_once(char::is_whitespace).unwrap_or((rest.trim(), ""));
                    if label.trim().is_empty() {
                        return Err(format!("line {line}: a key is its button, its id and then its label"));
                    }
                    let (key, id, label) = (button.to_string(), id.to_string(), label.trim().to_string());
                    Ok(AppKey { key, id, label, state: LabelState::Available })
                })
                .collect::<Result<Vec<_>, String>>()?;
            let keyboard = keyboard(said.keyboard)?;
            if let Some(fault) = keys_fault(&keys, keyboard) {
                return Err(fault);
            }
            if let (Some((line, _)), None) = (said.keys.first(), said.socket) {
                return Err(format!("line {line}: a 'key' needs a 'socket', for the program to be told of it"));
            }
            return Ok(External::Window(ExternalArea {
                area,
                window: window.to_string(),
                overlay: said.overlay.map(|(_, image)| image.to_string()),
                keyboard,
                require: needs,
                socket: said.socket.map(|(_, path)| path.to_string()),
                keys,
            }));
        };

        if let Some((line, _)) = said.overlay {
            return Err(format!("line {line}: 'overlay' is not for an area with 'tiles'"));
        }
        // Several programs' pictures at once: there is no saying which of
        // them a key would be for.
        if let Some((line, _)) = said.keyboard {
            return Err(format!("line {line}: 'keyboard' is not for an area with 'tiles'"));
        }
        let tiles = tiles
            .parse()
            .map_err(|_| format!("line {line}: 'tiles' is how many windows are shown at once, not '{tiles}'"))?;
        let aspect = match said.aspect {
            Some((line, shape)) => Aspect::parse(shape)
                .ok_or_else(|| format!("line {line}: 'aspect' is a width to a height, as 4:3 is, not '{shape}'"))?,
            None => Aspect::default(),
        };
        let windows = said
            .windows
            .iter()
            .map(|(line, window)| {
                let (label, window) = window
                    .split_once(char::is_whitespace)
                    .ok_or_else(|| format!("line {line}: a window in tiles is a label and then its name"))?;
                Ok(TileWindow { label: label.to_string(), window: window.trim().to_string() })
            })
            .collect::<Result<_, String>>()?;
        let area = TiledArea { area, tiles, aspect, windows, require: needs };
        match area.fault() {
            Some(fault) => Err(fault),
            None => Ok(External::Tiles(area)),
        }
    }
}

impl ExternalArea {
    /// Read the configuration of an area that is one window: a name and a
    /// value to a line, `#` starting a comment.
    ///
    /// ```text
    /// area    WPN
    /// window  sight-video
    /// overlay /usr/share/sight/reticle.svg    # optional
    /// keyboard no                             # optional: 'yes' if the program can be typed into
    /// require commander                       # optional: the roles any one of which will do
    /// ```
    pub fn parse(text: &str) -> Result<Self, String> {
        match External::parse(text)? {
            External::Window(area) => Ok(area),
            External::Tiles(_) => Err("'tiles' is not for an area of one window".to_string()),
            External::Views(_) => Err("'view' is not for an area of one window".to_string()),
            External::Form(_) => Err("'form' is not for an area of one window".to_string()),
        }
    }
}

impl TiledArea {
    /// Read the configuration of an area of tiles, which is one that says
    /// how many. Each window is a label - one word - and then its name.
    ///
    /// ```text
    /// area    SA
    /// tiles   4               # shown at once, at the most
    /// aspect  4:3             # optional: the shape of the pictures, 4:3 unless said
    /// window  Front  front-view
    /// window  Rear   rear-view
    /// ```
    pub fn parse(text: &str) -> Result<Self, String> {
        match External::parse(text)? {
            External::Tiles(area) => Ok(area),
            External::Window(_) => Err("no 'tiles'".to_string()),
            External::Views(_) => Err("no 'tiles'".to_string()),
            External::Form(_) => Err("no 'tiles'".to_string()),
        }
    }

    /// Why this cannot be an area of tiles, if it cannot.
    pub fn fault(&self) -> Option<String> {
        if self.tiles == 0 {
            return Some("'tiles' is how many windows are shown at once: one at the least".to_string());
        }
        if self.windows.is_empty() {
            return Some("no 'window'".to_string());
        }
        if self.windows.len() > BUTTONS {
            return Some(format!(
                "{} windows, and a button for {BUTTONS}: F1 to F{BUTTONS}",
                self.windows.len()
            ));
        }
        let (mut labels, mut names) = (Vec::new(), Vec::new());
        for window in &self.windows {
            if window.label.is_empty() || window.window.is_empty() {
                return Some("a window in tiles is a label and then its name".to_string());
            }
            for (said, what) in [(&mut labels, &window.label), (&mut names, &window.window)] {
                if said.contains(&what) {
                    return Some(format!("'{what}' is there twice"));
                }
                said.push(what);
            }
        }
        None
    }
}

impl FormArea {
    /// Read the configuration of an area that is a form, which is one that
    /// names a form. `form` is the form's name - one word: what it goes by
    /// where what the crew enters is sent - and then its title. Each `field`
    /// is its name, its kind (`text`, `number` or `decimal`), the most
    /// characters it holds, and then its title; the fields go down the page
    /// in the order they are given.
    ///
    /// ```text
    /// area    STR
    /// form    load         Load Record
    /// field   description  text    24  Description
    /// field   quantity     number   4  Quantity
    /// ```
    pub fn parse(text: &str) -> Result<Self, String> {
        match External::parse(text)? {
            External::Form(area) => Ok(area),
            External::Window(_) | External::Tiles(_) | External::Views(_) => Err("no 'form'".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_configuration_names_an_area_a_window_and_perhaps_an_overlay() {
        let text = "# The sight\narea WPN\nwindow\tsight-video   # by its application id\n\noverlay /usr/share/sight/reticle.svg\n";
        assert_eq!(
            ExternalArea::parse(text),
            Ok(ExternalArea {
                area: 1,
                window: "sight-video".to_string(),
                overlay: Some("/usr/share/sight/reticle.svg".to_string()),
                keyboard: false,
                require: Require::nothing(),
                socket: None,
                keys: Vec::new(),
            })
        );
        let map = ExternalArea::parse("window Map Viewer\narea BMS").unwrap();
        assert_eq!((map.area, map.window.as_str(), map.overlay), (7, "Map Viewer", None));
    }

    #[test]
    fn a_configuration_says_whether_a_program_can_be_typed_into() {
        let window = |text: &str| ExternalArea::parse(text).map(|area| area.keyboard);
        assert_eq!(window("area BMS\nwindow map"), Ok(false), "not unless it says so");
        assert_eq!(window("area BMS\nwindow map\nkeyboard yes"), Ok(true));
        assert_eq!(window("area BMS\nwindow map\nkeyboard no"), Ok(false));
        assert!(window("area BMS\nwindow map\nkeyboard qwerty").unwrap_err().contains("line 3: 'keyboard' is 'yes' or 'no'"));
        assert!(window("area BMS\nwindow map\nkeyboard yes\nkeyboard no").unwrap_err().contains("twice"));

        // Each view says for itself.
        let views = |text: &str| {
            ViewedArea::parse(text).map(|area| area.views.iter().map(|view| view.keyboard).collect::<Vec<_>>())
        };
        let text = "area BMS\nview Map\nwindow map\nview Pages\nkeyboard yes\nwindow pages";
        assert_eq!(views(text), Ok(vec![false, true]));
        assert!(views("area BMS\nkeyboard yes\nview Map\nwindow map").unwrap_err().contains("before the first 'view'"));
        assert!(views("area BMS\nview Map\nwindow map\nkeyboard yes\nkeyboard yes").unwrap_err().contains("twice"));

        // A page of several pictures has nowhere for a key to go, and a
        // form has a keyboard of its own.
        let error = |text: &str| External::parse(text).unwrap_err();
        assert!(error("area SA\ntiles 2\nwindow A a\nkeyboard yes").contains("not for an area with 'tiles'"));
        assert!(error("area STR\nform f F\nfield a text 4 A\nkeyboard yes").contains("not for an area with a 'form'"));
    }

    #[test]
    fn a_configuration_that_cannot_be_right_says_why() {
        let error = |text| ExternalArea::parse(text).unwrap_err();
        assert!(error("window w").contains("no 'area'"));
        assert!(error("area WPN").contains("no 'window'"));
        assert!(error("area GUN\nwindow w").contains("'GUN'"));
        assert!(error("area WPN\nwindow").contains("line 2"));
        assert!(error("area WPN\nwindow w\ncolour red").contains("'colour'"));
        assert!(error("area WPN\narea BMS\nwindow w").contains("twice"));
    }

    #[test]
    fn a_program_with_a_socket_has_buttons_down_the_sides() {
        let text = "area WPN\nwindow sight\nsocket /run/sight/hmi\nkey F1  zoom-in   Zoom In\nkey\tF12 laser Laser";
        let area = ExternalArea::parse(text).unwrap();
        assert_eq!(area.socket.as_deref(), Some("/run/sight/hmi"));
        let keys: Vec<_> = area.keys.iter().map(|k| (k.n(), k.id.as_str(), k.label.as_str(), k.state)).collect();
        assert_eq!(keys, [(1, "zoom-in", "Zoom In", LabelState::Available), (12, "laser", "Laser", LabelState::Available)]);
        // A socket alone: the program says its buttons itself.
        assert!(ExternalArea::parse("area WPN\nwindow sight\nsocket /s").unwrap().keys.is_empty());

        let error = |text: &str| External::parse(text).unwrap_err();
        assert!(error("area WPN\nwindow sight\nkey F1 a A").contains("needs a 'socket'"));
        assert!(error("area WPN\nwindow sight\nsocket /s\nkey F13 a A").contains("F1 to F12"));
        assert!(error("area WPN\nwindow sight\nsocket /s\nkey F1 a").contains("line 4: a key is"));
        assert!(error("area WPN\nwindow sight\nsocket /s\nkey F1 a A\nkey F1 b B").contains("'F1' is there twice"));
        assert!(error("area WPN\nwindow sight\nsocket /s\nkey F1 a A\nkey F2 a B").contains("'a' is there twice"));
        assert!(error("area WPN\nwindow s\nkeyboard yes\nsocket /s\nkey F5 a A").contains("keyboard"));
        assert!(error("area SA\ntiles 2\nwindow A a\nkey F1 a A").contains("'key' is not for an area with a 'tiles'"));
        assert!(error("area DEF\nview V\nwindow v\nsocket /s").contains("'socket' is not for an area with a 'view'"));
    }

    #[test]
    fn a_program_says_its_buttons_and_is_told_when_they_are_pressed() {
        let said = FromApp::parse(r#"{"keys":[{"key":"F2","id":"laser","label":"Laser","state":"selected"},{"key":"F3","id":"z","label":"Z"}]}"#);
        let Ok(FromApp::Keys(keys)) = said else { panic!("{said:?}") };
        assert_eq!((keys[0].n(), keys[0].state, keys[1].state), (2, LabelState::Selected, LabelState::Available));
        assert!(FromApp::parse(r#"{"keys":[{"key":"F2"}]}"#).is_err());
        assert_eq!(ToApp::Pressed("laser".to_string()).line(), r#"{"pressed":"laser"}"#);
        assert_eq!(ToApp::Released("laser".to_string()).line(), r#"{"released":"laser"}"#);
    }

    fn window(label: &str, window: &str) -> TileWindow {
        TileWindow { label: label.to_string(), window: window.to_string() }
    }

    #[test]
    fn a_configuration_of_tiles_says_how_many_and_names_each_window_with_its_label() {
        let text = "# What is round the vehicle\narea SA\ntiles 4   # at once\naspect 16:9\n\n\
                    window Front  front-view\nwindow\tRear\tRear View 2  # a name can have spaces\n";
        assert_eq!(
            TiledArea::parse(text),
            Ok(TiledArea {
                area: 0,
                tiles: 4,
                aspect: Aspect { width: 16, height: 9 },
                windows: vec![window("Front", "front-view"), window("Rear", "Rear View 2")],
                require: Require::nothing(),
            })
        );
        // The pictures are 4:3 unless it says, and the lines come in any order.
        let area = TiledArea::parse("window One w1\ntiles 1\narea DRV").unwrap();
        assert_eq!((area.area, area.tiles, area.aspect), (4, 1, Aspect { width: 4, height: 3 }));
    }

    #[test]
    fn a_configuration_is_of_tiles_if_it_says_how_many() {
        assert!(matches!(External::parse("area WPN\nwindow sight"), Ok(External::Window(_))));
        assert!(matches!(External::parse("area SA\ntiles 2\nwindow Front front"), Ok(External::Tiles(_))));
        // Each kind has what the other does not.
        let error = |text| External::parse(text).unwrap_err();
        assert!(error("area WPN\nwindow sight\nwindow other").contains("line 3: 'window' is given twice"));
        assert!(error("area WPN\nwindow sight\naspect 4:3").contains("line 3: 'aspect'"));
        assert!(error("area SA\ntiles 2\nwindow Front front\noverlay /x.svg").contains("line 4: 'overlay'"));
        assert!(ExternalArea::parse("area SA\ntiles 2\nwindow Front front").unwrap_err().contains("'tiles'"));
        assert!(TiledArea::parse("area WPN\nwindow sight").unwrap_err().contains("'tiles'"));
    }

    #[test]
    fn a_configuration_of_tiles_that_cannot_be_right_says_why() {
        let error = |text: &str| TiledArea::parse(text).unwrap_err();
        assert!(error("tiles 2\nwindow Front front").contains("no 'area'"));
        assert!(error("area SA\ntiles 2").contains("no 'window'"));
        assert!(error("area SA\ntiles 0\nwindow Front front").contains("one at the least"));
        assert!(error("area SA\ntiles many\nwindow Front front").contains("line 2: 'tiles'"));
        assert!(error("area SA\ntiles 2\ntiles 3\nwindow Front front").contains("twice"));
        assert!(error("area SA\ntiles 2\naspect wide\nwindow Front front").contains("line 3: 'aspect'"));
        assert!(error("area SA\ntiles 2\nwindow front").contains("line 3: a window in tiles"));
        assert!(error("area SA\ntiles 2\nwindow Front front\nwindow Front other").contains("'Front' is there twice"));
        assert!(error("area SA\ntiles 2\nwindow Front view\nwindow Rear view").contains("'view' is there twice"));

        // A button to a window, and there are twelve.
        let windows = |count| (1..=count).map(|n| format!("window W{n} w{n}\n")).collect::<String>();
        assert!(TiledArea::parse(&format!("area SA\ntiles 4\n{}", windows(12))).is_ok());
        assert!(error(&format!("area SA\ntiles 4\n{}", windows(13))).contains("13 windows"));
    }

    #[test]
    fn a_configuration_of_a_form_names_it_and_its_fields() {
        use hmi_entry::Kind;
        let text = "# What is being carried\narea STR\nform load  Load Record   # by this name, down the feed\n\n\
                    field description text 24 Description\nfield\tfrequency\tdecimal 7 Frequency (MHz)\n";
        let area = FormArea::parse(text).unwrap();
        assert_eq!((area.area, area.form.name.as_str(), area.form.title.as_str()), (5, "load", "Load Record"));
        let field = |field: &Field| (field.name.clone(), field.kind, field.length, field.title.clone());
        let fields: Vec<_> = area.form.fields.iter().map(field).collect();
        let field = |name: &str, kind, length, title: &str| (name.to_string(), kind, length, title.to_string());
        assert_eq!(
            fields,
            [field("description", Kind::Text, 24, "Description"), field("frequency", Kind::Decimal, 7, "Frequency (MHz)")]
        );
        // It is of a form because it names one.
        assert!(matches!(External::parse(text), Ok(External::Form(_))));
    }

    #[test]
    fn a_configuration_of_a_form_that_cannot_be_right_says_why() {
        let error = |text: &str| External::parse(text).unwrap_err();
        let form = "area STR\nform load Load Record\nfield a text 4 A";
        assert!(External::parse(form).is_ok());
        assert!(error("form load Load Record\nfield a text 4 A").contains("no 'area'"));
        assert!(error("area STR\nform load Load Record").contains("line 2: no 'field'"));
        assert!(error("area STR\nform load\nfield a text 4 A").contains("line 2: a form is its name"));
        assert!(error(&format!("{form}\nform other Other")).contains("line 4: 'form' is given twice"));
        assert!(error("area STR\nform load Load Record\nfield a words 4 A").contains("line 3: a field's kind"));
        assert!(error(&format!("{form}\nfield a number 2 B")).contains("line 2: 'a' is there twice"));

        // A form has nothing of a program's, and only a form has fields.
        assert!(error(&format!("{form}\nwindow w")).contains("line 4: 'window' is not for an area with a 'form'"));
        assert!(error(&format!("{form}\ntiles 2")).contains("line 4: 'tiles' is not for an area with a 'form'"));
        assert!(error(&format!("{form}\noverlay /x.svg")).contains("line 4: 'overlay'"));
        assert!(error("area STR\nwindow w\nfield a text 4 A").contains("line 3: 'field' is for an area with a 'form'"));
        assert!(ExternalArea::parse(form).unwrap_err().contains("'form'"));
        assert!(TiledArea::parse(form).unwrap_err().contains("'tiles'"));
        assert!(FormArea::parse("area WPN\nwindow sight").unwrap_err().contains("no 'form'"));
    }
}
