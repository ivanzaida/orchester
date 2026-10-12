mod preparation;

use std::sync::{Arc, atomic::Ordering};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::{InputSource, input_source},
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::integration::local::LocalPhase;
use orchester_workspace::tasks::local_integration::IntegrationView;

pub(super) use super::words::{phase_body, phase_title};
use super::{
  LocalPage, Mode, TaskContext,
  work::{Action, Request, Work},
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, Protected},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    toggle::{Toggle, ToggleKind},
  },
};

#[derive(Clone)]
struct Props {
  task: TaskContext,
  view: Arc<IntegrationView>,
  mode: Signal<Mode>,
  selected: Signal<Vec<orchester_core::integration::local::LocalPath>>,
  message: Signal<String>,
  name: Signal<String>,
  work: Work,
  stop: Arc<std::sync::atomic::AtomicBool>,
  busy: bool,
  editor_dirty: Signal<bool>,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.task == other.task && self.view == other.view && self.busy == other.busy
  }
}
impl DevtoolsInspectable for Props {}

pub(super) fn inspector(
  ctx: &mut Ctx,
  page: &LocalPage,
  task: &TaskContext,
  view: &IntegrationView,
  busy: bool,
) -> Column {
  let props = Props {
    task: task.clone(),
    view: Arc::new(view.clone()),
    mode: page.mode.clone(),
    selected: page.selected.clone(),
    message: page.message.clone(),
    name: page.name.clone(),
    work: page.work.clone(),
    stop: page.stop.clone(),
    busy,
    editor_dirty: page.editor_dirty.clone(),
  };
  Column::new()
    .spacing(SpacingSize::Md)
    .child(ctx.mount_keyed::<Inspector>("local-inspector", props))
}

struct Inspector {
  gate: Option<InputGate>,
  protected: parking_lot::Mutex<Option<Protected>>,
  agree: Signal<bool>,
  identity: parking_lot::Mutex<String>,
}
impl Component for Inspector {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      gate: ctx.use_context(),
      protected: parking_lot::Mutex::new(None),
      agree: ctx.signal(false),
      identity: parking_lot::Mutex::new(String::new()),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let phase = props
      .view
      .operation
      .as_ref()
      .map_or(LocalPhase::Review, |operation| operation.phase());
    let mode = props.mode.get();
    let guarded = mode != Mode::Review
      || matches!(
        phase,
        LocalPhase::AwaitingApproval | LocalPhase::TaskConflicts | LocalPhase::RestoreConflicts
      );
    let mut protected = self.protected.lock();
    if guarded && protected.is_none() {
      *protected = self
        .gate
        .as_ref()
        .map(|gate| gate.protect(Guarded::LocalIntegrationQuestion));
    } else if !guarded {
      *protected = None;
    }
    drop(protected);
    let mode_key = match &mode {
      Mode::Review => "review",
      Mode::Commit { tree, .. } => tree.as_str(),
      Mode::Preserve => "preserve",
      Mode::Abort => "abort",
      Mode::Cleanup => "cleanup",
      Mode::Cancel => "cancel",
    };
    let identity = format!(
      "{:?}:{}:{}:{:?}",
      phase,
      mode_key,
      props.view.checkout.head,
      (&props.view.checkout, props.selected.get())
    );
    let mut prior = self.identity.lock();
    if *prior != identity {
      self.agree.set(false);
      *prior = identity;
    }
    drop(prior);
    let panel = self.actions(ctx, &props, &mode, phase);
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(panel)
      .child(recovery(ctx, &props.view))
  }
}

impl Inspector {
  fn actions(&self, ctx: &mut Ctx, props: &Props, mode: &Mode, phase: LocalPhase) -> Element {
    if props.view.external.is_some() {
      return action_panel()
        .child(Text::new(&ctx.t("local_integration.externalTitle")).variant(TypographyStyle::Title))
        .child(Text::new(&ctx.t("local_integration.externalBody")).variant(TypographyStyle::Description))
        .into();
    }
    if props.busy {
      let stop = props.stop.clone();
      return action_panel()
        .child(
          button(ctx, props, "local-stop", "local_integration.pause", None, false)
            .on_click(move || stop.store(true, Ordering::Release)),
        )
        .into();
    }
    match decision(props, mode, phase) {
      Some(decision) => self.confirmation(ctx, props, (mode, phase), decision),
      None => self.available(ctx, props, phase),
    }
  }

  fn confirmation(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    (mode, phase): (&Mode, LocalPhase),
    decision: Decision,
  ) -> Element {
    let (title, body, agree, label, action) = decision;
    let mut panel = action_panel()
      .child(Text::new(&ctx.t(title)).variant(TypographyStyle::Title))
      .child(Text::new(&ctx.t(body)).variant(TypographyStyle::Description));
    let field = match mode {
      Mode::Commit { .. } => Some(super::fields::input(
        ctx,
        ("local-commit-message", "local_integration.message"),
        props.message.clone(),
        &props.view.editor_policy,
        true,
      )),
      Mode::Preserve => Some(super::fields::input(
        ctx,
        ("local-preservation-name", "local_integration.name"),
        props.name.clone(),
        &props.view.editor_policy,
        false,
      )),
      _ => None,
    };
    panel = panel.with_children(field);
    let checked = self.agree.clone();
    panel = panel.child(
      Toggle::new(
        "local-confirm-scope",
        ToggleKind::Checkbox,
        self.agree.get(),
        ctx.t(agree),
      )
      .on_change(move |on| {
        if input_source() != InputSource::Synthetic {
          checked.set(on);
        }
      }),
    );
    let ready = self.agree.get()
      && match mode {
        Mode::Commit { .. } => {
          !props.message.get().trim().is_empty() && !props.view.editor_policy.sensitive(&props.message.get())
        }
        Mode::Preserve => {
          !props.selected.get().is_empty()
            && !props.name.get().trim().is_empty()
            && !props.view.editor_policy.sensitive(&props.name.get())
        }
        _ => true,
      };
    let stale = phase == LocalPhase::AwaitingApproval && !props.view.checked_current;
    let unresolved =
      matches!(phase, LocalPhase::TaskConflicts | LocalPhase::RestoreConflicts) && !props.view.conflicts.is_empty();
    panel = panel.child(button(
      ctx,
      props,
      "local-confirm",
      label,
      Some(action),
      !ready || stale || unresolved || (matches!(mode, Mode::Abort | Mode::Cancel) && props.editor_dirty.get()),
    ));
    if stale {
      panel = panel
        .child(Text::new(&ctx.t("local_integration.error.stale")).variant(TypographyStyle::Description))
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
      .with_children(self.additional_decisions(ctx, props, (mode, phase)))
      .into()
  }

  fn additional_decisions(&self, ctx: &Ctx, props: &Props, (mode, phase): (&Mode, LocalPhase)) -> Vec<Element> {
    let mut actions = Vec::new();
    if phase == LocalPhase::TaskConflicts && *mode == Mode::Review {
      actions.push(mode_button(ctx, props, "local-review-abort", "local_integration.abort", Mode::Abort).into());
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
        "local-cancel-review",
        "local_integration.cancel",
        Mode::Review,
      )
      .into(),
    );
    actions
  }

  fn available(&self, ctx: &Ctx, props: &Props, phase: LocalPhase) -> Element {
    let panel = action_panel().child(Text::new(&ctx.t(phase_title(phase))).variant(TypographyStyle::Title));
    match phase {
      LocalPhase::Review => panel.with_children(self.preparation(ctx, props)),
      LocalPhase::Prepared => panel.with_children(self.prepared(ctx, props)),
      LocalPhase::Canceling => panel.child(button(
        ctx,
        props,
        "local-resume-cancel",
        "local_integration.reconcile",
        Some(Action::Reconcile),
        false,
      )),
      LocalPhase::Canceled => {
        if props
          .view
          .operation
          .as_ref()
          .and_then(|op| op.preservation())
          .is_some_and(|saved| saved.retained)
        {
          panel.child(mode_button(
            ctx,
            props,
            "local-cleanup-canceled",
            "local_integration.cleanup",
            Mode::Cleanup,
          ))
        } else {
          panel.child(button(
            ctx,
            props,
            "local-restart",
            "local_integration.refresh",
            Some(Action::Refresh),
            false,
          ))
        }
      }
      LocalPhase::MergedAwaitingRestore => panel.child(button(
        ctx,
        props,
        "local-restore",
        "local_integration.restore",
        Some(Action::Restore),
        false,
      )),
      LocalPhase::RestorationVerified => panel.child(mode_button(
        ctx,
        props,
        "local-review-cleanup",
        "local_integration.cleanup",
        Mode::Cleanup,
      )),
      LocalPhase::NeedsReconciliation => self.reconciliation(ctx, props, panel),
      LocalPhase::Complete => panel,
      _ => panel.child(button(
        ctx,
        props,
        "local-reconcile",
        "local_integration.reconcile",
        Some(Action::Reconcile),
        false,
      )),
    }
    .into()
  }

  fn reconciliation(&self, ctx: &Ctx, props: &Props, panel: Column) -> Column {
    let panel = panel.child(button(
      ctx,
      props,
      "local-reconcile",
      "local_integration.reconcile",
      Some(Action::Reconcile),
      false,
    ));
    if props
      .view
      .operation
      .as_ref()
      .is_some_and(|op| op.merged().is_none() && op.preservation().is_none())
    {
      panel.child(button(
        ctx,
        props,
        "local-review-current",
        "local_integration.refresh",
        Some(Action::Refresh),
        false,
      ))
    } else {
      panel
    }
  }
}

fn action_panel() -> Column {
  card("local-actions", CardSurface::Decision)
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
}
fn mode_button(ctx: &Ctx, props: &Props, id: &str, label: &str, chosen: Mode) -> ActionButton {
  let mode = props.mode.clone();
  ActionButton::new(id, ButtonKind::Secondary, ctx.t(label)).on_click(move || mode.set(chosen.clone()))
}

type Decision = (&'static str, &'static str, &'static str, &'static str, Action);
fn decision(props: &Props, mode: &Mode, phase: LocalPhase) -> Option<Decision> {
  Some(match mode {
    Mode::Commit { tree, .. } => (
      "local_integration.commitTitle",
      "local_integration.commitBody",
      "local_integration.commitBody",
      "local_integration.commit",
      Action::Commit {
        paths: props.selected.get(),
        tree: tree.clone(),
        message: props.message.get(),
      },
    ),
    Mode::Preserve => (
      "local_integration.preserveTitle",
      "local_integration.preserveBody",
      "local_integration.preserveBody",
      "local_integration.preserve",
      Action::Preserve {
        name: props.name.get(),
        paths: props.selected.get(),
      },
    ),
    Mode::Cancel => (
      "local_integration.cancelTitle",
      "local_integration.cancelBody",
      "local_integration.cancelAgree",
      "local_integration.cancelPreservation",
      Action::Cancel,
    ),
    Mode::Abort => (
      "local_integration.abortTitle",
      "local_integration.abortBody",
      "local_integration.abortAgree",
      "local_integration.abort",
      Action::Abort,
    ),
    Mode::Cleanup => (
      "local_integration.cleanupTitle",
      "local_integration.cleanupBody",
      "local_integration.cleanupAgree",
      "local_integration.cleanup",
      Action::Cleanup,
    ),
    Mode::Review => match phase {
      LocalPhase::AwaitingApproval => (
        "local_integration.checked",
        "local_integration.checkedBody",
        "local_integration.approval",
        "local_integration.merge",
        Action::Merge(Box::new(props.view.operation.as_ref()?.checked()?.clone())),
      ),
      LocalPhase::TaskConflicts => (
        "local_integration.continue",
        "local_integration.markers",
        "local_integration.resolved",
        "local_integration.continue",
        Action::Continue,
      ),
      LocalPhase::RestoreConflicts => (
        "local_integration.finishRestore",
        "local_integration.restoreBody",
        "local_integration.restoreReviewed",
        "local_integration.finishRestore",
        Action::FinishRestore,
      ),
      _ => return None,
    },
  })
}

fn button(ctx: &Ctx, props: &Props, id: &str, label: &str, action: Option<Action>, disabled: bool) -> ActionButton {
  let button = ActionButton::new(id, ButtonKind::Primary, ctx.t(label));
  if disabled {
    return button.disabled();
  }
  let Some(action) = action else {
    return button;
  };
  let request = Request {
    task: props.task.clone(),
    action,
    stop: props.stop.clone(),
  };
  let work = props.work.clone();
  button.on_click(move || {
    if input_source() == InputSource::Synthetic {
      return;
    }
    request.stop.store(false, Ordering::Release);
    work.run_if_idle(request.clone());
  })
}

fn recovery(ctx: &Ctx, view: &IntegrationView) -> Element {
  let mut panel = card("local-recovery", CardSurface::Outline)
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t("local_integration.recovery")).variant(TypographyStyle::Title));
  if let Some(operation) = &view.operation {
    panel = panel.child(Text::new(&operation.run().to_string()).variant(TypographyStyle::Mono));
    if let Some(saved) = operation.preservation() {
      panel = panel
        .child(Text::new(&saved.name).variant(TypographyStyle::Body))
        .child(
          Text::new(&ctx.t(if saved.retained {
            "local_integration.retained"
          } else {
            "local_integration.removed"
          }))
          .variant(TypographyStyle::Description),
        );
    }
    panel = panel
      .child(
        Text::new(operation.merged().unwrap_or(&ctx.t("local_integration.notRecorded"))).variant(TypographyStyle::Mono),
      )
      .child(Text::new(&ctx.t(phase_title(operation.phase()))).variant(TypographyStyle::Description));
  }
  panel.into()
}
