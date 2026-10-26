//! What the runs-on picker chose while it is open, and publishing it: the connection, its model and the reasoning
//! effort, filled from the published version when the picker opens and, for a connection chosen anew, with what is
//! kept on it or its first model and the CLI's own effort, as the settings page fills them. The model is one the
//! connection lists, typed for an API provider; the effort one it lists for the model (its other models' for a model it
//! does not list), *Default* first. Orchester cannot tell whether a provider takes an effort it does not list, so the
//! picker offers no field for one of the person's own; with no efforts the row is not shown. *Publish vN* appears once
//! the choice differs from the published version: it publishes the choice as the settings page's *Publish* does, and
//! says so when it replaces a draft saved there.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ConnectionId, orchestrator::OrchestratorSettings, resource::ModelId};
use orchester_workspace::orchestrator::settings::{Keep, OrchestratorSetup, Saving};

use super::{
  Publish, RunsOnProps,
  select::{Choice, select},
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  screens::orchestrator::settings::effort_word,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    pressable::opening,
    tinted_note::tinted_note,
  },
};

/// The picker's choice while it is open.
#[derive(Clone)]
pub(super) struct Pending {
  connection: Signal<String>,
  model: Signal<String>,
  effort: Signal<String>,
  model_open: Signal<bool>,
  effort_open: Signal<bool>,
  /// The choice was filled since the picker opened.
  seeded: Signal<bool>,
}

impl Pending {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      connection: ctx.signal(String::new()),
      model: ctx.signal(String::new()),
      effort: ctx.signal(String::new()),
      model_open: ctx.signal(false),
      effort_open: ctx.signal(false),
      seeded: ctx.signal(false),
    }
  }

  /// The picker closed: it fills its choice anew when it opens again.
  pub(super) fn forget(&self) {
    if self.seeded.get_untracked() {
      self.seeded.set(false);
      self.model_open.set(false);
      self.effort_open.set(false);
    }
  }

  /// Fills the choice from the published version, else the first connection the Orchestrator can run on, once since
  /// the picker opened.
  pub(super) fn seed(&self, setup: &OrchestratorSetup) {
    if self.seeded.get_untracked() {
      return;
    }
    let shown = setup
      .settings
      .clone()
      .or_else(|| setup.first_usable().and_then(|connection| setup.choosing(connection)));
    self.show(shown.as_ref());
    self.seeded.set(true);
  }

  fn show(&self, settings: Option<&OrchestratorSettings>) {
    let connection = settings.map(|settings| settings.connection().to_string()).unwrap_or_default();
    if self.connection.get_untracked() != connection {
      self.connection.set(connection);
    }
    self
      .model
      .set(settings.map(|settings| settings.model().as_str().to_owned()).unwrap_or_default());
    self.effort.set(
      settings
        .and_then(OrchestratorSettings::effort)
        .unwrap_or_default()
        .to_owned(),
    );
  }

  /// What choosing a connection's row does: it takes the connection with what is kept on it, else its first model and
  /// the CLI's own effort.
  pub(super) fn chooser(&self, setup: &OrchestratorSetup) -> Arc<dyn Fn(ConnectionId) + Send + Sync> {
    let (pending, setup) = (self.clone(), setup.clone());
    Arc::new(move |connection| {
      if pending.connection() == Some(connection) {
        return;
      }
      let settings = setup
        .settings
        .clone()
        .filter(|settings| settings.connection() == connection)
        .or_else(|| setup.choosing(connection));
      match settings {
        Some(settings) => pending.show(Some(&settings)),
        None => {
          pending.connection.set(connection.to_string());
          pending.model.set(String::new());
          pending.effort.set(String::new());
        }
      }
    })
  }

  pub(super) fn connection(&self) -> Option<ConnectionId> {
    self.connection.get().parse().ok()
  }

  /// The choice, when it is complete and its connection one the Orchestrator can run on.
  fn chosen(&self, setup: &OrchestratorSetup) -> Option<OrchestratorSettings> {
    let connection = self.connection()?;
    let usable = setup
      .connections()
      .into_iter()
      .any(|choice| choice.id == connection && choice.unavailable.is_none());
    let model = ModelId::new(self.model.get().trim()).ok()?;
    let effort = Some(self.effort.get()).filter(|effort| !effort.is_empty());
    usable.then(|| OrchestratorSettings::new(connection, model, effort))
  }

  /// The model's and the effort's rows for the chosen connection; nothing before one is chosen.
  pub(super) fn choices(&self, ctx: &Ctx, setup: &OrchestratorSetup) -> Vec<Element> {
    let Some(connection) = self.connection() else {
      return Vec::new();
    };
    let current = ModelId::new(&self.model.get()).ok();
    let models = setup
      .models(connection, current.as_ref())
      .into_iter()
      .map(|option| Choice {
        value: option.id.as_str().to_owned(),
        label: Arc::from(option.id.as_str()),
        detail: None,
      })
      .collect();
    // Models by the id the CLI takes, as the developer names them, never a prettified name.
    let model = select(
      "orchestrator-runs-on-model",
      &ctx.t("orchestrator.runsOn.model"),
      (&self.model, &self.model_open),
      (models, ExtraTypography::PropertyMono.role()),
    );
    let efforts = current.as_ref().map(|model| setup.efforts(connection, model)).unwrap_or_default();
    let efforts = if efforts.is_empty() {
      setup.any_efforts(connection)
    } else {
      efforts
    };
    let effort = (!efforts.is_empty()).then(|| {
      let options = std::iter::once(Choice {
        value: String::new(),
        label: ctx.t("orchestrator.runsOn.effortDefault"),
        detail: Some(ctx.t("orchestrator.runsOn.effortDefaultDetail")),
      })
      .chain(efforts.into_iter().map(|effort| Choice {
        detail: (effort == "max").then(|| ctx.t("orchestrator.runsOn.effortMaxDetail")),
        label: effort_word(ctx, &effort),
        value: effort,
      }))
      .collect();
      select(
        "orchestrator-runs-on-effort",
        &ctx.t("orchestrator.runsOn.effort"),
        (&self.effort, &self.effort_open),
        (options, TypographyStyle::Description),
      )
    });
    std::iter::once(model).chain(effort).collect()
  }
}

/// *Publish vN* with what it does, once the choice differs from the published version, and why the last press did not
/// publish; nothing else.
pub(super) fn apply(
  ctx: &Ctx,
  (props, setup): (&RunsOnProps, &OrchestratorSetup),
  pending: &Pending,
  publish: &Publish,
) -> Option<Element> {
  let state = publish.state().get();
  let failed = state.error.as_ref().map(|error| {
    tinted_note(
      "orchestrator-runs-on-error",
      (Icon::CircleAlert, PaletteColor::Danger),
      &ctx.t_args(
        "orchestrator.settings.saveFailed",
        [("reason", &*message_text(ctx, error))],
      ),
    )
  });
  let chosen = pending
    .chosen(setup)
    .filter(|chosen| setup.settings.as_ref() != Some(chosen));
  let Some(chosen) = chosen else {
    return failed.map(Into::into);
  };
  let next = setup.version.map_or(1, |version| version + 1).to_string();
  let note = if setup.draft.is_some() {
    ctx.t_args("orchestrator.runsOn.publishOverDraft", [("version", next.as_str())])
  } else {
    ctx.t_args("orchestrator.runsOn.publishNote", [("version", next.as_str())])
  };
  let button = ActionButton::new(
    "orchestrator-runs-on-publish",
    ButtonKind::Primary,
    ctx.t_args("orchestrator.settings.publish.publishAs", [("version", next.as_str())]),
  )
  .icon(Icon::Upload)
  .small();
  let button = if state.is_pending() {
    button.loading()
  } else {
    let (publish, root) = (publish.clone(), props.root.clone());
    button.on_click(move || {
      publish.run(Saving {
        root: root.clone(),
        settings: chosen.clone(),
        keep: Keep::Publish,
      });
    })
  };
  Some(
    Column::new()
      .id("orchestrator-runs-on-apply")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Sm, SpacingSize::Xs))
      .spacing(ExtraSpacing::Snug.role())
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(
            Text::new(&note)
              .variant(ExtraTypography::Note.role())
              .flex(1.0)
              .min_width(Dimension::Px(0.0)),
          )
          .child(button),
      )
      .with_children(failed)
      .into(),
  )
}

/// The picker's foot: that another account starts a new session, and *Settings ↗*.
pub(super) fn foot(ctx: &mut Ctx, props: &RunsOnProps) -> Element {
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
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    );
  Row::new()
    .id("orchestrator-runs-on-foot")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm)
        .bottom(SpacingSize::Xs),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&ctx.t("orchestrator.runsOn.newSession"))
        .variant(ExtraTypography::Note.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(settings)
    .into()
}
