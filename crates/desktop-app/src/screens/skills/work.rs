//! Skill writes off the UI thread, recorded in the shared activity log.
use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
  events::is_synthetic_input,
};
use orchester_core::resource::Skill;
use orchester_workspace::{
  activity::skill_operations,
  background,
  messages::Message,
  skills::{data::SkillEntry, editor},
  workspace::OpenProject,
};

use crate::services::Services;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved(pub SkillEntry);
impl DevtoolsInspectable for Saved {}
#[derive(Clone)]
pub enum Change {
  Save {
    before: Option<SkillEntry>,
    name: String,
    body: Skill,
    /// An MCP client's synthetic input wrote the edits it saves: the draft is marked as automation's (ORC-79).
    by_automation: bool,
  },
  Publish(SkillEntry),
}
/// Who wrote the editor's edits: automation when an MCP client's input edited the fields since the last save started,
/// or presses the control that saves, as the Instructions editor tells it (ORC-30).
#[derive(Clone, Default)]
pub struct Authorship {
  edited: Arc<AtomicBool>,
  last: Arc<AtomicBool>,
}
impl Authorship {
  /// A field changed: by automation when lurq is delivering an MCP client's input.
  pub fn edited(&self) {
    if is_synthetic_input() {
      self.edited.store(true, Ordering::SeqCst);
    }
  }
  /// A save starts: whether automation wrote what it takes. Edits from now on are the next save's.
  pub fn saving(&self) -> bool {
    let automated = self.edited.swap(false, Ordering::SeqCst) || is_synthetic_input();
    self.last.store(automated, Ordering::SeqCst);
    automated
  }
  /// A retry of the last save: automation's when that save was, or an MCP client's input is in it.
  pub fn retrying(&self) -> bool {
    let automated = self.last.load(Ordering::SeqCst) || self.saving();
    self.last.store(automated, Ordering::SeqCst);
    automated
  }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
  Save,
  Publish,
}
impl DevtoolsInspectable for Operation {}
pub type Work = FutureAction<(OpenProject, Change), Saved, Message>;
pub fn action(ctx: &mut Ctx) -> Work {
  let services = Services::of(ctx);
  ctx.future_action(move |(open, change): (OpenProject, Change)| {
    let services = services.clone();
    async move {
      let spec = match &change {
        Change::Save {
          before: Some(before),
          name,
          ..
        } => skill_operations::skill_draft(&open, before.id, name),
        Change::Save { name, .. } => skill_operations::skill_create(&open, name),
        Change::Publish(entry) => {
          skill_operations::skill_publish(&open, entry.id, &entry.name, super::words::next(entry.state))
        }
      };
      let publishing = matches!(change, Change::Publish(_));
      let creating = matches!(change, Change::Save { before: None, .. });
      let root = open.repo_root.clone();
      let sent = services.clone();
      let saved = services
        .activity
        .track(spec, async move {
          let (created_in, written) = (open.repo_root.clone(), open.repo_root.clone());
          let saved = background::run(move || match change {
            Change::Save {
              before,
              name,
              body,
              by_automation,
            } => editor::save_by_in(&open.repo_root, (before.as_ref(), &name, &body), by_automation),
            Change::Publish(entry) => editor::publish_in(&written, &entry),
          })
          .await??;
          // A new skill in a shared project is the team's only once its server takes it; one it refuses is taken back.
          if creating {
            crate::screens::hub::team::send_created(&sent, &created_in, saved.id).await?;
          }
          Ok(Saved(saved))
        })
        .await;
      // A version published in a shared project goes to its team server at once (02).
      if publishing && saved.is_ok() {
        crate::screens::hub::team::send_published(&services, &root).await;
      }
      saved
    }
  })
}
