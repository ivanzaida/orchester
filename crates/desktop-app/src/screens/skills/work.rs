//! Skill writes off the UI thread, recorded in the shared activity log.
use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
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
  },
  Publish(SkillEntry),
}
pub type Work = FutureAction<(OpenProject, Change), Saved, Message>;
pub fn action(ctx: &mut Ctx) -> Work {
  let activity = Services::of(ctx).activity;
  ctx.future_action(move |(open, change): (OpenProject, Change)| {
    let activity = activity.clone();
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
      activity
        .track(spec, async move {
          background::run(move || match change {
            Change::Save { before, name, body } => editor::save_in(&open.repo_root, before.as_ref(), &name, &body),
            Change::Publish(entry) => editor::publish_in(&open.repo_root, &entry, true),
          })
          .await?
          .map(Saved)
        })
        .await
    }
  })
}
