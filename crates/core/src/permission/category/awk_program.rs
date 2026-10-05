//! Reading an `awk` program to tell that it only reads the files on its command line and prints: an
//! allow-list of the names it may use. String and regular-expression literals are skipped; every other
//! name must be a keyword, a built-in function that neither reads nor runs anything, one of the
//! variables that only shape records ([`VARIABLES`]), or a name of the program's own (lower case, not a
//! built-in). That keeps out `getline`, `system`, `close`, `fflush`, the message catalogs, and the
//! special variables that name other files or the environment (`ARGV`, `ARGC`, `ENVIRON`, `FILENAME`,
//! `PROCINFO`, `SYMTAB`, …). `@` (gawk's `@load`, `@include`, indirect calls) is refused, and so is a
//! `print` or `printf` in a program that holds `>` or `|`, which could write a file or start a program.

/// Keywords and the built-in functions that compute or print only.
const KEYWORDS: [&str; 52] = [
  "BEGIN", "END", "function", "func", "if", "else", "while", "for", "do", "break", "continue", "next", "nextfile",
  "exit", "return", "delete", "in", "print", "printf", "length", "substr", "index", "split", "sub", "gsub", "match",
  "sprintf", "sin", "cos", "atan2", "exp", "log", "sqrt", "int", "rand", "srand", "tolower", "toupper", "gensub",
  "patsplit", "asort", "asorti", "strftime", "systime", "mktime", "and", "or", "xor", "compl", "lshift", "rshift",
  "strtonum",
];

/// Variables that shape records and output, which a program may read and set.
const VARIABLES: [&str; 12] = [
  "NR", "NF", "FNR", "FS", "OFS", "RS", "ORS", "SUBSEP", "CONVFMT", "OFMT", "RSTART", "RLENGTH",
];

/// Built-in functions of gawk that read, write or run something, which a lower-case name could call.
const REFUSED: [&str; 7] = [
  "getline",
  "system",
  "close",
  "fflush",
  "bindtextdomain",
  "dcgettext",
  "dcngettext",
];

/// Whether `program` only reads the files on the command line and prints.
pub(super) fn harmless(program: &str) -> bool {
  let Some(code) = code_of(program) else {
    return false;
  };
  if code.contains('@') || (code.contains("print") && code.contains(['>', '|'])) {
    return false;
  }
  names(&code).all(allowed)
}

/// Whether `name` may be set from the command line (`-v name=value`, an operand `name=value`).
pub(super) fn settable(name: &str) -> bool {
  VARIABLES.contains(&name) || own(name)
}

fn allowed(name: &str) -> bool {
  KEYWORDS.contains(&name) || VARIABLES.contains(&name) || own(name)
}

/// A name of the program's own: lower case, digits and `_`, and no built-in it could call.
fn own(name: &str) -> bool {
  name
    .chars()
    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    && !REFUSED.contains(&name)
}

/// The names in `code`.
fn names(code: &str) -> impl Iterator<Item = &str> {
  code
    .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
    .filter(|word| word.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_'))
}

/// `program` with its string and regular-expression literals blanked out; `None` when one is not closed.
/// A `/` starts a regular expression only where no operand comes before it; elsewhere it divides, and
/// what follows is read as code.
fn code_of(program: &str) -> Option<String> {
  let mut code = String::with_capacity(program.len());
  let mut chars = program.chars();
  while let Some(c) = chars.next() {
    let regex = c == '/'
      && code
        .trim_end()
        .chars()
        .last()
        .is_none_or(|before| "(,{};!~&|?:=\n".contains(before));
    if c == '"' || regex {
      skip_literal(&mut chars, c)?;
      code.push(' ');
    } else {
      code.push(c);
    }
  }
  Some(code)
}

/// Takes the rest of a literal that `end` closes, with its escapes.
fn skip_literal(chars: &mut std::str::Chars<'_>, end: char) -> Option<()> {
  loop {
    match chars.next()? {
      '\\' => {
        chars.next()?;
      }
      c if c == end => return Some(()),
      _ => {}
    }
  }
}
