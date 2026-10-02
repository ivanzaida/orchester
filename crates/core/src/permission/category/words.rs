//! Splitting a command line into its simple commands (`docs/design/orchester-workspace.md`, "Command
//! categories and approval policy"): parts joined by `&&`, `||`, `;`, `|`, `&` or a new line, each with
//! its words and the files its redirections write or read. Bash quoting by default; PowerShell's
//! (a backslash is a plain character) for the script a `pwsh -Command` runs. Nothing is guessed: a
//! variable, a substitution, a subshell or group, a here-document, a brace expansion or an unclosed
//! quote leaves the part unread, and it is then classified as Unknown.

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
}

impl RawPart {
  fn is_empty(&self) -> bool {
    self.words.is_empty() && self.writes.is_empty() && self.reads.is_empty() && !self.unread
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Redirect {
  Write,
  Read,
}

/// The parts being collected.
struct Splitter {
  quoting: Quoting,
  parts: Vec<RawPart>,
  part: RawPart,
  word: Option<String>,
  redirect: Option<Redirect>,
}

type Chars<'a> = std::iter::Peekable<std::str::Chars<'a>>;

/// The simple commands of `command`, in order.
pub(super) fn split(command: &str, quoting: Quoting) -> Vec<RawPart> {
  let mut splitter = Splitter {
    quoting,
    parts: Vec::new(),
    part: RawPart::default(),
    word: None,
    redirect: None,
  };
  let mut chars = command.chars().peekable();
  while let Some(c) = chars.next() {
    splitter.take(c, &mut chars);
  }
  splitter.end_part();
  splitter.parts
}

impl Splitter {
  fn take(&mut self, c: char, chars: &mut Chars<'_>) {
    match c {
      ' ' | '\t' | '\r' => self.end_word(),
      '\n' | ';' => self.end_part(),
      '&' => self.ampersand(chars),
      '|' => {
        chars.next_if(|next| matches!(next, '|' | '&'));
        self.end_part();
      }
      '>' => self.output(chars),
      '<' => self.input(chars),
      '\'' => self.single_quoted(chars),
      '"' => self.double_quoted(chars),
      '\\' if self.quoting == Quoting::Bash => match chars.next() {
        // A line continuation joins the lines.
        Some('\n') | None => {}
        Some(escaped) => self.push(escaped),
      },
      '$' => self.dollar(chars),
      '`' | '(' | ')' | '{' | '}' => self.part.unread = true,
      '#' if self.word.is_none() => while chars.next_if(|next| *next != '\n').is_some() {},
      other => self.push(other),
    }
  }

  fn push(&mut self, c: char) {
    self.word.get_or_insert_with(String::new).push(c);
  }

  fn end_word(&mut self) {
    let Some(word) = self.word.take() else {
      return;
    };
    match self.redirect.take() {
      Some(Redirect::Write) => self.part.writes.push(word),
      Some(Redirect::Read) => self.part.reads.push(word),
      None => self.part.words.push(word),
    }
  }

  fn end_part(&mut self) {
    self.end_word();
    // A redirection without its file is not read to the end.
    if self.redirect.take().is_some() {
      self.part.unread = true;
    }
    let part = std::mem::take(&mut self.part);
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
    chars.next_if_eq(&'&');
    self.end_part();
  }

  /// `>`, `>>`, `N>`; `>&N` and `N>&M` only join outputs and write no file.
  fn output(&mut self, chars: &mut Chars<'_>) {
    if self
      .word
      .as_deref()
      .is_some_and(|word| word.chars().all(|c| c.is_ascii_digit()))
    {
      self.word = None;
    }
    self.end_word();
    chars.next_if_eq(&'>');
    if chars.next_if_eq(&'&').is_some() {
      while chars.next_if(|next| next.is_ascii_digit() || *next == '-').is_some() {}
      return;
    }
    self.redirect = Some(Redirect::Write);
  }

  /// `<` reads a file; `<<` (a here-document) and `<(` (a process substitution) are not read.
  fn input(&mut self, chars: &mut Chars<'_>) {
    self.end_word();
    if chars.next_if(|next| matches!(next, '<' | '(')).is_some() {
      self.part.unread = true;
      return;
    }
    self.redirect = Some(Redirect::Read);
  }

  fn single_quoted(&mut self, chars: &mut Chars<'_>) {
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
        Some('$') => self.dollar(chars),
        Some('`') => self.part.unread = true,
        Some(quoted) => self.push(quoted),
        None => {
          self.part.unread = true;
          return;
        }
      }
    }
  }

  /// A `$` that starts a variable or substitution leaves the part unread; any other is a plain `$`.
  fn dollar(&mut self, chars: &mut Chars<'_>) {
    let expands = chars.peek().is_some_and(|next| {
      next.is_ascii_alphanumeric() || matches!(next, '_' | '{' | '(' | '@' | '*' | '#' | '?' | '!' | '$')
    });
    if expands {
      self.part.unread = true;
    } else {
      self.push('$');
    }
  }
}
