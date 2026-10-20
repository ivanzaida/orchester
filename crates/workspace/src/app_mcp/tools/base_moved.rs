//! A notice that a run's base branch moved (ORC-54), as `get_conversation` and `get_run` give it (kind
//! `base_moved`): where the base moved, what landed, the files both sides changed, and where the notice stands.

use serde_json::{Value, json};

use super::wire;
use crate::tasks::{
  detail::RunView,
  timeline::{
    ItemKind,
    base_moved::{self, BaseMovedMark, NoticeState},
  },
};

/// `mark`'s fields.
pub(super) fn shown(mark: &BaseMovedMark) -> Value {
  let notice = &mark.notice;
  let (state, agent, step) = match &mark.state {
    NoticeState::WritingStep => ("waits_for_writing_step", None, None),
    NoticeState::RunGoesOn => ("kept_until_run_goes_on", None, None),
    NoticeState::FollowUp => ("waits_for_follow_up", None, None),
    NoticeState::Delivered { agent, step } => ("delivered", Some(&**agent), Some(&**step)),
  };
  let tasks: Vec<Value> = notice
    .tasks
    .iter()
    .map(|task| json!({ "key": &task.key, "title": &task.title }))
    .collect();
  let commits: Vec<Value> = notice
    .commits
    .iter()
    .map(|commit| json!({ "revision": commit.revision.as_str(), "subject": &commit.subject }))
    .collect();
  json!({
    "branch": notice.branch.as_str(),
    "revision": notice.revision.as_str(),
    "tasks": tasks,
    "commits": commits,
    "overlapping_files": &notice.overlapping,
    "state": state,
    "delivered_to": agent.map(|agent| json!({ "agent": agent, "step": step })),
    "noticed_at": wire::time(notice.noticed_at),
    "delivered_at": notice.delivered().map(|(_, at)| wire::time(at)),
  })
}

/// Every notice of `view`'s run, oldest first, with its kind.
pub(super) fn of_run(view: &RunView) -> Vec<Value> {
  base_moved::all(view)
    .filter_map(|item| match &item.kind {
      ItemKind::BaseMoved(mark) => {
        let mut value = shown(mark);
        value["kind"] = json!("base_moved");
        Some(value)
      }
      _ => None,
    })
    .collect()
}
