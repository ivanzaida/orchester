//! The YAML front matter of a repository's `SKILL.md`: only its `name` and `description`, the two
//! keys both CLIs read. The scalar forms skill files use are understood — plain (continued on
//! indented lines), single- or double-quoted, and `|` / `>` blocks; other keys are skipped. A file
//! without front matter is all body.

/// A `SKILL.md` split into what its front matter says and its Markdown body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillMarkdown {
  pub name: Option<String>,
  pub description: Option<String>,
  pub body: String,
}

/// Splits `text` into its front matter's `name` and `description`, and the body after it.
pub fn parse(text: &str) -> SkillMarkdown {
  let text = text.strip_prefix('\u{feff}').unwrap_or(text);
  let lines: Vec<&str> = text.lines().collect();
  let close = (lines.first().map(|line| line.trim_end()) == Some("---"))
    .then(|| lines.iter().skip(1).position(|line| line.trim_end() == "---"))
    .flatten()
    .map(|position| position + 1);
  let Some(close) = close else {
    return SkillMarkdown {
      name: None,
      description: None,
      body: text.trim().to_owned(),
    };
  };
  let header = &lines[1..close];
  SkillMarkdown {
    name: value(header, "name"),
    description: value(header, "description"),
    body: lines[close + 1..].join("\n").trim().to_owned(),
  }
}

/// The value of top-level `key` in `header`, `None` when it is missing or empty.
fn value(header: &[&str], key: &str) -> Option<String> {
  let prefix = format!("{key}:");
  let index = header.iter().position(|line| line.starts_with(&prefix))?;
  let first = header[index][prefix.len()..].trim();
  let continuation: Vec<&str> = header[index + 1..]
    .iter()
    .take_while(|line| line.trim().is_empty() || line.starts_with([' ', '\t']))
    .copied()
    .collect();
  let lines = || std::iter::once(first).chain(continuation.iter().map(|line| line.trim()));
  let text = match first.chars().next() {
    Some('|' | '>') => block(first, &continuation),
    Some('"') => double_quoted(&folded(lines())),
    Some('\'') => single_quoted(&folded(lines())),
    _ => folded(lines()),
  };
  let text = text.trim().to_owned();
  (!text.is_empty()).then_some(text)
}

/// Lines folded into one: joined with spaces, a blank line kept as a line break.
fn folded<'a>(lines: impl Iterator<Item = &'a str>) -> String {
  let mut text = String::new();
  for line in lines {
    if line.is_empty() {
      text.push('\n');
    } else {
      if !text.is_empty() && !text.ends_with('\n') {
        text.push(' ');
      }
      text.push_str(line);
    }
  }
  text
}

/// A `|` (lines kept) or `>` (lines folded) block: its indented lines without their indentation.
fn block(indicator: &str, lines: &[&str]) -> String {
  let indent = lines
    .iter()
    .filter(|line| !line.trim().is_empty())
    .map(|line| line.len() - line.trim_start().len())
    .min()
    .unwrap_or(0);
  let content = lines.iter().map(|line| line.get(indent..).unwrap_or("").trim_end());
  if indicator.starts_with('|') {
    content.collect::<Vec<_>>().join("\n")
  } else {
    folded(content)
  }
}

/// A `"…"` scalar: YAML's escapes are JSON's for the text skill descriptions hold.
fn double_quoted(text: &str) -> String {
  let Some(end) = closing_quote(text) else {
    return text.trim_start_matches('"').to_owned();
  };
  let quoted = &text[..=end];
  serde_json::from_str(quoted).unwrap_or_else(|_| quoted.trim_matches('"').to_owned())
}

/// The byte offset of the `"` that closes the scalar opened at the start of `text`.
fn closing_quote(text: &str) -> Option<usize> {
  let mut escaped = false;
  for (offset, c) in text.char_indices().skip(1) {
    match c {
      '\\' if !escaped => escaped = true,
      '"' if !escaped => return Some(offset),
      _ => escaped = false,
    }
  }
  None
}

/// A `'…'` scalar, where `''` is a single quote.
fn single_quoted(text: &str) -> String {
  let inner = &text[1..];
  let mut value = String::new();
  let mut chars = inner.chars().peekable();
  while let Some(c) = chars.next() {
    if c == '\'' {
      if chars.peek() == Some(&'\'') {
        chars.next();
        value.push('\'');
      } else {
        break;
      }
    } else {
      value.push(c);
    }
  }
  value
}
