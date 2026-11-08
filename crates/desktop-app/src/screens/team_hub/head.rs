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

use super::{member, model::Listing};
use crate::{
  navigation::Navigation,
  routes::{HubFilter, HubView, ProjectPage, Route},
  theme::{ExtraSpacing, ExtraTypography, HubSpacing, HubText, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// The head's height (`Header` `NRqwv`).
const HEAD_HEIGHT: f32 = 32.0;

/// *Agents*, and at its right the team the project is shared with and, for an admin, New. `unreached`: the team server
/// did not answer the last read, which the head says in place of *Team* (`Publish while offline` A16 `x3ywL`). `saved`:
/// the panel's resource has a draft kept on this computer, which the head says before *Team* (`✓ Draft saved`, A13).
pub(super) fn head(ctx: &Ctx, team: &TeamConfiguration, (new, unreached, saved): (Option<Element>, bool, bool)) -> Row {
  let (glyph, color, words) = if unreached {
    (Icon::CloudOff, PaletteColor::Warning, "team_hub.unreached")
  } else {
    (Icon::Users, PaletteColor::TextMuted, "team_hub.team")
  };
  let mut at_right = Row::new()
    .id("team-hub-team")
    .describe("reached", (!unreached).to_string())
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color.clone()))
    .child(
      Text::new(&ctx.t(words))
        .id("team-hub-team-label")
        .variant(HubText::Hint.role())
        .color(if unreached { color } else { PaletteColor::TextMuted })
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
  if !team.may_change {
    at_right = at_right.child(member::role(ctx));
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
    .child(
      Row::new()
        .spacing(HubSpacing::NewGap.role())
        .align_items(Alignment::Center)
        .with_children(saved.then(|| draft_saved(ctx)))
        .child(at_right)
        .with_children(new),
    )
}

/// *✓ Draft saved* (`Draft saved` in `Header` of A13 `WMtxm`).
fn draft_saved(ctx: &Ctx) -> Row {
  Row::new()
    .id("team-hub-draft-saved")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Check, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("team_hub.draftSaved"))
        .id("team-hub-draft-saved-label")
        .variant(HubText::Hint.role())
        .nowrap(),
    )
}

/// What the filter shows besides the counts.
pub(super) struct FilterShown {
  pub project: ProjectId,
  pub filter: HubFilter,
  /// The resource the panel shows, which a filter that holds it keeps.
  pub selected: Option<ResourceId>,
  /// The list is read again: what it counts is the last answer.
  pub reading: bool,
  /// The person may change the team's resources: an admin of the team; a member reads another hint.
  pub may_change: bool,
  /// The resource the panel shows has a draft kept on this computer.
  pub draft_saved: bool,
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
        Text::new(&ctx.t(hint_key(shown.filter, shown.may_change)))
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

/// The filter's hint for `filter`: an admin's, or for a member what the kind is and that they fill in only what is
/// local.
const fn hint_key(filter: HubFilter, may_change: bool) -> &'static str {
  match (may_change, filter) {
    (true, HubFilter::All) => "team_hub.hint.all",
    (true, HubFilter::Profiles) => "team_hub.hint.profiles",
    (true, HubFilter::McpServers) => "team_hub.hint.mcpServers",
    (true, HubFilter::Instructions) => "team_hub.hint.instructions",
    (true, HubFilter::Skills) => "team_hub.hint.skills",
    (true, HubFilter::Tools) => "team_hub.hint.tools",
    (false, HubFilter::All) => "team_hub.member.hint.all",
    (false, HubFilter::Profiles) => "team_hub.member.hint.profiles",
    (false, HubFilter::McpServers) => "team_hub.member.hint.mcpServers",
    (false, HubFilter::Instructions) => "team_hub.member.hint.instructions",
    (false, HubFilter::Skills) => "team_hub.member.hint.skills",
    (false, HubFilter::Tools) => "team_hub.member.hint.tools",
  }
}
