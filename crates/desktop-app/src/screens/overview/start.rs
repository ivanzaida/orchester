//! A new project's getting-started list (`Start` `ru9Gh` in 03 `ArvFR`): in the digest's frame, its head *Set up Kontur
//! to run tasks* with *N of 4 done*, then a 60 high line per step (`orchester_workspace::overview::start`): a
//! `circle-check` in `success` once done, else a `circle-dashed`; the step in 13/500 (`text_secondary` once done) over
//! what was found in 12 `text_muted`; and the link to where it is done. Under the list, the Import resources hint. What
//! is done is read off the UI thread, again whenever a screen or automation saved agents, pipelines or connections.

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
use orchester_core::id::ProjectId;
use orchester_workspace::{
  background, messages,
  overview::start::{StartLine, StartStep, done, steps},
  tasks::ready_to_run::ProjectReadiness,
};

use super::{
  digest::frame,
  section::{link, muted},
};
use crate::{
  navigation::{Navigation, link::Open},
  project::ShownProject,
  routes::{ConnectionsView, ImportTab, ProjectPage, Route},
  theme::{ExtraSpacing, ExtraTypography, IconSize, OverviewText},
  ui::{
    failure::LastFailure,
    icon::{Icon, icon},
    message::message_text,
    pressable::Press,
    state::loading,
  },
};

/// The list's width (`Start` `ru9Gh`), its head's height (`ci1xJ`).
const WIDTH: f32 = 720.0;
const HEAD_HEIGHT: f32 = 36.0;

/// The list for `project`, which has `tasks` tasks.
pub(super) fn start(ctx: &mut Ctx, project: &ShownProject, tasks: usize) -> Element {
  let outside = project.feed.outside.get();
  let props = StartProps {
    project: project.open.project.id(),
    name: project.open.project.name().as_str().into(),
    repo_root: project.open.repo_root.clone(),
    tasks,
    revision: project.feed.revision.get() + outside.profiles + outside.pipelines + outside.connections,
  };
  ctx.mount_keyed::<GettingStarted>("overview-start", props)
}

#[derive(Clone, PartialEq)]
struct StartProps {
  project: ProjectId,
  name: Box<str>,
  repo_root: PathBuf,
  tasks: usize,
  revision: u64,
}

impl DevtoolsInspectable for StartProps {}

struct GettingStarted {
  /// A failed read's error, kept while the list is read again: a read that fails after one that worked says so
  /// instead of keeping the steps it read before.
  last: LastFailure,
}

impl Component for GettingStarted {
  type Props = StartProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self {
      last: LastFailure::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<StartProps>().clone();
    let read = ctx.future((props.repo_root.clone(), props.revision), |(repo_root, _)| async move {
      background::run(move || ProjectReadiness::load_in(&repo_root))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot read whether the project is ready to run");
          messages::store_error(&error)
        })
    });
    let state = read.state().get();
    let failed = self.last.follow(&state);
    let head_title = ctx.t_args("overview.start.head", [("project", &*props.name)]);
    let list = frame("overview-start-list");
    let list = match (failed, &state.data) {
      (Some((error, _)), _) => list.child(start_head(&head_title, None)).child(
        Row::new()
          .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Md))
          .child(muted(&message_text(ctx, &error)).id("overview-start-error")),
      ),
      (None, Some(readiness)) => {
        let lines = steps(readiness, props.tasks, &ctx.t("overview.listSeparator"));
        let count = ctx.t_args("overview.start.done", [("count", done(&lines).to_string())]);
        lines
          .iter()
          .fold(list.child(start_head(&head_title, Some(&count))), |list, line| {
            list.child(step_line(ctx, props.project, line))
          })
      }
      (None, None) => list
        .child(start_head(&head_title, None))
        .child(loading("overview-start-reading", &ctx.t("overview.start.reading")).padding(SpacingSize::Md)),
    };
    Column::new()
      .id("overview-start")
      .width(Dimension::Px(WIDTH))
      .spacing(SpacingSize::Lg)
      .child(list)
      .child(import_hint(ctx, props.project))
  }
}

fn start_head(title: &str, done: Option<&str>) -> Row {
  Row::new()
    .id("overview-start-head")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .child(icon(Icon::ListChecks, IconSize::Base, PaletteColor::TextSecondary))
    .child(Text::new(title).variant(OverviewText::Head.role()).nowrap())
    .with_children(done.map(|done| muted(done).id("overview-start-done")))
}

fn step_line(ctx: &mut Ctx, project: ProjectId, line: &StartLine) -> Row {
  let id = format!("overview-start-{}", line.step.name());
  let (glyph, color, words) = if line.done {
    (Icon::CircleCheck, PaletteColor::Success, PaletteColor::TextSecondary)
  } else {
    (Icon::CircleDashed, PaletteColor::TextMuted, PaletteColor::TextPrimary)
  };
  let (label, open) = destination(ctx, project, line.step);
  Row::new()
    .id(id.clone())
    .describe("done", line.done.to_string())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Md, color).id(format!("{id}-{}", if line.done { "done" } else { "todo" })))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Fine.role())
        .child(
          Text::new(&message_text(ctx, &line.step.title()))
            .variant(ExtraTypography::Strong.role())
            .color(words),
        )
        .child(
          Text::new(&message_text(ctx, &line.found))
            .id(format!("{id}-found"))
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextMuted)
            .width(Dimension::Pct(100.0)),
        ),
    )
    .child(link(&format!("{id}-open"), &label, Press::Open(open)))
}

/// Where a step is done, and its link's words.
fn destination(ctx: &mut Ctx, project: ProjectId, step: StartStep) -> (std::sync::Arc<str>, Open) {
  let (key, page) = match step {
    StartStep::Connect => (
      "overview.start.connections",
      ProjectPage::Connections(ConnectionsView::default()),
    ),
    StartStep::Profile => ("overview.start.agents", ProjectPage::Agents),
    StartStep::Pipeline => ("overview.start.pipelines", ProjectPage::Pipelines),
    StartStep::Task => ("overview.start.newTask", ProjectPage::NewTask),
  };
  (ctx.t(key), Navigation::of(ctx).to(Route::Project(project, page)))
}

/// *Agent files already in the repository … come in as drafts.* with *Import resources ↗*.
fn import_hint(ctx: &mut Ctx, project: ProjectId) -> Row {
  let open = Navigation::of(ctx).to(Route::Project(project, ProjectPage::Import(ImportTab::All)));
  Row::new()
    .id("overview-start-import")
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Download, IconSize::Base, PaletteColor::TextMuted))
    .child(
      muted(&ctx.t("overview.start.import"))
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(link(
      "overview-start-import-open",
      &ctx.t("overview.start.importLink"),
      Press::Open(open),
    ))
}
