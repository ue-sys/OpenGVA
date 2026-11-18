//! Roles, and what needs them: `/etc/opengva/access.conf`.

/// The roles any one of which will do for something: what its `require`
/// says. With none, nothing is required, and whoever is there may have it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Require(Vec<String>);

impl Require {
    /// Nothing is required.
    pub fn nothing() -> Self {
        Require::default()
    }

    /// Read what a `require` says: roles, with spaces between them. Any one
    /// of them will do.
    ///
    /// ```text
    /// require commander
    /// require commander gunner
    /// ```
    pub fn parse(roles: &str) -> Result<Self, String> {
        let mut require = Require::nothing();
        for role in roles.split_whitespace() {
            if !is_a_name(role) {
                return Err(format!(
                    "'{role}' cannot be a role: a role is a word of small letters, digits, '-' and '_', a letter first"
                ));
            }
            if !require.0.iter().any(|said| said == role) {
                require.0.push(role.to_string());
            }
        }
        if require.is_nothing() { Err("no role".to_string()) } else { Ok(require) }
    }

    /// Take another `require`'s roles as well: any one of all of them will
    /// do.
    pub fn or(&mut self, other: Require) {
        for role in other.0 {
            if !self.0.contains(&role) {
                self.0.push(role);
            }
        }
    }

    pub fn is_nothing(&self) -> bool {
        self.0.is_empty()
    }

    pub fn roles(&self) -> &[String] {
        &self.0
    }

    /// Whether whoever has `roles` has what is required.
    pub fn met_by(&self, roles: &[String]) -> bool {
        self.is_nothing() || self.0.iter().any(|role| roles.contains(role))
    }
}

/// Whether a word can be a role's name, or an account's: what a Unix group
/// or user can be called.
fn is_a_name(word: &str) -> bool {
    let mut characters = word.chars();
    characters.next().is_some_and(|first| first.is_ascii_lowercase())
        && word.len() <= 32
        && characters.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// How long the display is left alone before whoever was logged in no
/// longer is, unless the configuration says: fifteen minutes.
pub const IDLE_MS: u64 = 15 * 60 * 1000;

/// What is never asked about, by the names a configuration might try to
/// give a role to: the Alarms page and the Threats page, the acknowledging
/// of an alert, the hiding and showing of labels, and the SYS area - where
/// logging out is. They are what the display owes to whoever is looking at
/// it, with no exception made for anyone.
pub const NEVER: [&str; 5] = ["alarms", "threats", "acknowledge", "labels", "sys"];

/// A role, and the accounts that hold it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Role {
    name: String,
    accounts: Vec<String>,
}

/// Who may have what at this crew station: its roles and who holds them,
/// which of the HMI's own functions need one, and the idle time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    idle_ms: u64,
    roles: Vec<Role>,
    functions: Vec<(String, Require)>,
}

/// With no configuration there are no roles and nothing needs one.
impl Default for Config {
    fn default() -> Self {
        Config { idle_ms: IDLE_MS, roles: Vec::new(), functions: Vec::new() }
    }
}

impl Config {
    /// Read a configuration: a name and a value to a line, `#` starting a
    /// comment.
    ///
    /// ```text
    /// role      commander                     # a role
    /// role      maintainer
    /// idle      15m                           # optional: s, m or h
    /// function  alarms.override  commander    # a function, and the roles any one of which may use it
    /// ```
    ///
    /// `functions` are the HMI's own functions that can be given roles, by
    /// the names it has for them; anything else on a `function` line is
    /// refused, and what is in [`NEVER`] is refused by name. `members` says
    /// which accounts hold a role - on the image, the members of the Unix
    /// group of that name - and a role nobody holds is one nobody can prove.
    pub fn read(text: &str, functions: &[&str], members: impl Fn(&str) -> Vec<String>) -> Result<Self, String> {
        let mut config = Config::default();
        let mut idle = None;
        let mut asked: Vec<(usize, &str)> = Vec::new();
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
            match name {
                "role" => {
                    if !is_a_name(value) {
                        let why = Require::parse(value).err().unwrap_or_else(|| "a role is one word".to_string());
                        return Err(format!("line {number}: {why}"));
                    }
                    if config.roles.iter().any(|role| role.name == value) {
                        return Err(format!("line {number}: the role '{value}' is given twice"));
                    }
                    config.roles.push(Role { name: value.to_string(), accounts: members(value) });
                }
                "idle" => {
                    let time = milliseconds(value).ok_or_else(|| {
                        format!("line {number}: 'idle' is a time, as 15m is - in s, m or h - not '{value}'")
                    })?;
                    if idle.replace(time).is_some() {
                        return Err(format!("line {number}: 'idle' is given twice"));
                    }
                }
                "function" => {
                    let (function, roles) = value.split_once(char::is_whitespace).unwrap_or((value, ""));
                    if NEVER.contains(&function) {
                        return Err(format!(
                            "line {number}: '{function}' is never asked about: alerts, acknowledging them, the labels and SYS are for whoever is there"
                        ));
                    }
                    if !functions.contains(&function) {
                        return Err(format!(
                            "line {number}: '{function}' is not a function that can need a role: those are {}",
                            functions.join(", ")
                        ));
                    }
                    let require = Require::parse(roles).map_err(|why| format!("line {number}: {why}"))?;
                    match config.functions.iter_mut().find(|(said, _)| said == function) {
                        Some((_, said)) => said.or(require),
                        None => config.functions.push((function.to_string(), require)),
                    }
                    asked.push((number, function));
                }
                _ => return Err(format!("line {number}: '{name}' is not something access has")),
            }
        }
        config.idle_ms = idle.unwrap_or(IDLE_MS);
        // A function can be given a role that a later line names: it is
        // only now known whether every role asked for is one there is.
        for (number, function) in asked {
            config.knows(config.function(function)).map_err(|why| format!("line {number}: {why}"))?;
        }
        Ok(config)
    }

    /// The configuration to go by when the one there is cannot be read or
    /// cannot be right: no roles, so that whatever asks for one is a mistake
    /// and is left out; and for each of `functions`, a role that there is
    /// not, so that nobody can have it. What a configuration kept for some
    /// is not opened to all by the configuration's being broken.
    pub fn shut(functions: &[&str]) -> Self {
        // No role can be called this: it is not a name.
        let nobody = Require(vec!["-".to_string()]);
        Config {
            idle_ms: IDLE_MS,
            roles: Vec::new(),
            functions: functions.iter().map(|function| (function.to_string(), nobody.clone())).collect(),
        }
    }

    /// Whether this crew station has roles at all. Where it has none there
    /// is nobody to log in, and nothing to say about who has.
    pub fn has_roles(&self) -> bool {
        !self.roles.is_empty()
    }

    /// How long the display is left alone before whoever was logged in no
    /// longer is.
    pub fn idle_ms(&self) -> u64 {
        self.idle_ms
    }

    /// Whether every role a `require` names is one of this configuration's,
    /// and if not, which is not. Something that asks for a role there is not
    /// is not to be had by leaving the role out: it is a mistake to be
    /// reported, and what made it left out.
    pub fn knows(&self, require: &Require) -> Result<(), String> {
        match require.roles().iter().find(|role| !self.roles.iter().any(|known| known.name == **role)) {
            Some(role) => Err(format!("there is no role '{role}'")),
            None => Ok(()),
        }
    }

    /// What one of the HMI's own functions needs: nothing, unless the
    /// configuration says.
    pub fn function(&self, name: &str) -> &Require {
        const NOTHING: &Require = &Require(Vec::new());
        self.functions.iter().find(|(said, _)| said == name).map_or(NOTHING, |(_, require)| require)
    }

    /// The accounts whose PIN would prove a role that is required: those of
    /// the first role it names, then of the next, each once.
    pub fn accounts(&self, require: &Require) -> Vec<String> {
        let mut accounts: Vec<String> = Vec::new();
        for wanted in require.roles() {
            for role in self.roles.iter().filter(|role| role.name == *wanted) {
                for account in &role.accounts {
                    if !accounts.contains(account) {
                        accounts.push(account.clone());
                    }
                }
            }
        }
        accounts
    }

    /// The roles an account holds, in the configuration's order.
    pub fn roles_of(&self, account: &str) -> Vec<String> {
        self.roles
            .iter()
            .filter(|role| role.accounts.iter().any(|held| held == account))
            .map(|role| role.name.clone())
            .collect()
    }
}

/// A time as a configuration writes it - `90s`, `15m`, `1h` - in
/// milliseconds. `None` if it is not one, or is no time at all.
fn milliseconds(time: &str) -> Option<u64> {
    let unit = time.chars().last()?;
    let each = match unit {
        's' => 1_000,
        'm' => 60_000,
        'h' => 3_600_000,
        _ => return None,
    };
    let count: u64 = time[..time.len() - 1].parse().ok()?;
    count.checked_mul(each).filter(|&ms| ms > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FUNCTIONS: [&str; 2] = ["alarms.override", "threats.cancel"];

    /// Who holds what, where the tests are: one commander, two gunners, and
    /// a role that nobody holds.
    pub(crate) fn members(role: &str) -> Vec<String> {
        let held: &[&str] = match role {
            "commander" => &["cmdr"],
            "gunner" => &["smith", "jones"],
            "crew" => &["cmdr", "smith", "jones"],
            _ => &[],
        };
        held.iter().map(|account| account.to_string()).collect()
    }

    fn read(text: &str) -> Result<Config, String> {
        Config::read(text, &FUNCTIONS, members)
    }

    fn require(roles: &str) -> Require {
        Require::parse(roles).unwrap()
    }

    #[test]
    fn a_requirement_is_met_by_any_one_of_its_roles_and_nothing_required_by_anyone() {
        let either = require("commander gunner");
        let roles = |held: &[&str]| held.iter().map(|role| role.to_string()).collect::<Vec<_>>();
        assert!(either.met_by(&roles(&["gunner"])));
        assert!(either.met_by(&roles(&["crew", "commander"])));
        assert!(!either.met_by(&roles(&["crew"])));
        assert!(!either.met_by(&[]));
        assert!(Require::nothing().met_by(&[]));

        // A second line's roles are as good as the first's.
        let mut more = require("commander");
        more.or(require("gunner commander"));
        assert_eq!(more, either);
    }

    #[test]
    fn a_requirement_that_cannot_be_right_says_why() {
        assert!(Require::parse("").unwrap_err().contains("no role"));
        assert!(Require::parse("Commander").unwrap_err().contains("'Commander'"));
        assert!(Require::parse("crew 2ic").unwrap_err().contains("'2ic'"));
        assert!(Require::parse("a,b").unwrap_err().contains("'a,b'"));
    }

    #[test]
    fn a_configuration_names_roles_an_idle_time_and_what_functions_need() {
        let text = "# Who may have what\nrole commander   # the vehicle's\nrole\tgunner\n\nidle 5m\n\
                    function alarms.override commander\nfunction threats.cancel commander gunner\n";
        let config = read(text).unwrap();
        assert_eq!(config.idle_ms(), 300_000);
        assert_eq!(config.function("alarms.override"), &require("commander"));
        assert_eq!(config.function("threats.cancel"), &require("commander gunner"));
        assert_eq!(config.accounts(&require("commander")), ["cmdr"]);
        assert_eq!(config.accounts(&require("gunner commander")), ["smith", "jones", "cmdr"]);
        assert_eq!(config.roles_of("smith"), ["gunner"]);
        assert!(config.roles_of("nobody").is_empty());
        assert_eq!(config.knows(&require("gunner")), Ok(()));
        assert!(config.knows(&require("gunner driver")).unwrap_err().contains("'driver'"));

        // The lines come in any order, and a function's roles on two lines
        // are all of them.
        let config = read("function alarms.override gunner\nfunction alarms.override commander\nrole gunner\nrole commander").unwrap();
        assert_eq!(config.function("alarms.override"), &require("gunner commander"));
        assert_eq!(config.idle_ms(), IDLE_MS, "fifteen minutes unless it says");
        assert_eq!(read("idle 90s").unwrap().idle_ms(), 90_000);
        assert_eq!(read("idle 1h").unwrap().idle_ms(), 3_600_000);
    }

    #[test]
    fn with_no_configuration_nothing_needs_a_role() {
        for config in [Config::default(), read("").unwrap(), read("# nothing\n").unwrap()] {
            assert!(config.function("alarms.override").is_nothing());
            assert_eq!(config.idle_ms(), IDLE_MS);
            assert!(config.knows(&Require::nothing()).is_ok());
        }
    }

    #[test]
    fn what_the_display_owes_to_whoever_is_looking_cannot_be_given_a_role() {
        for never in NEVER {
            let why = read(&format!("role commander\nfunction {never} commander")).unwrap_err();
            assert!(why.contains("line 2") && why.contains("never asked about"), "{never}: {why}");
        }
    }

    #[test]
    fn a_configuration_that_cannot_be_right_says_why() {
        let why = |text| read(text).unwrap_err();
        assert!(why("role").contains("line 1: 'role' has no value"));
        assert!(why("role Commander").contains("line 1: 'Commander'"));
        assert!(why("role vehicle commander").contains("line 1"));
        assert!(why("role crew\nrole crew").contains("line 2: the role 'crew' is given twice"));
        assert!(why("idle soon").contains("line 1: 'idle' is a time"));
        assert!(why("idle 0m").contains("'idle' is a time"));
        assert!(why("idle 15").contains("'idle' is a time"));
        assert!(why("idle 5m\nidle 6m").contains("line 2: 'idle' is given twice"));
        assert!(why("colour red").contains("line 1: 'colour'"));
        assert!(why("function alarms.override").contains("line 1: no role"));
        assert!(why("role crew\nfunction reboot crew").contains("line 2: 'reboot' is not a function"));
        assert!(why("role crew\nfunction reboot crew").contains("alarms.override, threats.cancel"));
        // A role that no line names is a mistake, not a role nobody need have.
        assert!(why("role crew\nfunction alarms.override commander").contains("line 2: there is no role 'commander'"));
    }

    #[test]
    fn a_configuration_that_cannot_be_read_shuts_what_it_might_have_kept_for_some() {
        let shut = Config::shut(&FUNCTIONS);
        for function in FUNCTIONS {
            let needs = shut.function(function);
            assert!(!needs.is_nothing() && !needs.met_by(&["commander".to_string()]));
            assert!(shut.accounts(needs).is_empty(), "no account's PIN will do for {function}");
        }
        // And an area that asks for a role is a mistake, to be left out.
        assert!(shut.knows(&require("commander")).is_err());
        assert!(shut.knows(&Require::nothing()).is_ok(), "what asks for nothing is as it ever was");
    }

    #[test]
    fn a_role_nobody_holds_is_a_role_all_the_same() {
        let config = read("role maintainer\nfunction alarms.override maintainer").unwrap();
        assert!(config.knows(&require("maintainer")).is_ok());
        assert!(config.accounts(&require("maintainer")).is_empty());
    }
}
