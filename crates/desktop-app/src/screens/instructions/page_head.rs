//! The top of an instruction's page (`02b Instruction` `Sn37u`): the crumb back to Instructions, the page head with the
//! rule's name, scope and owner and the … menu with Remove, and the draft bar — what the rule is now (*Draft written
//! by automation · Published v3* when automation saved it), what publishing does, Discard draft and Publish.
use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::agents::data::VersionState;

use super::{
  state::{Selection, Tab, View},
  words,
};
use crate::{
  screens::hub::{
    draft_bar::{DraftBar, Standing},
    page_head::MORE_MENU_WIDTH,
    parts::tile,
  },
  theme::{ExtraSpacing, HubText, IconSize},
  ui::{
    icon::{Icon, icon},
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel},
    pressable::opening,
  },
};

/// ← Instructions, or ← Instructions / New instruction on a new rule.
pub fn crumb(ctx: &Ctx, selection: &Selection, new: bool) -> Element {
  let back = opening(selection.to(View::default()))
    .id("instruction-crumb")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::ArrowLeft, IconSize::Base, PaletteColor::TextMuted))
    .child(Text::new(&ctx.t("hub.tabs.instructions")).variant(HubText::Crumb.role()));
  Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(back)
    .with_children(new.then(|| {
      Text::new(&ctx.t("instructions.crumbNew"))
        .variant(HubText::Crumb.role())
        .color(PaletteColor::TextSecondary)
    }))
    .into()
}

/// The … menu under the head: Remove.
pub struct More {
  pub open: Signal<bool>,
  pub anchor: ElementRef,
  pub remove: Arc<dyn Fn() + Send + Sync>,
}

pub fn head(ctx: &mut Ctx, (title, sub): (&str, &str), more: Option<More>) -> Element {
  let menu = more.map(|more| {
    let remove = more.remove.clone();
    let item = MenuItem::new("instruction-remove", ctx.t("hub.remove.action"), move || remove())
      .icon(Icon::Trash2)
      .render(ctx, &more.open);
    let open = more.open.clone();
    let button = IconButton::new("instruction-more", Icon::Ellipsis, ctx.t("hub.more"))
      .small()
      .element(more.anchor.clone())
      .on_click(move || open.update(|open| *open = !*open))
      .render(ctx);
    let panel = menu_panel("instruction-more-menu")
      .width(Dimension::Px(MORE_MENU_WIDTH))
      .child(item);
    Column::new()
      .child(button)
      .child(menu_at_end(more.anchor, more.open, panel))
  });
  Row::new()
    .id("instruction-page-head")
    .width(Dimension::Pct(100.0))
    .spacing(lurq::app::theme::SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(tile(Icon::BookOpen, true))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Fine.role())
        .child(
          Text::new(title)
            .id("instruction-title")
            .variant(HubText::PageTitle.role()),
        )
        .child(Text::new(sub).id("instruction-sub").variant(HubText::Hint.role())),
    )
    .with_children(menu)
    .into()
}

/// What the bar says about the rule now.
pub struct Now {
  pub state: VersionState,
  pub dirty: bool,
  pub busy: bool,
  pub invalid: bool,
  pub automated: bool,
  pub saved_at: Option<orchester_core::time::Timestamp>,
}

pub fn bar(ctx: &Ctx, tab: Tab, now: &Now, actions: Vec<Element>) -> Element {
  let state = now.state;
  let version = state.published().map_or(0, |v| v.get());
  let (standing, status) = if now.busy {
    (Standing::Draft, ctx.t("instructions.saving"))
  } else if now.invalid {
    (Standing::Attention, ctx.t("instructions.notSaved"))
  } else if now.dirty {
    (Standing::Draft, ctx.t("instructions.unsaved"))
  } else if now.automated {
    (Standing::Draft, words::automation_version(ctx, state))
  } else {
    match state {
      VersionState::Published(_) => (Standing::Published, words::version(ctx, state)),
      VersionState::StaleDraft { .. } => (Standing::Attention, words::version(ctx, state)),
      VersionState::Draft { published: None } => (Standing::Draft, ctx.t("instructions.newDraft")),
      VersionState::Draft { .. } => {
        let saved = now.saved_at.map_or_else(
          || ctx.t("instructions.savedPlain"),
          |at| {
            ctx.t_args(
              "instructions.savedAt",
              [("time", orchester_workspace::display::clock(at))],
            )
          },
        );
        (
          Standing::Draft,
          ctx.t_args(
            "instructions.barDraft",
            [
              ("next", (version + 1).to_string()),
              ("saved", saved.to_string()),
              ("version", version.to_string()),
            ],
          ),
        )
      }
    }
  };
  let sub = words::footer(ctx, tab, state.published());
  // `instruction-save`: its status keeps the id the rule's save state had (`instruction-save-status`).
  let mut draft = DraftBar::new("instruction-save", standing, status).sub(sub);
  for action in actions {
    draft = draft.action(action);
  }
  draft.into()
}
