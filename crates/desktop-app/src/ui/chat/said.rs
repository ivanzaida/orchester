//! What a person's message says in a chat (ORC-312, ORC-316, `08e` `dJyUv` and `09e` `y5fJD` in `Mentions ·
//! redesign`): the words they wrote, without the block of facts Orchester added for the agent
//! (`orchester_workspace::mentions::facts`), and each of their @-mentions a link the chat follows (`links`): a task to
//! its page, a run to its task's Conversation, a file, by its name, to the Files screen at the version the chat's
//! files are in (the project's checkout, a run's worktree). A message without mentions is its words as written; one
//! with mentions is lurq's Markdown, the person's words escaped so that they show as written, line by line. What a
//! search found in it is filled behind its words ([`super::found`]). A message sent with a skill (ORC-634) shows the
//! skill's chip, without its `x`, 6 over its words (`r78GAg` in `OKzZ6`, `07c` `f0b717`), instead of the skill's block.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::TypographyStyle},
  components::{Column, Text},
  layout::Alignment,
  node::Element,
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  files::Version,
  mentions::{Found, Mention, facts, find},
  skills::offered::OfferedSkill,
};

use super::{Said, agent_text, found, links::APP_SCHEME, marked_words, names::ProjectNames};
use crate::{
  routes::{FilesView, ProjectPage, Route, TaskView},
  theme::ExtraSpacing,
  ui::slash::sent_skill_chip,
};

/// What `body`, a message a person sent in a chat of `project` as it was kept, shows, with their words as they wrote
/// them for its Copy: `None` when it says nothing. A file it mentions opens at `files_at`. What a search found in it
/// (`searched`) is filled behind its words. `id` names it, its words `{id}-text`, the chip of its skill
/// `{id}-skill-chip`.
pub fn said(
  ctx: &mut Ctx,
  id: &str,
  (body, searched): (&str, Option<&found::Found>),
  (project, files_at): (ProjectId, &Version),
) -> Option<Said> {
  let words = facts::said(body);
  let chip = OfferedSkill::carried_by(body).map(|skill| sent_skill_chip(ctx, id, &skill));
  let shown = (!words.trim().is_empty()).then(|| shown_words(ctx, id, (words, searched), (project, files_at)));
  let shown = match (chip, shown) {
    (None, None) => return None,
    (None, Some(shown)) => shown,
    (Some(chip), shown) => Column::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Start)
      .child(chip)
      .with_children(shown)
      .into(),
  };
  Some((shown, Arc::from(words)))
}

/// What `body`, a message Orchester composed as Markdown for the person to send (a Fix CI, ORC-335), shows: its
/// Markdown as the chat draws an agent's, each fenced block a code block, with its words for its Copy; `None` when it
/// says nothing. What a search found in it (`searched`) is filled behind its words. Its words are `{id}-text`.
pub fn composed(
  ctx: &mut Ctx,
  id: &str,
  (body, searched): (&str, Option<&found::Found>),
  project: ProjectId,
) -> Option<Said> {
  if body.trim().is_empty() {
    return None;
  }
  let words: Arc<str> = Arc::from(body);
  let links = super::links(ctx);
  let shown = agent_text(
    ctx,
    &format!("{id}-text"),
    &words,
    (project, &ProjectNames::default()),
    (&links, searched),
  );
  Some((shown, words))
}

/// `words`, as the person wrote them: as written without mentions, else lurq's Markdown with their links.
fn shown_words(
  ctx: &mut Ctx,
  id: &str,
  (words, searched): (&str, Option<&found::Found>),
  (project, files_at): (ProjectId, &Version),
) -> Element {
  let text_id = format!("{id}-text");
  let found = find(words);
  if found.is_empty() {
    let text = Text::new(words)
      .id(text_id)
      .variant(TypographyStyle::Body)
      .selectable(true);
    return marked_words(ctx, text, words, searched);
  }
  let linked: Arc<str> = Arc::from(linked(words, &found, (project, files_at)));
  let links = super::links(ctx);
  agent_text(
    ctx,
    &text_id,
    &linked,
    (project, &ProjectNames::default()),
    (&links, searched),
  )
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
