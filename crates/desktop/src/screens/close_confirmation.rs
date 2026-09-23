//! Close confirmation (`docs/architecture/storage.md`, "Lifecycle"): shown when closing would
//! interrupt active work. Placeholder presentation from the recovery panel (`iN90G`) and
//! buttons (`u1XBP`) until the confirmation is designed.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Modal, ModalTarget, Row, Text},
  node::Element,
};

use crate::{
  close::{ClosePhase, Lifecycle},
  root::centered,
  runner::ActiveWork,
  ui::{button::ActionButton, icon::Icon, panel::RecoveryPanel},
};

pub struct CloseConfirmation;

impl Component for CloseConfirmation {
  type Props = Lifecycle;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let lifecycle = ctx.props::<Lifecycle>().clone();
    let (active, waiting) = match lifecycle.phase().get() {
      ClosePhase::Confirming(active) => (active, false),
      ClosePhase::Waiting(active) => (active, true),
      ClosePhase::Open | ClosePhase::Closing => return Modal::new(Column::new()).open_when(false),
    };
    let (title, explanation) = if waiting {
      ("close.waitingTitle", "close.waitingDetail")
    } else {
      ("close.confirmTitle", "close.confirmDetail")
    };
    let panel = RecoveryPanel::new(Icon::AlertCircle, &ctx.t(title), &ctx.t(explanation))
      .detail(work_list(&active))
      .action(actions(ctx, &lifecycle, waiting));
    Modal::new(centered(panel.into()))
      .target(ModalTarget::Root)
      .dismiss_on_escape(false)
  }
}

/// Task, current step and agent of each active run; runtime data, not translated.
fn work_list(active: &[ActiveWork]) -> Element {
  Column::new()
    .spacing(SpacingSize::Xs)
    .with_children(active.iter().map(|work| {
      Row::new()
        .spacing(SpacingSize::Sm)
        .child(Text::new(work.task()).variant(TypographyStyle::Label))
        .child(Text::new(work.step()).variant(TypographyStyle::Caption))
        .child(Text::new(work.agent()).variant(TypographyStyle::Caption))
    }))
    .into()
}

fn actions(ctx: &Ctx, lifecycle: &Lifecycle, waiting: bool) -> Element {
  let quit = lifecycle.clone();
  let mut row = Row::new().spacing(SpacingSize::Sm);
  if !waiting {
    let wait = lifecycle.clone();
    row = row.child(
      ActionButton::new(ctx.t("close.waitForAgents"))
        .primary()
        .id("close-wait")
        .on_click(move || wait.wait_for_agents())
        .render(ctx),
    );
  }
  row = row.child(
    ActionButton::new(ctx.t("close.quitNow"))
      .id("close-quit-now")
      .on_click(move || quit.quit_now())
      .render(ctx),
  );
  if !waiting {
    let cancel = lifecycle.clone();
    row = row.child(
      ActionButton::new(ctx.t("common.cancel"))
        .id("close-cancel")
        .on_click(move || cancel.cancel())
        .render(ctx),
    );
  }
  row.into()
}
