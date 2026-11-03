//! A skill the person chose for a message to the Orchestrator (ORC-634, `orchester_core::conversation::skill`), as a
//! new turn on Claude Code loads it: the version the message names, read from the project, is written as a session-only
//! plugin that holds that skill alone, in `<project>/.orchester/chat-skills/<message>/`, and removed once the CLI is
//! done with it. A version the project no longer has, or a plugin that cannot be written, leaves the message as it was
//! kept: the agent still reads the skill's text in its block.

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

/// A skill placed as a plugin for one turn, removed when dropped: the CLI has exited by then.
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
  /// The skill the turn's message carries, placed as a plugin, and the person's words after its block; `None` when it
  /// carries none or the skill cannot be placed, and the message goes as kept.
  pub(super) fn place_skill(&self) -> Option<(SkillPlugin, &str)> {
    let (chosen, words) = carried(self.text.as_str())?;
    let folder = self
      .root
      .join(".orchester")
      .join("chat-skills")
      .join(self.message.to_string());
    match write_plugin(&self.root, &folder, &chosen) {
      Ok(()) => Some((
        SkillPlugin {
          folder,
          slug: chosen.slug,
        },
        words,
      )),
      Err(reason) => {
        // The block keeps the skill's text, so the agent still reads it: the turn goes on without the plugin.
        tracing::warn!(chat = %self.chat.id(), skill = chosen.slug.as_str(), %reason, "the chosen skill goes as text");
        remove_folder(&folder);
        None
      }
    }
  }
}

/// Writes `chosen` as the project at `root` holds that version into `folder`, a plugin of [`CLAUDE_PLUGIN`].
fn write_plugin(root: &Path, folder: &Path, chosen: &ChosenSkill) -> Result<(), String> {
  let db = ProjectDb::open(root).map_err(|error| error.to_string())?;
  let skill = db
    .version::<Skill>(chosen.resource, chosen.version)
    .map_err(|error| error.to_string())?
    .ok_or_else(|| format!("version {} is gone", chosen.version))?;
  let manifest = folder.join(".claude-plugin");
  let place = folder.join("skills").join(chosen.slug.as_str());
  remove_folder(folder);
  fs::create_dir_all(&manifest)
    .and_then(|()| fs::write(manifest.join("plugin.json"), manifest_json()))
    .and_then(|()| fs::create_dir_all(&place))
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
