//! The task Orchester makes to accept a module with a branch of its own (`docs/plan/orchestrator-autonomy/OA-20-…`):
//! its title, its criteria and its brief. The brief is Orchester's, not an agent's: the module's tasks with their
//! criteria, every step automation accepted unverified, and what the steps review, the module's whole diff against
//! its base branch.

/// A task of the module, as the acceptance's brief lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptedTask {
  pub key: String,
  pub title: String,
  pub criteria: Vec<String>,
  /// The steps of its latest run that automation accepted as they were, by their names.
  pub accepted_unverified: Vec<String>,
}

/// The title of the acceptance task of the module named `module`.
pub fn acceptance_title(module: &str) -> String {
  format!("Accept the module {module}")
}

/// What an acceptance is done by.
pub const ACCEPTANCE_CRITERIA: [&str; 3] = [
  "Every task listed in the brief is checked against its acceptance criteria on the module's branch as it is now.",
  "Every step that was accepted unverified is verified now, or named as still unverified with why.",
  "Each defect is reported with the file it is in, how to see it and why it matters; no defect means the module \
   is accepted.",
];

/// The brief of the acceptance of the module named `module`, whose work is on `branch` and is delivered into `into`.
pub fn acceptance_brief(module: &str, (branch, into): (&str, &str), tasks: &[AcceptedTask]) -> String {
  let mut brief = format!(
    "This task accepts the module {module} as a whole, before it is delivered. Its tasks were each built and \
     reviewed by themselves; nobody looked at all of it together. You did not write this work: review it as its \
     reader.\n\nThe module's work is on the branch {branch}, which this run started from, and is delivered into \
     {into}. Review the module's whole diff: `git diff {into}...HEAD`. Build it and run its checks as the project's \
     instructions say. Change nothing here unless a step of this pipeline is there to fix what the review finds: \
     report what you find.\n\nThe module's tasks:\n"
  );
  for task in tasks {
    brief.push_str(&format!("\n- {} — {}\n", task.key, task.title));
    for criterion in &task.criteria {
      brief.push_str(&format!("  - {criterion}\n"));
    }
    if !task.accepted_unverified.is_empty() {
      brief.push_str(&format!(
        "  - Accepted unverified by automation, so verify it now: {}.\n",
        task.accepted_unverified.join(", ")
      ));
    }
  }
  brief
}
