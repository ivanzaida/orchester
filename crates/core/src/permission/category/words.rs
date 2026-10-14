//! Splitting a command line into its simple commands (`docs/design/orchester-workspace.md`, "Command
//! categories and approval policy"): parts joined by `&&`, `||`, `;`, `|`, `&` or a new line, each with
//! its words and the files its redirections write or read. Bash quoting by default; PowerShell's
//! (a backslash is a plain character) for the script a `pwsh -Command` runs. Nothing is guessed: a
//! variable, a substitution, a subshell or group, a here-document, a brace expansion or an unclosed
//! quote leaves the part unread, and it is then classified as Unknown. The one variable read is one the
//! line set to a plain literal before ([`Variables`]): its uses are its value. The status an earlier
//! command left (`$?`, `${PIPESTATUS[0]}`, PowerShell's `$?` and `$LASTEXITCODE`) is a number or a
//! boolean whatever ran: it is kept as written and the part marked ([`RawPart::status`]), which only a
//! program that prints it may hold.

use super::{
  heredoc::Heredoc,
  variables::{Variables, is_assignment, name_char},
};

/// The longest status a `$` may start: `{PIPESTATUS[999]}`.
const STATUS_LONGEST: usize = 17;

/// The length of the bash status `ahead` starts with (after its `$`): `?`, `{?}` or `{PIPESTATUS[N]}`
/// with one to three digits.
fn bash_status(ahead: &str) -> Option<usize> {
  if ahead.starts_with("{?}") {
    return Some(3);
  }
  if ahead.starts_with('?') {
    return Some(1);
  }
  let index = ahead.strip_prefix("{PIPESTATUS[")?;
  let digits = index.chars().take_while(char::is_ascii_digit).count();
  ((1..=3).contains(&digits) && index[digits..].starts_with("]}")).then_some("{PIPESTATUS[".len() + digits + 2)
}

/// The length of the PowerShell status `ahead` starts with (after its `$`): `?` or `LASTEXITCODE`, without
/// case, with no name character, `?` or `:` (a drive or scope) after it.
fn powershell_status(ahead: &str) -> Option<usize> {
  ["?", "lastexitcode"].iter().find_map(|name| {
    let (head, rest) = (ahead.get(..name.len())?, ahead.get(name.len()..)?);
    let ends = rest
      .chars()
      .next()
      .is_none_or(|next| !(name_char(next) || matches!(next, '?' | ':')));
    (head.eq_ignore_ascii_case(name) && ends).then_some(name.len())
  })
}

/// Whether a redirection's word names a file descriptor (`2`, `-`, `2-`) rather than a file.
fn is_descriptor(word: &str) -> bool {
  let number = word.strip_suffix('-').unwrap_or(word);
  word == "-" || (!number.is_empty() && number.chars().all(|c| c.is_ascii_digit()))
}

/// The values of the shell's `HOME` and `USERPROFILE`; `None` when unset there.
#[derive(Clone, Debug, Default)]
pub(super) struct ShellHomes {
  pub(super) home: Option<String>,
  pub(super) profile: Option<String>,
}

/// The names of the home folder's variables.
const HOME_NAMES: [&str; 2] = ["HOME", "USERPROFILE"];

/// Whether `command` names a home variable other than to use it (`$HOME`, `${HOME}`).
fn sets_home(command: &str) -> bool {
  HOME_NAMES.iter().any(|name| {
    command.match_indices(name).any(|(at, _)| {
      let before = &command[..at];
      !(before.ends_with('$') || before.ends_with("${"))
    })
  })
}

/// Whose quoting rules a command line follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Quoting {
  /// Bash: a backslash escapes the next character outside single quotes.
  Bash,
  /// PowerShell: a backslash is a path separator; a backtick escapes, which is left unread.
  PowerShell,
}

/// One simple command as written.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct RawPart {
  pub(super) words: Vec<String>,
  /// Files its output redirections write (`> out.txt`, `2>> log`, `&> all`).
  pub(super) writes: Vec<String>,
  /// Files its input redirections read (`< in.txt`).
  pub(super) reads: Vec<String>,
  /// Something in it cannot be read to the end without running it.
  pub(super) unread: bool,
  /// It runs in the background (`&`), on after the line ends.
  pub(super) background: bool,
  /// It only sets a variable to a literal, whose later uses were read as its value.
  pub(super) assigns: bool,
  /// It reads a here-document, data that follows the line.
  pub(super) heredoc: bool,
  /// Its first word has a quote or an escape before its first `=`: no assignment, whatever it reads.
  pub(super) quoted_name: bool,
  /// A word of it holds the status an earlier command left; never a redirection's file, which is then
  /// not read.
  pub(super) status: bool,
}

impl RawPart {
  fn is_empty(&self) -> bool {
    self.words.is_empty() && self.writes.is_empty() && self.reads.is_empty() && !self.unread && !self.heredoc
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Redirect {
  Write,
  Read,
  /// `>&word` or `<&word`: a copy of a file descriptor when the word is one (`2`, `-`, `2-`), else the
  /// file it names, written (both outputs) or read.
  Duplicate {
    writes: bool,
  },
}

/// What starts or ends a part.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Joiner {
  /// The start or the end of the line.
  Edge,
  /// `;` or a new line.
  Sequence,
  /// `&&` or `||`: the part after it may not run.
  Condition,
  /// `|` or `|&`: both sides run in subshells of their own.
  Pipe,
  /// A lone `&`: the part before it runs in a subshell in the background.
  Background,
}

/// The parts being collected.
struct Splitter {
  /// What `$HOME` and `$USERPROFILE` are read as, when the line sets neither.
  homes: ShellHomes,
  quoting: Quoting,
  parts: Vec<RawPart>,
  part: RawPart,
  word: Option<String>,
  redirect: Option<Redirect>,
  variables: Variables,
  /// What started the part being collected.
  opened_by: Joiner,
  /// The word being collected has a quote or an escape before its first `=`.
  name_quoted: bool,
  /// The word being collected holds the status an earlier command left.
  status_word: bool,
  /// Here-documents whose bodies follow the current line, with the number of the part that reads each.
  heredocs: Vec<(usize, Heredoc)>,
}

type Chars<'a> = std::iter::Peekable<std::str::Chars<'a>>;

/// The simple commands of `command`, in order.
///
/// `home` is the home folder the agent's CLI was started with, which `$HOME` and `$USERPROFILE` (also
/// braced) are read as, unless the line may set either: any `HOME` or `USERPROFILE` in it that is not
/// such a use (`HOME=/x`, `export HOME`, `read HOME`) leaves them unread.
pub(super) fn split(command: &str, quoting: Quoting, homes: ShellHomes) -> Vec<RawPart> {
  let homes = if sets_home(command) {
    ShellHomes::default()
  } else {
    homes
  };
  let mut splitter = Splitter {
    homes,
    quoting,
    parts: Vec::new(),
    part: RawPart::default(),
    word: None,
    redirect: None,
    variables: Variables::default(),
    opened_by: Joiner::Edge,
    name_quoted: false,
    status_word: false,
    heredocs: Vec::new(),
  };
  let mut chars = command.chars().peekable();
  while let Some(c) = chars.next() {
    splitter.take(c, &mut chars);
  }
  splitter.end_part(Joiner::Edge);
  // A body that never came is not read.
  for (at, _) in std::mem::take(&mut splitter.heredocs) {
    splitter.parts[at].unread = true;
  }
  splitter.parts
}

impl Splitter {
  fn take(&mut self, c: char, chars: &mut Chars<'_>) {
    match c {
      // Bash keeps a carriage return in the word.
      ' ' | '\t' => self.end_word(),
      '\n' => {
        self.end_part(Joiner::Sequence);
        self.bodies(chars);
      }
      ';' => self.end_part(Joiner::Sequence),
      '&' => self.ampersand(chars),
      '|' => {
        let joiner = match chars.next_if(|next| matches!(next, '|' | '&')) {
          Some('|') => Joiner::Condition,
          _ => Joiner::Pipe,
        };
        self.end_part(joiner);
      }
      '>' => self.output(chars),
      '<' => self.input(chars),
      '\'' => self.single_quoted(chars),
      '"' => self.double_quoted(chars),
      '\\' if self.quoting == Quoting::Bash => match chars.next() {
        // A line continuation joins the lines.
        Some('\n') | None => {}
        Some(escaped) => {
          self.quoting_name();
          self.push(escaped);
        }
      },
      '$' => self.dollar(chars, false),
      '`' | '(' | ')' | '{' | '}' => self.part.unread = true,
      '#' if self.word.is_none() => while chars.next_if(|next| *next != '\n').is_some() {},
      other => self.push(other),
    }
  }

  fn push(&mut self, c: char) {
    self.word.get_or_insert_with(String::new).push(c);
  }

  /// Notes a quote or an escape in the word being collected, before its first `=` when it has none yet.
  fn quoting_name(&mut self) {
    if !self.word.as_deref().is_some_and(|word| word.contains('=')) {
      self.name_quoted = true;
    }
  }

  fn end_word(&mut self) {
    let quoted = std::mem::take(&mut self.name_quoted);
    let status = std::mem::take(&mut self.status_word);
    let Some(word) = self.word.take() else {
      return;
    };
    if status {
      // A file named by a status is no file the words name.
      self.part.unread |= self.redirect.is_some();
      self.part.status = true;
    }
    match self.redirect.take() {
      Some(Redirect::Write) => self.part.writes.push(word),
      Some(Redirect::Read) => self.part.reads.push(word),
      Some(Redirect::Duplicate { .. }) if is_descriptor(&word) => {}
      Some(Redirect::Duplicate { writes: true }) => self.part.writes.push(word),
      Some(Redirect::Duplicate { writes: false }) => self.part.reads.push(word),
      None => {
        if self.part.words.is_empty() {
          self.part.quoted_name = quoted;
        }
        self.part.words.push(word);
      }
    }
  }

  fn end_part(&mut self, closed_by: Joiner) {
    self.end_word();
    // A redirection without its file is not read to the end.
    if self.redirect.take().is_some() {
      self.part.unread = true;
    }
    let mut part = std::mem::take(&mut self.part);
    part.background = closed_by == Joiner::Background;
    if let [word] = part.words.as_slice()
      && is_assignment(word)
      && !part.quoted_name
    {
      // Only an assignment that surely runs in the shell itself sets what later parts read.
      let in_shell = matches!(self.opened_by, Joiner::Edge | Joiner::Sequence | Joiner::Background)
        && !matches!(closed_by, Joiner::Pipe | Joiner::Background);
      let alone = part.writes.is_empty() && part.reads.is_empty() && !part.unread;
      if in_shell && alone && self.variables.assign(word) {
        part.assigns = true;
      } else {
        self.variables.forget(word);
      }
    }
    self.opened_by = closed_by;
    if !part.is_empty() {
      self.parts.push(part);
    }
  }

  /// `&&` and a lone `&` end the part; `&>` and `&>>` redirect both outputs.
  fn ampersand(&mut self, chars: &mut Chars<'_>) {
    if chars.next_if_eq(&'>').is_some() {
      chars.next_if_eq(&'>');
      self.end_word();
      self.redirect = Some(Redirect::Write);
      return;
    }
    let joiner = if chars.next_if_eq(&'&').is_some() {
      Joiner::Condition
    } else {
      Joiner::Background
    };
    self.end_part(joiner);
  }

  /// `>`, `>>`, `N>`; `>&N` and `N>&M` only join outputs, while `>&file` writes the file. A `>|` or a
  /// `>(` is not read: the `|` or `(` that follows leaves the part unread.
  fn output(&mut self, chars: &mut Chars<'_>) {
    self.descriptor_number();
    self.end_word();
    chars.next_if_eq(&'>');
    self.redirect = Some(if chars.next_if_eq(&'&').is_some() {
      Redirect::Duplicate { writes: true }
    } else {
      Redirect::Write
    });
  }

  /// `<` reads a file, `<&N` copies a descriptor and `<&file` reads the file; `<<` (a here-document or
  /// here-string) and `<(` (a process substitution) are not read.
  fn input(&mut self, chars: &mut Chars<'_>) {
    self.descriptor_number();
    self.end_word();
    if chars.next_if_eq(&'<').is_some() {
      self.here_document(chars);
      return;
    }
    if chars.next_if_eq(&'(').is_some() {
      self.part.unread = true;
      return;
    }
    self.redirect = Some(if chars.next_if_eq(&'&').is_some() {
      Redirect::Duplicate { writes: false }
    } else {
      Redirect::Read
    });
  }

  /// `<<word` or `<<-word`: the part reads the body that follows the line; a here-string (`<<<`) is not
  /// read.
  fn here_document(&mut self, chars: &mut Chars<'_>) {
    if chars.next_if_eq(&'<').is_some() {
      self.part.unread = true;
      return;
    }
    match Heredoc::after_operator(chars) {
      Some(heredoc) => {
        self.part.heredoc = true;
        // The part is pushed when it ends: it is the next one, and holds the here-document at least.
        self.heredocs.push((self.parts.len(), heredoc));
      }
      None => self.part.unread = true,
    }
  }

  /// Takes the bodies of the line's here-documents, which follow it in order.
  fn bodies(&mut self, chars: &mut Chars<'_>) {
    for (at, heredoc) in std::mem::take(&mut self.heredocs) {
      if !heredoc.take_body(chars) {
        self.parts[at].unread = true;
      }
    }
  }

  /// Drops the word just before a redirection when it is the descriptor it redirects (`2>`).
  fn descriptor_number(&mut self) {
    if self
      .word
      .as_deref()
      .is_some_and(|word| word.chars().all(|c| c.is_ascii_digit()))
    {
      self.word = None;
    }
  }

  fn single_quoted(&mut self, chars: &mut Chars<'_>) {
    self.quoting_name();
    self.word.get_or_insert_with(String::new);
    loop {
      match chars.next() {
        Some('\'') => return,
        Some(quoted) => self.push(quoted),
        None => {
          self.part.unread = true;
          return;
        }
      }
    }
  }

  fn double_quoted(&mut self, chars: &mut Chars<'_>) {
    self.quoting_name();
    self.word.get_or_insert_with(String::new);
    loop {
      match chars.next() {
        Some('"') => return,
        Some('\\') if self.quoting == Quoting::Bash => match chars.next() {
          Some(escaped @ ('$' | '`' | '"' | '\\')) => self.push(escaped),
          Some('\n') => {}
          Some(other) => {
            self.push('\\');
            self.push(other);
          }
          None => self.part.unread = true,
        },
        Some('$') => self.dollar(chars, true),
        Some('`') => self.part.unread = true,
        Some(quoted) => self.push(quoted),
        None => {
          self.part.unread = true;
          return;
        }
      }
    }
  }

  /// A `$` is a plain `$` only where nothing can follow it: at the end of a word, or before the quote
  /// that closes a double-quoted string. Anything else may expand (`$x`, `$(…)`, `$'…'`, `$"…"`,
  /// `$[…]`) and leaves the part unread; PowerShell's `$null` is kept as a word, the null device.
  fn dollar(&mut self, chars: &mut Chars<'_>, quoted: bool) {
    if self.exit_status(chars) {
      return;
    }
    if self.quoting == Quoting::Bash && self.literal_variable(chars, quoted) {
      return;
    }
    let plain = match chars.peek() {
      None => true,
      Some(next) if quoted => *next == '"',
      Some(next) => matches!(next, ' ' | '\t' | '\r' | '\n' | ';' | '|' | '&' | '<' | '>'),
    };
    if plain {
      self.push('$');
    } else if !quoted && self.quoting == Quoting::PowerShell && self.word.is_none() && Self::null(chars) {
      self.word = Some("$null".to_owned());
    } else {
      self.part.unread = true;
    }
  }

  /// Whether the status an earlier command left follows the `$`: bash's `?`, `{?}` and `{PIPESTATUS[N]}`,
  /// PowerShell's `?` and `LASTEXITCODE` (without case, ending the name). It is taken, kept as written.
  fn exit_status(&mut self, chars: &mut Chars<'_>) -> bool {
    let ahead: String = chars.clone().take(STATUS_LONGEST + 1).collect();
    let length = match self.quoting {
      Quoting::Bash => bash_status(&ahead),
      Quoting::PowerShell => powershell_status(&ahead),
    };
    let Some(length) = length else {
      return false;
    };
    self.push('$');
    for c in chars.take(length).collect::<Vec<_>>() {
      self.push(c);
    }
    self.status_word = true;
    true
  }

  /// Whether a variable set to a literal follows (`name` or `{name}`); its value is taken in its place.
  fn literal_variable(&mut self, chars: &mut Chars<'_>, quoted: bool) -> bool {
    let mut ahead = chars.clone();
    let braced = ahead.next_if_eq(&'{').is_some();
    let mut name = String::new();
    while let Some(c) = ahead.next_if(|next| name_char(*next)) {
      name.push(c);
    }
    if braced && ahead.next_if_eq(&'}').is_none() {
      return false;
    }
    let home = match name.as_str() {
      "HOME" => self.homes.home.clone(),
      "USERPROFILE" => self.homes.profile.clone(),
      _ => None,
    }
    .filter(|home| quoted || !home.contains([' ', '\t', '*', '?', '[']));
    let value = self
      .variables
      .value(&name)
      .filter(|value| quoted || !value.contains('\\'));
    let Some(value) = value.map(str::to_owned).or(home) else {
      return false;
    };
    *chars = ahead;
    for c in value.chars() {
      self.push(c);
    }
    true
  }

  /// Whether `null` follows, ending the word; it is taken.
  fn null(chars: &mut Chars<'_>) -> bool {
    let rest: String = chars.clone().take(5).collect();
    let ends = rest
      .chars()
      .nth(4)
      .is_none_or(|next| matches!(next, ' ' | '\t' | '\r' | '\n' | ';' | '|' | '&' | ')'));
    if rest.get(..4).is_some_and(|name| name.eq_ignore_ascii_case("null")) && ends {
      chars.nth(3);
      return true;
    }
    false
  }
}
