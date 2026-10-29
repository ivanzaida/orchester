//! .NET static calls in a script `pwsh -NoProfile -Command` runs that only read (ORC-339): the string
//! operations of `[System.IO.Path]` (`GetFullPath`, `GetFileName`, `Combine`, …), which touch no file, and the
//! reads of `[System.IO.File]` and `[System.IO.Directory]` (`Exists`, `ReadAllText`, `GetFiles`, …), whose
//! paths are placed as a reader's are. Only a statement that is one such call alone is read: the type and
//! method from the lists, arguments that are plain quoted strings, nothing before or after it (no pipe, no
//! redirect, no variable, no subexpression). Any other statement is read as the script's other parts are.

/// A static .NET call that only reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Call {
  /// The type and method as the rules name them: `[system.io.path]::getfullpath`.
  pub(super) name: String,
  /// The arguments that name a file it reads; none for `[System.IO.Path]`'s string operations.
  pub(super) files: Vec<String>,
}

/// The methods of `[System.IO.Path]`: string operations on paths that open no file.
const PATH_METHODS: [&str; 17] = [
  "getfullpath",
  "getfilename",
  "getfilenamewithoutextension",
  "getdirectoryname",
  "getextension",
  "getpathroot",
  "getrelativepath",
  "combine",
  "join",
  "ispathrooted",
  "ispathfullyqualified",
  "hasextension",
  "changeextension",
  "gettemppath",
  "endsindirectoryseparator",
  "trimendingdirectoryseparator",
  "getinvalidpathchars",
];

/// The methods of `[System.IO.File]` that only read the file they name.
const FILE_METHODS: [&str; 4] = ["exists", "readalltext", "readalllines", "getlastwritetime"];

/// The methods of `[System.IO.Directory]` that only read the folder they name.
const DIRECTORY_METHODS: [&str; 5] = [
  "exists",
  "getfiles",
  "getdirectories",
  "getfilesystementries",
  "getcurrentdirectory",
];

/// The statements of a PowerShell `script`, split at `;` and new lines outside quotes; `None` when a quote is
/// not closed or the script holds what this split cannot follow (a backtick escape, a typographic quote).
pub(super) fn statements(script: &str) -> Option<Vec<&str>> {
  if script.contains([
    '`', '\u{2018}', '\u{2019}', '\u{201a}', '\u{201b}', '\u{201c}', '\u{201d}', '\u{201e}',
  ]) {
    return None;
  }
  let mut statements = Vec::new();
  let mut quote = None;
  let mut start = 0;
  for (at, c) in script.char_indices() {
    match (quote, c) {
      (None, '\'' | '"') => quote = Some(c),
      (Some(open), c) if c == open => quote = None,
      (None, ';' | '\n') => {
        statements.push(&script[start..at]);
        start = at + 1;
      }
      _ => {}
    }
  }
  if quote.is_some() {
    return None;
  }
  statements.push(&script[start..]);
  Some(
    statements
      .into_iter()
      .filter(|statement| !statement.trim().is_empty())
      .collect(),
  )
}

/// The call `statement` is, alone: `[System.IO.Path]::GetFullPath('src')`; `None` for anything else.
pub(super) fn call(statement: &str) -> Option<Call> {
  let (kind, rest) = statement.trim().strip_prefix('[')?.split_once("]::")?;
  let (method, rest) = rest.split_once('(')?;
  let arguments = arguments(rest.strip_suffix(')')?)?;
  let (kind, method) = (kind.trim().to_lowercase(), method.to_lowercase());
  let kind = kind.strip_prefix("system.").unwrap_or(&kind);
  let files = match kind {
    "io.path" if PATH_METHODS.contains(&method.as_str()) => Vec::new(),
    "io.file" if FILE_METHODS.contains(&method.as_str()) => arguments,
    "io.directory" if DIRECTORY_METHODS.contains(&method.as_str()) => arguments,
    _ => return None,
  };
  Some(Call {
    name: format!("[system.{kind}]::{method}"),
    files,
  })
}

/// The quoted strings between a call's parentheses, separated by commas: `'a', "b"`. A single-quoted string
/// holds a quote doubled; a double-quoted one no `$`, which could expand. Anything else is `None`.
fn arguments(inner: &str) -> Option<Vec<String>> {
  let mut arguments = Vec::new();
  let mut chars = inner.trim().chars().peekable();
  if chars.peek().is_none() {
    return Some(arguments);
  }
  loop {
    let mut value = String::new();
    match chars.next()? {
      '\'' => loop {
        match chars.next()? {
          '\'' if chars.next_if_eq(&'\'').is_some() => value.push('\''),
          '\'' => break,
          c => value.push(c),
        }
      },
      '"' => loop {
        match chars.next()? {
          '"' => break,
          '$' => return None,
          c => value.push(c),
        }
      },
      _ => return None,
    }
    arguments.push(value);
    while chars.next_if(|c| c.is_whitespace()).is_some() {}
    match chars.next() {
      None => return Some(arguments),
      Some(',') => while chars.next_if(|c| c.is_whitespace()).is_some() {},
      Some(_) => return None,
    }
  }
}
