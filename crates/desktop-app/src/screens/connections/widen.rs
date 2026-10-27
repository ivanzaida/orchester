//! What a save or Turn on widens, asked in place before it is done (the form's action bar, the inspector's footer,
//! as `Confirm · Turn off` `D2FVJJ` asks): the reasons, `connections::mcp::widenings`' words, and a way back first.
//! Each caller explicitly classifies its consent: domain setup permissions, automatic approvals, or always
//! person-only. The stored human opt-in permits only its matching app-owned question, never Settings itself.
//! No press is taken before one rendered pass; forwarded input rechecks live consent when the handler runs. An
//! approval card's question locks no window input: only its confirm refuses an MCP client's press, and the card
//! refuses a wider answer from one besides (`task::cards::decisions`); taking it back (*Keep asking*) only narrows,
//! so a client can still move, look and close it.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::is_synthetic_input,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::messages::Message;

use crate::{
  mcp::input_gate::{Guarded, InputGate, Protected, input_may_decide},
  theme::{ConnectionsText, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
  },
};

type Handler = Arc<dyn Fn() + Send + Sync>;

/// How the question is drawn where it is asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidenLook {
  /// A band at a form's or an inspector's foot: `warning_muted` under a `warning` line.
  Band,
  /// On an approval card (`HYB5f`).
  Card,
  /// The question box Project settings and its tabs ask in place (`Archive · question` `fw0eT`).
  Box,
}

#[derive(Clone)]
pub struct WidenProps {
  pub id: Arc<str>,
  pub look: WidenLook,
  /// The question confirms an approval rather than warning of a widening: its reasons and its confirm carry
  /// shield-check (`HYB5f`).
  pub affirming: bool,
  pub automation: Guarded,
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
      self.look,
      self.affirming,
      self.automation,
      &self.id,
      &self.title,
      &self.reasons,
      &self.confirm,
      &self.keep,
    ) == (
      other.look,
      other.affirming,
      other.automation,
      &other.id,
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
  /// The mounted question carries its exact consent classification.
  protected: parking_lot::Mutex<Option<(Guarded, Protected)>>,
}

impl Component for WidenConfirm {
  type Props = WidenProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self::new(ctx)
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<WidenProps>().clone();
    let (keep, confirm) = self.buttons(ctx, &props, ButtonKind::Secondary);
    match props.look {
      WidenLook::Card => card_question(&props, (keep, confirm)),
      WidenLook::Band => question(&props, (keep, confirm)),
      WidenLook::Box => crate::screens::project_settings::kit::question(
        &props.id,
        &props.title,
        &props.reasons,
        vec![confirm.into(), keep.into()],
      ),
    }
  }
}

/// [`WidenNotice`]'s question and the icon its confirm carries: *Save draft*'s save, *Publish version*'s arrow-up.
#[derive(Clone, PartialEq)]
pub struct NoticeProps {
  pub question: WidenProps,
  pub confirm_icon: Icon,
}

impl DevtoolsInspectable for NoticeProps {}

/// [`WidenConfirm`] as a warning notice in place, its confirm and *Keep* under its reasons (`Notice · warning`
/// `DAKkj` in `fXnX2`): where a page asks before it saves or publishes, among its other content.
pub struct WidenNotice(WidenConfirm);

impl Component for WidenNotice {
  type Props = NoticeProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<NoticeProps>().question.clone();
    Self(WidenConfirm::with(ctx, &props))
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let NoticeProps {
      question: props,
      confirm_icon,
    } = ctx.props::<NoticeProps>().clone();
    let (keep, confirm) = self.0.buttons(ctx, &props, ButtonKind::Ghost);
    let mut reasons = props.reasons.iter();
    let mut notice = Notice::new(props.id.clone(), NoticeTone::Warning, props.title.clone()).stacked();
    if let Some(first) = reasons.next() {
      notice = notice.body(first.clone());
    }
    for reason in reasons {
      notice = notice.part(
        Text::new(reason)
          .variant(ExtraTypography::Prose.role())
          .width(Dimension::Pct(100.0)),
      );
    }
    Element::from(notice.action(confirm.icon(confirm_icon)).action(keep))
  }
}

impl WidenConfirm {
  fn new(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<WidenProps>().clone();
    Self::with(ctx, &props)
  }

  /// The question asked with `props`, which its component's own props hold.
  fn with(ctx: &mut Ctx, props: &WidenProps) -> Self {
    let gate = ctx.use_context::<InputGate>();
    let policy = props.automation;
    let locks = locks_input(props);
    Self {
      protected: parking_lot::Mutex::new(
        gate
          .as_ref()
          .filter(|_| locks)
          .map(|gate| (policy, gate.protect(policy))),
      ),
      gate,
    }
  }

  /// The question's *Keep*, a button of `keep_kind`, and its confirm, each guarded as its consent says.
  fn buttons(&self, ctx: &mut Ctx, props: &WidenProps, keep_kind: ButtonKind) -> (ActionButton, ActionButton) {
    if let Some(gate) = self.gate.as_ref().filter(|_| locks_input(props)) {
      let mut protected = self.protected.lock();
      if protected
        .as_ref()
        .is_some_and(|(policy, _)| *policy != props.automation)
      {
        *protected = None;
        *protected = Some((props.automation, gate.protect(props.automation)));
      }
    }
    consent(ctx, &self.gate);

    // No press before the question lived through one pass: input meant for what showed before cannot land on it.
    let armed = ctx
      .future((), |()| async { Ok::<(), Message>(()) })
      .state()
      .get()
      .data
      .is_some();
    // An approval card's question only narrows when taken back: any input may. Elsewhere taking it back may discard
    // a person's draft, which stays theirs.
    let keep = if locks_input(props) {
      guarded_handler(self.gate.clone(), props.automation, props.on_keep.clone())
    } else {
      props.on_keep.clone()
    };
    // In the question box *Keep it* is the quiet way back after the action (`oLg02` in `fw0eT`).
    let keep_kind = if props.look == WidenLook::Box {
      ButtonKind::Ghost
    } else {
      keep_kind
    };
    let keep = ActionButton::new(format!("{}-keep", props.id), keep_kind, props.keep.clone()).on_click(move || keep());
    let confirm = ActionButton::new(
      format!("{}-confirm", props.id),
      ButtonKind::Primary,
      props.confirm.clone(),
    );
    let confirm = if props.affirming {
      confirm.icon(Icon::ShieldCheck)
    } else {
      confirm
    };
    let confirm = if armed {
      let handler = guarded_handler(self.gate.clone(), props.automation, props.on_confirm.clone());
      confirm.on_click(move || handler())
    } else {
      confirm.disabled()
    };
    (keep, confirm)
  }
}

/// Whether the question locks the window's input from MCP clients while it shows: every one but an approval card's.
fn locks_input(props: &WidenProps) -> bool {
  !(props.look == WidenLook::Card && props.automation == Guarded::ApprovalWidening)
}

fn guarded_handler(gate: Option<InputGate>, policy: Guarded, handler: Handler) -> Handler {
  Arc::new(move || {
    if !input_may_decide(gate.as_ref(), policy) {
      tracing::warn!("synthetic input reached a protected permission decision; not taken");
      return;
    }
    if is_synthetic_input() {
      tracing::info!("automation answered application permission review under stored consent");
    }
    handler();
  })
}

/// Asked for in every render, with or without the services: lurq keeps a component's futures by the order it
/// asks for them, and `armed` after it has the same type, so a skipped call would hand it this one's state.
fn consent(ctx: &mut Ctx, gate: &Option<InputGate>) {
  let services = ctx.use_context::<crate::services::Services>();
  let read = ctx.future((), move |()| {
    let workspace = services.as_ref().map(|services| services.workspace.clone());
    async move {
      let Some(workspace) = workspace else {
        return Ok(());
      };
      let read = workspace.clone();
      let result = orchester_workspace::background::run(move || {
        workspace.automation_changes_permissions()?;
        workspace.automation_answers_approvals()?;
        Ok::<(), orchester_store::StoreError>(())
      })
      .await;
      if !matches!(&result, Ok(Ok(()))) {
        read.invalidate_automation_consent();
      }
      result?.map_err(|error| orchester_workspace::messages::store_error(&error))
    }
  });
  read.state().get();
  if let Some(gate) = gate {
    gate.refresh_tools();
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
    .id(props.id.to_string())
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
      .child(icon(
        if props.affirming {
          Icon::ShieldCheck
        } else {
          Icon::ShieldAlert
        },
        IconSize::Base,
        PaletteColor::TextMuted,
      ))
      .child(
        Text::new(reason)
          .variant(lurq::app::theme::TypographyStyle::Description)
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
  });
  Column::new()
    .id(props.id.to_string())
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
