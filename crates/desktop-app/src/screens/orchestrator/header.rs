//! The Orchestrator's header (`Header` `s9sEw6` in `01 Conversation` `wRSL4`): one 52-high line over a `border` line,
//! 24 in at its start and 16 at its end, 8 apart: the conversation's title (14/600) and, in 12 `text_muted`, when it
//! started (*· today 16:02*); at its end *New* (a ghost with `square-pen`) and the History toggle (a `panel-right`
//! icon button, on the `pressed` fill while History is open). Orchestrator settings open from the composer's runs-on
//! chip and its picker.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{orchestrator::Chat, time::Timestamp};
use orchester_workspace::display;

use super::View;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{OrchestratorChat, ProjectPage, Route},
  theme::OrchestratorText,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    icon_button::IconButton,
  },
};

/// The header's height (`s9sEw6`).
const HEIGHT: f32 = 52.0;

/// Which conversation the header names.
#[derive(Clone, Copy)]
pub(super) enum Named<'a> {
  /// A new one, not kept yet.
  New,
  /// One being opened, with its title when History has it: loading never looks like a new conversation.
  Opening(Option<&'a str>),
  Chat(&'a Chat),
}

pub(super) fn header(ctx: &mut Ctx, view: &View, (project, chat): (&ShownProject, Named<'_>)) -> Element {
  let title = match chat {
    Named::New => ctx.t("orchestrator.newTitle"),
    Named::Opening(Some(title)) => title.into(),
    Named::Opening(None) => ctx.t("orchestrator.loading.title"),
    Named::Chat(chat) => chat.title().into(),
  };
  let meta = match chat {
    Named::New => None,
    Named::Opening(_) => Some(ctx.t("orchestrator.loading.meta")),
    Named::Chat(chat) => Some(started(ctx, chat.started())),
  };
  let history = view.history.clone();
  let mut toggle = IconButton::new("orchestrator-history", Icon::PanelRight, ctx.t("orchestrator.history"))
    .on_click(move || history.update(|open| *open = !*open));
  if view.history.get() {
    toggle = toggle.active();
  }
  Row::new()
    .id("orchestrator-header")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEIGHT))
    .flex_shrink(0.0)
    .padding(Padding::new().left(SpacingSize::Xl).right(SpacingSize::Lg))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&title)
        .id("orchestrator-title")
        .variant(OrchestratorText::HeaderTitle.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      Text::new(&meta.unwrap_or_default())
        .id("orchestrator-meta")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap()
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(new_conversation(ctx, project, chat))
    .child(toggle.render(ctx))
    .into()
}

/// *New*: nothing to leave while the conversation shown is a new one.
fn new_conversation(ctx: &mut Ctx, project: &ShownProject, chat: Named<'_>) -> ActionButton {
  let button =
    ActionButton::new("orchestrator-new", ButtonKind::Ghost, ctx.t("orchestrator.new")).icon(Icon::SquarePen);
  match chat {
    Named::Opening(_) | Named::Chat(_) => {
      let navigation = Navigation::of(ctx);
      button.opens(navigation.to(Route::Project(
        project.open.project.id(),
        ProjectPage::Orchestrator(OrchestratorChat::New),
      )))
    }
    Named::New => button.disabled(),
  }
}

/// *· today 16:02*, or *· Oct 8* for an earlier day.
fn started(ctx: &Ctx, at: Timestamp) -> std::sync::Arc<str> {
  if at.local_day_start() == Timestamp::now().local_day_start() {
    ctx.t_args("orchestrator.meta.today", [("time", display::clock(at).as_str())])
  } else {
    ctx.t_args("orchestrator.meta.on", [("date", &*super::date(ctx, at))])
  }
}
