//! A decision the person confirms in the inspector (`Commit review action` `EMZrV`, `Preservation action` `vhBRn`):
//! its heading and what it pins, a labeled field where the decision is named, the acknowledgement wrapping beside
//! its checkbox, the full-width action, the way back as a ghost button, and what follows in a note.

use lurq::{
  app::{ctx::Ctx, events::input_source, theme::SpacingSize},
  components::{Column, Text},
  node::Element,
};
use orchester_core::integration::local::LocalPhase;

use super::{Action, Inspector, Mode, Props, action_panel, button, guard_note, mode_button};
use crate::{
  theme::LocalText,
  ui::{
    button::ButtonKind,
    toggle::{Toggle, ToggleKind},
  },
};

/// A field's single line, 10 from each edge of its box (`Commit message input` `ys8f8`).
const FIELD_HEIGHT: f32 = 38.0;

/// What a decision says and does.
pub(super) struct Decision {
  title: &'static str,
  body: &'static str,
  agree: &'static str,
  label: &'static str,
  action: Action,
  /// The ghost button that leaves the decision without taking it.
  back: &'static str,
  /// What follows the decision, under its actions.
  note: Option<&'static str>,
}

impl Decision {
  const fn new(keys: [&'static str; 4], action: Action) -> Self {
    let [title, body, agree, label] = keys;
    Self {
      title,
      body,
      agree,
      label,
      action,
      back: "local_integration.cancel",
      note: None,
    }
  }
}

pub(super) fn decision(props: &Props, mode: &Mode, phase: LocalPhase) -> Option<Decision> {
  Some(match mode {
    Mode::Commit { tree, .. } => Decision {
      back: "local_integration.backToSelection",
      note: Some("local_integration.commitNote"),
      ..Decision::new(
        [
          "local_integration.commitTitle",
          "local_integration.commitBody",
          "local_integration.commitAgree",
          "local_integration.commit",
        ],
        Action::Commit {
          paths: props.selected.get().0,
          tree: tree.clone(),
          message: props.message.get(),
        },
      )
    },
    Mode::Preserve => Decision {
      back: "local_integration.cancelPreparation",
      note: Some("local_integration.preserveNote"),
      ..Decision::new(
        [
          "local_integration.preserveTitle",
          "local_integration.preserveBody",
          "local_integration.preserveAgree",
          "local_integration.preserve",
        ],
        Action::Preserve {
          name: props.name.get(),
          paths: props.selected.get().0,
        },
      )
    },
    Mode::Cancel if props.view.operation.as_ref()?.preservation().is_none() => Decision::new(
      [
        "local_integration.releaseTitle",
        "local_integration.releaseBody",
        "local_integration.releaseAgree",
        "local_integration.release",
      ],
      Action::Cancel,
    ),
    Mode::Cancel => Decision::new(
      [
        "local_integration.cancelTitle",
        "local_integration.cancelBody",
        "local_integration.cancelAgree",
        "local_integration.cancelPreservation",
      ],
      Action::Cancel,
    ),
    Mode::Abort => Decision::new(
      [
        "local_integration.abortTitle",
        "local_integration.abortBody",
        "local_integration.abortAgree",
        "local_integration.abort",
      ],
      Action::Abort,
    ),
    Mode::Cleanup => Decision::new(
      [
        "local_integration.cleanupTitle",
        "local_integration.cleanupBody",
        "local_integration.cleanupAgree",
        "local_integration.cleanup",
      ],
      Action::Cleanup,
    ),
    Mode::Review => review_decision(props, phase)?,
  })
}

fn review_decision(props: &Props, phase: LocalPhase) -> Option<Decision> {
  Some(match phase {
    LocalPhase::AwaitingApproval => {
      let operation = props.view.operation.as_ref()?;
      Decision::new(
        [
          "local_integration.checked",
          "local_integration.checkedBody",
          "local_integration.approval",
          "local_integration.merge",
        ],
        Action::Merge {
          exact: Box::new(operation.checked()?.clone()),
          generation: operation.version(),
        },
      )
    }
    LocalPhase::TaskConflicts => Decision::new(
      [
        "local_integration.continue",
        "local_integration.markers",
        "local_integration.resolved",
        "local_integration.continue",
      ],
      Action::Continue,
    ),
    LocalPhase::RestoreConflicts => Decision::new(
      [
        "local_integration.finishRestore",
        "local_integration.restoreBody",
        "local_integration.restoreReviewed",
        "local_integration.finishRestore",
      ],
      Action::FinishRestore,
    ),
    _ => return None,
  })
}

impl Inspector {
  pub(super) fn confirmation(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    (mode, phase): (&Mode, LocalPhase),
    decision: Decision,
  ) -> Element {
    let mut panel = action_panel(ctx, decision.title, Some(decision.body)).with_children(field(ctx, props, mode));
    if *mode == Mode::Preserve {
      // Restoration always follows the merge and pauses on conflicts; the box says so and cannot be changed.
      panel = panel.child(
        Toggle::new(
          "local-restore-after-merge",
          ToggleKind::Checkbox,
          true,
          ctx.t("local_integration.restoreAfterMerge"),
        )
        .label_style(LocalText::Explanation.role())
        .wrap_label(),
      );
    }
    let checked = self.agree.clone();
    let gate = self.gate.clone().unwrap_or_default();
    panel = panel.child(
      Toggle::new(
        "local-confirm-scope",
        ToggleKind::Checkbox,
        self.agree.get(),
        ctx.t(decision.agree),
      )
      .label_style(LocalText::Explanation.role())
      .wrap_label()
      .on_change(move |on| {
        if gate.local_input(input_source()).allowed() {
          checked.set(on);
        }
      }),
    );
    let stale = matches!(decision.action, Action::Merge { .. }) && !props.view.checked_current;
    let unresolved =
      matches!(decision.action, Action::Continue | Action::FinishRestore) && !props.view.conflicts.is_empty();
    let blocked = !self.ready(props, mode)
      || stale
      || unresolved
      || (matches!(mode, Mode::Abort | Mode::Cancel) && props.editor_dirty.get());
    panel = panel.child(button(
      ctx,
      props,
      "local-confirm",
      decision.label,
      Some(decision.action),
      blocked,
    ));
    if stale {
      panel = panel
        .child(Text::new(&ctx.t("local_integration.error.stale")).variant(LocalText::Explanation.role()))
        .child(button(
          ctx,
          props,
          "local-resync-review",
          "local_integration.reconcile",
          Some(Action::Reconcile),
          false,
        ));
    }
    panel
      .with_children(self.additional_decisions(ctx, props, (mode, phase), decision.back))
      .with_children(decision.note.map(|note| guard_note(ctx, note)))
      .into()
  }

  fn ready(&self, props: &Props, mode: &Mode) -> bool {
    let named = |value: String| !value.trim().is_empty() && !props.view.editor_policy.sensitive(&value);
    self.agree.get()
      && match mode {
        Mode::Commit { .. } => named(props.message.get()),
        Mode::Preserve => !props.selected.get().is_empty() && named(props.name.get()),
        _ => true,
      }
  }

  fn additional_decisions(
    &self,
    ctx: &Ctx,
    props: &Props,
    (mode, phase): (&Mode, LocalPhase),
    back: &str,
  ) -> Vec<Element> {
    let mut actions = Vec::new();
    if phase == LocalPhase::AwaitingApproval && *mode == Mode::Review {
      actions.extend(self.cancellation(ctx, props, "local-review-cancel"));
    }
    if phase == LocalPhase::TaskConflicts && *mode == Mode::Review {
      actions.push(
        mode_button(
          ctx,
          props,
          ("local-review-abort", ButtonKind::Secondary),
          "local_integration.abort",
          Mode::Abort,
        )
        .into(),
      );
    }
    if phase == LocalPhase::RestoreConflicts && *mode == Mode::Review {
      actions.push(
        button(
          ctx,
          props,
          "local-resume-restore",
          "local_integration.restore",
          Some(Action::Restore),
          false,
        )
        .into(),
      );
    }
    actions.push(
      mode_button(
        ctx,
        props,
        ("local-cancel-review", ButtonKind::Ghost),
        back,
        Mode::Review,
      )
      .into(),
    );
    actions
  }
}

/// The commit's message or the preservation's name, under its label (`Commit message label` `jPNSC`).
fn field(ctx: &mut Ctx, props: &Props, mode: &Mode) -> Option<Element> {
  let (id, label, value) = match mode {
    Mode::Commit { .. } => (
      "local-commit-message",
      "local_integration.message",
      props.message.clone(),
    ),
    Mode::Preserve => ("local-preservation-name", "local_integration.name", props.name.clone()),
    _ => return None,
  };
  let input = super::super::fields::input(ctx, (id, label), value, &props.view.editor_policy, FIELD_HEIGHT);
  Some(
    Column::new()
      .id(format!("{id}-field"))
      .spacing(SpacingSize::Sm)
      .child(
        Text::new(&ctx.t(label))
          .id(format!("{id}-label"))
          .variant(LocalText::Explanation.role()),
      )
      .child(input)
      .into(),
  )
}
