//! The Agents hub (`Agents · redesign` `ZsU3S`, ORC-107): Profiles, Instructions, Skills, Tools and Memory as one
//! page. Its head shows *Agents* with the open tab's save state and actions at the right (`Header` `LM3Mk`), then the
//! five tabs with their counts (`Tabs` `Cc3JP`), then the tab's body (`Body` `juZl6`). Each tab, and each item and
//! sub-tab it opens, keeps its own route, so links, history and Jump to land on it; the sidebar's entries open the
//! hub at their tab. Each tab's screen draws the hub around its body with [`page`], so its actions and save state
//! stay its own.

pub(crate) mod discard;
pub(crate) mod draft_bar;
pub(crate) mod import_preview;
pub(crate) mod import_result;
pub(crate) mod import_rows;
pub(crate) mod page_head;
pub(crate) mod parts;
pub(crate) mod pills;
pub(crate) mod question;
pub(crate) mod removal;
mod tabs;

use lurq::{
  app::{ctx::Ctx, theme::TypographyStyle},
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::messages::Message;

pub(crate) use self::tabs::HubTab;
use crate::{
  project::ShownProject,
  routes::{InstructionsView, ProjectPage, Route, SkillsFilter},
  theme::HubSpacing,
  ui::failure::{Failure, is_transient},
};

/// The pane's width (`Pane` `V7baZg`, `rA12N`); narrower when the window leaves less.
pub(crate) const PANE_WIDTH: f32 = 720.0;
/// The side panel's width beside an opened item (`Side` `y8gUZ`).
pub(crate) const SIDE_WIDTH: f32 = 384.0;

impl HubTab {
  /// The route of the tab's list.
  pub(crate) fn route(self, project: ProjectId) -> Route {
    Route::Project(
      project,
      match self {
        Self::Profiles => ProjectPage::Agents,
        Self::Instructions => ProjectPage::Instructions(InstructionsView::default()),
        Self::Skills => ProjectPage::Skills(SkillsFilter::default()),
        Self::Tools => ProjectPage::Tools,
        Self::Memory => ProjectPage::Memory(None),
      },
    )
  }

  /// The tab's bit in `ProjectFeed::hub_unread`.
  pub(crate) const fn bit(self) -> u8 {
    match self {
      Self::Profiles => 1,
      Self::Instructions => 2,
      Self::Skills => 4,
      Self::Tools => 8,
      Self::Memory => 16,
    }
  }
}

/// How the read of `tab`'s own data stands, as `failed` says (`LastFailure::follow`): while it failed the tab shows no
/// count, and its content is the failure (`Failure states · redesign` `SsC16`) saying `what` couldn't be read, with
/// Try again running `retry` and Open log opening the project's runner log.
pub(crate) fn read_failure(
  ctx: &Ctx,
  project: &ShownProject,
  (tab, id, what): (HubTab, &str, &str),
  failed: Option<(Message, bool)>,
  retry: impl Fn() + Send + Sync + 'static,
) -> Option<Failure> {
  project.feed.mark_hub_unread(
    tab.bit(),
    failed.as_ref().is_some_and(|(error, _)| !is_transient(error)),
  );
  let (error, retrying) = failed?;
  Some(
    Failure::new(id, what, error)
      .retry(retrying, retry)
      .log(project.open.repo_root.clone())
      .still_works(ctx.t("hub.failure.stillWorks")),
  )
}

/// What the head shows at its right for the open tab: its save state, then its actions.
#[derive(Default)]
pub(crate) struct Head {
  pub saved: Option<Element>,
  pub actions: Vec<Element>,
}

impl Head {
  pub(crate) fn actions(actions: impl IntoIterator<Item = Element>) -> Self {
    Self {
      saved: None,
      actions: actions.into_iter().collect(),
    }
  }

  #[must_use]
  pub(crate) fn saved(mut self, saved: Option<Element>) -> Self {
    self.saved = saved;
    self
  }
}

/// The hub on `tab` of `project`, with `head` at the head's right and `body` under the tabs; the body takes the rest
/// of the page and lays itself out ([`list_body`], [`item_body`]).
pub(crate) fn page(
  ctx: &mut Ctx,
  project: &ShownProject,
  (tab, head): (HubTab, Head),
  body: impl Into<Element>,
) -> Element {
  let title = Row::new()
    .id("agents-hub-head")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(HubSpacing::HeadTop.role())
        .bottom(HubSpacing::HeadBottom.role())
        .left(HubSpacing::Side.role())
        .right(HubSpacing::Side.role()),
    )
    .spacing(lurq::app::theme::SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("hub.title"))
        .id("agents-hub-title")
        .variant(TypographyStyle::Heading)
        .flex(1.0),
    )
    .with_children(head.saved)
    .with_children(head.actions);
  let tabs = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(HubSpacing::Side.role()))
    .child(tabs::strip(ctx, project, tab));
  Column::new()
    .id("agents-hub")
    .describe("tab", tab.name())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(title)
    .child(tabs)
    .child(
      Column::new()
        .id("agents-hub-body")
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .child(body),
    )
    .into()
}

/// A tab's list: `pane` from the body's top and left, the pane's width at most, scrolling under the tabs.
pub(crate) fn list_body(id: &str, pane: impl Into<Element>) -> Element {
  item_body(id, pane, None)
}

/// [`list_body`] with a pane `width` wide at most, e.g. Memory's 760 (`Pane` `Jy4nw` in 05).
pub(crate) fn list_body_wide(id: &str, pane: impl Into<Element>, width: f32) -> Element {
  body(id, (pane.into(), width), None)
}

/// An opened item: `pane` with `side` beside it, both scrolling as one page under the tabs (the design's tall
/// windows, 01c, 02, 04b, are whole pages that scroll).
pub(crate) fn item_body(id: &str, pane: impl Into<Element>, side: Option<Element>) -> Element {
  body(id, (pane.into(), PANE_WIDTH), side.map(|side| (side, SIDE_WIDTH)))
}

/// [`item_body`] with a pane and a side of their own widths, e.g. a memory record's 680 and 424 (05c).
pub(crate) fn page_body(id: &str, pane: (Element, f32), side: (Element, f32)) -> Element {
  body(id, pane, Some(side))
}

fn body(id: &str, (pane, width): (Element, f32), side: Option<(Element, f32)>) -> Element {
  let pane = Column::new()
    .id(format!("{id}-pane"))
    .flex_shrink(1.0)
    .width(Dimension::Px(width))
    .min_width(Dimension::Px(0.0))
    .child(pane);
  let row = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(HubSpacing::BodyTop.role())
        .left(HubSpacing::Side.role())
        .right(HubSpacing::Side.role())
        .bottom(HubSpacing::Sections.role()),
    )
    .spacing(HubSpacing::Beside.role())
    .align_items(Alignment::Start)
    .child(pane)
    .with_children(side.map(|(side, side_width)| {
      Column::new()
        .id(format!("{id}-side"))
        .width(Dimension::Px(side_width))
        .flex_shrink(0.0)
        .spacing(HubSpacing::SideSections.role())
        .child(side)
    }));
  ScrollVertical::new(row)
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .into()
}
