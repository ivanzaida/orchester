//! Paths as git writes them in a diff: `a/` and `b/` prefixes, `/dev/null` for a missing side,
//! and C-style quoting (`"a/na\303\257ve.txt"`) for names with special or non-ASCII characters.

const NULL: &str = "/dev/null";

/// A path after `--- ` or `+++ `: `None` for `/dev/null`, otherwise the path without its `a/` or
/// `b/` prefix. Git ends the line with a tab when the name contains a space.
pub fn side(raw: &str) -> Option<Option<String>> {
  let raw = raw.strip_suffix('\t').unwrap_or(raw);
  if raw == NULL {
    return Some(None);
  }
  let path = unquote(raw)?;
  Some(Some(strip_prefix(&path).to_owned()))
}

/// A path after `rename from `, `copy to ` and the like: no prefix, possibly quoted.
pub fn plain(raw: &str) -> Option<String> {
  unquote(raw).filter(|path| !path.is_empty())
}

/// Both paths of `diff --git a/<old> b/<new>`. Unquoted names containing ` b/` are ambiguous;
/// then the split that gives two equal names wins, which is the header of every change that is
/// not a rename or copy (and those name their paths again in later lines).
pub fn header(raw: &str) -> Option<(String, String)> {
  if raw.starts_with('"') {
    let end = closing_quote(raw)?;
    let old = unquote(&raw[..=end])?;
    let new = unquote(raw[end + 1..].strip_prefix(' ')?)?;
    return Some((strip_prefix(&old).to_owned(), strip_prefix(&new).to_owned()));
  }
  if let Some(space) = raw.find(" \"") {
    let new = unquote(&raw[space + 1..])?;
    return Some((strip_prefix(&raw[..space]).to_owned(), strip_prefix(&new).to_owned()));
  }
  let half = raw.len().checked_sub(1)? / 2;
  if raw.len() % 2 == 1 && raw.is_char_boundary(half) && raw.get(half..=half) == Some(" ") {
    let (old, new) = (&raw[..half], &raw[half + 1..]);
    if old
      .strip_prefix("a/")
      .is_some_and(|name| Some(name) == new.strip_prefix("b/"))
    {
      return Some((old[2..].to_owned(), new[2..].to_owned()));
    }
  }
  let (old, new) = raw.split_once(" b/")?;
  Some((strip_prefix(old).to_owned(), new.to_owned()))
}

fn strip_prefix(path: &str) -> &str {
  path
    .strip_prefix("a/")
    .or_else(|| path.strip_prefix("b/"))
    .unwrap_or(path)
}

/// The index of the quote closing the quoted name that starts `raw`.
fn closing_quote(raw: &str) -> Option<usize> {
  let mut escaped = false;
  for (index, byte) in raw.bytes().enumerate().skip(1) {
    match byte {
      _ if escaped => escaped = false,
      b'\\' => escaped = true,
      b'"' => return Some(index),
      _ => {}
    }
  }
  None
}

/// `raw` itself, or the name inside git's C-style quotes with its escapes resolved. Octal
/// escapes are bytes of the UTF-8 name.
fn unquote(raw: &str) -> Option<String> {
  let Some(inner) = raw.strip_prefix('"') else {
    return Some(raw.to_owned());
  };
  let inner = inner.strip_suffix('"')?;
  let mut bytes = Vec::with_capacity(inner.len());
  let mut rest = inner.bytes();
  while let Some(byte) = rest.next() {
    if byte != b'\\' {
      bytes.push(byte);
      continue;
    }
    let escaped = match rest.next()? {
      b'a' => 0x07,
      b'b' => 0x08,
      b't' => b'\t',
      b'n' => b'\n',
      b'v' => 0x0b,
      b'f' => 0x0c,
      b'r' => b'\r',
      digit @ b'0'..=b'3' => {
        let (second, third) = (rest.next()?, rest.next()?);
        let octal = [digit, second, third];
        if !octal.iter().all(|b| (b'0'..=b'7').contains(b)) {
          return None;
        }
        octal.iter().fold(0_u8, |value, b| value * 8 + (b - b'0'))
      }
      other => other,
    };
    bytes.push(escaped);
  }
  String::from_utf8(bytes).ok()
}
