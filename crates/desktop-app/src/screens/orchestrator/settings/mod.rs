//! Orchestrator settings (`Orchestrator settings · Profile` `jmyWu` in `Xir64`): in this version only what the
//! Orchestrator runs on, its Profile tab's *Connection & model* (`form`), saved as a draft and published as agent
//! profiles are (`publish`); its other tabs (Actions, Context, Conversations) are not built and show why. The head
//! (`Head` `b8gGj`: 24 over and 40 at its sides, 12 apart): the breadcrumb, *Orchestrator* (20/600) with what it is
//! for and, at the end, where the changes stand with *Save draft* and *Publish vN*; a summary of what the published
//! version runs on and does (`Permission summary` `c7khPh`); and the tabs.

mod form;
mod publish;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ConnectionId, orchestrator::OrchestratorSettings, resource::ModelId};
use orchester_workspace::{
  background,
  orchestrator::settings::{OrchestratorSetup, Readiness, Saving, save_in},
};

use self::publish::{Incomplete, Publishing, Save};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{OrchestratorChat, ProjectPage, Route},
  theme::{ExtraSpacing, IconSize, OrchestratorSpacing},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    icon::{Icon, icon},
    message::message_text,
    state::{load_error, loading},
    tabs::{PageTab, page_tabs},
  },
};

pub(super) fn screen(ctx: &mut Ctx, project: &ShownProject) -> Element {
  ctx.mount_keyed::<SettingsScreen>(
    &format!("orchestrator-settings-{}", project.open.project.id()),
    Props(project.clone()),
  )
}

#[derive(Clone, PartialEq)]
struct Props(ShownProject);

impl DevtoolsInspectable for Props {}

/// The choices as the form holds them: the connection's id, the model (empty: *Other model id…*, typed in
/// `custom`) and the effort (empty: the CLI's default).
#[derive(Clone)]
struct Choices {
  connection: Signal<String>,
  /// Whether the connection's menu is open.
  connection_open: Signal<bool>,
  model: Signal<String>,
  custom: Signal<String>,
  effort: Signal<String>,
  /// The connection the model and effort were filled for.
  filled_for: Signal<String>,
  /// The choices were filled from what is kept.
  seeded: Signal<bool>,
}

struct SettingsScreen {
  choices: Choices,
  save: Save,
  /// Counts saves, so what is kept is read again after each.
  revision: Signal<u64>,
}

impl Component for SettingsScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let choices = Choices {
      connection: ctx.signal(String::new()),
      connection_open: ctx.signal(false),
      model: ctx.signal(String::new()),
      custom: ctx.signal(String::new()),
      effort: ctx.signal(String::new()),
      filled_for: ctx.signal(String::new()),
      seeded: ctx.signal(false),
    };
    let save: Save = ctx.future_action(|Saving { root, settings, keep }: Saving| async move {
      background::run(move || save_in(&root, &settings, keep))
        .await
        .and_then(|saved| saved.map(drop))
    });
    let revision = ctx.signal(0);
    let kept = revision.clone();
    ctx.watch(&save.state(), move |state| {
      if state.is_fulfilled() {
        kept.update(|revision| *revision += 1);
      }
    });
    Self {
      choices,
      save,
      revision,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props(project) = ctx.props::<Props>().clone();
    let root = project.open.repo_root.clone();
    let read = ctx
      .future((root.clone(), self.revision.get()), |(root, _)| async move {
        background::run(move || OrchestratorSetup::load_in(&root))
          .await
          .and_then(|read| read)
      })
      .state()
      .get();
    let body: Element = match (&read.data, &read.error) {
      (Some(setup), _) => {
        if !self.choices.seeded.get() {
          self.choices.fill(setup);
        }
        self.choices.follow_connection(setup);
        form::form(ctx, &project, setup, &self.choices)
      }
      (None, Some(error)) => {
        let revision = self.revision.clone();
        load_error(
          ctx,
          "orchestrator-settings-error",
          (&ctx.t("orchestrator.loadFailed"), &message_text(ctx, error)),
          move || revision.update(|revision| *revision += 1),
        )
      }
      (None, None) => loading("orchestrator-settings-loading", &ctx.t("orchestrator.settings.loading")).into(),
    };
    let publishing = read.data.as_ref().map(|setup| Publishing {
      root: &root,
      setup,
      chosen: self.choices.chosen(setup),
      incomplete: self.choices.incomplete(setup),
    });
    let head = head(ctx, &project, (publishing.as_ref(), &self.save));
    Column::new()
      .id("screen-orchestrator_settings")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(head)
      .child(
        ScrollVertical::new(
          Column::new()
            .width(Dimension::Pct(100.0))
            .padding(
              Padding::new()
                .top(OrchestratorSpacing::Start.role())
                .bottom(SpacingSize::Section)
                .left(OrchestratorSpacing::SettingsPage.role())
                .right(OrchestratorSpacing::SettingsPage.role()),
            )
            .child(body),
        )
        .width(Dimension::Pct(100.0))
        .flex(1.0),
      )
  }
}

impl Choices {
  /// Shows what is kept: the draft, else the published version, else the first connection the Orchestrator can run
  /// on with its first model.
  fn fill(&self, setup: &OrchestratorSetup) {
    let shown = setup
      .current()
      .cloned()
      .or_else(|| setup.first_usable().and_then(|connection| setup.choosing(connection)));
    let connection = shown
      .as_ref()
      .map(|settings| settings.connection().to_string())
      .unwrap_or_default();
    self.show(shown.as_ref());
    self.connection.set(connection.clone());
    self.filled_for.set(connection);
    self.seeded.set(true);
  }

  /// A connection chosen anew starts with what is kept on it, else its first model and the CLI's own effort.
  fn follow_connection(&self, setup: &OrchestratorSetup) {
    let connection = self.connection.get();
    if connection == self.filled_for.get() {
      return;
    }
    self.filled_for.set(connection.clone());
    let chosen = connection
      .parse()
      .ok()
      .and_then(|connection| setup.choosing(connection));
    self.show(chosen.as_ref());
  }

  fn show(&self, settings: Option<&OrchestratorSettings>) {
    self.model.set(
      settings
        .map(|settings| settings.model().as_str().to_owned())
        .unwrap_or_default(),
    );
    self.custom.set(String::new());
    self.effort.set(
      settings
        .and_then(OrchestratorSettings::effort)
        .unwrap_or_default()
        .to_owned(),
    );
  }

  /// The model the form names: the chosen one, or the id typed under *Other model id…*.
  fn model_text(&self) -> String {
    let model = self.model.get();
    if model.is_empty() { self.custom.get() } else { model }
  }

  /// What the form still lacks for a choice: a connection the Orchestrator can run on, or then a model id.
  fn incomplete(&self, setup: &OrchestratorSetup) -> Option<Incomplete> {
    let usable = self.connection.get().parse().is_ok_and(|connection: ConnectionId| {
      setup
        .connections()
        .into_iter()
        .any(|choice| choice.id == connection && choice.unavailable.is_none())
    });
    if !usable {
      Some(Incomplete::Connection)
    } else if ModelId::new(self.model_text().trim()).is_err() {
      Some(Incomplete::Model)
    } else {
      None
    }
  }

  /// The form's choice, when it is complete and its connection one the Orchestrator can run on.
  fn chosen(&self, setup: &OrchestratorSetup) -> Option<OrchestratorSettings> {
    let connection = self.connection.get().parse().ok()?;
    let usable = setup
      .connections()
      .into_iter()
      .any(|choice| choice.id == connection && choice.unavailable.is_none());
    let model = ModelId::new(self.model_text().trim()).ok()?;
    let effort = Some(self.effort.get()).filter(|effort| !effort.is_empty());
    usable.then(|| OrchestratorSettings::new(connection, model, effort))
  }
}

/// The summary's height (`Permission summary` `c7khPh`).
const SUMMARY_HEIGHT: f32 = 24.0;

/// The head: the breadcrumb, the title with whether the changes are saved, the summary and the tabs.
fn head(ctx: &mut Ctx, project: &ShownProject, (publishing, save): (Option<&Publishing<'_>>, &Save)) -> Element {
  let id = project.open.project.id();
  let name = project.open.project.name().as_str();
  let navigation = Navigation::of(ctx);
  let crumbs = breadcrumb(
    ctx,
    "orchestrator-settings-crumbs",
    CrumbSeparator::Slash,
    vec![
      Crumb::to(
        name,
        navigation.to(Route::Project(id, ProjectPage::Overview(Default::default()))),
      ),
      Crumb::to(
        ctx.t("orchestrator.title"),
        navigation.to(Route::Project(id, ProjectPage::Orchestrator(OrchestratorChat::Latest))),
      ),
      Crumb::here(ctx.t("orchestrator.coordinator.settings")),
    ],
  );
  let title = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Start)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(SpacingSize::Xs)
        .child(Text::new(&ctx.t("orchestrator.title")).variant(TypographyStyle::Heading))
        .child(
          Text::new(&ctx.t_args("orchestrator.settings.description", [("project", name)]))
            .variant(TypographyStyle::Description),
        ),
    )
    .with_children(publishing.map(|publishing| publish::bar(ctx, save, publishing)));
  let tabs = tabs(ctx);
  Column::new()
    .id("orchestrator-settings-head")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Xl)
        .left(OrchestratorSpacing::SettingsPage.role())
        .right(OrchestratorSpacing::SettingsPage.role()),
    )
    .spacing(SpacingSize::Md)
    .child(crumbs)
    .child(title)
    .child(summary(
      ctx,
      publishing.map(|publishing| publishing.setup.readiness()).as_ref(),
    ))
    .child(tabs)
    .into()
}

/// What the Orchestrator runs on and does (`c7khPh`): each a 13 icon and 12 `text_secondary`, `·` between them.
fn summary(ctx: &Ctx, readiness: Option<&Readiness>) -> Element {
  let runs_on: Arc<str> = match readiness {
    Some(Readiness::Ready { connection, model }) => ctx.t_args(
      "orchestrator.settings.summary.runsOn",
      [("connection", &**connection), ("model", model.as_str())],
    ),
    _ => ctx.t("orchestrator.settings.summary.noConnection"),
  };
  let items = [
    (Icon::Waypoints, PaletteColor::TextMuted, runs_on),
    (
      Icon::FilePlus,
      PaletteColor::TextMuted,
      ctx.t("orchestrator.settings.summary.actions"),
    ),
    (
      Icon::Eye,
      PaletteColor::TextMuted,
      ctx.t("orchestrator.settings.summary.reads"),
    ),
    (
      Icon::ShieldCheck,
      PaletteColor::Success,
      ctx.t("orchestrator.settings.summary.checked"),
    ),
  ];
  let count = items.len();
  let mut row = Row::new()
    .id("orchestrator-settings-summary")
    .height(Dimension::Px(SUMMARY_HEIGHT))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center);
  for (index, (glyph, color, words)) in items.into_iter().enumerate() {
    row = row.child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Base, color))
        .child(Text::new(&words).variant(TypographyStyle::Description).nowrap()),
    );
    if index + 1 < count {
      row = row.child(
        Text::new(&ctx.t("orchestrator.dot"))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted),
      );
    }
  }
  row.into()
}

/// Profile, the tab shown; the others are not built yet and say so.
fn tabs(ctx: &mut Ctx) -> Element {
  let reason = ctx.t("orchestrator.settings.tabs.notBuilt");
  let tabs = vec![
    PageTab::new(
      "orchestrator-settings-tab-profile",
      ctx.t("orchestrator.settings.tabs.profile"),
      true,
      || {},
    ),
    PageTab::unavailable(
      "orchestrator-settings-tab-actions",
      ctx.t("orchestrator.settings.tabs.actions"),
      reason.clone(),
    ),
    PageTab::unavailable(
      "orchestrator-settings-tab-context",
      ctx.t("orchestrator.settings.tabs.context"),
      reason.clone(),
    ),
    PageTab::unavailable(
      "orchestrator-settings-tab-conversations",
      ctx.t("orchestrator.settings.tabs.conversations"),
      reason,
    ),
  ];
  page_tabs(ctx, "orchestrator-settings-tabs", tabs).into()
}
