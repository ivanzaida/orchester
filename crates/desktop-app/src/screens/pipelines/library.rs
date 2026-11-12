//! Pipelines (`01 · Pipelines` `x1lmXb`, `01b · none yet` `l4Udx`): the title with *New pipeline*, the pipelines in the
//! groups Published and Draft only, how many there are and how versions work; with none yet, the two ways to start one.
use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::Column,
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  background, messages,
  pipelines::library::{LibraryEntry, library_in},
  sharing::team::TeamConfiguration,
  tasks::outside::Outside,
};

use super::{
  library_row::{RowProps, STATE_WIDTH},
  parts,
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route},
  screens::hub::team,
  services::Services,
  theme::{ExtraSpacing, GroupText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    row_group::{GroupRow, RowGroup, note},
    skeleton::{self, BoneRow, Skeleton, Words, fading},
    state::load_error,
  },
};

pub struct Library {
  retry: Signal<u64>,
}
impl Component for Library {
  type Props = ShownProject;
  fn create(ctx: &mut Ctx) -> Self {
    Self { retry: ctx.signal(0) }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let project = ctx.props::<ShownProject>().clone();
    let revision = self.retry.get() + project.feed.outside.get().of(Outside::Pipelines);
    let hold = Services::of(ctx).hold;
    let loaded = ctx
      .future((project.open.repo_root.clone(), revision), move |(root, _)| {
        let hold = hold.clone();
        async move {
          hold.released().await;
          background::run(move || library_in(&root).map_err(|e| messages::store_error(&e))).await?
        }
      })
      .state()
      .get();
    let id = project.open.project.id();
    let team = team::of(&project);
    // An admin's new team pipeline is created on the team server (D-043); a member makes none.
    let page = parts::page().child(header(ctx, id, team::may_change(team.as_ref())));
    let pending = loaded.is_pending();
    let body = match (loaded.data, loaded.error) {
      (_, Some(error)) if !pending => {
        let retry = self.retry.clone();
        load_error(
          ctx,
          "pipelines-error",
          (&ctx.t("pipelines.loadFailed"), &message_text(ctx, &error)),
          move || retry.update(|v| *v += 1),
        )
      }
      (Some(entries), _) if entries.is_empty() => empty(ctx, id).into(),
      (Some(entries), _) => populated(ctx, (&project, team.as_ref()), &entries).into(),
      (None, _) => reading(ctx),
    };
    page.child(parts::body("pipelines-body", body))
  }
}

/// The bone of the line that counts the pipelines.
const COUNT_BONE: f32 = 130.0;

/// The library while it is read (`01 Pipelines · loading` `SJC78`): the group headings real and their counts, each
/// row's name, steps line, state and age bones, lower rows fading; the versions note real.
fn reading(ctx: &mut Ctx) -> Element {
  let row = |ctx: &Ctx, id: &str, (name, steps, state): (f32, f32, f32)| -> Element {
    let end = Column::new()
      .width(Dimension::Px(STATE_WIDTH))
      .align_items(Alignment::End)
      .spacing(ExtraSpacing::Tight.role())
      .child(skeleton::status(ctx, state))
      .child(skeleton::line(ctx, GroupText::Hint.role(), state));
    BoneRow::new(id, Words::Bone(name))
      .hint(Words::Bone(steps))
      .end(end)
      .end(skeleton::chevron())
      .build(ctx)
      .into()
  };
  let published = vec![
    row(ctx, "pipelines-reading-1", (150.0, 250.0, 72.0)),
    row(ctx, "pipelines-reading-2", (50.0, 230.0, 84.0)),
  ];
  // A pipeline holds no draft (D-032): every pipeline is under Published.
  let groups = fading(vec![skeleton::group(
    ctx,
    "pipelines-reading-published",
    (Words::Real(ctx.t("pipelines.groups.published")), true),
    None,
    published,
  )]);
  let foot = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(skeleton::line(ctx, GroupText::Hint.role(), COUNT_BONE))
    .child(note("pipelines-versions-note", &ctx.t("pipelines.versionsExplanation")));
  let pane = parts::pane("pipelines-reading").with_children(groups).child(foot);
  Skeleton::new("pipelines-loading", ctx.t("pipelines.reading")).show(ctx, pane)
}

/// The title with its actions (`Header` `rDRoY`): 24 over it and 12 under it; a member of a shared project adds none
/// of the team's pipelines.
fn header(ctx: &mut Ctx, project: ProjectId, may_change: bool) -> Element {
  let nav = Navigation::of(ctx);
  let actions = may_change.then(|| {
    ActionButton::new("pipeline-new", ButtonKind::Primary, ctx.t("pipelines.new"))
      .icon(Icon::Plus)
      .opens(nav.to(Route::Project(project, ProjectPage::NewPipeline)))
  });
  parts::header()
    .padding(
      Padding::new()
        .top(lurq::app::theme::SpacingSize::Xl)
        .bottom(lurq::app::theme::SpacingSize::Md)
        .left(ExtraSpacing::Roomy.role())
        .right(ExtraSpacing::Roomy.role()),
    )
    .child(parts::title_row("pipelines-title", &ctx.t("pipelines.title")).with_children(actions))
    .into()
}

fn populated(
  ctx: &mut Ctx,
  (shown, team): (&ShownProject, Option<&TeamConfiguration>),
  entries: &[LibraryEntry],
) -> lurq::components::Column {
  // A pipeline is stored only as published versions (D-032): every entry is published.
  let published: Vec<&LibraryEntry> = entries.iter().collect();
  let mut pane =
    parts::pane("pipelines-library").with_children(team.map(|team| team::line(ctx, "pipelines-team-line", team)));
  if !published.is_empty() {
    let (id, title) = ("pipelines-published", "pipelines.groups.published");
    let rows: Vec<Element> = published
      .iter()
      .map(|entry| {
        ctx.mount_keyed::<super::library_row::LibraryRow>(
          &format!("pipeline-row-{}", entry.id),
          RowProps {
            project: shown.open.project.id(),
            root: shown.open.repo_root.clone(),
            entry: (*entry).clone(),
            team: team.cloned(),
          },
        )
      })
      .collect();
    pane = pane.child(RowGroup::new(id, ctx.t(title)).count(rows.len().to_string()).rows(rows));
  }
  let count = ctx.t_args(
    "pipelines.count",
    [
      ("count", entries.len().to_string()),
      ("published", published.len().to_string()),
    ],
  );
  pane.child(
    lurq::components::Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(lurq::app::theme::SpacingSize::Sm)
      .child(
        lurq::components::Text::new(&count)
          .id("pipelines-count")
          .variant(crate::theme::GroupText::Hint.role()),
      )
      .child(note("pipelines-versions-note", &ctx.t("pipelines.versionsExplanation"))),
  )
}

/// No pipeline yet (`Pane` `O8C4Bn`): what a pipeline is, and starting from the starter. An empty pipeline is not
/// offered: it has no steps, so it can never be saved (D-032, D-040).
fn empty(ctx: &mut Ctx, project: ProjectId) -> lurq::components::Column {
  let nav = Navigation::of(ctx);
  let tile = |glyph| {
    lurq::components::Row::new()
      .width(Dimension::Px(crate::theme::ControlHeight::Md.value()))
      .height(Dimension::Px(crate::theme::ControlHeight::Md.value()))
      .align_items(lurq::layout::Alignment::Center)
      .justify(lurq::layout::layout_kind::Justify::Center)
      .rounded(lurq::app::theme::RadiusSize::Md)
      .border_inside(lurq::app::theme::BorderSize::Sm, PaletteColor::Border)
      .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary))
  };
  let starter = GroupRow::new("pipeline-empty-starter", ctx.t("pipelines.starter"))
    .tiled()
    .leading(tile(Icon::Sparkles))
    .hint(ctx.t("pipelines.starterLibraryHint"))
    .end(
      ActionButton::new(
        "pipeline-empty-create",
        ButtonKind::Secondary,
        ctx.t("pipelines.createFromStarter"),
      )
      .icon(Icon::Sparkles)
      .opens(nav.to(Route::Project(project, ProjectPage::NewPipeline))),
    );
  parts::pane("pipelines-empty")
    .child(
      lurq::components::Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .child(
          lurq::components::Text::new(&ctx.t("pipelines.emptyTitle"))
            .id("pipelines-empty-title")
            .variant(lurq::app::theme::TypographyStyle::Title),
        )
        .child(parts::lead("pipelines-empty-lead", &ctx.t("pipelines.emptyDetail"))),
    )
    .child(RowGroup::new("pipelines-start", ctx.t("pipelines.startFrom")).row(starter))
    .child(note("pipelines-empty-note", &ctx.t("pipelines.emptyCreationHint")))
}

pub(super) fn age(ctx: &Ctx, at: orchester_core::time::Timestamp) -> std::sync::Arc<str> {
  let minutes = (orchester_core::time::Timestamp::now().unix_millis() - at.unix_millis()).max(0) / 60_000;
  let (key, count) = match minutes {
    0 => return ctx.t("pipelines.justNow"),
    1..60 => ("pipelines.minutesAgo", minutes),
    60..1440 => ("pipelines.hoursAgo", minutes / 60),
    _ => ("pipelines.daysAgo", minutes / 1440),
  };
  ctx.t_args(key, [("count", count.to_string())])
}
