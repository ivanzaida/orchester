//! The headings of a Markdown file, as CommonMark reads them: `#` headings and underlined (setext)
//! ones, outside fenced code blocks and a leading YAML front matter.

/// A heading and the line it starts on (1-based). `text` is written the `#` way:
/// `## Backlog`, also for an underlined heading.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Heading {
  pub(crate) line: usize,
  pub(crate) text: String,
}

/// The opening of a fenced code block: its character and length.
struct Fence {
  marker: char,
  length: usize,
}

pub(crate) fn headings(source: &str) -> Vec<Heading> {
  let mut headings = Vec::new();
  let mut fence: Option<Fence> = None;
  // The previous line, when it is paragraph text an underline makes a heading of.
  let mut paragraph: Option<(usize, &str)> = None;
  for (index, line) in source.lines().enumerate().skip(front_matter(source)) {
    let number = index + 1;
    if let Some(open) = &fence {
      if closes(line, open) {
        fence = None;
      }
      continue;
    }
    let content = line.trim_start_matches(' ');
    let indented = line.len() - content.len() > 3;
    if !indented && let Some(open) = opens(content) {
      fence = Some(open);
      paragraph = None;
      continue;
    }
    if let Some((start, text)) = paragraph.filter(|_| !indented)
      && let Some(level) = underline(content)
    {
      headings.push(Heading {
        line: start,
        text: format!("{} {}", "#".repeat(level), text.trim()),
      });
      paragraph = None;
      continue;
    }
    if !indented && let Some(text) = atx(content) {
      headings.push(Heading { line: number, text });
      paragraph = None;
      continue;
    }
    paragraph = (!content.trim().is_empty() && !indented && !starts_block(content)).then_some((number, content));
  }
  headings
}

/// How many lines a YAML front matter takes at the start of `source`; 0 without one.
fn front_matter(source: &str) -> usize {
  let mut lines = source.lines();
  if lines.next().map(str::trim_end) != Some("---") {
    return 0;
  }
  lines
    .position(|line| matches!(line.trim_end(), "---" | "..."))
    .map_or(0, |closing| closing + 2)
}

/// A list item or quote: an underline after it is a rule, not a heading.
fn starts_block(content: &str) -> bool {
  let ordered = content.split_once(['.', ')']).is_some_and(|(number, rest)| {
    !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()) && rest.starts_with(' ')
  });
  ordered || content.starts_with(['>']) || ["- ", "* ", "+ "].iter().any(|marker| content.starts_with(marker))
}

/// A `#` heading: one to six `#` and a space or the end of the line.
fn atx(content: &str) -> Option<String> {
  let level = content.chars().take_while(|character| *character == '#').count();
  let rest = &content[level..];
  let heading = (1..=6).contains(&level) && (rest.is_empty() || rest.starts_with([' ', '\t']));
  heading.then(|| content.trim_end().to_owned())
}

/// The level of an underline: `===` for 1, `---` for 2.
fn underline(content: &str) -> Option<usize> {
  let content = content.trim_end();
  let level = match content.chars().next()? {
    '=' => 1,
    '-' => 2,
    _ => return None,
  };
  let marker = content.chars().next()?;
  content.chars().all(|character| character == marker).then_some(level)
}

fn opens(content: &str) -> Option<Fence> {
  let marker = content.chars().next().filter(|marker| matches!(marker, '`' | '~'))?;
  let length = content.chars().take_while(|character| *character == marker).count();
  // A backtick fence's info string cannot hold a backtick.
  let info_ok = marker != '`' || !content[length..].contains('`');
  (length >= 3 && info_ok).then_some(Fence { marker, length })
}

fn closes(line: &str, open: &Fence) -> bool {
  let content = line.trim_start_matches(' ');
  if line.len() - content.len() > 3 {
    return false;
  }
  let length = content
    .chars()
    .take_while(|character| *character == open.marker)
    .count();
  length >= open.length && content[length..].trim().is_empty()
}
