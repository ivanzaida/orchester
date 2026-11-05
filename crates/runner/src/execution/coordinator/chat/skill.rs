//! A skill the person chose for a message to the Orchestrator (ORC-634, ORC-663,
//! `orchester_core::conversation::skill`), as a new turn loads it as its CLI's own: the version the message names,
//! read from the project, is written in `<project>/.orchester/chat-skills/<message>/` and removed once the CLI is done
//! with it. For Claude Code that folder is a session-only plugin that holds the skill alone; for Codex it is a skills
//! folder of that skill alone (`<slug>/SKILL.md`), which the turn's App Server registers
//! (`orchester_core::connection::invocation::CodexChatSkill`). A version the project no longer has, or a folder that
//! cannot be written, leaves the message as it was kept: the agent still reads the skill's text in its block.

use std::{
  fs, io,
  path::{Path, PathBuf},
};

use orchester_core::{
  conversation::skill::{CLAUDE_PLUGIN, ChosenSkill, carried},
  resource::{SKILL_FILE, Skill, Slug},
  store::{BlobStore, ResourceStore},
};
use orchester_store::ProjectDb;

use super::turn::Turn;

/// How the CLI loads the skill's folder.
#[derive(Clone, Copy)]
pub(super) enum SkillLayout {
  /// A Claude Code plugin of [`CLAUDE_PLUGIN`]: `.claude-plugin/plugin.json` and `skills/<slug>/`.
  ClaudePlugin,
  /// A Codex skills folder: `<slug>/`.
  CodexRoot,
}

/// A skill placed for one turn, removed when dropped: the CLI has exited by then.
pub(super) struct SkillPlugin {
  pub(super) folder: PathBuf,
  pub(super) slug: Slug,
}

impl Drop for SkillPlugin {
  fn drop(&mut self) {
    remove_folder(&self.folder);
  }
}

impl Turn {
  /// The skill the turn's message carries, placed as `layout` says, and the person's words after its block; `None`
  /// when it carries none or the skill cannot be placed, and the message goes as kept.
  pub(super) fn place_skill(&self, layout: SkillLayout) -> Option<(SkillPlugin, &str)> {
    let (chosen, words) = carried(self.text.as_str())?;
    let folder = self
      .root
      .join(".orchester")
      .join("chat-skills")
      .join(self.message.to_string());
    match write_skill(&self.root, (&folder, layout), &chosen) {
      Ok(()) => Some((
        SkillPlugin {
          folder,
          slug: chosen.slug,
        },
        words,
      )),
      Err(reason) => {
        // The block keeps the skill's text, so the agent still reads it: the turn goes on without the folder.
        tracing::warn!(chat = %self.chat.id(), skill = chosen.slug.as_str(), %reason, "the chosen skill goes as text");
        remove_folder(&folder);
        None
      }
    }
  }
}

/// Writes `chosen` as the project at `root` holds that version into `folder`, laid out as `layout` says.
fn write_skill(root: &Path, (folder, layout): (&Path, SkillLayout), chosen: &ChosenSkill) -> Result<(), String> {
  let db = ProjectDb::open(root).map_err(|error| error.to_string())?;
  let skill = db
    .version::<Skill>(chosen.resource, chosen.version)
    .map_err(|error| error.to_string())?
    .ok_or_else(|| format!("version {} is gone", chosen.version))?;
  remove_folder(folder);
  let place = match layout {
    SkillLayout::ClaudePlugin => {
      let manifest = folder.join(".claude-plugin");
      fs::create_dir_all(&manifest)
        .and_then(|()| fs::write(manifest.join("plugin.json"), manifest_json()))
        .map_err(|error| error.to_string())?;
      folder.join("skills").join(chosen.slug.as_str())
    }
    SkillLayout::CodexRoot => folder.join(chosen.slug.as_str()),
  };
  fs::create_dir_all(&place)
    .and_then(|()| fs::write(place.join(SKILL_FILE), skill.body().document(&chosen.slug)))
    .map_err(|error| error.to_string())?;
  for file in skill.body().files() {
    let content = db.blob(&file.content).map_err(|error| error.to_string())?;
    let path = file.path.segments().fold(place.clone(), |path, name| path.join(name));
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(&path, content).map_err(|error| format!("{}: {error}", file.path.as_str()))?;
  }
  Ok(())
}

/// The plugin's manifest, as Claude Code reads it from `.claude-plugin/plugin.json`.
fn manifest_json() -> String {
  serde_json::json!({
    "name": CLAUDE_PLUGIN,
    "description": "The skill the person chose for this message in Orchester.",
    "version": "1.0.0",
  })
  .to_string()
}

fn remove_folder(folder: &Path) {
  if let Err(error) = fs::remove_dir_all(folder)
    && error.kind() != io::ErrorKind::NotFound
  {
    tracing::warn!(folder = %folder.display(), %error, "cannot clear a chat turn's skill plugin");
  }
}
