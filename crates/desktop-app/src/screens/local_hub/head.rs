//! The hub's head and filter in a local project (`Header`, `Filter` in L2): *Agents* with, at its right, that the
//! project is only on this computer, *Share with a team…* (which opens the Sharing tab of Project settings, the
//! existing Share flow), *Memory* and *Import* (the pages the old hub's tabs and head led to, which the design does not
//! draw in this head) and *New* of the kind the filter holds; then the kinds and *Archived* as a segmented control
//! with their counts and a line saying what the filter holds.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ProjectId;
use orchester_workspace::local_archive::Archivable;

use super::{
  leave::Leave,
  model::{Listing, view_of},
  new_menu::new_menu,
};
use crate::{
  routes::{HubFilter, HubView, ImportTab, InstructionsView, ProjectPage, ProjectSettingsTab, Route},
  theme::{ExtraSpacing, HubSpacing, HubText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// The head's height.
const HEAD_HEIGHT: f32 = 32.0;

/// *Agents*, and at its right that the project is local, *Share with a team…* and the New of the filter's kind.
pub(super) fn head(ctx: &mut Ctx, project: ProjectId, filter: HubFilter, leave: &Leave) -> Row {
  let share = ActionButton::new("local-hub-share", ButtonKind::Secondary, ctx.t("local_hub.share"))
    .icon(Icon::Users)
    .press(leave.press(Route::Project(
      project,
      ProjectPage::Settings(ProjectSettingsTab::Sharing),
    )));
  let memory = ActionButton::new("local-hub-memory", ButtonKind::Ghost, ctx.t("local_hub.memory"))
    .icon(Icon::Brain)
    .press(leave.press(Route::Project(project, ProjectPage::Memory(None))));
  let import = import_of(filter).map(|page| {
    ActionButton::new("local-hub-import", ButtonKind::Ghost, ctx.t("local_hub.import"))
      .icon(Icon::Download)
      .press(leave.press(Route::Project(project, page)))
  });
  // A filter of one kind makes one of it; All and Archived hold no one kind, so *New* offers each.
  let new: Element = match new_of(filter) {
    Some((page, words)) => ActionButton::new("local-hub-new", ButtonKind::Primary, ctx.t(words))
      .icon(Icon::Plus)
      .press(leave.press(Route::Project(project, page)))
      .into(),
    None => new_menu(ctx, project, leave),
  };
  Row::new()
    .id("local-hub-head")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("local_hub.title"))
        .id("local-hub-title")
        .variant(TypographyStyle::Heading)
        .flex(1.0),
    )
    .child(
      Row::new()
        .spacing(HubSpacing::NewGap.role())
        .align_items(Alignment::Center)
        .child(
          Row::new()
            .id("local-hub-local")
            .spacing(ExtraSpacing::Snug.role())
            .align_items(Alignment::Center)
            .child(icon(Icon::Laptop, IconSize::Base, PaletteColor::TextMuted))
            .child(
              Text::new(&ctx.t("local_hub.local"))
                .id("local-hub-local-label")
                .variant(HubText::Hint.role())
                .nowrap(),
            ),
        )
        .child(share)
        .child(memory)
        .with_children(import)
        .child(new),
    )
}

/// The page that imports what the kind `filter` holds from the repository; none under Archived and MCP servers, which
/// have nothing to import.
fn import_of(filter: HubFilter) -> Option<ProjectPage> {
  Some(match filter {
    HubFilter::All => ProjectPage::Import(ImportTab::All),
    HubFilter::Profiles => ProjectPage::Import(ImportTab::Agents),
    HubFilter::Instructions => ProjectPage::Instructions(InstructionsView::Import),
    HubFilter::Skills => ProjectPage::ImportSkills,
    HubFilter::Tools => ProjectPage::Import(ImportTab::Tools),
    HubFilter::McpServers | HubFilter::Archived => return None,
  })
}

/// The hub on the form that makes a new one of the kind `filter` holds, and the words of its button; none under All and
/// Archived.
pub(super) fn new_of(filter: HubFilter) -> Option<(ProjectPage, &'static str)> {
  let words = match filter {
    HubFilter::Profiles => "local_hub.new.profiles",
    HubFilter::McpServers => "local_hub.new.mcpServers",
    HubFilter::Instructions => "local_hub.new.instructions",
    HubFilter::Skills => "local_hub.new.skills",
    HubFilter::Tools => "local_hub.new.tools",
    HubFilter::All | HubFilter::Archived => return None,
  };
  Some((ProjectPage::hub(HubView::adding(filter)), words))
}

/// What the filter shows besides the counts.
pub(super) struct FilterShown {
  pub project: ProjectId,
  pub filter: HubFilter,
  /// The entry the panel shows, which a filter that holds it keeps.
  pub selected: Option<Archivable>,
  /// The list is read again: what it counts is the last answer.
  pub reading: bool,
}

/// The filter: a segment per kind and *Archived* with its count, and what the open one holds.
pub(super) fn filter(ctx: &Ctx, listing: Option<&Listing>, shown: &FilterShown, leave: &Leave) -> Row {
  let segments = HubFilter::LOCAL.into_iter().map(|choice| {
    let leave = leave.clone();
    let item = shown
      .selected
      .filter(|item| listing.is_some_and(|listing| listing.holds(choice, *item)));
    let route = Route::Project(shown.project, ProjectPage::hub(view_of(choice, item)));
    let label = ctx.t(label_key(choice));
    let face = match listing {
      Some(listing) => SegmentFace::Counted(label, listing.count(choice).to_string().into()),
      None => SegmentFace::Text(label),
    };
    Segment::new(
      format!("local-hub-filter-{}", choice.name()),
      face,
      choice == shown.filter,
      move || leave.go(route.clone()),
    )
  });
  Row::new()
    .id("local-hub-filter")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(segmented("local-hub-filter-tabs", segments, Focus::Pointer))
    .child(Row::new().flex(1.0))
    .child(if shown.reading {
      Element::from(reading_mark(ctx))
    } else {
      Element::from(
        Text::new(&ctx.t(hint_key(shown.filter)))
          .id("local-hub-filter-hint")
          .variant(HubText::Hint.role())
          .nowrap(),
      )
    })
}

/// *Reading…* in the hint's place while the list is read again.
fn reading_mark(ctx: &Ctx) -> Row {
  Row::new()
    .id("local-hub-reading")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("state", "loading")
    .child(icon(Icon::LoaderCircle, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("local_hub.rereading"))
        .id("local-hub-reading-label")
        .variant(HubText::Hint.role())
        .nowrap(),
    )
}

const fn label_key(filter: HubFilter) -> &'static str {
  match filter {
    HubFilter::All => "local_hub.filter.all",
    HubFilter::Profiles => "local_hub.filter.profiles",
    HubFilter::McpServers => "local_hub.filter.mcpServers",
    HubFilter::Instructions => "local_hub.filter.instructions",
    HubFilter::Skills => "local_hub.filter.skills",
    HubFilter::Tools => "local_hub.filter.tools",
    HubFilter::Archived => "local_hub.filter.archived",
  }
}

const fn hint_key(filter: HubFilter) -> &'static str {
  match filter {
    HubFilter::All => "local_hub.hint.all",
    HubFilter::Profiles => "local_hub.hint.profiles",
    HubFilter::McpServers => "local_hub.hint.mcpServers",
    HubFilter::Instructions => "local_hub.hint.instructions",
    HubFilter::Skills => "local_hub.hint.skills",
    HubFilter::Tools => "local_hub.hint.tools",
    HubFilter::Archived => "local_hub.hint.archived",
  }
}
