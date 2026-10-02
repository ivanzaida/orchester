//! Splitting a direct command into program and arguments (`docs/architecture/native-agent.md` §4.6), the
//! way the platform's programs read their arguments: POSIX quoting on macOS, the `CommandLineToArgvW`
//! rules on Windows (as Rust's standard library reads them). No shell runs the result, so shell operators
//! outside quotes are refused rather than passed on as text the model meant as a pipe or a redirection.

use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SyntaxError {
  #[error("the command is blank")]
  Blank,
  #[error("the command has a quote that is never closed")]
  UnclosedQuote,
  #[error("`{0}` is a shell operator, and the command runs without a shell; pass shell: true to use one")]
  Operator(String),
}

/// How a command line is split into arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandSyntax {
  Posix,
  Windows,
}

/// Characters that are shell operators outside quotes on both platforms.
const OPERATORS: [char; 5] = [';', '&', '|', '<', '>'];

impl CommandSyntax {
  /// The syntax of the platform Orchester runs on.
  pub const HOST: Self = if cfg!(windows) { Self::Windows } else { Self::Posix };

  /// The program and its arguments.
  pub fn split(self, command: &str) -> Result<Vec<String>, SyntaxError> {
    let argv = match self {
      Self::Posix => posix(command)?,
      Self::Windows => windows(command)?,
    };
    if argv.first().is_none_or(String::is_empty) {
      return Err(SyntaxError::Blank);
    }
    Ok(argv)
  }
}

/// A word being collected; `None` between words.
#[derive(Default)]
struct Words {
  argv: Vec<String>,
  current: Option<String>,
}

impl Words {
  fn push(&mut self, c: char) {
    self.current.get_or_insert_with(String::new).push(c);
  }

  fn start(&mut self) {
    self.current.get_or_insert_with(String::new);
  }

  fn end(&mut self) {
    if let Some(word) = self.current.take() {
      self.argv.push(word);
    }
  }

  fn finish(mut self) -> Vec<String> {
    self.end();
    self.argv
  }
}

fn posix(command: &str) -> Result<Vec<String>, SyntaxError> {
  let mut words = Words::default();
  let mut chars = command.chars().peekable();
  while let Some(c) = chars.next() {
    match c {
      '\n' | '\r' => return Err(SyntaxError::Operator("newline".to_owned())),
      ' ' | '\t' => words.end(),
      '\'' => {
        words.start();
        loop {
          match chars.next() {
            Some('\'') => break,
            Some(quoted) => words.push(quoted),
            None => return Err(SyntaxError::UnclosedQuote),
          }
        }
      }
      '"' => posix_double_quoted(&mut chars, &mut words)?,
      '\\' => words.push(chars.next().unwrap_or('\\')),
      '`' | '(' | ')' => return Err(SyntaxError::Operator(c.to_string())),
      '$' if matches!(chars.peek(), Some('(' | '{')) => {
        return Err(SyntaxError::Operator(format!("${}", chars.next().unwrap_or('('))));
      }
      c if OPERATORS.contains(&c) => return Err(SyntaxError::Operator(c.to_string())),
      c => words.push(c),
    }
  }
  Ok(words.finish())
}

/// The rest of a double-quoted part: a backslash escapes only `$`, `` ` ``, `"`, `\` and a newline, and
/// substitutions are refused because a shell would run them.
fn posix_double_quoted(
  chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
  words: &mut Words,
) -> Result<(), SyntaxError> {
  words.start();
  loop {
    match chars.next() {
      Some('"') => return Ok(()),
      Some('\\') => match chars.next() {
        Some(escaped @ ('$' | '`' | '"' | '\\' | '\n')) => words.push(escaped),
        Some(other) => {
          words.push('\\');
          words.push(other);
        }
        None => return Err(SyntaxError::UnclosedQuote),
      },
      Some('`') => return Err(SyntaxError::Operator("`".to_owned())),
      Some('$') if matches!(chars.peek(), Some('(' | '{')) => {
        return Err(SyntaxError::Operator(format!("${}", chars.next().unwrap_or('('))));
      }
      Some(quoted) => words.push(quoted),
      None => return Err(SyntaxError::UnclosedQuote),
    }
  }
}

fn windows(command: &str) -> Result<Vec<String>, SyntaxError> {
  let command = command.trim_start_matches([' ', '\t']);
  let (program, rest) = windows_program(command)?;
  let mut words = Words::default();
  words.argv.push(program);
  let mut in_quotes = false;
  let mut backslashes = 0_usize;
  let mut chars = rest.chars().peekable();
  while let Some(c) = chars.next() {
    if c == '\\' {
      backslashes += 1;
      words.start();
      continue;
    }
    let pending = if c == '"' { backslashes / 2 } else { backslashes };
    (0..pending).for_each(|_| words.push('\\'));
    let escaped_quote = c == '"' && backslashes % 2 == 1;
    backslashes = 0;
    match c {
      '"' if escaped_quote => words.push('"'),
      '"' if in_quotes && chars.peek() == Some(&'"') => {
        chars.next();
        words.push('"');
      }
      '"' => {
        in_quotes = !in_quotes;
        words.start();
      }
      ' ' | '\t' if !in_quotes => words.end(),
      '\n' | '\r' if !in_quotes => return Err(SyntaxError::Operator("newline".to_owned())),
      c if !in_quotes && OPERATORS.contains(&c) => return Err(SyntaxError::Operator(c.to_string())),
      c => words.push(c),
    }
  }
  (0..backslashes).for_each(|_| words.push('\\'));
  if in_quotes {
    return Err(SyntaxError::UnclosedQuote);
  }
  Ok(words.finish())
}

/// The program name, which `CommandLineToArgvW` reads without escapes: quoted up to the next quote, or
/// up to the first space or tab.
fn windows_program(command: &str) -> Result<(String, &str), SyntaxError> {
  if let Some(quoted) = command.strip_prefix('"') {
    let (program, rest) = quoted.split_once('"').ok_or(SyntaxError::UnclosedQuote)?;
    return Ok((program.to_owned(), rest));
  }
  let end = command.find([' ', '\t']).unwrap_or(command.len());
  let program = &command[..end];
  if program.contains(['\n', '\r']) {
    return Err(SyntaxError::Operator("newline".to_owned()));
  }
  if let Some(operator) = program.chars().find(|c| OPERATORS.contains(c)) {
    return Err(SyntaxError::Operator(operator.to_string()));
  }
  Ok((program.to_owned(), &command[end..]))
}
