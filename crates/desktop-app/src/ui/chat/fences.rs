//! What an agent wrote, split at its fenced code blocks that stand on their own, outside a list or a quote, so the
//! chat draws each of them with a head and a Copy of its own (`Code block` `PqqIK` in 07 `GgXmd`, `chLpT` in 09
//! `w7TIt`, ORC-323) and the Markdown around them as lurq draws it. A block in a list or a quote stays lurq's.

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

/// A part of a message, in the order it was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece<'a> {
  /// Markdown between code blocks, as written.
  Text(&'a str),
  /// A fenced code block.
  Code(CodeBlock),
}

/// A fenced code block: the language and the file its fence names, and its code without the fence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeBlock {
  pub language: Option<String>,
  pub file: Option<String>,
  pub code: String,
}

/// `text` split at its fenced code blocks that stand on their own; Markdown between them that holds only blank lines
/// is left out.
pub fn pieces(text: &str) -> Vec<Piece<'_>> {
  let mut options = Options::empty();
  options.insert(Options::ENABLE_TABLES);
  options.insert(Options::ENABLE_STRIKETHROUGH);
  options.insert(Options::ENABLE_TASKLISTS);
  options.insert(Options::ENABLE_FOOTNOTES);
  options.insert(Options::ENABLE_MATH);
  let mut pieces = Vec::new();
  let (mut depth, mut at) = (0_usize, 0_usize);
  let mut block: Option<CodeBlock> = None;
  for (event, range) in Parser::new_ext(text, options).into_offset_iter() {
    match event {
      Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) if depth == 0 => {
        push_text(&mut pieces, &text[at..range.start]);
        let (language, file) = label(&info);
        block = Some(CodeBlock {
          language,
          file,
          code: String::new(),
        });
        depth += 1;
      }
      Event::End(TagEnd::CodeBlock) if depth == 1 && block.is_some() => {
        if let Some(mut code) = block.take() {
          if code.code.ends_with('\n') {
            code.code.pop();
          }
          pieces.push(Piece::Code(code));
        }
        at = range.end;
        depth -= 1;
      }
      Event::Text(words) => {
        if let Some(code) = block.as_mut() {
          code.code.push_str(&words);
        }
      }
      Event::Start(_) => depth += 1,
      Event::End(_) => depth = depth.saturating_sub(1),
      _ => {}
    }
  }
  push_text(&mut pieces, &text[at..]);
  pieces
}

fn push_text<'a>(pieces: &mut Vec<Piece<'a>>, text: &'a str) {
  if !text.trim().is_empty() {
    pieces.push(Piece::Text(text));
  }
}

/// The language and the file a fence's info names: `rust`, `rust sender.rs`, `rust:sender.rs` or
/// `rust title="sender.rs"`.
fn label(info: &str) -> (Option<String>, Option<String>) {
  let mut words = info.split_whitespace();
  let Some(first) = words.next() else {
    return (None, None);
  };
  let (language, file) = match first.split_once(':') {
    Some((language, file)) => (language, Some(file)),
    None => (first, None),
  };
  let file = file
    .or_else(|| words.next().map(|word| word.strip_prefix("title=").unwrap_or(word)))
    .map(|file| file.trim_matches('"'))
    .filter(|file| !file.is_empty())
    .map(str::to_owned);
  ((!language.is_empty()).then(|| language.to_owned()), file)
}
