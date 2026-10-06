//! An inline approval card's answers as the chat narrows, from a request seeded as the runner records it:
//! on one line at 1440; at 960 *Always allow…* on a line of its own above the buttons; at 960 with Details
//! open each button on a row of its own, full width. No answer ever shrinks or leaves the card, and the
//! card's status label stays whole while its title wraps. The transcript rows are `narrow_marks`'.

use lurq::core::ElementRect;
use orchester_core::run::PendingToolApproval;
use orchester_store::ProjectDb;

use crate::{harness::Desktop, native_run::native_run, task_run::has, unreadable_run::at};

const DIALOG: &str = "tool-approval-dialog";
const ANSWERS: [&str; 4] = [
  "conversation-approval-always",
  "conversation-approval-deny",
  "conversation-approval-similar",
  "conversation-approval-approve",
];

pub(crate) fn open_conversation(desktop: &mut Desktop) {
  desktop.wait_for("nav-tasks");
  desktop.press("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-page");
  desktop.click("task-tab-conversation");
}

/// Whether `inner` lies within `outer`, to half a pixel.
pub(crate) fn inside(inner: ElementRect, outer: ElementRect) -> bool {
  inner.x >= outer.x - 0.5
    && inner.y >= outer.y - 0.5
    && inner.x + inner.width <= outer.x + outer.width + 0.5
    && inner.y + inner.height <= outer.y + outer.height + 0.5
}

/// Opens or closes the Conversation tab's Details until `open`.
pub(crate) fn details(desktop: &mut Desktop, open: bool) {
  if has(desktop, "conversation-details-head") != open {
    desktop.click("conversation-details");
  }
  desktop.wait_until("Details", |desktop| has(desktop, "conversation-details-head") == open);
}

/// KON-1 waiting on Orchester's `git_commit`, its Conversation tab open with the dialog put off; the
/// card's id.
fn waiting_commit() -> (Desktop, String) {
  let mut seeded = None;
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let (run, invocation) = native_run(repo);
    let request = PendingToolApproval {
      id: format!("{}:call_commit", invocation.id),
      run: run.id(),
      execution: invocation.execution,
      invocation: invocation.id,
      agent: "Builder".to_owned(),
      command: false,
      scope: Some(r#"{"message":"Add release notes for 0.4.0"}"#.to_owned()),
      server: "Orchester".to_owned(),
      action: "git_commit".to_owned(),
      target: None,
      requested_at: at(20),
      native: None,
    };
    ProjectDb::open(repo)
      .expect("open the project")
      .request_tool_approval(&request)
      .expect("pending approval");
    seeded = Some(request.id);
  });
  desktop.wait_for(DIALOG);
  desktop.press("tool-approval-decide-later");
  desktop.wait_until("the dialog closes", |desktop| !has(desktop, DIALOG));
  open_conversation(&mut desktop);
  desktop.wait_for("conversation-approval-approve");
  (desktop, format!("conversation-approval-{}", seeded.expect("seeded")))
}

/// The bounds of the card's status label, found by its words (the only card on the page).
fn status_label(desktop: &mut Desktop) -> ElementRect {
  desktop
    .tree
    .find_element_mut(|element| element.text_content() == Some("Waiting for your answer"))
    .expect("the status label")
    .bounds()
}

/// The answers' bounds in [`ANSWERS`] order, each inside `card`, none narrower than at 1440 (`wide`).
fn answers(desktop: &mut Desktop, card: &str, wide: Option<&[ElementRect]>) -> Vec<ElementRect> {
  let frame = desktop.bounds(card);
  let label = status_label(desktop);
  assert!(
    inside(label, frame),
    "the status label stays whole in the card ({label:?} in {frame:?})"
  );
  ANSWERS
    .iter()
    .enumerate()
    .map(|(index, answer)| {
      let bounds = desktop.bounds(answer);
      assert!(
        inside(bounds, frame),
        "{answer} lies inside the card ({bounds:?} in {frame:?})"
      );
      if let Some(wide) = wide {
        assert!(
          bounds.width >= wide[index].width - 0.5,
          "{answer} keeps its label ({bounds:?})"
        );
      }
      bounds
    })
    .collect()
}

#[test]
fn narrow_answers_card_keeps_every_answer_whole_inside_it() {
  let (mut desktop, card) = waiting_commit();
  desktop.tree.resize(1440, 960);
  details(&mut desktop, false);
  let wide = answers(&mut desktop, &card, None);
  assert!(
    wide
      .iter()
      .all(|answer| (answer.y + answer.height / 2.0 - (wide[3].y + wide[3].height / 2.0)).abs() < 1.0)
  );

  // 960: *Always allow…* above, the buttons on one line under it, at the right.
  desktop.tree.resize(960, 960);
  details(&mut desktop, false);
  let [always, deny, similar, once] = answers(&mut desktop, &card, Some(&wide)).try_into().expect("four");
  assert!(
    always.y + always.height <= deny.y + 0.5,
    "Always allow… is above the buttons"
  );
  assert!(deny.y == similar.y && similar.y == once.y, "the buttons share a line");
  let frame = desktop.bounds(&card);
  assert!(
    frame.x + frame.width - (once.x + once.width) < 20.0,
    "Approve once ends at the right"
  );

  // 960 with Details: each button on a full-width row of its own.
  details(&mut desktop, true);
  let [always, deny, similar, once] = answers(&mut desktop, &card, Some(&wide)).try_into().expect("four");
  assert!(always.y + always.height <= deny.y + 0.5);
  assert!(deny.y + deny.height <= similar.y + 0.5 && similar.y + similar.height <= once.y + 0.5);
  assert!(
    deny.width == similar.width && similar.width == once.width,
    "full-width rows"
  );
}
