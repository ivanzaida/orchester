//! History (`Panel · History` `pHsY5` in `kPs8W`): the project's conversations beside the chat, 400 wide after a
//! `border` line. Its head (14 over, 16 at its sides, 10 under): *History* (13/600), the project's name (11.5
//! `text_muted`), the *Esc* key and *Close*; a search field over the list (16 at its sides, 8 under); the list,
//! under a `border` line, by when each conversation last changed (*Today*, *Yesterday*, *Earlier*: Geist Mono 11
//! `text_muted`, tracking 0.6), each a row (`Conversation row` `V37fU`: radius 6, 8 by 10 inside) with its title
//! (12.5/500), when (11 `text_muted`) and how many messages (11.5 `text_muted`), the one shown on `hover`; and a note
//! at its foot. Opening one keeps the draft; Esc closes it.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, VirtualizedList},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ChatId, ProjectId},
  time::Timestamp,
};
use orchester_store::ChatListing;
use orchester_workspace::display;

use crate::{
  navigation::Navigation,
  routes::{OrchestratorChat, ProjectPage, Route},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, OrchestratorText},
  ui::{field::FieldInput, icon::Icon, icon_button::IconButton, kbd::key_cap, pressable::pressable},
};

/// The panel's width (`pHsY5`).
const WIDTH: f32 = 400.0;

/// When a conversation last changed, as the list groups it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Day {
  Today,
  Yesterday,
  Earlier,
}

impl Day {
  fn of(at: Timestamp, today: Timestamp) -> Self {
    let day = at.local_day_start();
    if day == today {
      Self::Today
    } else if day == Timestamp::from_unix_millis(today.unix_millis() - 1).local_day_start() {
      Self::Yesterday
    } else {
      Self::Earlier
    }
  }

  const fn key(self) -> &'static str {
    match self {
      Self::Today => "orchestrator.historyPanel.today",
      Self::Yesterday => "orchestrator.historyPanel.yesterday",
      Self::Earlier => "orchestrator.historyPanel.earlier",
    }
  }
}

/// A row of the list: a day's heading or a conversation.
#[derive(Clone, PartialEq)]
enum Item {
  Day(Day),
  Chat {
    listing: ChatListing,
    current: bool,
    today: bool,
    project: ProjectId,
  },
}

impl lurq::app::component::DevtoolsInspectable for Item {}

impl Item {
  fn key(&self) -> String {
    match self {
      Self::Day(day) => format!("day-{day:?}"),
      Self::Chat { listing, .. } => listing.chat.id().to_string(),
    }
  }
}

/// The panel over `chats` of the project (`name`), `current` the one shown; `open` closes it and `search` filters
/// it by title.
pub(super) fn panel(
  ctx: &mut Ctx,
  (open, search): (&Signal<bool>, &Signal<String>),
  (project, name): (ProjectId, &str),
  (chats, current): (&[ChatListing], Option<ChatId>),
) -> Element {
  let head = head(ctx, open, name);
  let field = FieldInput::new(
    "orchestrator-history-search",
    ctx.t_args("orchestrator.historyPanel.search", [("project", name)]),
    search.clone(),
  )
  .search()
  .placeholder(ctx.t_args("orchestrator.historyPanel.search", [("project", name)]))
  .render(ctx);
  let items = items(chats, (current, project), &search.get());
  let list: Element = if items.is_empty() {
    let words = if chats.is_empty() {
      "orchestrator.historyPanel.empty"
    } else {
      "orchestrator.historyPanel.noMatch"
    };
    Column::new()
      .id("orchestrator-history-empty")
      .flex(1.0)
      .padding(SpacingSize::Lg)
      .child(Text::new(&ctx.t(words)).variant(TypographyStyle::Caption))
      .into()
  } else {
    VirtualizedList::new_shared(ctx, Arc::new(items))
      .list_key("orchestrator-history-list")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .mount_keyed::<ItemView, _, _, _>(Item::key, Clone::clone)
  };
  Column::new()
    .id("orchestrator-history-panel")
    .width(Dimension::Px(WIDTH))
    .height(Dimension::Pct(100.0))
    .flex_shrink(0.0)
    .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(head)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::new()
            .left(SpacingSize::Lg)
            .right(SpacingSize::Lg)
            .bottom(SpacingSize::Sm),
        )
        .child(field),
    )
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
        .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(list),
    )
    .child(foot(ctx, name))
    .into()
}

/// The panel's head: its title, the project's name, *Esc* and *Close*.
fn head(ctx: &mut Ctx, open: &Signal<bool>, name: &str) -> Row {
  let close = open.clone();
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Section.role())
        .bottom(ExtraSpacing::Loose.role())
        .left(SpacingSize::Lg)
        .right(SpacingSize::Lg),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("orchestrator.historyPanel.title")).variant(OrchestratorText::PanelTitle.role()))
    .child(Text::new(name).variant(ExtraTypography::Note.role()).nowrap())
    .child(Spacer::new().flex(1.0))
    .child(key_cap("orchestrator-history-esc", &ctx.t("orchestrator.keys.esc")))
    .child(
      IconButton::new(
        "orchestrator-history-close",
        Icon::X,
        ctx.t("orchestrator.historyPanel.close"),
      )
      .inspector()
      .on_click(move || close.set(false))
      .render(ctx),
    )
}

/// The note at the panel's foot.
fn foot(ctx: &Ctx, name: &str) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Md)
        .bottom(ExtraSpacing::Section.role())
        .left(SpacingSize::Lg)
        .right(SpacingSize::Lg),
    )
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&ctx.t_args("orchestrator.historyPanel.note", [("project", name)]))
        .id("orchestrator-history-note")
        .variant(TypographyStyle::Caption)
        .width(Dimension::Pct(100.0)),
    )
}

/// The list's rows: `chats` whose title has every word of `search`, under the day each last changed.
fn items(chats: &[ChatListing], (current, project): (Option<ChatId>, ProjectId), search: &str) -> Vec<Item> {
  let words: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
  let today = Timestamp::now().local_day_start();
  let mut items = Vec::new();
  let mut last_day = None;
  for listing in chats {
    let title = listing.chat.title().to_lowercase();
    if !words.iter().all(|word| title.contains(word.as_str())) {
      continue;
    }
    let day = Day::of(listing.last, today);
    if last_day != Some(day) {
      items.push(Item::Day(day));
      last_day = Some(day);
    }
    items.push(Item::Chat {
      listing: listing.clone(),
      current: current == Some(listing.chat.id()),
      today: day == Day::Today,
      project,
    });
  }
  items
}

struct ItemView;

impl Component for ItemView {
  type Props = Item;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    match ctx.props::<Item>().clone() {
      Item::Day(day) => Row::new()
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::new()
            .left(SpacingSize::Xs)
            .right(SpacingSize::Xs)
            .top(ExtraSpacing::Loose.role())
            .bottom(SpacingSize::Xs),
        )
        .child(Text::new(&ctx.t(day.key())).variant(OrchestratorText::Eyebrow.role()))
        .into(),
      Item::Chat {
        listing,
        current,
        today,
        project,
      } => chat_row(ctx, &listing, (current, today), project),
    }
  }
}

/// A conversation's row; pressing it opens the conversation.
fn chat_row(ctx: &mut Ctx, listing: &ChatListing, (current, today): (bool, bool), project: ProjectId) -> Element {
  let id = listing.chat.id();
  let when: Arc<str> = if today {
    display::clock(listing.last).into()
  } else {
    super::date(ctx, listing.last)
  };
  let count = listing.messages.to_string();
  let meta = match (current, listing.messages) {
    (true, 1) => ctx.t("orchestrator.historyPanel.current.one"),
    (true, _) => ctx.t_args("orchestrator.historyPanel.current.other", [("count", count.as_str())]),
    (false, 1) => ctx.t("orchestrator.historyPanel.messages.one"),
    (false, _) => ctx.t_args("orchestrator.historyPanel.messages.other", [("count", count.as_str())]),
  };
  let navigation = Navigation::of(ctx);
  let route = Route::Project(project, ProjectPage::Orchestrator(OrchestratorChat::Chat(id)));
  let row = pressable(move || navigation.go(route.clone()))
    .id(format!("orchestrator-history-{id}"))
    .describe("current", current.to_string())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()));
  let row = if current {
    row.background(ExtraColor::Hover.role())
  } else {
    row
  };
  row
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Fine.role())
        .child(
          Row::new()
            .width(Dimension::Pct(100.0))
            .spacing(SpacingSize::Sm)
            .align_items(Alignment::Center)
            .child(
              Text::new(listing.chat.title())
                .variant(OrchestratorText::Emphasis.role())
                .flex_shrink(1.0)
                .nowrap(),
            )
            .child(Spacer::new().flex(1.0))
            .child(Text::new(&when).variant(TypographyStyle::Caption).nowrap()),
        )
        .child(Text::new(&meta).variant(ExtraTypography::Note.role()).nowrap()),
    )
    .into()
}
