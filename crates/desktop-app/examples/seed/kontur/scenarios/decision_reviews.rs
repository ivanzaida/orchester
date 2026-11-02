//! 05 a–c decision-review fixtures, included in cards-auto-mode; recorded requests, without a runner.
use orchester_core::run::{
  PendingToolApproval, ReviewQuestion, after_review,
  review::{DecisionVerdict, Probability, Review, ReviewFailure, Risks},
};

use super::cards::{at_work, hold};
use crate::kontur::{Kontur, SeedError, catalogue::Published};

pub fn seed(kontur: &mut Kontur, pipeline: &Published) -> Result<(), SeedError> {
  for (title, command, review, tool) in [
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
      },
      false,
    ),
    ("Review the tool request", "replace_files", verdict(0.41, 0.78), true),
  ] {
    let mut task = kontur.task(
      (title, title, &["The request stays with the person."]),
      Some(pipeline),
      kontur.ago(50),
    )?;
    let (run, invocation) = at_work(kontur, &mut task, pipeline, kontur.ago(30))?;
    kontur.seeded(&task, "decision review left to the person");
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
    let came = after_review(ReviewQuestion::Request, &review).expect_err("all fixtures ask the person");
    kontur.db.record_came_to_person(run.id(), &request.id, Some(&came))?;
    hold(kontur, run, (&invocation, true))?;
  }
  Ok(())
}

fn verdict(approve: f64, discard: f64) -> Review {
  let p = |value| Probability::new(value).expect("fixture probability");
  Review::Decided {
    decision: DecisionVerdict {
      model: "jev".to_owned(),
      version: None,
      approve: p(approve),
      threshold: p(0.8),
      risks: Risks {
        writes_outside_worktree: p(0.02),
        reaches_network: p(0.1),
        reads_credential: p(0.0),
        discards_work: p(discard),
        does_not_serve_task: p(0.3),
      },
    },
  }
}
