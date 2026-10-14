//! What the empty project's card lists under its actions (`Ready` `VY4QP` in `a5CcQx`): over a `border`
//! line and 12 under it, *Ready to run* in 11/500 `text_muted`, then 6 apart a line each for the agents,
//! the pipeline and the connection: a 13 `circle-check` in `success` (a `circle-alert` in `warning` when
//! not ready), the line's name in 12/500 on 90, and what was found in 12 `text_secondary`, 8 apart. What
//! counts as ready is `orchester_workspace::tasks::ready_to_run`; it is read off the UI thread, again
//! whenever the project's agents, pipelines or connections change.

use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  background, messages,
  tasks::ready_to_run::{CheckKind, ProjectReadiness, ReadyCheck},
};

use crate::{
  project::ShownProject,
  theme::{ExtraSpacing, IconSize, OverviewText},
  ui::{
    icon::{Icon, icon},
    message::message_text,
  },
};

/// A line's name (`Agents` `PFyfe`).
const LABEL_WIDTH: f32 = 90.0;

/// The list for `project`, read again when a screen or automation saved its agents, pipelines or
/// connections.
pub(super) fn ready(ctx: &mut Ctx, project: &ShownProject) -> Element {
  let outside = project.feed.outside.get();
  let props = ReadyProps {
    repo_root: project.open.repo_root.clone(),
    revision: project.feed.revision.get() + outside.profiles + outside.pipelines + outside.connections,
  };
  ctx.mount_keyed::<ReadyToRun>("overview-ready", props)
}

#[derive(Clone, PartialEq)]
struct ReadyProps {
  repo_root: PathBuf,
  revision: u64,
}

impl DevtoolsInspectable for ReadyProps {}

struct ReadyToRun;

impl Component for ReadyToRun {
  type Props = ReadyProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let ReadyProps { repo_root, revision } = ctx.props::<ReadyProps>().clone();
    let read = ctx.future((repo_root, revision), |(repo_root, _)| async move {
      background::run(move || ProjectReadiness::load_in(&repo_root))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot read whether the project is ready to run");
          messages::store_error(&error)
        })
    });
    let state = read.state().get();
    let list = Column::new()
      .id("overview-ready")
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(SpacingSize::Md))
      .spacing(ExtraSpacing::Snug.role())
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(Text::new(&ctx.t("overview.ready.title")).variant(OverviewText::ReadyHeading.role()));
    // A failed refresh keeps the lines read before, with the error under them so they don't pass for current.
    let list = match state.data {
      Some(readiness) => {
        let separator = ctx.t("overview.listSeparator");
        let lines: Vec<Element> = readiness
          .checks(&separator)
          .iter()
          .map(|check| line(ctx, check).into())
          .collect();
        list.with_children(lines)
      }
      None if state.error.is_none() => list.child(
        Text::new(&ctx.t("overview.ready.reading"))
          .id("overview-ready-reading")
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted),
      ),
      None => list,
    };
    match state.error {
      Some(error) => list.child(
        Text::new(&message_text(ctx, &error))
          .id("overview-ready-error")
          .variant(TypographyStyle::Description)
          .color(PaletteColor::Danger),
      ),
      None => list,
    }
  }
}

/// A line, under `overview-ready-<kind>`; its icon's id says whether it is `-ready` or `-missing`.
fn line(ctx: &Ctx, check: &ReadyCheck) -> Row {
  let id = format!("overview-ready-{}", name(check.kind));
  let (glyph, tone, state) = if check.ready {
    (Icon::CircleCheck, PaletteColor::Success, "ready")
  } else {
    (Icon::CircleAlert, PaletteColor::Warning, "missing")
  };
  Row::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .describe("ready", state)
    .child(icon(glyph, IconSize::Base, tone).id(format!("{id}-{state}")))
    .child(
      Text::new(&message_text(ctx, &check.kind.label()))
        .variant(OverviewText::ReadyLabel.role())
        .width(Dimension::Px(LABEL_WIDTH))
        .flex_shrink(0.0),
    )
    .child(
      Text::new(&message_text(ctx, &check.detail))
        .id(format!("{id}-detail"))
        .variant(OverviewText::ReadyDetail.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

const fn name(kind: CheckKind) -> &'static str {
  match kind {
    CheckKind::Agents => "agents",
    CheckKind::Pipeline => "pipeline",
    CheckKind::Connection => "connection",
  }
}
