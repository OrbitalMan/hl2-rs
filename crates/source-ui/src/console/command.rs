//! Console commands and cvars as an app describes them, once each: name, help text (`name args:
//! description`, the usage line `help` prints and the completion help line marks the argument
//! being typed in), what each argument completes from, the handler that runs it and, for a
//! cvar, its current value and how a typed value is stored. The app owns the table and the
//! context the handlers run on ([`Context`]).

use std::fmt;

/// The app a console's commands run on.
pub trait Context: Sized + 'static {
    /// What a command hands back to the app.
    type Effect;
    /// The lists of names the app supplies for argument completion ([`Arg::Names`]).
    type Names: Copy + PartialEq + fmt::Debug + 'static;
}

/// What an argument completes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arg<N> {
    /// One of the app's lists of names.
    Names(N),
    /// An implemented command or cvar name.
    Command,
    /// One of these literal values.
    Values(&'static [&'static str]),
    /// Free text or a number: nothing to complete.
    Free,
}

/// What a command returns: an effect for the app, or the error line the console prints.
pub type Outcome<E> = Result<Option<E>, String>;

/// Runs a command with its arguments (the words after its name).
pub type Handler<C> = fn(&mut C, &'static Spec<C>, &[String]) -> Outcome<<C as Context>::Effect>;

/// A cvar's current value, as the console prints it (`None` while unset).
pub type Value<C> = fn(&C) -> Option<String>;

/// Stores a typed value, or says why it is not one the cvar takes.
pub type Set<C> = fn(&mut C, &str) -> Result<(), Invalid>;

/// An implemented command or cvar.
pub struct Spec<C: Context> {
    pub name: &'static str,
    /// `name args: description`, as `help` prints it.
    pub help: &'static str,
    /// Completion for each argument, in order.
    pub args: &'static [Arg<C::Names>],
    pub run: Handler<C>,
    /// A cvar's current value: completion shows it.
    pub value: Option<Value<C>>,
    /// A cvar the app's own handler stores typed values in with this setter.
    pub setting: Option<Setting<C>>,
}

impl<C: Context> Clone for Spec<C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C: Context> Copy for Spec<C> {}

impl<C: Context> fmt::Debug for Spec<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Spec")
            .field("name", &self.name)
            .field("help", &self.help)
            .field("args", &self.args)
            .field("run", &self.run)
            .field("value", &self.value)
            .field("setting", &self.setting)
            .finish()
    }
}

impl<C: Context> Spec<C> {
    /// A cvar: completion shows its current value.
    pub fn is_cvar(&self) -> bool {
        self.value.is_some()
    }

    /// The error line for wrong arguments: the help text.
    pub fn usage(&self) -> String {
        self.help.to_string()
    }
}

/// How a cvar stores a typed value.
pub struct Setting<C> {
    pub set: Set<C>,
    /// The name the console prints for it, when an alias answers as what it aliases.
    pub prints_as: Option<&'static str>,
}

impl<C> Clone for Setting<C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C> Copy for Setting<C> {}

impl<C> fmt::Debug for Setting<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Setting")
            .field("set", &self.set)
            .field("prints_as", &self.prints_as)
            .finish()
    }
}

/// Why a typed value was not stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Invalid {
    /// The console prints the cvar's usage line.
    Usage,
    /// The console prints the cvar's own message.
    Message(&'static str),
}

/// A number or other value parsed from a typed argument.
pub fn parse<T: std::str::FromStr>(arg: &str) -> Result<T, Invalid> {
    arg.parse().map_err(|_| Invalid::Usage)
}

/// `0` or `1`.
pub fn flag(arg: &str) -> Result<bool, Invalid> {
    boolean(arg).ok_or(Invalid::Usage)
}

/// `0` is false, `1` true.
pub fn boolean(value: &str) -> Option<bool> {
    match value {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

/// The implemented command or cvar named `name` (case-insensitive).
pub fn find<C: Context>(commands: &'static [Spec<C>], name: &str) -> Option<&'static Spec<C>> {
    commands.iter().find(|s| s.name.eq_ignore_ascii_case(name))
}

/// A help text's argument words (after the name, before the `:`) and its description.
pub fn usage<C: Context>(spec: &Spec<C>) -> (Vec<String>, String) {
    let (head, description) = spec.help.split_once(": ").unwrap_or((spec.help, ""));
    let args = head.split_whitespace().skip(1).map(str::to_owned).collect();
    (args, description.to_owned())
}

/// Runs one command (its name, then its arguments) through its [`Spec`] handler.
pub fn run<C: Context>(
    context: &mut C,
    commands: &'static [Spec<C>],
    tokens: &[String],
) -> Outcome<C::Effect> {
    let Some(spec) = find(commands, &tokens[0]) else {
        return Err(format!("Unknown or unimplemented command: {}", tokens[0]));
    };
    (spec.run)(context, spec, &tokens[1..])
}

/// Semicolons outside quotes separate commands; quoted arguments preserve spaces and separators.
pub fn tokenize(line: &str) -> Result<Vec<Vec<String>>, String> {
    let mut commands = Vec::new();
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quoted = false;
    let mut started = false;
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            ';' if !quoted => {
                if started {
                    tokens.push(std::mem::take(&mut token));
                    started = false;
                }
                if !tokens.is_empty() {
                    commands.push(std::mem::take(&mut tokens));
                }
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    tokens.push(std::mem::take(&mut token));
                    started = false;
                }
            }
            _ => {
                token.push(c);
                started = true;
            }
        }
    }
    if quoted {
        return Err("Unterminated quoted argument; no commands executed".into());
    }
    if started {
        tokens.push(token);
    }
    if !tokens.is_empty() {
        commands.push(tokens);
    }
    Ok(commands)
}
