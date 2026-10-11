//! What a save or Turn on widens, asked in place before it is done (the form's action bar, the inspector's footer,
//! as `Confirm · Turn off` `D2FVJJ` asks): the reasons, `connections::mcp::widenings`' words, and a way back first.
//! Only a person confirms it. While it shows, the window takes no MCP client's input (`InputGate::protect`: lurq
//! denies its input tools, also a call queued earlier or one whose answer timed out, and the server refuses them
//! with `person_only`), it takes no press before it lived through one pass, and none while a client's input is
//! being applied. Automation widens through the domain tools, which a person lets it do in Settings.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::messages::Message;

use crate::{
  mcp::input_gate::{Guarded, InputGate, Protected},
  theme::{ConnectionsText, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

type Handler = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone)]
pub struct WidenProps {
  pub id: &'static str,
  pub card: bool,
  pub title: Arc<str>,
  pub reasons: Arc<[Arc<str>]>,
  pub confirm: Arc<str>,
  pub keep: Arc<str>,
  pub on_confirm: Handler,
  pub on_keep: Handler,
}

/// The props are equal when they say the same: the handlers are made anew on every render.
impl PartialEq for WidenProps {
  fn eq(&self, other: &Self) -> bool {
    (
      self.card,
      self.id,
      &self.title,
      &self.reasons,
      &self.confirm,
      &self.keep,
    ) == (
      other.card,
      other.id,
      &other.title,
      &other.reasons,
      &other.confirm,
      &other.keep,
    )
  }
}

impl DevtoolsInspectable for WidenProps {}

pub struct WidenConfirm {
  gate: Option<InputGate>,
  /// While the question shows, the window takes no MCP client's input.
  _protected: Option<Protected>,
}

impl Component for WidenConfirm {
  type Props = WidenProps;

  fn create(ctx: &mut Ctx) -> Self {
    let gate = ctx.use_context::<InputGate>();
    Self {
      _protected: gate.as_ref().map(|gate| gate.protect(Guarded::WideningQuestion)),
      gate,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<WidenProps>().clone();
    // No press before the question lived through one pass: input meant for what showed before cannot land on it.
    let armed = ctx
      .future((), |()| async { Ok::<(), Message>(()) })
      .state()
      .get()
      .data
      .is_some();
    let keep = ActionButton::new(format!("{}-keep", props.id), ButtonKind::Secondary, props.keep.clone()).on_click({
      let keep = props.on_keep.clone();
      move || keep()
    });
    let confirm = ActionButton::new(
      format!("{}-confirm", props.id),
      ButtonKind::Primary,
      props.confirm.clone(),
    );
    let confirm = if armed {
      let (gate, go) = (self.gate.clone(), props.on_confirm.clone());
      confirm.on_click(move || {
        if gate.as_ref().is_some_and(InputGate::input_in_flight) {
          tracing::warn!("an MCP client's window input reached a widening's confirmation; not taken");
          return;
        }
        go();
      })
    } else {
      confirm.disabled()
    };
    if props.card {
      card_question(&props, (keep, confirm))
    } else {
      question(&props, (keep, confirm))
    }
  }
}

fn question(props: &WidenProps, (keep, confirm): (ActionButton, ActionButton)) -> Column {
  let reasons = props.reasons.iter().map(|reason| {
    Text::new(reason)
      .variant(ExtraTypography::Note.role())
      .color(PaletteColor::TextSecondary)
      .width(Dimension::Pct(100.0))
  });
  Column::new()
    .id(props.id)
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, SpacingSize::Md))
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::WarningMuted)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Warning))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Start)
        .child(icon(Icon::TriangleAlert, IconSize::Md, PaletteColor::Warning))
        .child(
          Column::new()
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .spacing(ExtraSpacing::Fine.role())
            .child(
              Text::new(&props.title)
                .id(format!("{}-title", props.id))
                .variant(ConnectionsText::ConfirmTitle.role()),
            )
            .with_children(reasons),
        ),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .child(Spacer::new().flex(1.0))
        .child(keep)
        .child(confirm),
    )
}
fn card_question(props: &WidenProps, (keep, confirm): (ActionButton, ActionButton)) -> Column {
  let lines = props.reasons.iter().map(|reason| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Start)
      .child(icon(Icon::ShieldAlert, IconSize::Base, PaletteColor::TextMuted))
      .child(
        Text::new(reason)
          .variant(lurq::app::theme::TypographyStyle::Description)
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
  });
  Column::new()
    .id(props.id)
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Lg)
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Tile.role())
    .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
    .child(
      Text::new(&props.title)
        .id(format!("{}-title", props.id))
        .variant(ExtraTypography::Strong.role()),
    )
    .with_children(lines)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .padding(Padding::new().top(ExtraSpacing::Snug.role()))
        .child(confirm)
        .child(keep),
    )
}
