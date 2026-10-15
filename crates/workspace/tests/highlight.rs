//! Syntax highlighting of diff lines: tokens and their kinds for a Rust hunk, a TypeScript file read
//! with the JavaScript grammar, a file without a grammar, and what a line shows of tabs and
//! control characters.

use orchester_core::diff::Diff;
use orchester_workspace::highlight::{Token, TokenKind, highlight, highlight_lines, plain};

fn file_diff(path: &str, lines: &[&str]) -> Diff {
  let mut text = format!(
    "diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1,{old} +1,{new} @@\n",
    old = lines.iter().filter(|line| !line.starts_with('+')).count(),
    new = lines.iter().filter(|line| !line.starts_with('-')).count(),
  );
  for line in lines {
    text.push_str(line);
    text.push('\n');
  }
  Diff::parse(&text).expect("a readable diff")
}

fn kind_of(tokens: &[Token], text: &str) -> Option<TokenKind> {
  tokens.iter().find(|token| &*token.text == text).map(|token| token.kind)
}

#[test]
fn highlight_gives_rust_keywords_strings_and_comments_their_kinds() {
  let diff = file_diff(
    "src/main.rs",
    &[
      " fn main() {",
      "-    println!(\"hello\");",
      "+    let name = \"world\"; // greet",
      " }",
    ],
  );
  let tokens = highlight(&diff.files()[0]).expect("Rust has a grammar");
  let lines = &tokens[0];
  assert_eq!(kind_of(&lines[0], "fn"), Some(TokenKind::Keyword));
  assert_eq!(kind_of(&lines[0], "main"), Some(TokenKind::Function));
  assert_eq!(kind_of(&lines[1], "\"hello\""), Some(TokenKind::String));
  assert_eq!(kind_of(&lines[2], "let"), Some(TokenKind::Keyword));
  assert_eq!(kind_of(&lines[2], "\"world\""), Some(TokenKind::String));
  assert_eq!(kind_of(&lines[2], "// greet"), Some(TokenKind::Comment));
  // The tokens of a line spell the line.
  let spelled: String = lines[2].iter().map(|token| &*token.text).collect();
  assert_eq!(spelled, "    let name = \"world\"; // greet");
}

#[test]
fn typescript_is_read_with_the_javascript_grammar() {
  let diff = file_diff("src/auth/github.ts", &["+export async function signIn() {", "+}"]);
  let tokens = highlight(&diff.files()[0]).expect("JavaScript reads TypeScript");
  assert_eq!(kind_of(&tokens[0][0], "function"), Some(TokenKind::Keyword));
}

#[test]
fn a_file_without_a_grammar_is_not_highlighted() {
  let diff = file_diff("notes.unknownext", &["+plain words"]);
  assert!(highlight(&diff.files()[0]).is_none());
}

#[test]
fn shown_text_expands_tabs_and_replaces_control_characters() {
  let tokens = plain("a\tb\u{1b}[31m");
  assert_eq!(&*tokens[0].text, "a   b\u{FFFD}[31m");
}

/// One file of microsoft/vscode#337624: an added line of 1,674 characters, cut inside a string,
/// before the context line 130.
const VSCODE_337624: &str =
  include_str!("../../core/tests/fixtures/github/gh_api_compare_vscode_337624_excerpt.stdout");

#[test]
fn highlight_after_a_cut_line_continues_from_the_other_side() {
  let diff = Diff::parse(VSCODE_337624).expect("a readable diff");
  let file = &diff.files()[0];
  let lines = file.hunks()[0].lines();
  assert!(lines[3].is_cut(), "line 128 is cut inside its string");
  let tokens = highlight(file).expect("JavaScript reads TypeScript");
  // Line 130, `if (type === 'agentView') {`, and 131 read as code, not as the cut line's string.
  assert_eq!(kind_of(&tokens[0][4], "if"), Some(TokenKind::Keyword));
  assert_eq!(kind_of(&tokens[0][4], "'agentView'"), Some(TokenKind::String));
  assert_eq!(kind_of(&tokens[0][5], "push"), Some(TokenKind::Function));
  assert_eq!(kind_of(&tokens[0][5], "localize"), Some(TokenKind::Function));
}

#[test]
fn highlight_reads_operators_and_variants_as_text_and_self_async_and_await_as_keywords() {
  let diff = file_diff(
    "src/sender.rs",
    &[
      "+    pub async fn send(&self) -> Result<Delivery> {",
      "+        let response = self.client.post(&self.url).send().await?;",
      "+        attempt += 1;",
      "+        Ok(r) if a < b && c || d <= e * 2 => return Ok(r),",
    ],
  );
  let tokens = highlight(&diff.files()[0]).expect("Rust has a grammar");
  let lines = &tokens[0];
  assert_eq!(kind_of(&lines[0], "async"), Some(TokenKind::Keyword));
  assert_eq!(
    kind_of(&lines[0], "self"),
    Some(TokenKind::Keyword),
    "self as a parameter"
  );
  assert_eq!(kind_of(&lines[1], "self"), Some(TokenKind::Keyword));
  assert_eq!(kind_of(&lines[1], "await"), Some(TokenKind::Keyword));
  assert_eq!(kind_of(&lines[2], "1"), Some(TokenKind::Constant));
  assert!(
    lines[3]
      .iter()
      .all(|token| !token.text.contains("Ok") || token.kind == TokenKind::Plain),
    "an enum variant reads as text: {:?}",
    lines[3]
  );
  for (line, operators) in [
    (1, ["=", "&", "?"].as_slice()),
    (2, &["+="]),
    (3, &["<", "&&", "||", "<=", "*", "=>"]),
  ] {
    for token in lines[line]
      .iter()
      .filter(|token| operators.iter().any(|operator| token.text.contains(operator)))
    {
      assert_eq!(token.kind, TokenKind::Plain, "{:?} on line {line}", token.text);
    }
  }
  let spelled: String = lines[1].iter().map(|token| &*token.text).collect();
  assert_eq!(
    spelled,
    "        let response = self.client.post(&self.url).send().await?;"
  );
}

#[test]
fn highlight_lines_reads_a_whole_file_from_its_start() {
  let lines = ["/* a comment", "   that goes on */", "fn main() {}"];
  let tokens = highlight_lines("src/lib.rs", &lines, &|| true).expect("Rust has a grammar");
  assert_eq!(tokens.len(), 3);
  assert_eq!(kind_of(&tokens[1], "   that goes on */"), Some(TokenKind::Comment));
  assert_eq!(kind_of(&tokens[2], "fn"), Some(TokenKind::Keyword));
  assert!(highlight_lines("notes.unknownext", &lines, &|| true).is_none());
  assert!(
    highlight_lines("src/lib.rs", &lines, &|| false).is_none(),
    "superseded, it stops"
  );
}
