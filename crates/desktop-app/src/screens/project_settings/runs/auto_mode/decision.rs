//! Decision reviewer controls and destination read-back (03 noZlm and 04 c–e).
use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    theme::PaletteColor,
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{DecisionCheckResult, DecisionConnection},
  resource::ModelId,
};
use orchester_workspace::{
  background,
  connections::decision,
  messages::{self, Message},
  project_settings::{self, Settings},
  workspace::OpenProject,
};

use super::{AutoMode, Props, Stored};
use crate::{
  navigation::Navigation,
  routes::{ConnectionsView, ProjectPage, Route},
  screens::project_settings::Status,
  services::Services,
  theme::{ExtraSpacing, ProjectSettingsText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    icon::{Icon, icon},
    message::message_text,
  },
};

/// The composed threshold field in 04 c (Ivnd4).
const THRESHOLD_WIDTH: f32 = 192.0;
pub(super) type Check = FutureAction<(OpenProject, DecisionConnection, Option<ModelId>), Stored, Message>;

pub(super) fn check_action(ctx: &mut Ctx) -> Check {
  let Services {
    vault,
    activity,
    changes,
    ..
  } = Services::of(ctx);
  ctx.future_action(
    move |(open, connection, model): (OpenProject, DecisionConnection, Option<ModelId>)| {
      let (vault, activity, changes) = (vault.clone(), activity.clone(), changes.clone());
      async move {
        activity
          .track(
            orchester_workspace::activity::operations::connection_check(&open, connection.name().as_str()),
            async {
              decision::check_in(&open.repo_root, vault.get(), connection.id(), model.as_ref())
                .await
                .map_err(|e| e.message())?;
              changes.record(
                open.project.id(),
                orchester_workspace::tasks::outside::Outside::Connections,
              );
              background::run(move || {
                project_settings::load_in(&open.repo_root)
                  .map(Stored)
                  .map_err(|e| messages::store_error(&e))
              })
              .await?
            },
          )
          .await
      }
    },
  )
}

impl AutoMode {
  pub(super) fn selected_decision<'a>(&self, settings: &'a Settings) -> Option<&'a DecisionConnection> {
    settings
      .decision_connections
      .iter()
      .find(|c| c.id().to_string() == self.connection.get())
  }

  pub(super) fn follow_check(&self, ctx: &mut Ctx) {
    let (tab, props) = (self.clone(), ctx.props::<Props>().clone());
    ctx.watch(&self.check.state(), move |state| {
      if let Some(Stored(settings)) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
        tab.refresh(settings, false);
        // Runs shows what the check read until the settings' query has it, as after a save.
        props.kept.keep(super::work::Read(settings.clone()));
        props.tab.project.feed.saved();
      }
    });
  }
}

pub(super) fn threshold_field(ctx: &mut Ctx, tab: &AutoMode, busy: bool) -> Element {
  let (typing, authority, leaving) = (tab.threshold_editing.clone(), tab.clone(), tab.clone());
  let status = ctx.props::<Props>().tab.status.clone();
  let mut field = FieldInput::new(
    "settings-reviewer-threshold",
    ctx.t("project_settings.decision.threshold"),
    tab.threshold.clone(),
  )
  .prefix(ctx.t("project_settings.decision.prefix"))
  .unit(ctx.t("project_settings.decision.suffix"))
  .mono()
  .width(Dimension::Px(THRESHOLD_WIDTH))
  .on_typed(move |_| {
    authority.note_input();
    typing.set(true);
  })
  .on_leave(move || {
    if let Ok(value) = project_settings::decision_reviewer::threshold(&leaving.threshold.get_untracked()) {
      leaving.threshold.set(format!("{:.2}", value.value()));
    }
    leaving.threshold_editing.set(false);
    if !leaving.dirty() {
      status.set(Status::Saved);
    }
  });
  if busy {
    field = field.locked();
  }
  if project_settings::decision_reviewer::threshold(&tab.threshold.get()).is_err() {
    field = field.error(ctx.t("project_settings.decision.invalidThreshold"));
  }
  field.render(ctx)
}

pub(super) fn destination_line(
  ctx: &mut Ctx,
  tab: &AutoMode,
  props: &Props,
  connection: &DecisionConnection,
) -> Element {
  let mut draft = tab.stored.get().0;
  if let Some(asked) = tab.draft() {
    draft.auto_mode = asked;
  }
  let row = project_settings::decision_reviewer::row(&draft.auto_mode, &draft.connections, &draft.decision_connections);
  let failed = row.unavailable.is_some();
  let checking = tab.check.state().get();
  let (text, color, glyph) = destination_status(
    ctx,
    connection,
    &row,
    checking.error.as_ref(),
    tab.threshold_editing.get(),
  );
  if tab.threshold_editing.get() && row.last_test.is_some() && !failed {
    return Text::new(&text)
      .id("settings-reviewer-destination")
      .variant(ProjectSettingsText::Hint.role())
      .width(Dimension::Pct(100.0))
      .into();
  }
  let action = if failed || checking.error.is_some() {
    let (check, open, connection, model) = (
      tab.check.clone(),
      props.tab.project.open.clone(),
      connection.clone(),
      row.model,
    );
    let button = ActionButton::new(
      "settings-reviewer-check",
      ButtonKind::Ghost,
      ctx.t("project_settings.decision.check"),
    )
    .small()
    .tight()
    .in_form()
    .on_click(move || check.run((open.clone(), connection.clone(), model.clone())));
    if checking.is_pending() {
      button.loading()
    } else {
      button
    }
  } else {
    ActionButton::new(
      "settings-reviewer-test",
      ButtonKind::Ghost,
      ctx.t("project_settings.decision.test"),
    )
    .small()
    .tight()
    .in_form()
    .opens(Navigation::of(ctx).to(Route::Project(
      props.tab.project.open.project.id(),
      ProjectPage::Connections(ConnectionsView::decision(connection.id())),
    )))
  };
  if tab.threshold_editing.get() {
    props.tab.status.set(Status::Unsaved);
  }
  Row::new()
    .id("settings-reviewer-destination")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, crate::theme::IconSize::Sm, color.clone()))
    .child(
      Text::new(&text)
        .variant(ProjectSettingsText::Hint.role())
        .color(color)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(action)
    .into()
}

fn destination_text(
  ctx: &Ctx,
  connection: &DecisionConnection,
  figure: Option<&orchester_core::connection::DecisionThresholdFigures>,
  editing: bool,
) -> std::sync::Arc<str> {
  if let Some(figure) = figure {
    let percent = figure
      .approves_on_its_own
      .saturating_mul(100)
      .checked_div(figure.requests_tested)
      .unwrap_or(0);
    if !editing {
      let key = if connection.is_on_this_computer() {
        "project_settings.decision.localTested"
      } else {
        "project_settings.decision.remoteTested"
      };
      return ctx.t_args(
        key,
        [
          ("host", connection.base_url().host().to_owned()),
          ("threshold", format!("{:.2}", figure.threshold.value())),
          ("percent", percent.to_string()),
        ],
      );
    }
    let separator = ctx.t("project_settings.groupSeparator");
    return ctx.t_args(
      "project_settings.decision.figure",
      [
        ("threshold", format!("{:.2}", figure.threshold.value())),
        ("percent", percent.to_string()),
        (
          "person",
          orchester_core::tokens::grouped(figure.comes_to_person, &separator),
        ),
        (
          "total",
          orchester_core::tokens::grouped(figure.requests_tested, &separator),
        ),
      ],
    );
  }
  if connection.is_on_this_computer() {
    ctx.t("project_settings.decision.local")
  } else {
    ctx.t_args(
      "project_settings.decision.remote",
      [("host", connection.base_url().host())],
    )
  }
}

fn destination_status(
  ctx: &Ctx,
  connection: &DecisionConnection,
  row: &project_settings::decision_reviewer::ReviewerRow,
  error: Option<&Message>,
  editing: bool,
) -> (std::sync::Arc<str>, PaletteColor, Icon) {
  if let Some(error) = error {
    return (message_text(ctx, error), PaletteColor::Warning, Icon::TriangleAlert);
  }
  if matches!(row.last_check, Some(DecisionCheckResult::NothingAnswers)) {
    let address = connection
      .base_url()
      .as_str()
      .split_once("://")
      .map_or(connection.base_url().as_str(), |(_, rest)| rest)
      .split('/')
      .next()
      .expect("a validated server URL has an authority");
    return (
      ctx.t_args("project_settings.decision.nothingAnswers", [("address", address)]),
      PaletteColor::Warning,
      Icon::TriangleAlert,
    );
  }
  if row.unavailable.is_some() {
    return (
      ctx.t("project_settings.decision.unavailable"),
      PaletteColor::Warning,
      Icon::TriangleAlert,
    );
  }
  (
    destination_text(ctx, connection, row.last_test.as_ref(), editing),
    PaletteColor::TextMuted,
    if connection.is_on_this_computer() {
      Icon::ShieldCheck
    } else {
      Icon::Send
    },
  )
}
