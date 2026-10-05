//! Output limits of the file tools (`docs/architecture/native-agent.md` §6.4). A cut result says so in
//! its text, so that the model can narrow its request.

/// The most bytes any result holds, after the tool's own limit.
pub(super) const RESULT_CAP: usize = 128 * 1024;

/// `text` within [`RESULT_CAP`], with a note when it was cut.
pub(super) fn capped(text: String) -> String {
  if text.len() <= RESULT_CAP {
    return text;
  }
  let note = format!("\n[truncated: the result was cut at {RESULT_CAP} bytes]");
  let mut cut = text;
  cut.truncate(floor_boundary(&cut, RESULT_CAP - note.len()));
  cut.push_str(&note);
  cut
}

/// `line` with at most `max_chars` characters, marked when it was cut.
pub(super) fn cut_line(line: &str, max_chars: usize) -> String {
  match line.char_indices().nth(max_chars) {
    Some((end, _)) => format!("{}… [line cut at {max_chars} characters]", &line[..end]),
    None => line.to_owned(),
  }
}

/// The largest char boundary of `text` not after `index`.
fn floor_boundary(text: &str, index: usize) -> usize {
  (0..=index.min(text.len()))
    .rev()
    .find(|at| text.is_char_boundary(*at))
    .unwrap_or(0)
}
