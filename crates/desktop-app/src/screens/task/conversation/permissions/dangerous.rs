//! The run's permissions menu while its project is in Dangerous auto-approve (`docs/design/approval-modes.md`): the
//! menu's frame and heading, then in place of the ways of answering one line that the project approves every request,
//! and a link to Project settings › Runs, where the Approve mode is changed. The run's own choices are kept and come
//! back once the project leaves Dangerous auto-approve.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  ChipProps,
  menu::{WIDTH, heading_row},
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, ProjectSettingsTab, Route},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// The menu while the project approves every request (Dangerous auto-approve): no way of answering to choose, as none
/// would change anything; one line that says so and a link to where it is changed, Project settings › Runs.
pub(super) fn panel(ctx: &mut Ctx, props: &ChipProps, heading: &str) -> Element {
  let open = Navigation::of(ctx).to(Route::Project(
    props.project.0,
    ProjectPage::Settings(ProjectSettingsTab::Runs),
  ));
  let note = ctx.t_args(
    "conversation.permissions.dangerous.note",
    [("project", &*props.project.1)],
  );
  Column::new()
    .id("conversation-permissions-menu")
    .width(Dimension::Px(WIDTH))
    .padding(ExtraSpacing::Snug.role())
    .spacing(ExtraSpacing::Tight.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(RadiusSize::Lg)
    .box_shadow(ShadowStyle::Lg)
    .child(heading_row(
      "conversation-permissions-heading",
      Some(Icon::Shield),
      heading,
    ))
    .child(
      Row::new()
        .id("conversation-permissions-dangerous")
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Start)
        .child(icon(Icon::TriangleAlert, IconSize::Md, PaletteColor::Danger))
        .child(
          Text::new(&note)
            .id("conversation-permissions-dangerous-note")
            .variant(ExtraTypography::Note.role())
            .flex(1.0)
            .min_width(Dimension::Px(0.0)),
        ),
    )
    .child(
      Row::new()
        .id("conversation-permissions-foot")
        .width(Dimension::Pct(100.0))
        .padding(Padding::new().top(SpacingSize::Sm).bottom(SpacingSize::Xs))
        .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(
          ActionButton::new(
            "conversation-permissions-settings-link",
            ButtonKind::Ghost,
            ctx.t("conversation.permissions.dangerous.settings"),
          )
          .icon(Icon::ExternalLink)
          .opens(open),
        ),
    )
    .into()
}
