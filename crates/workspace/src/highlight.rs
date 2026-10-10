//! Syntax highlighting of a changed file's diff lines with syntect's bundled Sublime grammars. A
//! grammar's scopes are reduced to the few kinds of token the design colors (Pencil `code-*`,
//! mapped like GitHub Dark maps them), so the UI draws tokens through theme roles only.
//!
//! A diff shows fragments of a file, so each hunk starts from the grammar's initial state; within
//! a hunk, removed lines continue the old side and added lines the new side, and context lines
//! belong to both. A line cut short takes its side's state with it (see `highlight_hunk`). Highlighting is CPU work for
//! a background thread; its result is only colors, and a line the grammar cannot read stays plain.

use std::sync::{Arc, OnceLock};

use orchester_core::diff::{FileDiff, Hunk, LineKind};
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};

/// Columns between tab stops when a tab is shown as spaces.
const TAB_WIDTH: usize = 4;

/// What a token is, for its color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokenKind {
  Plain,
  Keyword,
  String,
  Comment,
  Constant,
  Function,
  Variable,
}

/// Text of one kind within a line, ready to show: tabs expanded, control characters replaced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
  pub kind: TokenKind,
  pub text: Arc<str>,
}

/// The tokens of one diff line.
pub type LineTokens = Arc<[Token]>;

/// Tokens of every kept line of a file, hunk by hunk.
pub type FileTokens = Vec<Vec<LineTokens>>;

/// `text` as one plain token, e.g. before highlighting finished or without a grammar.
pub fn plain(text: &str) -> LineTokens {
  let mut column = 0;
  Arc::from([Token {
    kind: TokenKind::Plain,
    text: display(text, &mut column).into(),
  }])
}

/// Highlights `file` with the grammar its path names; `None` when no bundled grammar knows it.
pub fn highlight(file: &FileDiff) -> Option<FileTokens> {
  let syntaxes = syntaxes();
  let syntax = syntax_for(syntaxes, file.path())?;
  Some(
    file
      .hunks()
      .iter()
      .map(|hunk| highlight_hunk(syntaxes, syntax, hunk))
      .collect(),
  )
}

fn syntaxes() -> &'static SyntaxSet {
  static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
  SYNTAXES.get_or_init(SyntaxSet::load_defaults_nonewlines)
}

/// The grammar for `path`, by file name or extension. The bundled set has no TypeScript grammar;
/// JavaScript's reads most of it.
fn syntax_for<'a>(syntaxes: &'a SyntaxSet, path: &str) -> Option<&'a SyntaxReference> {
  let name = path.rsplit('/').next().unwrap_or(path);
  let extension = name.rsplit_once('.').map_or(name, |(_, extension)| extension);
  let extension = match extension {
    "ts" | "tsx" | "mts" | "cts" => "js",
    other => other,
  };
  syntaxes
    .find_syntax_by_extension(name)
    .or_else(|| syntaxes.find_syntax_by_extension(extension))
    .filter(|syntax| syntax.name != "Plain Text")
}

/// A grammar's state on one side of a hunk.
#[derive(Clone)]
struct Side {
  parse: ParseState,
  scopes: ScopeStack,
  /// The grammar failed on this side; the rest of its lines stay plain.
  failed: bool,
}

impl Side {
  fn new(syntax: &SyntaxReference) -> Self {
    Self {
      parse: ParseState::new(syntax),
      scopes: ScopeStack::new(),
      failed: false,
    }
  }

  /// The tokens of `line`, advancing the state.
  fn tokens(&mut self, syntaxes: &SyntaxSet, line: &str) -> LineTokens {
    if !self.failed {
      match self.try_tokens(syntaxes, line) {
        Some(tokens) => return tokens,
        None => self.failed = true,
      }
    }
    plain(line)
  }

  fn try_tokens(&mut self, syntaxes: &SyntaxSet, line: &str) -> Option<LineTokens> {
    let operations = self.parse.parse_line(line, syntaxes).ok()?;
    let mut tokens: Vec<Token> = Vec::new();
    let (mut start, mut column) = (0, 0);
    for (position, operation) in operations {
      let position = position.min(line.len());
      if position > start {
        push(&mut tokens, kind(&self.scopes), &line[start..position], &mut column);
        start = position;
      }
      self.scopes.apply(&operation).ok()?;
    }
    if start < line.len() {
      push(&mut tokens, kind(&self.scopes), &line[start..], &mut column);
    }
    Some(tokens.into())
  }
}

/// A line cut to its beginning leaves its side's state wherever the cut fell, e.g. inside a
/// string, so after it that side continues from the other side's state (the lines around a
/// change are alike), and after a cut context line both start again from the initial state.
fn highlight_hunk(syntaxes: &SyntaxSet, syntax: &SyntaxReference, hunk: &Hunk) -> Vec<LineTokens> {
  let (mut old, mut new) = (Side::new(syntax), Side::new(syntax));
  hunk
    .lines()
    .iter()
    .map(|line| {
      let text = line.text();
      match line.kind() {
        LineKind::Removed => {
          let tokens = old.tokens(syntaxes, text);
          if line.is_cut() {
            old = new.clone();
          }
          tokens
        }
        LineKind::Added => {
          let tokens = new.tokens(syntaxes, text);
          if line.is_cut() {
            new = old.clone();
          }
          tokens
        }
        LineKind::Context => {
          // The old side reads the line too, so its state stays in step; its tokens are the same.
          old.tokens(syntaxes, text);
          let tokens = new.tokens(syntaxes, text);
          if line.is_cut() {
            (old, new) = (Side::new(syntax), Side::new(syntax));
          }
          tokens
        }
      }
    })
    .collect()
}

/// Words the bundled grammars leave unscoped that read as keywords: Rust's grammar predates `async` and
/// `await`, and leaves `self` plain in a parameter list.
const BARE_KEYWORDS: [&str; 3] = ["async", "await", "self"];

/// Appends `text` as `kind`; in plain text the [`BARE_KEYWORDS`] become keywords.
fn push(tokens: &mut Vec<Token>, kind: TokenKind, text: &str, column: &mut usize) {
  if kind != TokenKind::Plain {
    append(tokens, kind, text, column);
    return;
  }
  let mut rest = text;
  while !rest.is_empty() {
    let word = rest.find(is_word).unwrap_or(rest.len());
    append(tokens, TokenKind::Plain, &rest[..word], column);
    rest = &rest[word..];
    let end = rest.find(|character| !is_word(character)).unwrap_or(rest.len());
    let kind = if BARE_KEYWORDS.contains(&&rest[..end]) {
      TokenKind::Keyword
    } else {
      TokenKind::Plain
    };
    append(tokens, kind, &rest[..end], column);
    rest = &rest[end..];
  }
}

fn is_word(character: char) -> bool {
  character.is_alphanumeric() || character == '_'
}

/// Appends `text` as `kind`, merged into the previous token when that is of the same kind.
fn append(tokens: &mut Vec<Token>, kind: TokenKind, text: &str, column: &mut usize) {
  if text.is_empty() {
    return;
  }
  let text = display(text, column);
  match tokens.last_mut() {
    Some(last) if last.kind == kind => last.text = format!("{}{text}", last.text).into(),
    _ => tokens.push(Token {
      kind,
      text: text.into(),
    }),
  }
}

/// `text` as it is shown from `column` on: tabs as spaces to the next tab stop, other control
/// characters as the replacement character, so nothing in a diff can steer the text renderer.
fn display(text: &str, column: &mut usize) -> String {
  let mut shown = String::with_capacity(text.len());
  for character in text.chars() {
    if character == '\t' {
      let spaces = TAB_WIDTH - *column % TAB_WIDTH;
      shown.extend(std::iter::repeat_n(' ', spaces));
      *column += spaces;
    } else {
      shown.push(if character.is_control() { '\u{FFFD}' } else { character });
      *column += 1;
    }
  }
  shown
}

/// The kind of the innermost scope a rule knows; scopes without a rule (punctuation, meta)
/// take the kind of the scope around them.
fn kind(scopes: &ScopeStack) -> TokenKind {
  let rules = rules();
  for scope in scopes.as_slice().iter().rev() {
    if let Some((_, kind)) = rules.iter().find(|(prefix, _)| prefix.is_prefix_of(*scope)) {
      return *kind;
    }
  }
  TokenKind::Plain
}

/// Scope prefixes to kinds, most specific first, following GitHub Dark (`primer/github-vscode-
/// theme`): parameters and plain variables read as text, other variables and declared names as
/// `code-variable`, tags as `code-constant`; as the Kontur design colors code, numbers and the language's
/// own constants are its only constants (enum variants such as `Ok`, named constants and built-in types
/// read as text), operators read as text, and `self` or `this` as keywords.
fn rules() -> &'static [(Scope, TokenKind)] {
  static RULES: OnceLock<Vec<(Scope, TokenKind)>> = OnceLock::new();
  RULES.get_or_init(|| {
    [
      ("comment", TokenKind::Comment),
      ("string", TokenKind::String),
      ("constant.numeric", TokenKind::Constant),
      ("constant.language", TokenKind::Constant),
      ("constant.character", TokenKind::String),
      ("constant", TokenKind::Plain),
      ("variable.language", TokenKind::Keyword),
      ("variable.function", TokenKind::Function),
      ("variable.parameter", TokenKind::Plain),
      ("variable.other", TokenKind::Plain),
      ("variable", TokenKind::Variable),
      ("entity.name.function", TokenKind::Function),
      ("support.function", TokenKind::Function),
      ("entity.name.tag", TokenKind::Constant),
      ("entity.name", TokenKind::Variable),
      ("entity.other.inherited-class", TokenKind::Variable),
      ("support", TokenKind::Plain),
      ("keyword.operator", TokenKind::Plain),
      ("keyword", TokenKind::Keyword),
      ("storage", TokenKind::Keyword),
    ]
    .into_iter()
    .map(|(prefix, kind)| (Scope::new(prefix).expect("the rule prefixes are valid scopes"), kind))
    .collect()
  })
}
