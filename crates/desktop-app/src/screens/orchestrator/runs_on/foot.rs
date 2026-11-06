//! What the runs-on picker says under its columns and at its foot (`Data note` `hv0Vz`, `Foot` `y1rd84` in `05b · On
//! DeepSeek` `AZp3I`; `Foot` `SwrjI` in `v6mWp8`). The notes, 8 from the sides and the foot and 6 apart: why the last
//! choice was refused, why a connection the keys tried to choose cannot be chosen, on an API provider where prompts and
//! code go (11.5 `text_secondary` beside a 12 `send` in `warning`, on `warning_muted`, radius 6, 8 by 10), and that
//! choosing replaces a draft saved in the Orchestrator's settings. The foot, 8 by 12 and 32 high with the 1 `border`
//! line at its top, says that another account starts a new session (11.5/1.4 `text_muted`) and offers *Settings ↗*
//! (12 `text_secondary`); with no connection at all no line is over it (`k95WLp`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Stack, Text},
  layout::Alignment,
  node::{CursorIcon, Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{messages::Message, orchestrator::runs_on::RunsOn};

use super::{
  RunsOnProps,
  rail::{Entry, Sources, tip},
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, OrchestratorText},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    pressable::opening,
    tinted_note::tinted_note,
  },
};

/// The foot's line's height (the top stroke of `Foot` `SwrjI`).
const RULE_HEIGHT: f32 = 1.0;

/// The notes under the columns: why the last choice was `refused`, why the connection the keys tried to choose is
/// `unavailable`, where an API provider shown sends prompts, and that choosing replaces a draft; nothing when there is
/// none.
pub(super) fn notes(
  ctx: &Ctx,
  (sources, viewing): (&Sources<'_>, Option<&Entry>),
  (refused, unavailable): (Option<&Message>, Option<&Entry>),
) -> Option<Element> {
  let refused = refused.map(|error| {
    tinted_note(
      "orchestrator-runs-on-error",
      (Icon::CircleAlert, PaletteColor::Danger),
      &ctx.t_args(
        "orchestrator.settings.saveFailed",
        [("reason", &*message_text(ctx, error))],
      ),
    )
    .into()
  });
  let unavailable = unavailable.map(|entry| {
    tinted_note(
      "orchestrator-runs-on-unavailable",
      (Icon::CircleAlert, PaletteColor::TextSecondary),
      &tip(ctx, entry),
    )
    .into()
  });
  let sent_to = viewing.and_then(|entry| match &entry.runs_on {
    RunsOn::ApiProvider { base_url, .. } => Some(sent_to(ctx, base_url.host())),
    RunsOn::SignIn(_) | RunsOn::Native => None,
  });
  let over_draft = (viewing.is_some() && sources.setup.draft.is_some()).then(|| {
    tinted_note(
      "orchestrator-runs-on-over-draft",
      (Icon::Info, PaletteColor::TextMuted),
      &ctx.t("orchestrator.runsOn.overDraft"),
    )
    .into()
  });
  let notes: Vec<Element> = [refused, unavailable, sent_to, over_draft]
    .into_iter()
    .flatten()
    .collect();
  (!notes.is_empty()).then(|| {
    Column::new()
      .width(Dimension::Pct(100.0))
      .flex_shrink(0.0)
      .padding(
        Padding::new()
          .left(SpacingSize::Sm)
          .right(SpacingSize::Sm)
          .bottom(SpacingSize::Sm),
      )
      .spacing(ExtraSpacing::Snug.role())
      .with_children(notes)
      .into()
  })
}

/// Where an API provider at `host` sends prompts, repository content and command output.
fn sent_to(ctx: &Ctx, host: &str) -> Element {
  Row::new()
    .id("orchestrator-runs-on-sent-to")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .background(PaletteColor::WarningMuted)
    .rounded(RadiusSize::Md)
    .child(icon(Icon::Send, IconSize::Sm, PaletteColor::Warning))
    .child(
      Text::new(&ctx.t_args("orchestrator.runsOn.sentTo", [("host", host)]))
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextSecondary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// The picker's foot, with the line at its top when `ruled`: a stack, as only a stack lays out what is placed at a
/// point of it, so the line is drawn inside the foot as the design's stroke is.
pub(super) fn foot(ctx: &mut Ctx, props: &RunsOnProps, ruled: bool) -> Element {
  let navigation = Navigation::of(ctx);
  let settings = opening(navigation.to(Route::Project(props.project, ProjectPage::OrchestratorSettings)))
    .id("orchestrator-settings")
    .describe("a11y_name", ctx.t("orchestrator.runsOn.settings"))
    .flex_shrink(0.0)
    .rounded(ExtraRadius::Link.role())
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .cursor(CursorIcon::Pointer)
    .child(
      Text::new(&ctx.t("orchestrator.runsOn.settings"))
        .variant(OrchestratorText::PickerLink.role())
        .nowrap(),
    );
  let words = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("orchestrator.runsOn.newSession"))
        .variant(ExtraTypography::Note.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(settings);
  let rule = ruled.then(|| {
    Row::new()
      .id("orchestrator-runs-on-rule")
      .absolute(0.0, 0.0, Dimension::Pct(100.0), Dimension::Px(RULE_HEIGHT))
      .pointer_events_none()
      .background(PaletteColor::Border)
  });
  Stack::new()
    .id("orchestrator-runs-on-foot")
    .width(Dimension::Pct(100.0))
    .flex_shrink(0.0)
    .child(words)
    .with_children(rule)
    .into()
}
