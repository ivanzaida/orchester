//! A message cut into its Markdown and its top-level fenced code blocks: lurq's Markdown component
//! lays a code block out at the text's width and cuts its long lines off, so the chat lays out these
//! blocks itself, scrolling sideways ([`super::code_block`]). A fence counts as top-level when it
//! starts at the line's first column; one inside a list or a quote stays in its Markdown.

/// A run of Markdown, or a fenced code block with its language and its lines as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Segment<'a> {
  Markdown(String),
  Code { language: Option<&'a str>, code: &'a str },
}

/// The fence that opened a code block: its character, how many of them, the block's language and
/// where its lines start.
struct Fence<'a> {
  marker: char,
  length: usize,
  language: Option<&'a str>,
  start: usize,
}

impl<'a> Fence<'a> {
  /// A fence of three or more backticks or tildes at the start of `line`; the block's lines start at
  /// `end`, where `line` ends.
  fn open(line: &'a str, end: usize) -> Option<Self> {
    let marker = line.chars().next().filter(|marker| matches!(marker, '`' | '~'))?;
    let length = line.chars().take_while(|char| *char == marker).count();
    let info = line[length..].trim();
    // A backtick fence's info string has no backtick: "```a``b" is inline code.
    if length < 3 || (marker == '`' && info.contains('`')) {
      return None;
    }
    Some(Self {
      marker,
      length,
      language: info.split_whitespace().next(),
      start: end,
    })
  }

  /// Whether `line` closes the block: indented at most three spaces, at least as many of the same
  /// character, and nothing after them.
  fn closes(&self, line: &str) -> bool {
    let unindented = line.trim_start_matches(' ');
    if line.len() - unindented.len() > 3 {
      return false;
    }
    let length = unindented.chars().take_while(|char| *char == self.marker).count();
    length >= self.length && unindented[length..].trim().is_empty()
  }
}

/// `source` as Markdown and code blocks, in order. A block whose fence is not closed yet, as while a
/// message grows, runs to the end.
pub fn segments(source: &str) -> Vec<Segment<'_>> {
  let mut segments = Vec::new();
  let (mut text_start, mut offset) = (0, 0);
  let mut open: Option<Fence<'_>> = None;
  for line in source.split_inclusive('\n') {
    let end = offset + line.len();
    match &open {
      None => {
        if let Some(fence) = Fence::open(line, end) {
          push_markdown(&mut segments, &source[text_start..offset]);
          open = Some(fence);
        }
      }
      Some(fence) if fence.closes(line) => {
        segments.push(code(fence, &source[fence.start..offset]));
        open = None;
        text_start = end;
      }
      Some(_) => {}
    }
    offset = end;
  }
  match &open {
    Some(fence) => segments.push(code(fence, &source[fence.start..])),
    None => push_markdown(&mut segments, &source[text_start..]),
  }
  segments
}

fn code<'a>(fence: &Fence<'a>, lines: &'a str) -> Segment<'a> {
  Segment::Code {
    language: fence.language,
    code: lines.trim_end_matches(['\r', '\n']),
  }
}

fn push_markdown(segments: &mut Vec<Segment<'_>>, text: &str) {
  if !text.trim().is_empty() {
    segments.push(Segment::Markdown(keep_line_breaks(text)));
  }
}

/// `text` with each line break it was written with kept as one, as a chat reads: Markdown would join
/// the lines of a paragraph into one. Two spaces before a line break make it a hard break; they do
/// nothing at the end of a block (a heading, a table row, the last line of a paragraph).
pub fn keep_line_breaks(text: &str) -> String {
  let lines: Vec<&str> = text.lines().collect();
  let mut kept = String::with_capacity(text.len() + 2 * lines.len());
  for (index, line) in lines.iter().enumerate() {
    kept.push_str(line);
    let continued = lines.get(index + 1).is_some_and(|next| !next.trim().is_empty());
    if continued && !line.trim().is_empty() && !line.ends_with("  ") && !line.ends_with('\\') {
      kept.push_str("  ");
    }
    kept.push('\n');
  }
  kept
}
