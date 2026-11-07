//! The hub's head and filter (`Header` `NRqwv`, `Filter` `NXs2z` in `t4mtQL`): *Agents* with the team's address at its
//! right, then the kinds as a segmented control with their counts and a line saying what the filter holds.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::{ProjectId, ResourceId};
use orchester_workspace::sharing::team::TeamConfiguration;

use super::model::Listing;
use crate::{
  navigation::Navigation,
  routes::{HubFilter, HubView, ProjectPage, Route},
  theme::{ExtraSpacing, ExtraTypography, HubText, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// The head's height (`Header` `NRqwv`).
const HEAD_HEIGHT: f32 = 32.0;

/// *Agents*, and at its right the team the project is shared with and, for an admin, New.
pub(super) fn head(ctx: &Ctx, team: &TeamConfiguration, new: Option<Element>) -> Row {
  let mut at_right = Row::new()
    .id("team-hub-team")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Users, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("team_hub.team"))
        .variant(HubText::Hint.role())
        .nowrap(),
    );
  if let Some((_, host)) = &team.server {
    at_right = at_right.child(
      Text::new(host)
        .id("team-hub-team-host")
        .variant(ExtraTypography::PropertyMono.role())
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    );
  }
  Row::new()
    .id("team-hub-head")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("team_hub.title"))
        .id("team-hub-title")
        .variant(TypographyStyle::Heading)
        .flex(1.0),
    )
    .child(at_right)
    .with_children(new)
}

/// What the filter shows besides the counts.
pub(super) struct FilterShown {
  pub project: ProjectId,
  pub filter: HubFilter,
  /// The resource the panel shows, which a filter that holds it keeps.
  pub selected: Option<ResourceId>,
  /// The list is read again: what it counts is the last answer.
  pub reading: bool,
}

/// The filter: a segment per kind with its count, and what the open one holds. `listing` is `None` before there is an
/// answer to count, and while the last read failed: a segment then shows no count rather than a stale one.
pub(super) fn filter(ctx: &Ctx, listing: Option<&Listing>, shown: &FilterShown, navigation: &Navigation) -> Row {
  let segments = HubFilter::ALL.into_iter().map(|choice| {
    let navigation = navigation.clone();
    let item = shown
      .selected
      .filter(|item| listing.is_some_and(|listing| listing.holds(choice, *item)));
    let route = Route::Project(shown.project, ProjectPage::hub(HubView::new(choice, item)));
    let label = ctx.t(label_key(choice));
    let face = match listing {
      Some(listing) => SegmentFace::Counted(label, listing.count(choice).to_string().into()),
      None => SegmentFace::Text(label),
    };
    Segment::new(
      format!("team-hub-filter-{}", choice.name()),
      face,
      choice == shown.filter,
      move || navigation.go(route.clone()),
    )
  });
  Row::new()
    .id("team-hub-filter")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(segmented("team-hub-filter-tabs", segments, Focus::Pointer))
    .child(Row::new().flex(1.0))
    .child(if shown.reading {
      Element::from(reading_mark(ctx))
    } else {
      Element::from(
        Text::new(&ctx.t(hint_key(shown.filter)))
          .id("team-hub-filter-hint")
          .variant(HubText::Hint.role())
          .nowrap(),
      )
    })
}

/// *Reading…* in the hint's place while the list is read again.
fn reading_mark(ctx: &Ctx) -> Row {
  Row::new()
    .id("team-hub-reading")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("state", "loading")
    .child(icon(Icon::LoaderCircle, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("team_hub.rereading"))
        .id("team-hub-reading-label")
        .variant(HubText::Hint.role())
        .nowrap(),
    )
}

const fn label_key(filter: HubFilter) -> &'static str {
  match filter {
    HubFilter::All => "team_hub.filter.all",
    HubFilter::Profiles => "team_hub.filter.profiles",
    HubFilter::McpServers => "team_hub.filter.mcpServers",
    HubFilter::Instructions => "team_hub.filter.instructions",
    HubFilter::Skills => "team_hub.filter.skills",
    HubFilter::Tools => "team_hub.filter.tools",
  }
}

const fn hint_key(filter: HubFilter) -> &'static str {
  match filter {
    HubFilter::All => "team_hub.hint.all",
    HubFilter::Profiles => "team_hub.hint.profiles",
    HubFilter::McpServers => "team_hub.hint.mcpServers",
    HubFilter::Instructions => "team_hub.hint.instructions",
    HubFilter::Skills => "team_hub.hint.skills",
    HubFilter::Tools => "team_hub.hint.tools",
  }
}
