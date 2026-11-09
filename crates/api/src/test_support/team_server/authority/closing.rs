//! Closings and blocks that happen on the stand-in without a request of the desktop's, for the QA app and the tests:
//! another member's patch, a module's delivery accepted, a work's success reported. Each goes through the rules and
//! the records the requests meet ([`super::lifecycle`], [`super::edits`]), so the reads say who closed or blocked the
//! task and when as the contract orders ("Who closed it, who blocked it").

use serde_json::{Map, Value, json};

use super::{Authority, Person, edits, lifecycle, listing};

impl super::TeamServer {
  /// The id of the task numbered `number` that [`Self::authority_tasks`] set, as the reads give it.
  pub fn authority_task_id(&self, number: u32) -> Option<String> {
    self.state.lock().authority.task_id_of(number)
  }

  /// `by` moves the task numbered `number` to `workflow` (`done` or `canceled`) now, as their own patch does: the task
  /// reads closed by them, and its own claim is released.
  pub fn task_closed_by(&self, number: u32, by: Person, workflow: &'static str) {
    let mut fields = Map::new();
    fields.insert("workflow".to_owned(), json!(workflow));
    self.change_now(number, by, fields);
    let mut state = self.state.lock();
    if let Some(id) = state.authority.task_id_of(number) {
      lifecycle::release(&mut state.authority, &id);
    }
  }

  /// `by` sets the blocked reason of the task numbered `number` to `reason` now, as their own patch does: the task
  /// reads blocked by them.
  pub fn task_blocked_by(&self, number: u32, by: Person, reason: &str) {
    let mut fields = Map::new();
    fields.insert("blocked_reason".to_owned(), json!(reason));
    self.change_now(number, by, fields);
  }

  /// The delivery of the module at `index` into base is accepted: each of its open tasks whose work succeeded is Done
  /// by itself, as `delivery.report` makes it.
  pub fn module_delivered(&self, index: usize) {
    lifecycle::delivered(&mut self.state.lock().authority, index);
  }

  /// The work of the task numbered `number` reports `succeeded` with no patch: the task is Done by itself when its work
  /// delivers no changes to base ([`Self::authority_delivers`]), as `work.report` makes it.
  pub fn work_succeeded(&self, number: u32) {
    let mut state = self.state.lock();
    if let Some(id) = state.authority.task_id_of(number) {
      lifecycle::succeeded(&mut state.authority, &id, &[]);
    }
  }

  fn change_now(&self, number: u32, by: Person, fields: Map<String, Value>) {
    let mut state = self.state.lock();
    let Some(project) = state.projects.first() else {
      return;
    };
    let (Some(id), Some(key)) = (project["id"].as_str(), project["key"].as_str()) else {
      return;
    };
    let (id, key) = (id.to_owned(), key.to_owned());
    let read = listing::tasks((&id, &key), &state)
      .into_iter()
      .find(|task| task["number"] == number);
    if let Some(read) = read {
      edits::change(&mut state.authority, &read, (fields, by));
    }
  }
}

impl Authority {
  /// The id of the task numbered `number` that [`super::TeamServer::authority_tasks`] set.
  pub(super) fn task_id_of(&self, number: u32) -> Option<String> {
    self
      .tasks
      .iter()
      .flatten()
      .find(|task| task.number == number)
      .map(listing::task_id)
  }
}
