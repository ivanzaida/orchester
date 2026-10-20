//! What the project's Orchestrator chat wrote to a run's agents, as the task page marks it (ORC-45): a line with the
//! Orchestrator's `waypoints` mark saying how it went out, sent by the chat itself or proposed by it and applied by the
//! person, and *Open in Orchestrator chat*, which opens the chat's conversation it came from. Which entry is the
//! chat's and how it went out are `orchester_workspace::tasks::conversation::orchestrator`'s; this only draws them.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::Element,
};
use orchester_core::{
  id::{ChatId, ProjectId},
  orchestrator::{FromOrchestrator, SentWay},
};

use crate::{
  navigation::Navigation,
  routes::{OrchestratorChat, ProjectPage, Route},
  theme::IconSize,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// The line under what the chat wrote, `{id}-sent`: how it went out, when that was recorded, and the way back to its
/// conversation, when that was; `None` for one kept before either was.
pub(super) fn sent_line(ctx: &mut Ctx, id: &str, (project, from): (ProjectId, FromOrchestrator)) -> Option<Element> {
  let words = from.way.map(|way| {
    ctx.t(match way {
      SentWay::Direct => "conversation.orchestrator.sentDirectly",
      SentWay::AppliedByPerson => "conversation.orchestrator.appliedByYou",
    })
  });
  let open = from.chat.map(|chat| open_chat(ctx, id, (project, chat)));
  if words.is_none() && open.is_none() {
    return None;
  }
  let way = from.way.map_or("unknown", SentWay::word);
  Some(
    Row::new()
      .id(format!("{id}-sent"))
      .describe("sent", way)
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::Waypoints, IconSize::Sm, PaletteColor::TextMuted))
      .with_children(words.map(|words| Text::new(&words).variant(TypographyStyle::Caption)))
      .with_children(open)
      .into(),
  )
}

/// *Open in Orchestrator chat*, `{id}-open-chat`: opens conversation `chat` of `project`'s Orchestrator.
pub(super) fn open_chat(ctx: &mut Ctx, id: &str, (project, chat): (ProjectId, ChatId)) -> ActionButton {
  let route = Route::Project(project, ProjectPage::Orchestrator(OrchestratorChat::Chat(chat)));
  let label: Arc<str> = ctx.t("conversation.orchestrator.openChat");
  ActionButton::new(format!("{id}-open-chat"), ButtonKind::Ghost, label)
    .icon(Icon::ArrowUpRight)
    .compact()
    .opens(Navigation::of(ctx).to(route))
}
