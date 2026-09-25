//! Close with active work (02.C `Xz30C`, dialog `um9B1`) and its waiting state (02.D `a0tTV`,
//! dialog `KS2RU`), from `docs/architecture/storage.md`, "Lifecycle". [`CloseConfirmation`]
//! shows it while the close flow asks; [`CloseDialog`] renders it from plain props.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Modal, ModalTarget, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use crate::{
  close::{ClosePhase, Lifecycle},
  runner::ActiveWork,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    backdrop::backdrop,
    button::ActionButton,
    icon::{Icon, IconSize, icon},
  },
};

/// Dialog width (`um9B1`), action bar and task row heights (`anyo4`, `uHX7b`), status label
/// height (`JIbWj`) and the fixed row cells (`F4NwB`, `c2rkx`, `VizsW`, `S2kP9L`).
const DIALOG_WIDTH: f32 = 880.0;
const ACTIONS_HEIGHT: f32 = 76.0;
const ROW_HEIGHT: f32 = 48.0;
const STATUS_HEIGHT: f32 = 26.0;
const KEY_WIDTH: f32 = 72.0;
const STEP_WIDTH: f32 = 140.0;
const STATUS_WIDTH: f32 = 112.0;
const SAVING_STATUS_WIDTH: f32 = 200.0;
const MODEL_WIDTH: f32 = 110.0;

type Handler = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseStage {
  /// The user decides (`um9B1`).
  Confirm,
  /// "Wait for agents to save" was chosen (`KS2RU`).
  Waiting,
}

/// What the dialog's buttons do.
#[derive(Clone)]
pub struct CloseActions {
  pub wait_for_agents: Handler,
  pub quit_now: Handler,
  pub cancel: Handler,
}

#[derive(Clone)]
pub struct CloseDialogProps {
  pub stage: CloseStage,
  pub work: Arc<[ActiveWork]>,
  pub actions: CloseActions,
}

impl PartialEq for CloseDialogProps {
  fn eq(&self, other: &Self) -> bool {
    self.stage == other.stage
      && self.work == other.work
      && Arc::ptr_eq(&self.actions.wait_for_agents, &other.actions.wait_for_agents)
      && Arc::ptr_eq(&self.actions.quit_now, &other.actions.quit_now)
      && Arc::ptr_eq(&self.actions.cancel, &other.actions.cancel)
  }
}

impl lurq::app::component::DevtoolsInspectable for CloseDialogProps {}

/// The dialog on its backdrop (`a8U756`).
pub struct CloseDialog;

impl Component for CloseDialog {
  type Props = CloseDialogProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<CloseDialogProps>().clone();
    let dialog = Column::new()
      .id("close-dialog")
      .describe("stage", stage_name(props.stage))
      .width(Dimension::Px(DIALOG_WIDTH))
      .background(ExtraColor::Surface.role())
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(RadiusSize::Lg)
      .child(header(ctx, props.stage))
      .child(body(ctx, props.stage, &props.work))
      .child(actions(ctx, props.stage, &props.actions));
    backdrop(dialog)
  }
}

fn stage_name(stage: CloseStage) -> &'static str {
  match stage {
    CloseStage::Confirm => "confirm",
    CloseStage::Waiting => "waiting",
  }
}

/// Title and explanation (`o85uW`, `TMVyq`).
fn header(ctx: &Ctx, stage: CloseStage) -> Element {
  let (title, explanation) = match stage {
    CloseStage::Confirm => ("lifecycle.close.confirmTitle", "lifecycle.close.confirmDetail"),
    CloseStage::Waiting => ("lifecycle.close.waitingTitle", "lifecycle.close.waitingDetail"),
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&ctx.t(title)).variant(ExtraTypography::DialogTitle.role()))
    .child(Text::new(&ctx.t(explanation)).variant(ExtraTypography::Explanation.role()))
    .into()
}

/// The runs at work and what each choice does (`b8tUT3`, `TUufp`).
fn body(ctx: &Ctx, stage: CloseStage, work: &[ActiveWork]) -> Element {
  let project = work.first().map_or("", ActiveWork::project);
  let heading = match stage {
    CloseStage::Confirm => ctx.t_args("lifecycle.close.inProgress", [("project", project)]),
    CloseStage::Waiting => {
      let key = if work.len() == 1 {
        "lifecycle.close.savingRuns.one"
      } else {
        "lifecycle.close.savingRuns.other"
      };
      let count = work.len().to_string();
      ctx.t_args(key, [("count", count.as_str()), ("project", project)])
    }
  };
  let rows = Column::new()
    .width(Dimension::Pct(100.0))
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .with_children(work.iter().map(|work| work_row(ctx, stage, work)));
  let closing = match stage {
    CloseStage::Confirm => choices(ctx),
    CloseStage::Waiting => Text::new(&ctx.t("lifecycle.close.waitingNote"))
      .variant(ExtraTypography::Detail.role())
      .color(PaletteColor::TextMuted)
      .into(),
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Step.role())
    .padding_horizontal(ExtraSpacing::Rail.role())
    .padding_bottom(SpacingSize::Lg)
    .spacing(SpacingSize::Md)
    .child(
      Text::new(&heading)
        .variant(TypographyStyle::Link)
        .color(PaletteColor::TextSecondary),
    )
    .child(rows)
    .child(closing)
    .into()
}

/// A task row (`clVjO` as in `uHX7b`): key, title, step · agent, status, provider · model.
fn work_row(ctx: &Ctx, stage: CloseStage, work: &ActiveWork) -> Element {
  let (status_icon, status, status_width) = match stage {
    CloseStage::Confirm => (Icon::Loader, "lifecycle.close.running", STATUS_WIDTH),
    CloseStage::Waiting => (Icon::Loader, "lifecycle.close.saving", SAVING_STATUS_WIDTH),
  };
  let step = ctx.t_args(
    "lifecycle.close.stepAgent",
    [("step", work.step()), ("agent", work.agent())],
  );
  let status = Row::new()
    .height(Dimension::Px(STATUS_HEIGHT))
    .padding_horizontal(SpacingSize::Sm)
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::AccentMuted)
    .rounded(RadiusSize::Sm)
    .child(icon(status_icon, IconSize::Small, PaletteColor::TextPrimary))
    .child(Text::new(&ctx.t(status)).variant(TypographyStyle::Link).nowrap());
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Text::new(work.task_key())
        .variant(TypographyStyle::Mono)
        .width(Dimension::Px(KEY_WIDTH)),
    )
    .child(
      Text::new(work.task_title())
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary)
        .nowrap()
        .flex(1.0),
    )
    .child(
      Text::new(&step)
        .variant(ExtraTypography::Footnote.role())
        .nowrap()
        .width(Dimension::Px(STEP_WIDTH)),
    )
    .child(Row::new().width(Dimension::Px(status_width)).child(status))
    .child(
      Text::new(work.model())
        .variant(ExtraTypography::Footnote.role())
        .nowrap()
        .width(Dimension::Px(MODEL_WIDTH)),
    )
    .into()
}

/// What each option does (`WAT7q`).
fn choices(ctx: &Ctx) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(ExtraSpacing::Stack.role())
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Notice.role())
    .child(choice(
      ctx,
      Icon::Save,
      "lifecycle.close.waitForAgents",
      "lifecycle.close.waitForAgentsDetail",
    ))
    .child(choice(
      ctx,
      Icon::Power,
      "lifecycle.close.quitNow",
      "lifecycle.close.quitNowDetail",
    ))
    .into()
}

fn choice(ctx: &Ctx, glyph: Icon, title: &str, detail: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .child(icon(glyph, IconSize::Option, PaletteColor::TextSecondary))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(SpacingSize::Xs)
        .child(Text::new(&ctx.t(title)).variant(ExtraTypography::NoticeHeading.role()))
        .child(Text::new(&ctx.t(detail)).variant(ExtraTypography::Detail.role())),
    )
    .into()
}

/// Quit now on the left, Cancel and Wait for agents to save on the right (`anyo4`); while
/// waiting, Cancel is gone and the primary becomes an indicator (`ZLe7v`).
fn actions(ctx: &Ctx, stage: CloseStage, actions: &CloseActions) -> Element {
  let quit = actions.quit_now.clone();
  let mut row = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ACTIONS_HEIGHT))
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      ActionButton::new(ctx.t("lifecycle.close.quitNow"))
        .icon(Icon::Power)
        .id("close-quit-now")
        .in_form()
        .on_click(move || quit())
        .render(ctx),
    )
    .child(Spacer::new().flex(1.0));
  match stage {
    CloseStage::Confirm => {
      let cancel = actions.cancel.clone();
      let wait = actions.wait_for_agents.clone();
      row = row
        .child(
          ActionButton::new(ctx.t("common.cancel"))
            .icon(Icon::X)
            .id("close-cancel")
            .in_form()
            .on_click(move || cancel())
            .render(ctx),
        )
        .child(
          ActionButton::new(ctx.t("lifecycle.close.waitForAgents"))
            .primary()
            .icon(Icon::Save)
            .id("close-wait")
            .in_form()
            .on_click(move || wait())
            .render(ctx),
        );
    }
    CloseStage::Waiting => row = row.child(waiting_indicator(ctx)),
  }
  row.into()
}

/// "Closes when every run is saved": shaped like a secondary button, but not one (`coL8l`).
fn waiting_indicator(ctx: &Ctx) -> Element {
  let form = ctx.theme().form();
  let height = form.button.secondary.height;
  drop(form);
  Row::new()
    .id("close-waiting")
    .height(height)
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(icon(Icon::Loader, IconSize::Button, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t("lifecycle.close.closesWhenSaved"))
        .variant(TypographyStyle::Button)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .into()
}

/// Shows [`CloseDialog`] while the close flow waits for the user or for agents to save.
pub struct CloseConfirmation;

impl Component for CloseConfirmation {
  type Props = Lifecycle;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let lifecycle = ctx.props::<Lifecycle>().clone();
    let (stage, work) = match lifecycle.phase().get() {
      ClosePhase::Confirming(work) => (CloseStage::Confirm, work),
      ClosePhase::Waiting(work) => (CloseStage::Waiting, work),
      ClosePhase::Open | ClosePhase::Closing => return Modal::new(Column::new()).open_when(false),
    };
    let (wait, quit, cancel) = (lifecycle.clone(), lifecycle.clone(), lifecycle);
    let props = CloseDialogProps {
      stage,
      work,
      actions: CloseActions {
        wait_for_agents: Arc::new(move || wait.wait_for_agents()),
        quit_now: Arc::new(move || quit.quit_now()),
        cancel: Arc::new(move || cancel.cancel()),
      },
    };
    Modal::new(ctx.mount_keyed::<CloseDialog>("close-dialog", props))
      .target(ModalTarget::Root)
      .dismiss_on_escape(false)
  }
}
