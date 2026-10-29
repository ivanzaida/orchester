//! `browser_console` and `browser_network`: what a tab reported since it opened, newest last, filtered by literal text
//! and cut to the newest `limit` entries. Requests show their method, URL, status, type and size, never headers or
//! bodies.

use orchester_core::orchester_tools::browser::PAGE_DATA_NOTICE;

use crate::browser::Tab;

/// The console messages of `tab`, only errors and uncaught exceptions when `errors_only`, only those holding
/// `pattern` when one is given.
pub(super) fn console(tab: &Tab, errors_only: bool, pattern: Option<&str>, limit: u32) -> String {
  let log = tab.log();
  let lines: Vec<String> = log
    .console()
    .filter(|message| !errors_only || message.is_error())
    .filter(|message| pattern.is_none_or(|pattern| message.text.contains(pattern)))
    .map(|message| format!("[{}] {}", message.level, message.text))
    .collect();
  listed(&lines, limit, "console messages", tab)
}

/// The requests of `tab`, only those whose URL holds `url_pattern` when one is given.
pub(super) fn network(tab: &Tab, url_pattern: Option<&str>, limit: u32) -> String {
  let log = tab.log();
  let lines: Vec<String> = log
    .requests()
    .filter(|request| url_pattern.is_none_or(|pattern| request.url.contains(pattern)))
    .map(|request| {
      let status = match (&request.failed, request.status) {
        (Some(failed), _) => format!("failed ({failed})"),
        (None, Some(status)) => status.to_string(),
        (None, None) => "pending".to_owned(),
      };
      let size = request.size.map_or_else(String::new, |size| format!(" {size} B"));
      format!("{} {} {}{size} {}", request.method, status, request.kind, request.url)
    })
    .collect();
  listed(&lines, limit, "requests", tab)
}

/// The newest `limit` of `lines`, saying how many older ones were left out.
fn listed(lines: &[String], limit: u32, what: &str, tab: &Tab) -> String {
  if lines.is_empty() {
    return format!("No {what} in tab {} match.", tab.id());
  }
  let limit = usize::try_from(limit).unwrap_or(usize::MAX);
  let older = lines.len().saturating_sub(limit);
  let mut text = format!(
    "{PAGE_DATA_NOTICE}\nTab {}, {what} since it opened, newest last:",
    tab.id()
  );
  if older > 0 {
    text.push_str(&format!(
      "\n[{older} older {what} left out; raise limit or narrow the pattern]"
    ));
  }
  for line in &lines[older..] {
    text.push('\n');
    text.push_str(line);
  }
  text
}
