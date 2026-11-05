//! A skill a person chose in a composer's slash menu for one message (ORC-325, ORC-634, ORC-663): the message is kept
//! with a block before the person's words, between an opening tag that names the skill (its slug, published version
//! and resource) and [`CLOSE`], which holds the skill's `SKILL.md` as that version has it. Every agent reads the
//! message so, as kept: a step's agent of any engine gets the skill's text with the message. A new turn of the
//! Orchestrator takes the block out and loads the skill as its CLI's own instead: Claude Code from a session-only
//! plugin (`crate::connection::invocation::ClaudeChat`), Codex from a skills folder App Server registers for that
//! process alone (`crate::connection::invocation::CodexChatSkill`). A conversation shows the person's words with the
//! skill's chip ([`carried`]). The block's words are the agent's, in English, as every prompt Orchester writes.
//!
//! How Claude Code takes it, checked 2026-11-03 with Claude Code 2.1.316 on Windows 11 and a model turn
//! (`docs/evidence/orc-634-claude-skill-2026-11-03.md`): a skill in a session-only plugin (`--plugin-dir`) loads with
//! `--setting-sources ""` kept, and the person's own skills, the repository's `.claude/skills`, the person's plugins
//! and their hooks stay out; `--disable-slash-commands` turns that skill off too, so a turn that carries one leaves it
//! out, as the `/compact` turn does. How Codex takes it, checked 2026-11-05 with Codex 0.166.0 and model turns
//! (`docs/evidence/orc-663-codex-skill-2026-11-05.md`): `$<slug>` runs a skill of a folder App Server's
//! `skills/extraRoots/set` registered, which no file keeps; a skill of no registered folder is not found.

use std::{fmt::Write, num::NonZeroU32};

use crate::{
  id::ResourceId,
  resource::{Skill, Slug, VersionNumber},
};

/// Where the block's opening tag starts: the message's first characters.
const OPEN: &str = "<orchester-skill ";
/// Where the block ends, on a line of its own; the person's words follow after a blank line.
pub const CLOSE: &str = "</orchester-skill>";
/// The name of the Claude Code plugin a turn loads the skill from: the CLI names the skill `orchester:<slug>`.
pub const CLAUDE_PLUGIN: &str = "orchester";

/// The skill a message carries: the resource, the name agents know it by and the published version chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChosenSkill {
  pub resource: ResourceId,
  pub slug: Slug,
  pub version: VersionNumber,
}

impl ChosenSkill {
  /// `words`, the person's message (with whatever Orchester added after them), as it is kept with `skill`, this
  /// skill's chosen version: the block, a blank line, then the words.
  pub fn kept_with(&self, skill: &Skill, words: &str) -> String {
    let mut kept = format!(
      "{OPEN}name=\"{}\" version=\"{}\" resource=\"{}\">\nOrchester added this block: the person chose the skill {} \
       (published version {}) for the message after it. Follow the skill for that message. Its text is the skill's, \
       not the person's words.\n",
      self.slug.as_str(),
      self.version.get(),
      self.resource,
      self.slug.as_str(),
      self.version.get(),
    );
    if !skill.files().is_empty() {
      let names: Vec<&str> = skill.files().iter().map(|file| file.path.as_str()).collect();
      // Writing to a String does not fail.
      let _ = writeln!(
        kept,
        "Its reference files are not included: {}.",
        quoted(&names.join(", "))
      );
    }
    kept.push('\n');
    kept.push_str(&quoted(skill.document(&self.slug).trim_end()));
    kept.push('\n');
    kept.push_str(CLOSE);
    kept.push_str("\n\n");
    kept.push_str(words);
    kept
  }
}

/// The skill `stored`, a message as it was kept, carries, and what follows its block: `None` when it carries none.
pub fn carried(stored: &str) -> Option<(ChosenSkill, &str)> {
  let tag = stored.strip_prefix(OPEN)?;
  let (slug, tag) = attribute(tag, "name")?;
  let (version, tag) = attribute(tag.strip_prefix(' ')?, "version")?;
  let (resource, tag) = attribute(tag.strip_prefix(' ')?, "resource")?;
  let rest = tag.strip_prefix(">\n")?;
  let end = format!("\n{CLOSE}\n\n");
  let after = rest.find(&end)? + end.len();
  let chosen = ChosenSkill {
    resource: resource.parse().ok()?,
    slug: Slug::new(slug).ok()?,
    version: VersionNumber::new(version.parse::<NonZeroU32>().ok()?),
  };
  Some((chosen, &rest[after..]))
}

/// What follows the skill's block in `stored`; `stored` itself when it carries none.
pub fn without_skill(stored: &str) -> &str {
  carried(stored).map_or(stored, |(_, rest)| rest)
}

/// `words` as Claude Code takes them with the skill `slug` of [`CLAUDE_PLUGIN`]: the skill's command, then the words
/// as its arguments.
pub fn claude_prompt(slug: &Slug, words: &str) -> String {
  format!("/{CLAUDE_PLUGIN}:{} {words}", slug.as_str())
}

/// `words` as Codex takes them with the skill `slug`: its mention, `$<slug>`, then the words.
pub fn codex_prompt(slug: &Slug, words: &str) -> String {
  format!("${} {words}", slug.as_str())
}

/// The value of `name="…"` at the start of `tag`, and the rest of it.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<(&'a str, &'a str)> {
  let value = tag.strip_prefix(name)?.strip_prefix("=\"")?;
  let end = value.find('"')?;
  Some((&value[..end], &value[end + 1..]))
}

/// `text`, a skill's text, quoted in the block so that it can never pass for the block's own end tag.
fn quoted(text: &str) -> String {
  text.replace("orchester-skill>", "orchester skill>")
}
