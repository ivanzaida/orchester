//! The popover of the MCP item: where Orchester's MCP server listens and *Let automation answer
//! approvals*, an application setting that is off until a person turns it on here. While it is off,
//! `decide_approval` is refused. The popover takes the operations popover's panel (`E1msJS`) and opens
//! upward from the item; Escape or a click outside closes it. The design has no frame for it yet.

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction, FutureState, Placement},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Popup, Rect, Text},
  core::{ElementRef, Signal},
  node::{Element, border::Border, dimension::Dimension},
};

use super::{POPOVER_GAP, item::value};
use crate::{
  app_mcp, background,
  messages::{self, Message},
  theme::{ExtraColor, ExtraShadow, ExtraSpacing, ExtraTypography},
  ui::{field::wrapping_field_error, message_text, switch::Switch},
  workspace::Workspace,
};

/// Popover width, the operations popover's (`E1msJS`).
const WIDTH: f32 = 420.0;
const DIVIDER_HEIGHT: f32 = 1.0;

#[derive(Clone)]
pub struct McpPopoverProps {
  pub port: std::num::NonZeroU16,
  /// The MCP item, which the popover hangs from.
  pub anchor: ElementRef,
  pub open: Signal<bool>,
}

impl PartialEq for McpPopoverProps {
  fn eq(&self, other: &Self) -> bool {
    self.port == other.port
  }
}

impl lurq::app::component::DevtoolsInspectable for McpPopoverProps {}

type SaveAction = FutureAction<bool, bool, Message>;

pub struct McpPopover {
  /// The setting as the person last chose it; `None` until they do.
  saved: Signal<Option<bool>>,
}

impl Component for McpPopover {
  type Props = McpPopoverProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      saved: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<McpPopoverProps>().clone();
    // Hooks in the same order on every render: the action, then the read, which follows each save.
    let save = save_action(ctx);
    let state = save.state().get();
    let read = stored(ctx, state.data);
    let fixed = state.is_pending() || read.is_pending();
    // The person's choice once it saved; the stored value until then, or after a save failed.
    let shown = match (&state.error, self.saved.get()) {
      (None, Some(chosen)) => Some(chosen),
      _ => read.data,
    };
    let panel = Column::new()
      .id("status-mcp-popover")
      .width(Dimension::Px(WIDTH))
      .padding(SpacingSize::Md)
      .spacing(ExtraSpacing::Stack.role())
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .box_shadow(ExtraShadow::Popover.role())
      .child(Text::new(&ctx.t("appMcp.title")).variant(ExtraTypography::NoticeHeading.role()))
      .child(value(&app_mcp::endpoint(props.port), PaletteColor::TextPrimary))
      .child(
        Text::new(&ctx.t_args("appMcp.explanation", [("pid", std::process::id().to_string().as_str())]))
          .variant(ExtraTypography::CellLabel.role())
          .width(Dimension::Pct(100.0)),
      )
      .child(Rect::new(Dimension::Pct(100.0), Dimension::Px(DIVIDER_HEIGHT)).background(ExtraColor::Hairline.role()))
      .child(self.switch(ctx, (shown, &save), fixed))
      .with_children(
        state
          .error
          .or(read.error)
          .map(|error| wrapping_field_error(&message_text(ctx, &error))),
      );
    Popup::new(props.anchor, panel)
      .open(props.open)
      .placement(Placement::TopEnd)
      .offset(0.0, POPOVER_GAP)
  }
}

impl McpPopover {
  /// *Let automation answer approvals*, fixed while it is read or saved.
  fn switch(&self, ctx: &Ctx, (allowed, save): (Option<bool>, &SaveAction), fixed: bool) -> Element {
    let switch = Switch::new(
      "status-mcp-answer-approvals",
      ctx.t("appMcp.answerApprovals"),
      allowed.unwrap_or(false),
    )
    .note(ctx.t("appMcp.answerApprovalsNote"));
    match allowed {
      Some(allowed) if !fixed => {
        let (save, saved) = (save.clone(), self.saved.clone());
        switch.render(move || {
          saved.set(Some(!allowed));
          save.run(!allowed);
        })
      }
      _ => switch.locked(),
    }
  }
}

/// Saves the setting the person chose.
fn save_action(ctx: &mut Ctx) -> SaveAction {
  let workspace = workspace(ctx);
  ctx.future_action(move |allowed: bool| {
    let workspace = workspace.clone();
    async move {
      background::run(move || workspace.set_automation_answers_approvals(allowed))
        .await?
        .map(|()| allowed)
        .map_err(|error| {
          tracing::error!(%error, "Let automation answer approvals was not saved");
          messages::store_error(&error)
        })
    }
  })
}

/// The setting as stored, read when the popover opens and again after each save (`saved`).
fn stored(ctx: &mut Ctx, saved: Option<bool>) -> FutureState<bool, Message> {
  let workspace = workspace(ctx);
  let read = ctx.future(saved, move |_| {
    let workspace = workspace.clone();
    async move {
      background::run(move || workspace.automation_answers_approvals())
        .await?
        .map_err(|error| {
          tracing::error!(%error, "Let automation answer approvals cannot be read");
          messages::store_error(&error)
        })
    }
  });
  read.state().get()
}

fn workspace(ctx: &mut Ctx) -> Workspace {
  ctx
    .use_context::<Workspace>()
    .expect("the root provides the workspace whenever Orchester's MCP server runs")
}
