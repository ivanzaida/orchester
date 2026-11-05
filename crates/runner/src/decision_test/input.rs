//! Keep missing historical context explicit, without inventing permissions or current task details.
use orchester_core::{
  permission::AgentPermissions,
  resource::ModelId,
  run::review::{ReviewHold, ReviewInput, ReviewedRequest, ReviewedStep, ReviewedTask},
};
use orchester_store::RecordedDecisionReview;
use serde_json::{Value, json};

use crate::decision_review::system_one_request;

pub(super) fn request(
  record: &RecordedDecisionReview,
  model: &ModelId,
  root: &std::path::Path,
) -> Result<(Value, Vec<String>), ReviewHold> {
  let approval = &record.request;
  let unknown = AgentPermissions::before_permissions();
  let permissions = record.permissions.as_ref().unwrap_or(&unknown);
  let step = ReviewedStep {
    name: record.step_name.clone().unwrap_or_default(),
    repository: permissions.repository(),
    network: permissions.network().clone(),
    writable_folders: permissions
      .granted_folders()
      .map(|folders| {
        folders
          .entries()
          .iter()
          .map(|folder| folder.lexical_path(root).display().to_string())
          .collect()
      })
      .unwrap_or_default(),
  };
  let task = record.task.clone().unwrap_or(ReviewedTask {
    title: String::new(),
    brief: String::new(),
    criteria: vec![],
  });
  let input = ReviewInput::new(
    ReviewedRequest {
      command: approval.command,
      server: approval.server.clone(),
      action: approval.action.clone(),
      arguments: (!approval.command).then(|| approval.scope.clone()).flatten(),
      note: approval.command.then(|| approval.scope.clone()).flatten(),
      working_folder: approval.target.clone(),
    },
    step,
    task,
    &record.messages,
  )?;
  let omitted = omitted(record);
  let mut body = system_one_request(&input, model);
  for path in &omitted {
    if let Some((parent, key)) = path.rsplit_once('/')
      && let Some(object) = body["state"].pointer_mut(parent).and_then(Value::as_object_mut)
    {
      object.remove(key);
    }
  }
  body["state"]["not_stored"] = json!(omitted);
  Ok((body, omitted))
}

fn omitted(record: &RecordedDecisionReview) -> Vec<String> {
  let mut paths = vec![
    "/step/command_parts_orchester_could_not_read",
    "/step/project_variables_that_change_what_a_build_runs",
    "/step/roots",
    "/step/run_branch",
  ];
  if record.permissions.is_none() {
    paths.extend([
      "/step/repository",
      "/step/network",
      "/step/writable_folders_besides_the_worktree",
    ]);
  }
  if record.step_name.is_none() {
    paths.push("/step/step");
  }
  if record.task.is_none() {
    paths.push("/task");
  }
  if record.messages.is_empty() {
    paths.push("/agent_data/data/agent_last_messages");
  }
  if !record.request.command {
    paths.push("/agent_data/data/agent_note");
  }
  if record.request.scope.is_none() {
    paths.push(if record.request.command {
      "/agent_data/data/agent_note"
    } else {
      "/agent_data/data/request/arguments"
    });
  }
  if record.request.target.is_none() {
    paths.push("/agent_data/data/working_folder");
  }
  paths.into_iter().map(str::to_owned).collect()
}
