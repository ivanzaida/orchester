//! Select resources (`DsqQJ`; its states in `oUQ3O`): the project to import from and this one, the kind tabs, the
//! search, and the source's pipelines, agent profiles, instructions and tools to select. Secrets are not imported:
//! their tab only says why.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, SelectOption, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::import::CatalogResource;
use orchester_store::RegisteredProject;
use orchester_workspace::import::{data::Catalogs, review, select};

use super::{
  Picked, Source, SourceChoice, State, Step, frame,
  resource_row::{self, ResourceRowProps},
  words,
};
use crate::{
  project::ShownProject,
  routes::{ImportTab, ProjectPage, Route},
  theme::{ExtraSpacing, ExtraTypography, IconSize, ImportSpacing, ImportText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, SelectField, field},
    icon::{Icon, icon},
    inspector::note,
    message::message_text,
    notice::{Notice, NoticeTone},
    state::loading,
    tabs::{PageTab, counted, page_tabs_with_note},
    unavailable::reason_at_end,
  },
};

/// The source select's width, the search's, and the list's columns (`zghjN`, `p7wre`, `LZuTH`).
const SOURCE_WIDTH: f32 = 280.0;
const SEARCH_WIDTH: f32 = 260.0;
pub(super) const VERSION_WIDTH: f32 = 190.0;
pub(super) const DEPENDS_WIDTH: f32 = 300.0;
/// The list's heading row and the destination's tile (`LZuTH`, `OBEz2`).
const HEADING_HEIGHT: f32 = 32.0;
const TILE: f32 = 18.0;

/// The step's body and its footer.
pub(super) fn step(
  ctx: &mut Ctx,
  state: &State,
  (project, tab): (&ShownProject, ImportTab),
  (projects, source): (&[RegisteredProject], &Source),
) -> (Element, Element) {
  let mut body = body(SpacingSize::Lg).child(source_row(ctx, state, project, projects));
  let catalogs = match source {
    Source::Ready(catalogs) => Some(&**catalogs),
    _ => None,
  };
  if matches!(source, Source::Ready(_) | Source::Reading(_)) {
    body = body.child(tabs(ctx, state, tab, catalogs));
  }
  body = body.child(content(ctx, state, tab, source));
  (body.into(), footer(ctx, state, source))
}

/// A step's body under the header: `top` over its parts (16 here, `GIBmW`; 14 in Resolve dependencies, `m7iSW`),
/// 24 at its sides, 12 between them.
pub(super) fn body(top: SpacingSize) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding(Padding::new().top(top).left(SpacingSize::Xl).right(SpacingSize::Xl))
    .spacing(SpacingSize::Md)
}

/// The project to import from, this project, and the search (`Source row` `IIxM0`).
fn source_row(ctx: &mut Ctx, state: &State, project: &ShownProject, projects: &[RegisteredProject]) -> Row {
  let label = ctx.t("import.source");
  let options: Vec<SelectOption<SourceChoice>> = projects
    .iter()
    .map(|project| SelectOption::new(SourceChoice(Some(project.id)), project.name.as_str()))
    .collect();
  let select = SelectField::new("import-source", label.clone(), state.source.clone(), options);
  let select = if projects.is_empty() {
    select.placeholder(ctx.t("import.noSources")).locked()
  } else {
    select
  };
  let name = project.open.project.name().as_str();
  let initial: String = name
    .chars()
    .next()
    .map(|first| first.to_uppercase().collect())
    .unwrap_or_default();
  let destination = Column::new()
    .spacing(ExtraSpacing::Snug.role())
    .child(Text::new(&ctx.t("import.destination")).variant(TypographyStyle::FieldLabel))
    .child(
      Row::new()
        .height(Dimension::Px(crate::theme::ControlHeight::Md.value()))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Row::new()
            .width(Dimension::Px(TILE))
            .height(Dimension::Px(TILE))
            .align_items(Alignment::Center)
            .justify(Justify::Center)
            .rounded(RadiusSize::Sm)
            .background(PaletteColor::SurfaceRaised)
            .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
            .child(Text::new(&initial).variant(ImportText::Initial.role())),
        )
        .child(
          Text::new(&ctx.t_args("import.destinationValue", [("project", name)]))
            .id("import-destination")
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextPrimary)
            .nowrap(),
        ),
    );
  let search = FieldInput::new("import-search", ctx.t("import.search"), state.search.clone())
    .search()
    .placeholder(ctx.t("import.search"))
    .clearable(ctx.t("import.clearSearch"))
    .width(Dimension::Px(SEARCH_WIDTH))
    .render(ctx);
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::End)
    .child(Column::new().width(Dimension::Px(SOURCE_WIDTH)).child(field(
      "import-source-field",
      Some(&label),
      select.render(ctx),
      None,
    )))
    .child(
      Row::new()
        .height(Dimension::Px(crate::theme::ControlHeight::Md.value()))
        .align_items(Alignment::Center)
        .child(icon(Icon::ArrowRight, IconSize::Md, PaletteColor::TextMuted)),
    )
    .child(destination)
    .child(Spacer::new().flex(1.0))
    .child(search)
}

/// The kind tabs with their counts once the source is read, and why secrets are not imported (`Kind tabs` `ptqq0`).
fn tabs(ctx: &mut Ctx, state: &State, shown: ImportTab, catalogs: Option<&Catalogs>) -> Element {
  let tabs: Vec<PageTab> = ImportTab::ALL
    .into_iter()
    .map(|tab| {
      let label = ctx.t(tab_key(tab));
      let id = format!("import-tab-{}", tab.name());
      if tab == ImportTab::Secrets {
        return PageTab::unavailable(id, label, ctx.t("import.secretsReason"));
      }
      let label = match catalogs {
        Some(catalogs) => counted(ctx, &label, select::count(catalogs, tab.kind())),
        None => label,
      };
      let open = state
        .navigation
        .to(Route::Project(state.project, ProjectPage::Import(tab)));
      PageTab::to(id, label, tab == shown, open)
    })
    .collect();
  page_tabs_with_note(
    ctx,
    "import-tabs",
    tabs,
    reason_at_end("import-secrets-reason", &ctx.t("import.secretsReason")),
  )
  .into()
}

const fn tab_key(tab: ImportTab) -> &'static str {
  match tab {
    ImportTab::All => "import.tabs.all",
    ImportTab::Pipelines => "import.tabs.pipelines",
    ImportTab::Agents => "import.tabs.agents",
    ImportTab::Instructions => "import.tabs.instructions",
    ImportTab::Tools => "import.tabs.tools",
    ImportTab::Secrets => "import.tabs.secrets",
  }
}

/// What the body shows under the tabs: the list, or why there is none.
fn content(ctx: &mut Ctx, state: &State, tab: ImportTab, source: &Source) -> Element {
  match source {
    Source::Listing => loading("import-listing", &ctx.t("import.empty.listing")).into(),
    Source::Unlisted(error) => Notice::new("import-unlisted", NoticeTone::Danger, message_text(ctx, error))
      .action(retry(ctx, state))
      .into(),
    Source::NoProjects => frame::centered(
      "import-no-sources",
      (Icon::Folders, PaletteColor::TextMuted),
      (
        &ctx.t("import.empty.noSourcesTitle"),
        &ctx.t("import.empty.noSourcesText"),
      ),
      Some(
        ActionButton::new(
          "import-all-projects",
          ButtonKind::Secondary,
          ctx.t("import.empty.openAllProjects"),
        )
        .icon(Icon::Folders)
        .opens(state.navigation.to(Route::AllProjects))
        .into(),
      ),
    ),
    Source::Reading(chosen) => {
      let name = chosen.name.as_str();
      frame::centered(
        "import-reading",
        (Icon::LoaderCircle, PaletteColor::TextSecondary),
        (
          &ctx.t_args("import.empty.readingTitle", [("project", name)]),
          &ctx.t_args("import.empty.readingText", [("project", name)]),
        ),
        None,
      )
    }
    Source::Failed(notice) => {
      let glyph = if notice.refused { Icon::Lock } else { Icon::CircleAlert };
      Notice::new(
        "import-unreadable",
        NoticeTone::Danger,
        message_text(ctx, &notice.title),
      )
      .icon(glyph)
      .body(message_text(ctx, &notice.body))
      .action(retry(ctx, state))
      .into()
    }
    Source::Ready(catalogs) => listed(ctx, state, tab, catalogs),
  }
}

/// Reads the projects again.
fn retry(ctx: &Ctx, state: &State) -> ActionButton {
  let revision = state.revision.clone();
  ActionButton::new("import-retry", ButtonKind::Secondary, ctx.t("import.tryAgain"))
    .on_click(move || revision.update(|revision| *revision += 1))
}

/// The source's resources of the tab that match the search, or why there are none.
fn listed(ctx: &mut Ctx, state: &State, tab: ImportTab, catalogs: &Catalogs) -> Element {
  let name = catalogs.source.project().name().as_str();
  if tab == ImportTab::Secrets {
    let all = state
      .navigation
      .to(Route::Project(state.project, ProjectPage::Import(ImportTab::All)));
    let target = catalogs.target.project().name().as_str();
    return frame::centered(
      "import-secrets",
      (Icon::KeyRound, PaletteColor::TextMuted),
      (
        &ctx.t("import.empty.secretsTitle"),
        &ctx.t_args("import.empty.secretsText", [("project", target)]),
      ),
      Some(
        ActionButton::new("import-show-all", ButtonKind::Secondary, ctx.t("import.empty.showAll"))
          .icon(Icon::List)
          .opens(all)
          .into(),
      ),
    );
  }
  if catalogs.source.resources().is_empty() {
    return frame::centered(
      "import-nothing",
      (Icon::Inbox, PaletteColor::TextMuted),
      (
        &ctx.t("import.empty.nothingTitle"),
        &ctx.t_args("import.empty.nothingText", [("project", name)]),
      ),
      None,
    );
  }
  let search = state.search.get();
  let shown = select::shown(catalogs, tab.kind(), &search);
  if shown.is_empty() {
    return no_match(ctx, state, name, search.trim());
  }
  let plan = state.plan(catalogs);
  let rows: Vec<Element> = shown
    .iter()
    .map(|entry| row(ctx, state, catalogs, (entry, &plan)))
    .collect();
  let list = Column::new()
    .id("import-list")
    .width(Dimension::Pct(100.0))
    .rounded(RadiusSize::Lg)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .clip()
    .child(headings(ctx))
    .with_children(rows);
  ScrollVertical::new(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .padding_bottom(SpacingSize::Lg)
      .child(list)
      .child(note("import-note", Icon::Info, &ctx.t("import.dependencyNote"))),
  )
  .width(Dimension::Pct(100.0))
  .flex(1.0)
  .min_height(Dimension::Px(0.0))
  .into()
}

/// No resource of the tab matches the search, or the tab has none.
fn no_match(ctx: &Ctx, state: &State, project: &str, search: &str) -> Element {
  if search.is_empty() {
    return frame::centered(
      "import-no-match",
      (Icon::SearchX, PaletteColor::TextMuted),
      (
        &ctx.t("import.empty.kindTitle"),
        &ctx.t_args("import.empty.kindText", [("project", project)]),
      ),
      None,
    );
  }
  let cleared = state.search.clone();
  frame::centered(
    "import-no-match",
    (Icon::SearchX, PaletteColor::TextMuted),
    (
      &ctx.t("import.empty.noMatchTitle"),
      &ctx.t_args("import.empty.noMatchText", [("project", project), ("search", search)]),
    ),
    Some(
      ActionButton::new(
        "import-clear-search",
        ButtonKind::Secondary,
        ctx.t("import.clearSearch"),
      )
      .icon(Icon::X)
      .on_click(move || cleared.set(String::new()))
      .into(),
    ),
  )
}

/// RESOURCE, VERSION and DEPENDS ON (`List header` `LZuTH`).
fn headings(ctx: &Ctx) -> Row {
  let heading = |key| {
    Text::new(&ctx.t(key))
      .variant(ExtraTypography::ColumnHeading.role())
      .nowrap()
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADING_HEIGHT))
    .padding(
      Padding::new()
        .left(ImportSpacing::CheckColumn.role())
        .right(SpacingSize::Sm),
    )
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(Row::new().flex(1.0).child(heading("import.columns.resource")))
    .child(
      Row::new()
        .width(Dimension::Px(VERSION_WIDTH))
        .child(heading("import.columns.version")),
    )
    .child(
      Row::new()
        .width(Dimension::Px(DEPENDS_WIDTH))
        .child(heading("import.columns.dependsOn")),
    )
}

fn row(
  ctx: &mut Ctx,
  state: &State,
  catalogs: &Catalogs,
  (entry, plan): (&CatalogResource, &orchester_core::import::Plan),
) -> Element {
  let id = entry.resource().id();
  let line = select::line(catalogs, entry, plan);
  let chosen = state
    .picked
    .get()
    .0
    .iter()
    .find(|(picked, _)| *picked == id)
    .map(|(_, pick)| *pick);
  let subject = review::subject(&catalogs.source, orchester_core::import::Entity::Resource(id), None)
    .unwrap_or(review::Subject::Pipeline);
  let props = ResourceRowProps {
    id,
    name: entry.resource().name().as_str().into(),
    summary: words::summary(ctx, &line.summary, subject),
    included_with: line
      .included_with
      .as_ref()
      .map(|name| ctx.t_args("import.includedWith", [("name", name.as_str())])),
    depends_on: words::depends_on(ctx, &line.depends_on),
    picks: entry
      .picks()
      .into_iter()
      .map(|pick| (pick, words::pick(ctx, pick)))
      .collect(),
    pick: chosen.unwrap_or_else(|| entry.default_pick()),
    selected: chosen.is_some(),
    picked: state.picked.clone(),
  };
  resource_row::mount(ctx, props)
}

/// What is selected and what it needs resolved, why Resolve dependencies is unavailable, Cancel and Resolve
/// dependencies (`s4V5W0`).
fn footer(ctx: &mut Ctx, state: &State, source: &Source) -> Element {
  let Picked(picked) = state.picked.get();
  let text = match source {
    Source::Ready(catalogs) if !picked.is_empty() => {
      let plan = state.plan(catalogs);
      let dependencies = match review::dependencies(&plan) {
        0 => ctx.t("import.toResolve.none"),
        1 => ctx.t("import.toResolve.one"),
        count => ctx.t_args("import.toResolve.other", [("count", count.to_string())]),
      };
      let selected = ctx.t_args("import.selected", [("count", picked.len().to_string())]);
      words::parts(ctx, &[selected, dependencies])
    }
    _ => ctx.t("import.nothingSelected"),
  };
  let reason = match source {
    Source::NoProjects => Some(ctx.t("import.blocked.noSources")),
    Source::Unlisted(_) | Source::Failed(_) => Some(ctx.t("import.blocked.noSource")),
    Source::Ready(_) if picked.is_empty() => Some(ctx.t("import.blocked.selectOne")),
    Source::Listing | Source::Reading(_) | Source::Ready(_) => None,
  };
  let navigation = state.navigation.clone();
  let cancel = ActionButton::new("import-cancel", ButtonKind::Secondary, ctx.t("import.cancel"))
    .on_click(move || navigation.back());
  let next = ActionButton::new("import-next", ButtonKind::Primary, ctx.t("import.resolve")).icon(Icon::ArrowRight);
  let step = state.step.clone();
  let next = match (&reason, source) {
    (Some(reason), _) => next.unavailable(reason.clone()),
    (None, Source::Ready(_)) => next.on_click(move || step.set(Step::Resolve)),
    (None, _) => next.unavailable(ctx.t("import.blocked.noSource")),
  };
  let mut end: Vec<Element> = reason.iter().map(|reason| frame::blocked(reason)).collect();
  end.push(cancel.into());
  end.push(next.into());
  frame::footer(&text, end)
}
