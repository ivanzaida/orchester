//! A card's facts (`Facts` `THNwH` in `b55NqO`, the cards of `UKgrR` and `Ee2JE`): rows 6 apart of a key, 72 wide in
//! 11.5 `text_muted`, then 12 on its links (12.5/500 `accent_text` over a 1 `text_muted` line) and values (12.5
//! `text_secondary`, a branch or command in Geist Mono 11.5) and what it adds in 12 `text_muted`; and, under them,
//! *What changes* over the draft's diff (`WLVSI`): its lines 20 high on `code_bg` inside a `border` stroke, radius 8,
//! 4 over and under them, each with its numbers before and after (44 wide, Geist Mono 11 `text_muted`, at their end),
//! its sign (24 wide, centred, `−` in `danger`, `+` in `success`) and its text (Geist Mono 12), removed lines on
//! `diff_remove`, added ones on `diff_add`.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::{Alignment, text_style::TextAlign},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ProjectId,
  orchestrator::change::{Change, Publishing},
  project::TaskKey,
  time::Timestamp,
  tokens::compact_tokens,
};
use orchester_workspace::{
  display,
  orchestrator::facts::{Detail, Facts, LineChange, Place, Publication, RunFacts, Stands, TextDiff, TextLine},
};

use super::{widens, words::View};
use crate::{
  navigation::Navigation,
  routes::{AgentTab, InstructionsView, PipelinePanel, ProjectPage, Route, TaskTab, TaskView, ToolTab},
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, OrchestratorText},
  ui::pressable::opening,
};

/// A fact's key column (`u5vGv`).
const KEY_WIDTH: f32 = 72.0;
/// A diff line (`kzMBc`).
const LINE_HEIGHT: f32 = 20.0;
/// A diff line's numbers before and after (`ey7iM`, `DbWq7`).
const NUMBER_WIDTH: f32 = 44.0;
/// A diff line's sign (`OtSVu`).
const SIGN_WIDTH: f32 = 24.0;

/// A part of a fact's value.
enum Part {
  /// Words that open `route`.
  Link(Arc<str>, Route),
  Value(Arc<str>),
  Mono(Arc<str>),
  Aside(Arc<str>),
}

/// The facts of card `id`, read as `view` shows them, in project `project`.
pub(super) fn rows(ctx: &mut Ctx, id: &str, view: &View, project: ProjectId) -> Option<Column> {
  let rows: Vec<(Arc<str>, Vec<Part>)> = match view.facts? {
    Facts::Publication(publication) => publication_rows(ctx, view, publication, project),
    Facts::Run(run) => run_rows(ctx, view, run, project),
  };
  let navigation = Navigation::of(ctx);
  Some(
    Column::new()
      .id(format!("{id}-facts"))
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .with_children(
        rows
          .into_iter()
          .enumerate()
          .map(|(index, (key, parts))| row(&format!("{id}-fact-{index}"), &key, parts, &navigation)),
      ),
  )
}

fn row(id: &str, key: &str, parts: Vec<Part>, navigation: &Navigation) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Text::new(key)
        .variant(OrchestratorText::FactKey.role())
        .width(Dimension::Px(KEY_WIDTH))
        .flex_shrink(0.0),
    )
    .with_children(parts.into_iter().map(|part| part_element(part, navigation)))
}

fn part_element(part: Part, navigation: &Navigation) -> Element {
  match part {
    Part::Link(words, route) => opening(navigation.to(route))
      .padding_bottom(ExtraSpacing::Hair.role())
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::TextMuted))
      .child(Text::new(&words).variant(OrchestratorText::FactLink.role()).nowrap())
      .into(),
    Part::Value(words) => Text::new(&words)
      .variant(OrchestratorText::FactValue.role())
      .flex_shrink(1.0)
      .into(),
    Part::Mono(words) => Text::new(&words)
      .variant(OrchestratorText::FactMono.role())
      .flex_shrink(1.0)
      .into(),
    Part::Aside(words) => Text::new(&words)
      .variant(OrchestratorText::FactAside.role())
      .flex_shrink(1.0)
      .into(),
  }
}

/// Where the draft of `publication` is edited.
pub(super) fn draft_route(project: ProjectId, publication: (Publishing, orchester_core::id::ResourceId)) -> Route {
  let (of, resource) = publication;
  Route::Project(
    project,
    match of {
      Publishing::Pipeline => ProjectPage::Pipeline(resource, PipelinePanel::default()),
      Publishing::Profile => ProjectPage::Agent(resource, AgentTab::default()),
      Publishing::Instruction => ProjectPage::Instructions(InstructionsView::default()),
      Publishing::Skill => ProjectPage::Skill(resource),
      Publishing::Tool => ProjectPage::Tool(resource, ToolTab::default()),
    },
  )
}

/// The page of task `key`, on `tab`; `None` for a key people cannot read.
pub(super) fn task_route(project: ProjectId, key: &str, tab: TaskTab) -> Option<Route> {
  let key: TaskKey = key.parse().ok()?;
  Some(Route::Project(project, ProjectPage::Task(key, TaskView::of(tab))))
}

fn publication_rows(
  ctx: &Ctx,
  view: &View,
  publication: &Publication,
  project: ProjectId,
) -> Vec<(Arc<str>, Vec<Part>)> {
  let waits = view.phase().waits();
  let version = publication.version.to_string();
  let before = publication.version.saturating_sub(1).to_string();
  let versions = match (waits, publication.before_published) {
    (true, Some(published)) => ctx.t_args(
      "orchestrator.change.facts.versionsWaits",
      [
        ("before", before.as_str()),
        ("date", &super::super::date(ctx, published)),
        ("version", version.as_str()),
        ("time", &display::clock(publication.saved)),
      ],
    ),
    (true, None) => ctx.t_args(
      "orchestrator.change.facts.versionWaits",
      [
        ("version", version.as_str()),
        ("time", &display::clock(publication.saved)),
      ],
    ),
    (false, Some(_)) => ctx.t_args(
      "orchestrator.change.facts.versions",
      [("before", before.as_str()), ("version", version.as_str())],
    ),
    (false, None) => ctx.t_args("orchestrator.change.facts.version", [("version", version.as_str())]),
  };
  let route = draft_route(project, (publication.of, publication.resource));
  let mut rows = vec![(
    ctx.t(kind_key(publication.of)),
    vec![
      Part::Link(publication.name.as_str().into(), route),
      Part::Value(versions),
    ],
  )];
  rows.extend(detail_rows(ctx, publication, project));
  if waits && publication.keeping > 0 && publication.version > 1 {
    // On the row of where it applies, which follows the first; else on the first, which names the draft.
    let at = usize::from(!publication.places.is_empty());
    rows[at].1.push(Part::Aside(keeping(ctx, publication.keeping, &before)));
  }
  if waits && !publication.widens.is_empty() {
    let separator = ctx.t("orchestrator.change.facts.separator");
    let widens: Vec<String> = publication
      .widens
      .iter()
      .map(|reason| widens::compact(ctx, reason).to_string())
      .collect();
    rows.push((
      ctx.t("orchestrator.change.facts.widens"),
      vec![Part::Value(widens.join(&separator).into())],
    ));
  }
  rows
}

fn keeping(ctx: &Ctx, count: usize, before: &str) -> Arc<str> {
  if count == 1 {
    return ctx.t_args("orchestrator.change.facts.keepingOne", [("before", before)]);
  }
  ctx.t_args(
    "orchestrator.change.facts.keeping",
    [("count", count.to_string().as_str()), ("before", before)],
  )
}

const fn kind_key(of: Publishing) -> &'static str {
  match of {
    Publishing::Pipeline => "orchestrator.change.facts.pipeline",
    Publishing::Profile => "orchestrator.change.facts.profile",
    Publishing::Instruction => "orchestrator.change.facts.instruction",
    Publishing::Skill => "orchestrator.change.facts.skill",
    Publishing::Tool => "orchestrator.change.facts.tool",
  }
}

/// The rows a publication's kind adds: where it applies, its steps, model, command or files.
fn detail_rows(ctx: &Ctx, publication: &Publication, project: ProjectId) -> Vec<(Arc<str>, Vec<Part>)> {
  let mut rows = Vec::new();
  if !publication.places.is_empty() {
    let places = publication
      .places
      .iter()
      .map(|place| place_part(ctx, place, project))
      .collect();
    rows.push((ctx.t("orchestrator.change.facts.appliesTo"), places));
  }
  let detail = match &publication.detail {
    Some(Detail::Steps { count, changed }) => (
      "orchestrator.change.facts.steps",
      Part::Value(ctx.t_args(
        "orchestrator.change.facts.stepsCount",
        [("count", count.to_string()), ("changed", changed.to_string())],
      )),
    ),
    Some(Detail::Model { connection, model }) => (
      "orchestrator.change.facts.model",
      Part::Value(ctx.t_args(
        "orchestrator.change.facts.modelOf",
        [("connection", connection.as_str()), ("model", model.as_str())],
      )),
    ),
    Some(Detail::Files { count: 1 }) => (
      "orchestrator.change.facts.files",
      Part::Value(ctx.t("orchestrator.change.facts.filesOne")),
    ),
    Some(Detail::Files { count }) => (
      "orchestrator.change.facts.files",
      Part::Value(ctx.t_args("orchestrator.change.facts.filesCount", [("count", count.to_string())])),
    ),
    Some(Detail::Command { program, args }) => (
      "orchestrator.change.facts.command",
      Part::Mono(
        std::iter::once(program.as_str())
          .chain(args.iter().map(String::as_str))
          .collect::<Vec<_>>()
          .join(" ")
          .into(),
      ),
    ),
    None => return rows,
  };
  rows.push((ctx.t(detail.0), vec![detail.1]));
  rows
}

fn place_part(ctx: &Ctx, place: &Place, project: ProjectId) -> Part {
  match place {
    Place::Project => Part::Value(ctx.t("orchestrator.change.facts.project")),
    Place::Role(role) => Part::Value(ctx.t_args("orchestrator.change.facts.role", [("role", role.as_str())])),
    Place::Pipeline { pipeline, name } => Part::Link(
      name.as_str().into(),
      Route::Project(project, ProjectPage::Pipeline(*pipeline, PipelinePanel::default())),
    ),
    Place::Step { pipeline, name, step } => Part::Link(
      ctx.t_args(
        "orchestrator.change.facts.step",
        [("pipeline", name.as_str()), ("step", step.as_str())],
      ),
      Route::Project(project, ProjectPage::Pipeline(*pipeline, PipelinePanel::default())),
    ),
  }
}

fn run_rows(ctx: &Ctx, view: &View, run: &RunFacts, project: ProjectId) -> Vec<(Arc<str>, Vec<Part>)> {
  let number = run.number.to_string();
  let label = ctx.t_args(
    "orchestrator.change.facts.runLink",
    [("run", number.as_str()), ("pipeline", run.pipeline.as_str())],
  );
  let summary = run
    .task
    .as_ref()
    .and_then(|(key, _)| task_route(project, key, TaskTab::Summary));
  let mut first = vec![match summary.clone() {
    Some(route) => Part::Link(label, route),
    None => Part::Value(label),
  }];
  first.extend(stands(ctx, view, run).map(Part::Value));
  let mut rows = vec![(ctx.t("orchestrator.change.facts.run"), first)];
  match view.action.change.as_ref().map(|change| &change.change) {
    Some(Change::Cancel { .. }) => {
      if let (Some((key, title)), Some(route)) = (&run.task, summary) {
        let words = ctx.t_args(
          "orchestrator.change.facts.taskLink",
          [("task", key.as_str()), ("title", title.as_str())],
        );
        rows.push((ctx.t("orchestrator.change.facts.task"), vec![Part::Link(words, route)]));
      }
    }
    Some(Change::Merge { .. }) => rows.extend(merge_rows(ctx, run, project)),
    _ => {}
  }
  rows
}

fn merge_rows(ctx: &Ctx, run: &RunFacts, project: ProjectId) -> Vec<(Arc<str>, Vec<Part>)> {
  let mut rows = Vec::new();
  // A run that never started made no branch.
  if let Some(branch) = &run.branch {
    let branch: Arc<str> = match &run.into {
      Some(into) => ctx.t_args(
        "orchestrator.change.facts.branchInto",
        [("branch", branch.as_str()), ("into", into.as_str())],
      ),
      None => branch.as_str().into(),
    };
    rows.push((ctx.t("orchestrator.change.facts.branch"), vec![Part::Mono(branch)]));
  }
  if let Some((files, added, removed)) = run.changes {
    let words = if files == 1 {
      ctx.t_args(
        "orchestrator.change.facts.changesCountOne",
        [("added", added.to_string()), ("removed", removed.to_string())],
      )
    } else {
      ctx.t_args(
        "orchestrator.change.facts.changesCount",
        [
          ("files", files.to_string()),
          ("added", added.to_string()),
          ("removed", removed.to_string()),
        ],
      )
    };
    let route = run
      .task
      .as_ref()
      .and_then(|(key, _)| task_route(project, key, TaskTab::Changes));
    let part = match route {
      Some(route) => Part::Link(words, route),
      None => Part::Value(words),
    };
    rows.push((ctx.t("orchestrator.change.facts.changes"), vec![part]));
  }
  rows
}

/// Where the run stands, after its link.
fn stands(ctx: &Ctx, view: &View, run: &RunFacts) -> Option<Arc<str>> {
  let step = |(number, name): &(usize, String)| {
    ctx.t_args(
      "orchestrator.change.facts.stepLabel",
      [("number", format!("{number:02}")), ("step", name.clone())],
    )
  };
  let clock = |at: Timestamp| display::clock(at);
  if view.action.started.is_some() {
    let started = clock(view.at);
    return Some(match &run.stands {
      Stands::Going { step: Some(at), .. } => ctx.t_args(
        "orchestrator.change.facts.started",
        [("time", started.as_str()), ("step", &step(at))],
      ),
      _ => ctx.t_args("orchestrator.change.facts.startedAt", [("time", started.as_str())]),
    });
  }
  Some(match &run.stands {
    Stands::Going { step: Some(at), tokens } => ctx.t_args(
      "orchestrator.change.facts.going",
      [("step", &*step(at)), ("tokens", &compact_tokens(*tokens))],
    ),
    Stands::Going { step: None, tokens } => ctx.t_args(
      "orchestrator.change.facts.goingTokens",
      [("tokens", compact_tokens(*tokens))],
    ),
    Stands::Cancelled { step: Some(at), tokens } => ctx.t_args(
      "orchestrator.change.facts.cancelled",
      [("step", &*step(at)), ("tokens", &compact_tokens(*tokens))],
    ),
    Stands::Cancelled { step: None, tokens } => ctx.t_args(
      "orchestrator.change.facts.cancelledTokens",
      [("tokens", compact_tokens(*tokens))],
    ),
    Stands::Ready { since: Some(since) } => ctx.t_args("orchestrator.change.facts.ready", [("time", clock(*since))]),
    Stands::Ready { since: None } => ctx.t("orchestrator.change.facts.readyNow"),
    Stands::Merged { at, .. } => ctx.t_args("orchestrator.change.facts.merged", [("time", clock(*at))]),
    Stands::Failed => ctx.t("orchestrator.change.facts.failed"),
  })
}

/// *What changes* and the draft's diff, while it waits; nothing when the text does not change.
pub(super) fn diff(ctx: &Ctx, id: &str, view: &View) -> Option<Column> {
  let (true, Some(Facts::Publication(publication))) = (view.phase().waits(), view.facts) else {
    return None;
  };
  let diff: &TextDiff = publication.diff.as_ref()?;
  let added = diff.added.to_string();
  let label = if diff.removed == 1 {
    ctx.t_args("orchestrator.change.facts.whatChangesOne", [("added", added.as_str())])
  } else {
    ctx.t_args(
      "orchestrator.change.facts.whatChanges",
      [
        ("removed", diff.removed.to_string().as_str()),
        ("added", added.as_str()),
      ],
    )
  };
  let lines = Column::new()
    .id(format!("{id}-diff"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Xs))
    .background(ExtraColor::CodeBackground.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .with_children(diff.lines.iter().map(line));
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(Text::new(&label).variant(OrchestratorText::DiffLabel.role()))
      .child(lines)
      .with_children(
        diff
          .cut
          .then(|| Text::new(&ctx.t("orchestrator.change.facts.diffCut")).variant(OrchestratorText::DiffLabel.role())),
      ),
  )
}

fn line(line: &TextLine) -> Row {
  let number = |number: Option<usize>| {
    Text::new(&number.map(|number| number.to_string()).unwrap_or_default())
      .variant(OrchestratorText::DiffNumber.role())
      .text_align(TextAlign::Right)
      .width(Dimension::Px(NUMBER_WIDTH))
      .flex_shrink(0.0)
  };
  let (background, sign, color, kind) = match line.change {
    LineChange::Same => (None, " ", PaletteColor::TextMuted, "same"),
    LineChange::Removed => (Some(ExtraColor::DiffRemoved), "−", PaletteColor::Danger, "removed"),
    LineChange::Added => (Some(ExtraColor::DiffAdded), "+", PaletteColor::Success, "added"),
  };
  let row = Row::new()
    .describe("change", kind)
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(LINE_HEIGHT))
    .align_items(Alignment::Center)
    .child(number(line.old))
    .child(number(line.new))
    .child(
      Text::new(sign)
        .variant(OrchestratorText::DiffCode.role())
        .color(color)
        .text_align(TextAlign::Center)
        .width(Dimension::Px(SIGN_WIDTH))
        .flex_shrink(0.0),
    )
    .child(
      Text::new(&line.text)
        .variant(OrchestratorText::DiffCode.role())
        .selectable(true)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    );
  match background {
    Some(background) => row.background(background.role()),
    None => row,
  }
}
