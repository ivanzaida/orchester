//! The Orchestrator's header (`Header` `nKHIi`): 16 over, 32 at its sides and 12 under, 8 apart, over a `border`
//! line: the breadcrumb, the conversation's title (20/600) with *History*, *Manage context* and *New conversation*,
//! and the coordinator's line (`Coordinator` `CUWYM`): a 20 avatar with `waypoints`, *Orchestrator* and what it
//! runs on, *Settings*, and at the end whether the conversation is kept and since when.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{orchestrator::Chat, time::Timestamp};
use orchester_workspace::{display, orchestrator::settings::Readiness};

use super::View;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{OrchestratorChat, ProjectPage, Route},
  theme::{ConversationText, ExtraTypography, IconSize},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// The coordinator's avatar (`X8sblp`).
const AVATAR_SIZE: f32 = 20.0;

/// Which conversation the header names.
#[derive(Clone, Copy)]
pub(super) enum Named<'a> {
  /// A new one, not kept yet.
  New,
  /// One being opened, with its title when History has it: loading never looks like a new conversation (`kDOn9`).
  Opening(Option<&'a str>),
  Chat(&'a Chat),
}

/// What the header shows.
pub(super) struct Head<'a> {
  pub(super) project: &'a ShownProject,
  pub(super) chat: Named<'a>,
  /// Where the chat stands with what the Orchestrator runs on; `None` until read.
  pub(super) readiness: Option<&'a Readiness>,
}

pub(super) fn header(ctx: &mut Ctx, view: &View, head: &Head<'_>) -> Element {
  let project = head.project.open.project.id();
  let navigation = Navigation::of(ctx);
  let crumbs = breadcrumb(
    ctx,
    "orchestrator-crumbs",
    CrumbSeparator::Slash,
    vec![
      Crumb::to(
        head.project.open.project.name().as_str(),
        navigation.to(Route::Project(project, ProjectPage::Overview(Default::default()))),
      ),
      Crumb::here(ctx.t("orchestrator.title")),
    ],
  );
  let title = match head.chat {
    Named::New => ctx.t("orchestrator.newTitle"),
    Named::Opening(Some(title)) => title.into(),
    Named::Opening(None) => ctx.t("orchestrator.loading.title"),
    Named::Chat(chat) => chat.title().into(),
  };
  let history = view.history.clone();
  let start_new = new_conversation(ctx, head, &navigation);
  let title_row = Row::new()
    .id("orchestrator-title-row")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&title)
        .id("orchestrator-title")
        .variant(TypographyStyle::Heading)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(Spacer::new().flex(1.0))
    .child(
      ActionButton::new("orchestrator-history", ButtonKind::Ghost, ctx.t("orchestrator.history"))
        .icon(Icon::History)
        .on_click(move || history.update(|open| *open = !*open)),
    )
    .child(
      ActionButton::new(
        "orchestrator-context",
        ButtonKind::Ghost,
        ctx.t("orchestrator.manageContext"),
      )
      .icon(Icon::Layers2)
      .unavailable(ctx.t("orchestrator.manageContextUnavailable")),
    )
    .child(start_new);
  Column::new()
    .id("orchestrator-header")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Lg)
        .bottom(SpacingSize::Md)
        .left(SpacingSize::Section)
        .right(SpacingSize::Section),
    )
    .spacing(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(crumbs)
    .child(title_row)
    .child(coordinator(ctx, head))
    .into()
}

/// *New conversation*: nothing to leave while the conversation shown is a new one.
fn new_conversation(ctx: &Ctx, head: &Head<'_>, navigation: &Navigation) -> ActionButton {
  let project = head.project.open.project.id();
  let button = ActionButton::new(
    "orchestrator-new",
    ButtonKind::Secondary,
    ctx.t("orchestrator.newConversation"),
  )
  .icon(Icon::MessageSquarePlus);
  match head.chat {
    Named::Opening(_) | Named::Chat(_) => button.opens(navigation.to(Route::Project(
      project,
      ProjectPage::Orchestrator(OrchestratorChat::New),
    ))),
    Named::New => button.disabled(),
  }
}

/// The coordinator's line: who answers, on what, *Settings*, and the conversation's meta.
fn coordinator(ctx: &mut Ctx, head: &Head<'_>) -> Element {
  let runs_on = match head.readiness {
    Some(Readiness::Ready { connection, model }) => ctx.t_args(
      "orchestrator.coordinator.model",
      [("connection", &**connection), ("model", model.as_str())],
    ),
    Some(_) => ctx.t("orchestrator.coordinator.notSet"),
    None => "".into(),
  };
  let project = head.project.open.project.id();
  let navigation = Navigation::of(ctx);
  let meta = match head.chat {
    Named::New => ctx.t("orchestrator.meta.unsaved"),
    Named::Opening(_) => ctx.t("orchestrator.loading.meta"),
    Named::Chat(chat) => started(ctx, chat.started()),
  };
  Row::new()
    .id("orchestrator-coordinator")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Row::new()
        .size(Dimension::Px(AVATAR_SIZE), Dimension::Px(AVATAR_SIZE))
        .flex_shrink(0.0)
        .background(PaletteColor::SurfaceRaised)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Md)
        .align_items(Alignment::Center)
        .justify(Justify::Center)
        .child(icon(Icon::Waypoints, IconSize::Xs, PaletteColor::TextSecondary)),
    )
    .child(
      Row::new()
        .spacing(crate::theme::ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(
          Text::new(&ctx.t("orchestrator.title"))
            .variant(ConversationText::Name.role())
            .nowrap(),
        )
        .child(
          Text::new(&ctx.t("orchestrator.dot"))
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextMuted),
        )
        .child(
          Text::new(&runs_on)
            .id("orchestrator-runs-on")
            .variant(TypographyStyle::Description)
            .nowrap(),
        ),
    )
    .child(
      ActionButton::new(
        "orchestrator-settings",
        ButtonKind::Ghost,
        ctx.t("orchestrator.coordinator.settings"),
      )
      .icon(Icon::Settings)
      .small()
      .opens(navigation.to(Route::Project(project, ProjectPage::OrchestratorSettings))),
    )
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&meta)
        .id("orchestrator-meta")
        .variant(ExtraTypography::Note.role())
        .nowrap(),
    )
    .into()
}

/// *Project conversation · started* and when: the time today, else the date.
fn started(ctx: &Ctx, at: Timestamp) -> std::sync::Arc<str> {
  if at.local_day_start() == Timestamp::now().local_day_start() {
    ctx.t_args(
      "orchestrator.meta.startedToday",
      [("time", display::clock(at).as_str())],
    )
  } else {
    ctx.t_args("orchestrator.meta.started", [("date", &*super::date(ctx, at))])
  }
}
