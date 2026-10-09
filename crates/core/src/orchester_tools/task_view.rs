//! What `get_task` answers: the run's task as the run started it (title, brief, acceptance criteria), the plans a
//! person approved in the run, and the person's decisions and notes there: changes requested on a plan, answers to the
//! agents' questions and the messages the person wrote to the run.

use std::fmt::Write;

use crate::{
  conversation::{Message, Sender},
  id::RunId,
  question::{AskedQuestions, QuestionState},
  run::{Plan, PlanDecision},
  task::Task,
};

/// Everything `get_task` reads of a run.
#[derive(Clone, Copy, Debug)]
pub struct TaskView<'a> {
  pub run: RunId,
  pub task: &'a Task,
  pub plans: &'a [Plan],
  pub questions: &'a [AskedQuestions],
  /// The task's messages; only the person's to this run are told.
  pub messages: &'a [Message],
}

impl TaskView<'_> {
  /// The answer, as Markdown.
  pub fn text(&self) -> String {
    let details = self.task.details();
    let mut text = format!("# {} · {}\n\n## Brief\n\n", self.task.key(), details.title.as_str());
    text.push_str(if details.brief.trim().is_empty() {
      "(none)"
    } else {
      details.brief.trim()
    });
    text.push_str("\n\n## Acceptance criteria\n\n");
    if details.acceptance_criteria.is_empty() {
      text.push_str("(none)\n");
    }
    for criterion in &details.acceptance_criteria {
      // Writing to a String cannot fail.
      let _ = writeln!(text, "- {criterion}");
    }
    self.push_plans(&mut text);
    self.push_decisions(&mut text);
    text
  }

  fn push_plans(&self, text: &mut String) {
    let approved: Vec<&Plan> = self
      .plans
      .iter()
      .filter(|plan| plan.decision() == Some(&PlanDecision::Approved))
      .collect();
    if approved.is_empty() {
      return;
    }
    text.push_str("\n## Approved plans\n");
    for plan in approved {
      let _ = write!(
        text,
        "\n### {} · version {}\n\n{}\n",
        plan.step,
        plan.version,
        plan.text.trim()
      );
    }
  }

  fn push_decisions(&self, text: &mut String) {
    let mut lines = Vec::new();
    for plan in self.plans {
      if let Some(PlanDecision::ChangesRequested { note }) = plan.decision() {
        lines.push(format!(
          "- On the plan of {} (version {}), changes requested: {}",
          plan.step,
          plan.version,
          note.as_str()
        ));
      }
    }
    for asked in self.questions {
      let QuestionState::Answered { answers, .. } = &asked.state else {
        continue;
      };
      for (question, answer) in asked.request.questions().iter().zip(answers) {
        lines.push(format!(
          "- Asked: {}\n  Answered:\n  {}",
          question.text(),
          answer.for_agent().replace('\n', "\n  ")
        ));
      }
    }
    for message in self.messages {
      if message.run() == Some(self.run) && matches!(message.sender(), Sender::Person { .. }) {
        lines.push(format!("- Wrote: {}", message.body().as_str().replace('\n', "\n  ")));
      }
    }
    if lines.is_empty() {
      return;
    }
    text.push_str("\n## The person's decisions and notes\n\n");
    for line in lines {
      text.push_str(&line);
      text.push('\n');
    }
  }
}
