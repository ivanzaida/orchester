//! Estimates (`Estimates` `vLiTy` in `02 · Runs`, the Runs window `g406O`): *Let Orchester estimate story points*, the
//! project's own setting since ORC-103; opening a project that had none copied the application-wide value into it
//! (ORC-595), and the row shows only the project's. Orchester's estimator then sizes new tasks, and tasks whose brief
//! or criteria change, on auto mode's reviewer's language model, or the default language reviewer for a decision
//! reviewer, never changing points a person set. It is read through its query, which the project's loader read as the
//! project opened, and saved off the UI thread; until it is first read, while it saves, and when no language reviewer
//! is available, its switch cannot be flipped. Only a person flips it: the switch refuses lurq's synthetic input, an
//! MCP client's included, and says so under the row. Unlike the application-wide Automation section it moved from, the
//! row does not close the whole window to MCP input while it shows, so Runs' other settings stay open to automation
//! as before. While estimates are on and the estimator's last request could not run, the row says why in its hint's
//! place, in `warning` (ORC-611), instead of only in the log.

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
  node::Element,
};
use orchester_core::time::Timestamp;
use orchester_workspace::{
  background,
  messages::{self, Message},
};

use super::super::{
  TabProps,
  kit::{Group, SettingRow, Tone},
};
use crate::{
  mcp::input_gate::{InputGate, persons_press},
  queries::{self, Session, project},
  screens::reviewer_trouble,
  services::Services,
  ui::{
    field::field_error,
    message::message_text,
    skeleton::{self, Skeleton},
    toggle::{Toggle, ToggleKind},
  },
};

/// The switch's element id.
const ID: &str = "settings-estimate-points";

type SaveAction = FutureAction<bool, bool, Message>;

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub tab: TabProps,
  pub settings: Option<orchester_workspace::project_settings::Settings>,
  pub read_error: Option<Message>,
}

impl lurq::app::component::DevtoolsInspectable for Props {}
pub(super) struct Estimates {
  /// What the person last chose; `None` until they do.
  saved: Signal<Option<bool>>,
  save: SaveAction,
  gate: Option<InputGate>,
}

impl Component for Estimates {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let workspace = Services::of(ctx).workspace;
    let root = ctx.props::<Props>().tab.project.open.repo_root.clone();
    // The workspace counts the change once saved, which reads the setting again (`queries`).
    let save = ctx.future_action(move |allowed: bool| {
      let (workspace, root) = (workspace.clone(), root.clone());
      async move {
        background::run(move || workspace.set_estimates_story_points(&root, allowed))
          .await?
          .map(|()| allowed)
          .map_err(|error| {
            tracing::error!(%error, "Let Orchester estimate story points was not saved");
            messages::store_error(&error)
          })
      }
    });
    Self {
      saved: ctx.signal(None),
      save,
      gate: ctx.use_context::<InputGate>(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let (hint, info, available) = estimate_words(ctx, props.settings.as_ref());
    let state = self.save.state().get();
    let root = props.tab.project.open.repo_root.clone();
    let session = Session::of(ctx);
    let read = queries::state(&ctx.query(project::estimates(session, root.clone())));
    let fixed = state.is_pending() || read.is_pending();
    // The person's choice until the stored value is it again; the stored value until they choose, or once a save
    // failed.
    queries::settle(&self.saved, read.data.as_ref(), state.is_pending());
    let shown = match (&state.error, self.saved.get()) {
      (None, Some(chosen)) => Some(chosen),
      _ => read.data,
    };
    let switch = Toggle::new(
      ID,
      ToggleKind::Switch,
      shown.unwrap_or(false),
      ctx.t("project_settings.estimates.label"),
    )
    .hide_label();
    let switch = if shown.is_some() && !fixed && available {
      self.listen(switch)
    } else {
      switch
    };
    let problem = match (props.read_error.as_ref().or(read.error.as_ref()), &state.error) {
      (Some(reason), _) => Some(("project_settings.estimates.unreadable", reason)),
      (None, Some(reason)) => Some(("project_settings.estimates.notSaved", reason)),
      (None, None) => None,
    }
    .map(|(key, reason)| {
      let reason = message_text(ctx, reason);
      Element::from(field_error(
        &format!("{ID}-error"),
        &ctx.t_args(key, [("reason", &*reason)]),
      ))
    });
    let row = SettingRow::new(ID, ctx.t("project_settings.estimates.label"));
    // Why the last estimate gave no points, in the hint's place while estimates are on.
    let row = match could_not_run(ctx, &root, shown) {
      Some(said) => row.toned(said, Tone::Warning),
      None => row.hint(hint),
    };
    // Until the setting is read, the switch is a bone of its size: no state shows that may not be the stored one.
    let control: Element = if shown.is_none() && read.is_pending() {
      Skeleton::new(format!("{ID}-reading"), ctx.t("project_settings.reading"))
        .inline()
        .show(ctx, skeleton::switch())
    } else {
      switch.into()
    };
    let row = row.info(info).control(control).under_all(problem).render(ctx);
    Element::from(Group::new("settings-estimates", ctx.t("project_settings.estimates.title")).row(row))
  }
}

impl Estimates {
  /// `switch`, saving what a person chooses; synthetic input never changes it, and is refused to the client that sent
  /// it.
  fn listen(&self, switch: Toggle) -> Toggle {
    let (saved, gate, save) = (self.saved.clone(), self.gate.clone(), self.save.clone());
    switch.on_change(move |chosen| {
      if !persons_press(gate.as_ref(), "change whether Orchester estimates story points") {
        return;
      }
      saved.set(Some(chosen));
      save.run(chosen);
    })
  }
}

/// Why the estimator's last request on the reviewer gave no points (ORC-611), while estimates are on (`shown`); read
/// when the row shows and again as the switch changes.
fn could_not_run(ctx: &mut Ctx, root: &Path, shown: Option<bool>) -> Option<Arc<str>> {
  let health = reviewer_trouble::health(ctx, root.to_path_buf(), format!("{shown:?}"));
  let now = Timestamp::now();
  let trouble = health.filter(|_| shown == Some(true))?.estimates(now)?;
  let reason = reviewer_trouble::reason(ctx, trouble, now);
  Some(ctx.t_args("project_settings.estimates.couldNotRun", [("reason", &*reason)]))
}

fn estimate_words(
  ctx: &Ctx,
  settings: Option<&orchester_workspace::project_settings::Settings>,
) -> (std::sync::Arc<str>, std::sync::Arc<str>, bool) {
  use orchester_core::run::review::{ReviewerConnectionId, resolve_estimator};
  let Some(settings) = settings else {
    return (
      ctx.t("project_settings.estimates.hint"),
      ctx.t("project_settings.estimates.info"),
      false,
    );
  };
  if !matches!(
    settings.auto_mode.reviewer.as_ref().map(|choice| choice.connection),
    Some(ReviewerConnectionId::Decision { .. })
  ) {
    return (
      ctx.t("project_settings.estimates.hint"),
      ctx.t("project_settings.estimates.info"),
      true,
    );
  }
  match resolve_estimator(&settings.auto_mode, &settings.connections) {
    Ok(estimator) => (
      ctx.t_args(
        "project_settings.estimates.decisionHint",
        [("connection", estimator.connection.name().as_str())],
      ),
      ctx.t_args(
        "project_settings.estimates.decisionInfo",
        [
          ("connection", estimator.connection.name().as_str()),
          ("model", estimator.model.as_str()),
        ],
      ),
      true,
    ),
    Err(_) => (
      ctx.t("project_settings.estimates.noLanguage"),
      ctx.t("project_settings.estimates.noLanguage"),
      false,
    ),
  }
}
