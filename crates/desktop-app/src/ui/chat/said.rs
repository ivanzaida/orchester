//! What a person's message says in a chat (ORC-312, `08e` `dJyUv` and `09e` `y5fJD` in `Mentions · redesign`): the
//! words they wrote, without the block of facts Orchester added for the agent (`orchester_workspace::mentions::facts`),
//! and each of their @-mentions a link the chat follows (`links`): a task to its page, a run to its task's
//! Conversation. A message without mentions is its words as written; one with mentions is lurq's Markdown, the
//! person's words escaped so that they show as written, line by line.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::TypographyStyle},
  components::Text,
  node::Element,
};
use orchester_core::id::ProjectId;
use orchester_workspace::mentions::{Found, Mention, facts, find};

use super::{agent_text, links::APP_SCHEME, names::ProjectNames};
use crate::routes::{ProjectPage, Route, TaskView};

/// What `body`, a message a person sent in a chat of `project` as it was kept, shows: `None` when it says nothing.
/// `id` names it, its words `{id}-text`.
pub fn said(ctx: &mut Ctx, id: &str, body: &str, project: ProjectId) -> Option<Element> {
  let words = facts::said(body);
  if words.trim().is_empty() {
    return None;
  }
  let text_id = format!("{id}-text");
  let found = find(words);
  if found.is_empty() {
    return Some(
      Text::new(words)
        .id(text_id)
        .variant(TypographyStyle::Body)
        .selectable(true)
        .into(),
    );
  }
  let linked: Arc<str> = Arc::from(linked(words, &found, project));
  let links = super::links(ctx);
  Some(agent_text(
    ctx,
    &text_id,
    &linked,
    (project, &ProjectNames::default()),
    &links,
  ))
}

/// `words` as Markdown that shows them as written, with each mention `found` in them a link to its place in
/// `project`.
pub fn linked(words: &str, found: &[Found], project: ProjectId) -> String {
  let mut linked = String::with_capacity(words.len() * 2);
  let mut at = 0;
  for mention in found {
    escape_into(&mut linked, &words[at..mention.range.start]);
    let (shown, view) = match &mention.mention {
      Mention::Task(key) => (key.to_string(), TaskView::Summary),
      Mention::Run(key, number) => (format!("{key} · #{}", number.get()), TaskView::Conversation),
    };
    linked.push('[');
    escape_into(&mut linked, &shown);
    linked.push_str("](");
    linked.push_str(APP_SCHEME);
    let route = Route::Project(project, ProjectPage::Task(mention.mention.task().clone(), view));
    linked.push_str(&route.path());
    linked.push(')');
    at = mention.range.end;
  }
  escape_into(&mut linked, &words[at..]);
  linked
}

/// Writes `text` so that Markdown shows it as written: its punctuation escaped, a line break kept as one, the spaces
/// that start a line kept without making it code.
fn escape_into(out: &mut String, text: &str) {
  let mut line_start = out.is_empty() || out.ends_with('\n');
  let mut chars = text.chars().peekable();
  while let Some(ch) = chars.next() {
    match ch {
      '\n' => {
        // A line that goes on in the same paragraph breaks there; an empty line starts a new paragraph.
        let ends_paragraph = out.ends_with('\n') || chars.peek().is_none_or(|next| *next == '\n');
        if !ends_paragraph {
          out.push('\\');
        }
        out.push('\n');
        line_start = true;
        continue;
      }
      ' ' | '\t' if line_start => out.push('\u{a0}'),
      ch if ch.is_ascii_punctuation() => {
        out.push('\\');
        out.push(ch);
      }
      ch => out.push(ch),
    }
    line_start = line_start && ch.is_whitespace();
  }
}
