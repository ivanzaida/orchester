//! 05 a–c and 06 a–b decision-review fixtures in cards-auto-mode; recorded requests, without a runner.
use orchester_core::run::{
  PendingToolApproval, ReviewQuestion, after_review,
  review::{DecisionVerdict, Probability, Review, ReviewFailure, Risks},
};

use super::cards::{at_work, hold};
use crate::kontur::{Kontur, SeedError, catalogue::Published};

pub fn seed(kontur: &mut Kontur, pipeline: &Published) -> Result<(), SeedError> {
  for (title, command, review, tool) in requests() {
    let state = if review.approves() {
      "decision model approved"
    } else {
      "decision review left to the person"
    };
    let mut task = kontur.task((title, title, &[state]), Some(pipeline), kontur.ago(50))?;
    let (run, invocation) = at_work(kontur, &mut task, pipeline, kontur.ago(30))?;
    kontur.seeded(&task, state);
    let request = PendingToolApproval {
      id: format!("{}:decision", invocation.id),
      run: run.id(),
      execution: invocation.execution,
      invocation: invocation.id,
      agent: "Builder".to_owned(),
      command: !tool,
      scope: tool.then(|| r#"{"path":"crates/workspace"}"#.to_owned()),
      server: if tool { "files" } else { "command" }.to_owned(),
      action: command.to_owned(),
      target: None,
      requested_at: kontur.ago(28),
      native: None,
    };
    kontur.db.request_tool_approval(&request)?;
    kontur.db.record_review(run.id(), &request.id, &review)?;
    let came = match after_review(ReviewQuestion::Request, &review) {
      Ok(answer) => {
        use orchester_core::run::{DecidedBy, ToolApprovalDecision};
        kontur
          .db
          .claim_tool_approval(run.id(), &request.id, DecidedBy::Reviewer, kontur.ago(27))?;
        kontur.db.record_request_answer(run.id(), &request.id, &answer)?;
        kontur
          .db
          .delivered_tool_approval(run.id(), &request.id, ToolApprovalDecision::Approved)?;
        hold(kontur, run, (&invocation, false))?;
        continue;
      }
      Err(came) => came,
    };
    kontur.db.record_came_to_person(run.id(), &request.id, Some(&came))?;
    if title == "Review two waiting commands" {
      let mut second = request.clone();
      second.id.push_str("-second");
      second.action = "git merge --no-ff feature".to_owned();
      kontur.db.request_tool_approval(&second)?;
      kontur.db.record_review(run.id(), &second.id, &review)?;
      kontur.db.record_came_to_person(run.id(), &second.id, Some(&came))?;
    }
    hold(kontur, run, (&invocation, true))?;
  }
  Ok(())
}

fn verdict(approve: f64, discard: f64) -> Review {
  let p = |value| Probability::new(value).expect("fixture probability");
  Review::Decided {
    decision: DecisionVerdict {
      model: "jev".to_owned(),
      version: Some("jev-1.14.0".to_owned()),
      elapsed_ms: Some(183),
      approve: p(approve),
      threshold: p(0.8),
      risks: Risks {
        writes_outside_worktree: p(0.02),
        reaches_network: p(0.1),
        reads_credential: p(0.0),
        discards_work: p(discard),
        does_not_serve_task: p(0.3),
      },
    }
    .into(),
  }
}

fn requests() -> [(&'static str, &'static str, Review, bool); 7] {
  [
    (
      "Restore the reviewed files",
      "git checkout 4595e2173 -- crates/desktop-app crates/workspace",
      verdict(0.41, 0.78),
      false,
    ),
    (
      "Merge the base branch",
      "git merge --no-ff master",
      verdict(0.72, 0.49),
      false,
    ),
    (
      "Check the translations",
      "cargo test -p orchester-app --test i18n locales",
      Review::Failed {
        failure: ReviewFailure::TimedOut,
        model: Some("jev".to_owned()),
        detail: None,
        decision: true,
      },
      false,
    ),
    ("Review the tool request", "replace_files", verdict(0.41, 0.78), true),
    (
      "Review two waiting commands",
      "git merge --no-ff master",
      verdict(0.72, 0.49),
      false,
    ),
    (
      "Check the app with jev",
      "cargo check -p orchester-app --tests",
      verdict(0.97, 0.11),
      false,
    ),
    (
      "Review an approved tool request",
      "read_file",
      verdict(0.86, 0.11),
      true,
    ),
  ]
}
