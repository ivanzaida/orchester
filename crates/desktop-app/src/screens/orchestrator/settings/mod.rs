//! Orchestrator settings (`Orchestrator settings · Profile` `jmyWu` in `Xir64`): in this version only what the
//! Orchestrator runs on, its Profile tab's *Connection & model* (`form`), saved as soon as it changes; its other
//! tabs (Actions, Context, Conversations) are not built and show why. The head (`Head` `b8gGj`: 24 over and 40 at
//! its sides, 12 apart): the breadcrumb, *Orchestrator* (20/600) with what it is for and, at the end, whether the
//! changes are saved; a summary of what it runs on and does (`Permission summary` `c7khPh`); and the tabs.

mod form;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::orchestrator::OrchestratorSettings;
use orchester_workspace::{
  background,
  messages::Message,
  orchestrator::settings::{OrchestratorSetup, Readiness, save_in},
};
use parking_lot::Mutex;

use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{OrchestratorChat, ProjectPage, Route},
  theme::{ExtraSpacing, IconSize, OrchestratorSpacing},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    field::field_error,
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

/// Saving what the Orchestrator runs on, in the project at the path.
type Save = FutureAction<(std::path::PathBuf, OrchestratorSettings), OrchestratorSettings, Message>;

/// The choices as the selects hold them: the connection's id, the model and the effort (empty: the CLI's default).
#[derive(Clone)]
struct Choices {
  connection: Signal<String>,
  model: Signal<String>,
  effort: Signal<String>,
  /// The choices were filled from what is saved.
  seeded: Signal<bool>,
}

struct SettingsScreen {
  choices: Choices,
  save: Save,
  /// What was last asked to be saved: the same choice is not saved twice in a row, so a refused one waits for a
  /// change.
  asked: Arc<Mutex<Option<OrchestratorSettings>>>,
  /// Counts saves, so what is saved is read again after each.
  revision: Signal<u64>,
}

impl Component for SettingsScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let choices = Choices {
      connection: ctx.signal(String::new()),
      model: ctx.signal(String::new()),
      effort: ctx.signal(String::new()),
      seeded: ctx.signal(false),
    };
    let save: Save = ctx.future_action(
      |(root, settings): (std::path::PathBuf, OrchestratorSettings)| async move {
        let saved = settings.clone();
        background::run(move || save_in(&root, &settings))
          .await
          .and_then(|saved| saved)
          .map(|()| saved)
      },
    );
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
      asked: Arc::default(),
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
          self.choices.fill(setup.settings.as_ref());
        }
        self.save_changes(setup, &root);
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
    let head = head(ctx, &project, (read.data.as_ref(), &self.save));
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
  /// Shows `settings` in the selects.
  fn fill(&self, settings: Option<&OrchestratorSettings>) {
    self.connection.set(
      settings
        .map(|settings| settings.connection().to_string())
        .unwrap_or_default(),
    );
    self.model.set(
      settings
        .map(|settings| settings.model().as_str().to_owned())
        .unwrap_or_default(),
    );
    self.effort.set(
      settings
        .and_then(OrchestratorSettings::effort)
        .unwrap_or_default()
        .to_owned(),
    );
    self.seeded.set(true);
  }
}

impl SettingsScreen {
  /// Saves what the selects hold once it differs from what is saved: a connection chosen anew starts with its first
  /// model and the CLI's own effort.
  fn save_changes(&self, setup: &OrchestratorSetup, root: &std::path::Path) {
    if !self.choices.seeded.get() || self.save.state().get().is_pending() {
      return;
    }
    let Ok(connection) = self.choices.connection.get().parse() else {
      return;
    };
    let saved = setup.settings.as_ref();
    let wanted = if saved.is_some_and(|saved| saved.connection() == connection) {
      let Ok(model) = orchester_core::resource::ModelId::new(&self.choices.model.get()) else {
        return;
      };
      let effort = Some(self.choices.effort.get()).filter(|effort| !effort.is_empty());
      Some(OrchestratorSettings::new(connection, model, effort))
    } else {
      // Another connection starts with its own first model and the CLI's effort, which the selects show at once.
      let chosen = setup.choosing(connection);
      if let Some(chosen) = &chosen {
        self.choices.model.set(chosen.model().as_str().to_owned());
        self.choices.effort.set(chosen.effort().unwrap_or_default().to_owned());
      }
      chosen
    };
    let Some(wanted) = wanted.filter(|wanted| Some(wanted) != saved) else {
      return;
    };
    let mut asked = self.asked.lock();
    if asked.as_ref() == Some(&wanted) {
      return;
    }
    *asked = Some(wanted.clone());
    self.save.run((root.to_path_buf(), wanted));
  }
}

/// The head: the breadcrumb, the title with whether the changes are saved, the summary and the tabs.
fn head(ctx: &mut Ctx, project: &ShownProject, (setup, save): (Option<&OrchestratorSetup>, &Save)) -> Element {
  let id = project.open.project.id();
  let name = project.open.project.name().as_str();
  let (overview, chat) = (Navigation::of(ctx), Navigation::of(ctx));
  let crumbs = breadcrumb(
    ctx,
    "orchestrator-settings-crumbs",
    CrumbSeparator::Slash,
    vec![
      Crumb::link(name, move || {
        overview.go(Route::Project(id, ProjectPage::Overview(Default::default())))
      }),
      Crumb::link(ctx.t("orchestrator.title"), move || {
        chat.go(Route::Project(id, ProjectPage::Orchestrator(OrchestratorChat::Latest)));
      }),
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
    .with_children(setup.is_some().then(|| save_state(ctx, save)));
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
    .child(summary(ctx, setup.map(OrchestratorSetup::readiness).as_ref()))
    .child(tabs)
    .into()
}

/// Whether the changes are saved: being saved, saved, or refused and why.
fn save_state(ctx: &mut Ctx, save: &Save) -> Element {
  let state = save.state().get();
  if let Some(error) = &state.error {
    let why = message_text(ctx, error);
    return field_error(
      "orchestrator-settings-save-error",
      &ctx.t_args("orchestrator.settings.saveFailed", [("reason", &*why)]),
    )
    .into();
  }
  let (glyph, words) = if state.is_pending() {
    (Icon::LoaderCircle, ctx.t("orchestrator.settings.saving"))
  } else {
    (Icon::Check, ctx.t("orchestrator.settings.saved"))
  };
  Row::new()
    .id("orchestrator-settings-saved")
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&words)
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
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
