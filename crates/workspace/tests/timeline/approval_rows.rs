//! Requests the run's approval mode approved without the person (ORC-354, `docs/design/approval-modes.md`): a quiet row
//! of their own kind, which follows the tool group its call is in, and Autopilot's grouped per step.

use orchester_core::{
  id::{InvocationId, RunId, StepExecutionId},
  run::{AnswerKind, DecidedBy, PendingToolApproval, RequestAnswer, ToolApprovalDecision},
};
use orchester_workspace::tasks::timeline::{
  ApprovalCard, Item, ItemKind,
  view::{Choice, Row, shown},
};

use super::{around, as_written, at, at_millis, head, item, kinds, kinds_of};

fn request(id: &str, execution: StepExecutionId) -> PendingToolApproval {
  PendingToolApproval {
    id: id.into(),
    run: RunId::generate(),
    execution,
    invocation: InvocationId::generate(),
    agent: "Builder".into(),
    command: true,
    scope: None,
    server: "Claude Code".into(),
    action: "cargo build -p webhooks".into(),
    target: None,
    requested_at: at_millis(2_500),
    native: None,
  }
}

/// Request `id` of `execution`, decided `decision` by `decided_by`, as the run's mode recorded `answer`.
fn answered(
  (id, execution): (&str, StepExecutionId),
  (decision, decided_by): (ToolApprovalDecision, DecidedBy),
  answer: Option<RequestAnswer>,
) -> Item {
  let request = request(id, execution);
  Item {
    key: format!("approval-{id}").into(),
    execution: Some(execution),
    at: request.requested_at,
    kind: ItemKind::Approval(ApprovalCard {
      request,
      step: "Implement".into(),
      decided: Some(Some(decision)),
      decided_by: Some(decided_by),
      decided_at: Some(at(3)),
      review: None,
      reviewing: false,
      for_project: false,
      note: None,
      answer,
    }),
  }
}

fn autopilot(id: &str, execution: StepExecutionId) -> Item {
  answered(
    (id, execution),
    (ToolApprovalDecision::Approved, DecidedBy::AutoApproved),
    Some(RequestAnswer::by(AnswerKind::Autopilot)),
  )
}

fn card(item: &Item) -> &ApprovalCard {
  match &item.kind {
    ItemKind::Approval(card) => card,
    kind => panic!("a request, not {kind:?}"),
  }
}

#[test]
fn timeline_each_answered_request_reads_its_quiet_row_kind() {
  let execution = StepExecutionId::generate();
  let approved = (ToolApprovalDecision::Approved, DecidedBy::Permissions);
  let profile = answered(("1", execution), approved, Some(RequestAnswer::by(AnswerKind::Profile)));
  let reviewer = answered(
    ("2", execution),
    (ToolApprovalDecision::Approved, DecidedBy::Reviewer),
    Some(RequestAnswer::reviewer(
      "Deletes generated fixtures inside the worktree.",
    )),
  );
  let kind = |item: &Item| card(item).quiet().map(|answer| answer.kind);
  assert_eq!(kind(&autopilot("0", execution)), Some(AnswerKind::Autopilot));
  assert_eq!(kind(&profile), Some(AnswerKind::Profile));
  assert_eq!(kind(&reviewer), Some(AnswerKind::Reviewer));
  assert_eq!(
    card(&reviewer).quiet().and_then(|answer| answer.reason.as_deref()),
    Some("Deletes generated fixtures inside the worktree.")
  );
  assert!(card(&autopilot("0", execution)).by_autopilot());
  assert!(!card(&profile).by_autopilot());

  // A person's answer stays a card; so does one recorded before answers were, and a denial.
  let person = answered(
    ("3", execution),
    (ToolApprovalDecision::Approved, DecidedBy::Person),
    Some(RequestAnswer::by(AnswerKind::Person)),
  );
  let unrecorded = answered(("4", execution), approved, None);
  let denied = answered(
    ("5", execution),
    (ToolApprovalDecision::Denied, DecidedBy::PersonDeny),
    Some(RequestAnswer::by(AnswerKind::Reviewer)),
  );
  for item in [person, unrecorded, denied] {
    assert_eq!(kind(&item), None, "{item:?}");
  }
}

#[test]
fn timeline_a_quiet_row_follows_the_tool_group_its_call_is_in() {
  let execution = StepExecutionId::generate();
  let items = around(execution, autopilot("1", execution));
  assert_eq!(kinds_of(&items), ["tools", "approval"], "the group is not split");
  let ItemKind::Tools(group) = &items[0].kind else {
    panic!("the group first");
  };
  assert_eq!(group.summary.commands, 2);
}

#[test]
fn timeline_autopilot_rows_of_one_step_group_into_one_row() {
  let (implement, review) = (StepExecutionId::generate(), StepExecutionId::generate());
  let approved = (ToolApprovalDecision::Approved, DecidedBy::Permissions);
  let mut items = vec![item(
    "step-implement",
    implement,
    ItemKind::Step(head(implement, 3, "Implement")),
  )];
  items.extend((1..=5).map(|index| autopilot(&format!("a{index}"), implement)));
  items.push(answered(
    ("p", implement),
    approved,
    Some(RequestAnswer::by(AnswerKind::Profile)),
  ));
  items.push(autopilot("a6", implement));
  items.push(item("step-review", review, ItemKind::Step(head(review, 4, "Review"))));
  items.push(autopilot("r1", review));
  let folded = shown(
    &items,
    &Choice::default(),
    &|key, default| key.starts_with("step") || default,
    &as_written,
  );
  assert_eq!(
    kinds(&folded.rows),
    ["step", "approvals", "item", "approvals", "step", "approvals"]
  );
  let Row::Approvals {
    key,
    items: grouped,
    open,
  } = &folded.rows[1]
  else {
    panic!("the step's Autopilot rows");
  };
  assert_eq!(grouped.len(), 5, "consecutive rows of the step");
  assert!(!open, "they start folded");
  assert_eq!(&**key, "approvals-approval-a1");

  let opened = shown(&items, &Choice::default(), &|_, _| true, &as_written);
  assert!(
    matches!(&opened.rows[1], Row::Approvals { open: true, .. }),
    "a person opened them"
  );
}
