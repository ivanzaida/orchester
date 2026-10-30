//! A JavaScript dialog of the person's own tab (`03 · JavaScript dialog` `t08Ur`), which a headless frame does not
//! show: drawn over the page only, on the scrim, near its top: where it comes from (*localhost:5173 says*), its
//! message, a field for a prompt's answer, and its buttons: OK for an alert, Cancel and OK for a confirm or a prompt,
//! Stay and Leave for a page that asks before it unloads. The answer goes back to the tab
//! (`Page.handleJavaScriptDialog`). An agent's tab never shows one: the runner dismisses it and tells the agent.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::MouseEvent,
    theme::{BorderSize, PaletteColor, ShadowStyle, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_ipc::browser::{BrowserCommand, BrowserDialog, BrowserTabId, DialogKind};

use crate::{
  browser::{model, state::Showing},
  theme::{BrowserSpacing, BrowserText, ExtraColor, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    icon::{Icon, icon},
  },
};

/// The dialog's width (`Dialog · confirm` `Mcxt7`).
const DIALOG_WIDTH: f32 = 420.0;

/// The dialog waiting on `tab`, over the page.
pub fn dialog(ctx: &mut Ctx, showing: &Showing, tab: &BrowserTabId) -> Option<Element> {
  let dialog = showing.view.as_ref()?.dialog(tab)?.clone();
  let origin = showing
    .strip
    .find(tab)
    .map(|shown| model::host(&shown.url))
    .unwrap_or_default();
  Some(ctx.mount_keyed::<Dialog>(
    "browser-dialog",
    DialogProps {
      dialog,
      origin,
      tab: tab.clone(),
      showing: showing.clone(),
    },
  ))
}

#[derive(Clone, PartialEq)]
struct DialogProps {
  dialog: BrowserDialog,
  origin: String,
  tab: BrowserTabId,
  showing: Showing,
}

impl lurq::app::component::DevtoolsInspectable for DialogProps {}

struct Dialog {
  answer: Signal<String>,
}

impl Component for Dialog {
  type Props = DialogProps;

  fn create(ctx: &mut Ctx) -> Self {
    let start = ctx.props::<DialogProps>().dialog.default_prompt.clone();
    Self {
      answer: ctx.signal(start),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let DialogProps {
      dialog,
      origin,
      tab,
      showing,
    } = ctx.props::<DialogProps>().clone();
    let message = if dialog.kind == DialogKind::BeforeUnload && dialog.message.trim().is_empty() {
      ctx.t("browser.dialog.leaveMessage").to_string()
    } else {
      dialog.message.clone()
    };
    let answer = |accept: bool| {
      let (commands, tab, answer) = (showing.commands.clone(), tab.clone(), self.answer.clone());
      let prompt = dialog.kind == DialogKind::Prompt;
      move || {
        commands.send(BrowserCommand::AnswerDialog {
          tab: tab.clone(),
          accept,
          text: (prompt && accept).then(|| answer.get_untracked()),
        });
      }
    };
    let (cancel_key, ok_key) = match dialog.kind {
      DialogKind::BeforeUnload => ("browser.dialog.stay", "browser.dialog.leave"),
      _ => ("browser.dialog.cancel", "browser.dialog.ok"),
    };
    let cancel = (dialog.kind != DialogKind::Alert).then(|| {
      ActionButton::new("browser-dialog-cancel", ButtonKind::Secondary, ctx.t(cancel_key)).on_click(answer(false))
    });
    let ok = ActionButton::new("browser-dialog-ok", ButtonKind::Primary, ctx.t(ok_key)).on_click(answer(true));
    let field = (dialog.kind == DialogKind::Prompt).then(|| {
      FieldInput::new(
        "browser-dialog-answer",
        ctx.t("browser.dialog.answer"),
        self.answer.clone(),
      )
      .element(showing.dialog_answer.clone())
      .render(ctx)
    });
    let actions = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .justify(Justify::End)
      .with_children(cancel)
      .child(ok);
    let card = card(ctx, (dialog.kind, &origin, &message), field, actions);
    Column::new()
      .id("browser-dialog")
      .absolute(0.0, 0.0, Dimension::Pct(100.0), Dimension::Pct(100.0))
      .background(ExtraColor::Scrim.role())
      .padding(Padding::new().top(lurq::app::theme::SpacingSize::Xl))
      .align_items(Alignment::Center)
      // The page under the scrim takes no input while its dialog waits.
      .on_mouse_down(|event: MouseEvent| event.stop_propagation())
      .on_mouse_up(|event: MouseEvent| event.stop_propagation())
      .child(card)
  }
}

/// The dialog's card: where it comes from, its message, a prompt's field and its buttons.
fn card(ctx: &Ctx, (kind, origin, message): (DialogKind, &str, &str), field: Option<Element>, actions: Row) -> Column {
  Column::new()
    .id("browser-dialog-box")
    .describe("kind", kind_name(kind))
    .width(Dimension::Px(DIALOG_WIDTH))
    .padding(BrowserSpacing::Dialog.role())
    .spacing(lurq::app::theme::SpacingSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
    .rounded(lurq::app::theme::RadiusSize::Lg)
    .box_shadow(ShadowStyle::Lg)
    .child(
      Row::new()
        .spacing(crate::theme::ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::Globe, IconSize::Base, PaletteColor::TextMuted))
        .child(
          Text::new(&ctx.t_args("browser.dialog.says", [("origin", origin)]))
            .id("browser-dialog-origin")
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextMuted),
        ),
    )
    .child(
      Text::new(message)
        .id("browser-dialog-message")
        .variant(BrowserText::DialogMessage.role())
        .width(Dimension::Pct(100.0)),
    )
    .with_children(field)
    .child(actions)
}

const fn kind_name(kind: DialogKind) -> &'static str {
  match kind {
    DialogKind::Alert => "alert",
    DialogKind::Confirm => "confirm",
    DialogKind::Prompt => "prompt",
    DialogKind::BeforeUnload => "beforeunload",
  }
}
