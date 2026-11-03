//! What a person's message says in a chat (ORC-312, ORC-316, `08e` `dJyUv` and `09e` `y5fJD` in `Mentions ·
//! redesign`): the words they wrote, without the block of facts Orchester added for the agent
//! (`orchester_workspace::mentions::facts`), and each of their @-mentions a link the chat follows (`links`): a task to
//! its page, a run to its task's Conversation, a file, by its name, to the Files screen at the version the chat's
//! files are in (the project's checkout, a run's worktree). A message without mentions is its words as written; one
//! with mentions is lurq's Markdown, the person's words escaped so that they show as written, line by line. What a
//! search found in it is filled behind its words ([`super::found`]).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::TypographyStyle},
  components::Text,
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  files::Version,
  mentions::{Found, Mention, facts, find},
};

use super::{Said, agent_text, found, links::APP_SCHEME, marked_words, names::ProjectNames};
use crate::routes::{FilesView, ProjectPage, Route, TaskView};

/// What `body`, a message a person sent in a chat of `project` as it was kept, shows, with their words as they wrote
/// them for its Copy: `None` when it says nothing. A file it mentions opens at `files_at`. What a search found in it
/// (`searched`) is filled behind its words. `id` names it, its words `{id}-text`.
pub fn said(
  ctx: &mut Ctx,
  id: &str,
  (body, searched): (&str, Option<&found::Found>),
  (project, files_at): (ProjectId, &Version),
) -> Option<Said> {
  let words = facts::said(body);
  if words.trim().is_empty() {
    return None;
  }
  let text_id = format!("{id}-text");
  let found = find(words);
  if found.is_empty() {
    let text = Text::new(words)
      .id(text_id)
      .variant(TypographyStyle::Body)
      .selectable(true);
    return Some((marked_words(ctx, text, words, searched), Arc::from(words)));
  }
  let linked: Arc<str> = Arc::from(linked(words, &found, (project, files_at)));
  let links = super::links(ctx);
  let shown = agent_text(
    ctx,
    &text_id,
    &linked,
    (project, &ProjectNames::default()),
    (&links, searched),
  );
  Some((shown, Arc::from(words)))
}

/// `words` as Markdown that shows them as written, with each mention `found` in them a link to its place in
/// `project`; a file's to the Files screen at `files_at`.
pub fn linked(words: &str, found: &[Found], (project, files_at): (ProjectId, &Version)) -> String {
  let mut linked = String::with_capacity(words.len() * 2);
  let mut at = 0;
  for mention in found {
    escape_into(&mut linked, &words[at..mention.range.start]);
    let (shown, page) = match &mention.mention {
      Mention::Task(key) => (key.to_string(), ProjectPage::Task(key.clone(), TaskView::Summary)),
      Mention::Run(key, number) => (
        format!("{key} · #{}", number.get()),
        ProjectPage::Task(key.clone(), TaskView::Conversation),
      ),
      Mention::File(path) => (
        path.name().to_owned(),
        ProjectPage::Files(FilesView::new(Some(path.as_str()), files_at.clone())),
      ),
    };
    linked.push('[');
    escape_into(&mut linked, &shown);
    linked.push_str("](");
    linked.push_str(APP_SCHEME);
    linked.push_str(&Route::Project(project, page).path());
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
