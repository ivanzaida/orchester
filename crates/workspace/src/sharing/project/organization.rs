//! A shared task's organization and the server project's tags (`docs/architecture/server.md`, "Task
//! organization", decisions 23, 24 and 26): a change goes with the task's revision; when it meets a
//! newer one, the organization the person changed is put on what the server has now with core's
//! `TaskOrganization::rebase` and sent again without asking. Orchester's own status changes go with
//! `actor: orchester`. Tags are created, renamed, merged and deleted on the server; the cache takes
//! the tags and the tasks the server rewrote.

use orchester_api::{SharedTag, TagMerge, TagName, TaskChanges};
use orchester_core::{
  id::{TagId, TaskId},
  name::Name,
  task::{
    TaskUpdate,
    organization::{ChangeActor, Tag, TaskOrganization},
  },
};

use super::{convert, store::RemoteTaskStore};
use crate::sharing::SharingError;

/// How many times a change is merged into a task that changed meanwhile before Orchester stops.
const ATTEMPTS: usize = 3;

impl RemoteTaskStore {
  /// Changes the organization of `task` from `base`, as the person saw it, to `edited`, by `actor`;
  /// whether it changed. A change made meanwhile on the server is kept: only what the person
  /// changed is sent.
  pub fn change_organization(
    &mut self,
    task: TaskId,
    (base, edited): (&TaskOrganization, &TaskOrganization),
    actor: ChangeActor,
  ) -> Result<bool, SharingError> {
    for _ in 0..ATTEMPTS {
      let current = self.cached(task)?;
      let value = edited.rebase(base, current.organization());
      let mut changes = convert::with_organization(TaskChanges::default(), current.organization(), &value);
      if convert::is_empty(&changes) {
        return Ok(false);
      }
      changes.actor = Some(actor);
      match self.patch(task, current.revision(), changes)? {
        TaskUpdate::Saved(_) => return Ok(true),
        TaskUpdate::Conflict { .. } => {}
      }
    }
    Err(SharingError::KeptChanging)
  }

  /// Applies `update` to the organization the cache has for `task`, as a bulk change does; whether
  /// it changed.
  pub fn update_organization(
    &mut self,
    task: TaskId,
    update: impl FnOnce(TaskOrganization) -> TaskOrganization,
  ) -> Result<bool, SharingError> {
    let base = self.cached(task)?.organization().clone();
    let edited = update(base.clone());
    self.change_organization(task, (&base, &edited), ChangeActor::Person)
  }

  pub fn create_tag(&mut self, name: &Name) -> Result<Tag, SharingError> {
    self.server.writable()?;
    let request = TagName {
      name: name.as_str().to_owned(),
    };
    let created = self.block_on(
      self
        .server
        .call(|client, token, project| async move { client.create_tag(&token, project, &request).await }),
    )?;
    self.refresh_tags()?;
    tag(&created)
  }

  pub fn rename_tag(&mut self, id: TagId, name: &Name) -> Result<(), SharingError> {
    self.server.writable()?;
    let request = TagName {
      name: name.as_str().to_owned(),
    };
    self.block_on(
      self
        .server
        .call(|client, token, project| async move { client.rename_tag(&token, project, id, &request).await }),
    )?;
    self.refresh_tags()
  }

  /// Moves every task of `source` to `target` and removes `source`.
  pub fn merge_tags(&mut self, source: TagId, target: TagId) -> Result<(), SharingError> {
    self.server.writable()?;
    let into = TagMerge { into: target };
    let changed = self.block_on(
      self
        .server
        .call(|client, token, project| async move { client.merge_tag(&token, project, source, &into).await }),
    )?;
    self.cache_tags(&changed.tags)?;
    self.cache(&changed.tasks, None)
  }

  /// Removes `id` from every task and from the project.
  pub fn delete_tag(&mut self, id: TagId) -> Result<(), SharingError> {
    self.server.writable()?;
    let changed = self.block_on(
      self
        .server
        .call(|client, token, project| async move { client.delete_tag(&token, project, id).await }),
    )?;
    self.cache_tags(&changed.tags)?;
    self.cache(&changed.tasks, None)
  }

  /// Reads the project's tags from the server into the cache.
  fn refresh_tags(&mut self) -> Result<(), SharingError> {
    let tags = self.block_on(
      self
        .server
        .call(|client, token, project| async move { client.tags(&token, project).await }),
    )?;
    self.cache_tags(&tags)
  }

  fn cache_tags(&mut self, tags: &[SharedTag]) -> Result<(), SharingError> {
    let tags = tags.iter().map(tag).collect::<Result<Vec<_>, _>>()?;
    self.db.cache_shared_tags(&tags)?;
    Ok(())
  }
}

pub fn tag(tag: &SharedTag) -> Result<Tag, SharingError> {
  Ok(Tag::new(
    tag.id,
    Name::new(&tag.name).map_err(|_| SharingError::InvalidAnswer("tag name"))?,
  ))
}
