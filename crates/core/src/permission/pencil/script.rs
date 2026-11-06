//! A Pencil `execute` snippet read by its tokens, as far as telling that it only reads the document: it names no
//! function that changes the design, none of JavaScript's ways to reach a function by a name built at run time, and
//! no member by a key that is not written out. Strings, templates and regular expressions are read as such, so a word
//! in them is no call; a snippet that cannot be read to its end (an unterminated string, an escape outside one) does
//! not read only.

/// The functions of `execute` that change the design, and the ways JavaScript reaches a function or the host by a
/// name it builds or does not write out (`eval`, `Function`, a constructor, the global object, the host's network
/// and storage).
const DENIED: &[&str] = &[
  "Insert",
  "Copy",
  "Update",
  "Replace",
  "Move",
  "Delete",
  "Generate",
  "SetVariables",
  "eval",
  "Function",
  "constructor",
  "prototype",
  "__proto__",
  "__defineGetter__",
  "__defineSetter__",
  "__lookupGetter__",
  "__lookupSetter__",
  "getPrototypeOf",
  "setPrototypeOf",
  "getOwnPropertyDescriptor",
  "getOwnPropertyDescriptors",
  "defineProperty",
  "defineProperties",
  "globalThis",
  "window",
  "self",
  "global",
  "this",
  "with",
  "Reflect",
  "Proxy",
  "import",
  "require",
  "module",
  "exports",
  "process",
  "Deno",
  "Bun",
  "fetch",
  "XMLHttpRequest",
  "WebSocket",
  "Worker",
  "importScripts",
  "postMessage",
  "navigator",
  "localStorage",
  "sessionStorage",
  "indexedDB",
  "setTimeout",
  "setInterval",
];

/// The function of `execute` that writes files: its third argument is the folder or file it writes.
const EXPORT: &str = "Export";

/// Words after which an expression starts: a `/` there opens a regular expression, a `[` an array.
const EXPRESSION_AFTER: &[&str] = &[
  "return",
  "typeof",
  "instanceof",
  "in",
  "of",
  "new",
  "delete",
  "void",
  "throw",
  "case",
  "do",
  "else",
  "yield",
  "await",
];

/// Punctuators of more than one character, longest first.
const PUNCTUATORS: &[&str] = &[
  ">>>=", "...", "===", "!==", "**=", "<<=", ">>=", ">>>", "&&=", "||=", "??=", "?.", "=>", "==", "!=", "<=", ">=",
  "&&", "||", "??", "++", "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "**", "<<", ">>",
];

/// Opens a template's substitution, closed by `}`.
const SUBSTITUTION: &str = "${";

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
  /// An identifier or a keyword.
  Word(String),
  Number,
  /// A string, or a template without substitutions, by its value; `None` for an escape that is not read here.
  Text(Option<String>),
  /// A regular expression, or the start of a template with substitutions: no value is read.
  Opaque,
  Punct(String),
}

/// The files the snippet `source` exports to (`Export`'s third argument, a written-out string) when it only reads the
/// document; `None` when it may do more, or cannot be read.
pub(super) fn reads_only(source: &str) -> Option<Vec<String>> {
  let tokens = Lexer::tokens(source)?;
  let mut exports = Vec::new();
  for (index, token) in tokens.iter().enumerate() {
    let before = index.checked_sub(1).and_then(|before| tokens.get(before));
    match token {
      Token::Word(word) if DENIED.contains(&word.as_str()) => return None,
      Token::Word(word) if word == EXPORT => exports.push(export_path(before, &tokens[index + 1..])?),
      Token::Punct(open) if open == "[" && member_access(before) => written_key(&tokens[index + 1..])?,
      _ => {}
    }
  }
  Some(exports)
}

/// Whether a `[` after `before` reads a member, not an array.
fn member_access(before: Option<&Token>) -> bool {
  match before {
    Some(Token::Word(word)) => !EXPRESSION_AFTER.contains(&word.as_str()),
    Some(Token::Punct(punct)) => matches!(punct.as_str(), ")" | "]" | "}" | "?."),
    Some(Token::Number | Token::Text(_) | Token::Opaque) => true,
    None => false,
  }
}

/// A member key written out after its `[`: a number, or a string that names nothing denied.
fn written_key(after: &[Token]) -> Option<()> {
  let key = match after.first()? {
    Token::Number => true,
    Token::Text(Some(key)) => !DENIED.contains(&key.as_str()),
    _ => false,
  };
  (key && matches!(after.get(1), Some(Token::Punct(close)) if close == "]")).then_some(())
}

/// The path of an `Export` called directly, `Export(nodes, format, "<path>", …)`, after `before`, its tokens after the
/// name being `after`; `None` for one passed on, reached as a member, or whose path is not a written-out string.
fn export_path(before: Option<&Token>, after: &[Token]) -> Option<String> {
  if matches!(before, Some(Token::Punct(dot)) if dot == "." || dot == "?.") {
    return None;
  }
  let (Token::Punct(open), rest) = after.split_first()? else {
    return None;
  };
  if open != "(" {
    return None;
  }
  match arguments(rest)?.get(2)? {
    [Token::Text(Some(path))] => Some(path.clone()),
    _ => None,
  }
}

/// The arguments of a call whose tokens after its `(` are `tokens`, up to its `)`; `None` when it is not closed.
fn arguments(tokens: &[Token]) -> Option<Vec<&[Token]>> {
  let mut arguments = Vec::new();
  let (mut depth, mut start) = (0_usize, 0);
  for (index, token) in tokens.iter().enumerate() {
    let Token::Punct(punct) = token else {
      continue;
    };
    match punct.as_str() {
      "(" | "[" | "{" | SUBSTITUTION => depth += 1,
      ")" if depth == 0 => {
        arguments.push(&tokens[start..index]);
        return Some(arguments);
      }
      ")" | "]" | "}" => depth = depth.checked_sub(1)?,
      "," if depth == 0 => {
        arguments.push(&tokens[start..index]);
        start = index + 1;
      }
      _ => {}
    }
  }
  None
}

/// Reads a snippet into [`Token`]s.
struct Lexer {
  chars: Vec<char>,
  at: usize,
  tokens: Vec<Token>,
  /// Open braces, those of substitutions included.
  braces: usize,
  /// The count of open braces at each open substitution, innermost last: its `}` returns to the template.
  substitutions: Vec<usize>,
}

impl Lexer {
  /// The tokens of `source`; `None` when it cannot be read to its end.
  fn tokens(source: &str) -> Option<Vec<Token>> {
    let mut lexer = Self {
      chars: source.chars().collect(),
      at: 0,
      tokens: Vec::new(),
      braces: 0,
      substitutions: Vec::new(),
    };
    while lexer.next()? {}
    lexer.substitutions.is_empty().then_some(lexer.tokens)
  }

  fn peek(&self, ahead: usize) -> Option<char> {
    self.chars.get(self.at + ahead).copied()
  }

  /// Reads the next token; `Some(false)` at the end, `None` on what cannot be read.
  fn next(&mut self) -> Option<bool> {
    while self.peek(0).is_some_and(char::is_whitespace) {
      self.at += 1;
    }
    let Some(first) = self.peek(0) else {
      return Some(false);
    };
    match first {
      '/' if self.peek(1) == Some('/') => self.line_comment(),
      '/' if self.peek(1) == Some('*') => self.block_comment()?,
      '\'' | '"' => {
        self.at += 1;
        let text = self.string(first)?;
        self.tokens.push(Token::Text(text));
      }
      '`' => {
        self.at += 1;
        self.template(true)?;
      }
      '0'..='9' => self.number(),
      '.' if self.peek(1).is_some_and(|next| next.is_ascii_digit()) => self.number(),
      '/' if self.regex_allowed() => self.regex()?,
      '}' if self.substitutions.last() == Some(&self.braces) => {
        self.substitutions.pop();
        self.braces -= 1;
        self.at += 1;
        self.tokens.push(Token::Punct("}".to_owned()));
        self.template(false)?;
      }
      // An escape outside a string may spell a word, `Insert` by its code points: not read.
      '\\' => return None,
      _ if is_word_start(first) => self.word(),
      _ => self.punctuator()?,
    }
    Some(true)
  }

  fn line_comment(&mut self) {
    while self.peek(0).is_some_and(|c| c != '\n') {
      self.at += 1;
    }
  }

  fn block_comment(&mut self) -> Option<()> {
    self.at += 2;
    while !(self.peek(0)? == '*' && self.peek(1) == Some('/')) {
      self.at += 1;
    }
    self.at += 2;
    Some(())
  }

  fn number(&mut self) {
    while self
      .peek(0)
      .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    {
      self.at += 1;
    }
    self.tokens.push(Token::Number);
  }

  fn word(&mut self) {
    let start = self.at;
    while self.peek(0).is_some_and(is_word_part) {
      self.at += 1;
    }
    self
      .tokens
      .push(Token::Word(self.chars[start..self.at].iter().collect()));
  }

  fn punctuator(&mut self) -> Option<()> {
    let rest: String = self.chars[self.at..].iter().take(4).collect();
    let punct = PUNCTUATORS
      .iter()
      .find(|punct| rest.starts_with(**punct))
      .map_or_else(|| rest.chars().take(1).collect(), |punct| (*punct).to_owned());
    match punct.as_str() {
      "{" => self.braces += 1,
      "}" => self.braces = self.braces.checked_sub(1)?,
      _ => {}
    }
    self.at += punct.chars().count();
    self.tokens.push(Token::Punct(punct));
    Some(())
  }

  /// Whether a `/` here opens a regular expression rather than divides.
  fn regex_allowed(&self) -> bool {
    match self.tokens.last() {
      None => true,
      Some(Token::Punct(punct)) => !matches!(punct.as_str(), ")" | "]" | "}"),
      Some(Token::Word(word)) => EXPRESSION_AFTER.contains(&word.as_str()),
      Some(Token::Number | Token::Text(_) | Token::Opaque) => false,
    }
  }

  fn regex(&mut self) -> Option<()> {
    self.at += 1;
    let mut class = false;
    loop {
      match self.peek(0)? {
        '\n' => return None,
        '\\' => self.at += 1,
        '[' => class = true,
        ']' => class = false,
        '/' if !class => break,
        _ => {}
      }
      self.at += 1;
    }
    self.at += 1;
    while self.peek(0).is_some_and(is_word_part) {
      self.at += 1;
    }
    self.tokens.push(Token::Opaque);
    Some(())
  }

  /// A string's value up to its closing `quote`, the opening one read; `Some(None)` for one whose escapes are not read
  /// here, `None` for one not closed on its line.
  fn string(&mut self, quote: char) -> Option<Option<String>> {
    let mut value = Some(String::new());
    loop {
      let c = self.peek(0)?;
      self.at += 1;
      match c {
        '\n' => return None,
        '\\' => self.escape(&mut value)?,
        _ if c == quote => return Some(value),
        _ => push(&mut value, c),
      }
    }
  }

  /// A template's text from here, after its opening backtick (`opening`) or a substitution's `}`, up to its closing
  /// backtick or its next substitution.
  fn template(&mut self, opening: bool) -> Option<()> {
    let mut value = Some(String::new());
    loop {
      let c = self.peek(0)?;
      self.at += 1;
      match c {
        '`' => {
          if opening {
            self.tokens.push(Token::Text(value));
          }
          return Some(());
        }
        '$' if self.peek(0) == Some('{') => {
          self.at += 1;
          if opening {
            self.tokens.push(Token::Opaque);
          }
          self.braces += 1;
          self.substitutions.push(self.braces);
          self.tokens.push(Token::Punct(SUBSTITUTION.to_owned()));
          return Some(());
        }
        '\\' => self.escape(&mut value)?,
        _ => push(&mut value, c),
      }
    }
  }

  /// The escape after a `\` in a string or template, into `value`: the escapes of a path's characters are read, any
  /// other leaves the value unread.
  fn escape(&mut self, value: &mut Option<String>) -> Option<()> {
    let c = self.peek(0)?;
    self.at += 1;
    match c {
      '\\' | '\'' | '"' | '/' | '`' | '$' => push(value, c),
      '\n' => {}
      _ => *value = None,
    }
    Some(())
  }
}

fn push(value: &mut Option<String>, c: char) {
  if let Some(value) = value {
    value.push(c);
  }
}

fn is_word_start(c: char) -> bool {
  c.is_alphabetic() || c == '_' || c == '$'
}

fn is_word_part(c: char) -> bool {
  c.is_alphanumeric() || c == '_' || c == '$'
}
