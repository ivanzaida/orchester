//! Approvals' rows (`Auto mode` `WSQqT` in `02 · Runs`, as `m0eUB` redraws it): *Allow Autopilot for agents in this
//! project*, Manual's reviewer's connection, model and effort as three selects with the domain's effective-reviewer
//! read-back in the row's (i) and, in its hint's place in `warning` while the reviewer cannot run, why (ORC-611), and
//! *Approved for every run*.
use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Row, SelectOption},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::project_settings::{self, Settings};

use super::{AutoMode, Props};
use crate::{
  screens::project_settings::{
    kit::{Group, SettingRow, Tone},
    work::Save,
  },
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    choice_field::{Choice, ChoiceField},
    field::{FieldInput, SelectField},
    icon::Icon,
    message::message_text,
  },
};

/// The reviewer's selects (`Claude · Work` `d5VoC`, `Opus 5.5` `jEztM`, `Effort · Medium` `mqIoa` in Approvals 02c
/// `m0eUB`).
const CONNECTION_WIDTH: f32 = 150.0;
/// Model fields of decision reviewers (`04 b/c`).
const REMOTE_MODEL_WIDTH: f32 = 120.0;
const LOCAL_MODEL_WIDTH: f32 = 206.0;
const MODEL_WIDTH: f32 = 156.0;
const EFFORT_WIDTH: f32 = 120.0;

/// The group, its Reviewer row saying why the reviewer cannot run (`cannot_run`) while it cannot.
pub fn group(
  ctx: &mut Ctx,
  (tab, props): (&AutoMode, &Props),
  (settings, cannot_run): (&Settings, Option<Arc<str>>),
  save: &Save,
) -> Group {
  let options = project_settings::review_options(settings, &tab.connection.get(), &tab.model.get());
  let busy = save.state().get().is_pending();
  let autopilot = super::autopilot::row(ctx, (tab, props), settings, (save, busy));
  let reviewer = if options.connections.is_empty() && options.decisions.is_empty() {
    unavailable_reviewer(ctx, props)
  } else {
    let busy = busy || tab.check.state().get().is_pending();
    reviewer_row(ctx, (tab, props), (options, cannot_run), busy)
  };
  let rules = super::rules::row(ctx, props, settings);
  Group::new("settings-auto", ctx.t("project_settings.auto"))
    .row(autopilot)
    .row(reviewer)
    .row(rules)
    .foot(&ctx.t("project_settings.approvalsFoot"))
}
fn unavailable_reviewer(ctx: &mut Ctx, props: &Props) -> Element {
  let nav = crate::navigation::Navigation::of(ctx);
  let project = props.tab.project.open.project.id();
  let open = ActionButton::new(
    "settings-auto-open-connections",
    ButtonKind::Secondary,
    ctx.t("project_settings.openConnections"),
  )
  .icon(Icon::Plug)
  .in_form()
  .opens(nav.to(crate::routes::Route::Project(
    project,
    crate::routes::ProjectPage::Connections(crate::routes::ConnectionsView::default()),
  )));
  SettingRow::new("settings-auto-reviewer", ctx.t("project_settings.reviewer"))
    .info(ctx.t("project_settings.reviewerDetail"))
    .toned(ctx.t("project_settings.reviewerUnavailable"), Tone::Warning)
    .control(open)
    .render(ctx)
}
fn reviewer_row(
  ctx: &mut Ctx,
  (tab, props): (&AutoMode, &Props),
  (options, cannot_run): (project_settings::ReviewOptions, Option<Arc<str>>),
  busy: bool,
) -> Element {
  let settings = tab.stored.get().0;
  let decision = tab.selected_decision(&settings);
  let connection = connection_select(ctx, tab, &options, busy);
  let row = reviewer_label(ctx, (&options, cannot_run), decision.is_some());
  // A model is named by the id its CLI takes, as its developer names it (the person's decision for ORC-103).
  let model = select(
    ctx,
    (
      "settings-reviewer-model",
      "project_settings.model",
      decision.map_or(MODEL_WIDTH, |c| {
        if c.is_on_this_computer() {
          LOCAL_MODEL_WIDTH
        } else {
          REMOTE_MODEL_WIDTH
        }
      }),
    ),
    &tab.model,
    options
      .models
      .into_iter()
      .map(|model| {
        let label = model.as_str().into();
        (model, label)
      })
      .collect(),
    busy,
  );
  let effort = if decision.is_some() {
    super::decision::threshold_field(ctx, tab, busy)
  } else {
    select(
      ctx,
      ("settings-reviewer-effort", "project_settings.effort", EFFORT_WIDTH),
      &tab.effort,
      std::iter::once((String::new(), ctx.t("project_settings.defaultEffort")))
        .chain(
          options
            .efforts
            .into_iter()
            .map(|effort| (effort.clone(), effort.into())),
        )
        .collect(),
      busy,
    )
  };
  let selects = Row::new()
    .id("settings-reviewer-selects")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(connection)
    .child(model)
    .child(effort);
  let row = row.control(selects);
  let row = if let Some(question) = tab.hosted_question(ctx, props) {
    row.under(question)
  } else if let Some(connection) = decision {
    row.under(super::decision::destination_line(ctx, tab, props, connection))
  } else {
    row
  };
  row.render(ctx)
}

fn default_connection_label(ctx: &Ctx, name: Option<&str>) -> Arc<str> {
  name
    .map(|name| ctx.t_args("project_settings.defaultConnection", [("connection", name)]))
    .unwrap_or_else(|| ctx.t("project_settings.default"))
}

/// The reviewer's connection (`kGBHZ`): the default, the connections a reviewer runs on, and those on an API provider
/// listed faded with a lock and why, the reason wrapped, as the profile's connection menu lists them. Locked while a
/// save is under way.
fn connection_select(ctx: &mut Ctx, tab: &AutoMode, options: &project_settings::ReviewOptions, busy: bool) -> Element {
  let default = default_connection_label(ctx, options.default_connection.as_deref());
  if busy {
    return locked_connection(ctx, tab, options, default);
  }
  let choice = |value: String, label: Arc<str>, unavailable: Option<Arc<str>>| Choice {
    value,
    label,
    icon: Icon::Bot,
    detail: None,
    separator: false,
    warning: false,
    trigger: None,
    unavailable,
  };
  let reason = ctx.t("project_settings.reviewerApiProvider");
  let mut choices: Vec<_> = std::iter::once(choice(String::new(), default, None))
    .chain(options.connections.iter().map(|(id, name)| {
      let mut option = choice(id.clone(), name.as_str().into(), None);
      option.detail = Some(project_settings::default_model(&tab.stored.get().0, id).into());
      option
    }))
    .chain(
      options
        .on_providers
        .iter()
        .map(|(id, name)| choice(id.clone(), name.as_str().into(), Some(reason.clone()))),
    )
    .collect();
  let decisions_at = choices.len();
  choices.extend(options.decisions.iter().map(|connection| {
    let mut option = choice(connection.id().to_string(), connection.name().as_str().into(), None);
    option.icon = if connection.is_on_this_computer() {
      Icon::Monitor
    } else {
      Icon::Scale
    };
    option.detail = Some(connection.base_url().host().into());
    option
  }));
  let mut add = choice(
    "add-decision".into(),
    ctx.t("project_settings.decision.add"),
    Some(ctx.t("project_settings.decision.addUnavailable")),
  );
  add.separator = true;
  add.icon = Icon::Plus;
  choices.push(add);
  let headings = vec![
    (0, ctx.t("project_settings.decision.languages")),
    (decisions_at, ctx.t("project_settings.decision.decisions")),
  ];
  ChoiceField {
    id: "settings-reviewer-connection".into(),
    name: ctx.t("project_settings.connection"),
    value: tab.connection.clone(),
    open: tab.connection_open.clone(),
    choices,
    width: CONNECTION_WIDTH,
    icon_in_field: true,
    placeholder: None,
    mono: false,
  }
  .render_grouped(ctx, headings)
}

fn select(
  ctx: &mut Ctx,
  (id, key, width): (&str, &str, f32),
  value: &Signal<String>,
  options: Vec<(String, Arc<str>)>,
  locked: bool,
) -> Element {
  if locked {
    let text = options
      .iter()
      .find(|(option, _)| *option == value.get_untracked())
      .map(|(_, label)| label.to_string())
      .unwrap_or_else(|| value.get_untracked());
    FieldInput::new(id.to_owned(), ctx.t(key), Signal::new(text))
      .locked()
      .width(Dimension::Px(width))
      .render(ctx)
  } else {
    let field = SelectField::new(
      id.to_owned(),
      ctx.t(key),
      value.clone(),
      options
        .into_iter()
        .map(|(value, label)| SelectOption::new(value, label))
        .collect(),
    )
    .width(Dimension::Px(width));
    let field = if id == "settings-reviewer-model" {
      field.mono()
    } else {
      field
    };
    field.render(ctx)
  }
}

fn reviewer_label(
  ctx: &Ctx,
  (options, cannot_run): (&project_settings::ReviewOptions, Option<Arc<str>>),
  decision: bool,
) -> SettingRow {
  let row = SettingRow::new("settings-auto-reviewer", ctx.t("project_settings.reviewer"));
  if decision {
    return row.hint(ctx.t("project_settings.reviewerHint"));
  }
  let detail = ctx.t("project_settings.reviewerDetail");
  match &options.resolved {
    Ok(shown) => {
      let readback = ctx.t_args(
        "project_settings.reviewedBy",
        [("model", shown.model.as_str()), ("connection", &shown.connection)],
      );
      let row = row.info(ctx.t_args(
        "project_settings.reviewerInfo",
        [("detail", &*detail), ("readback", &*readback)],
      ));
      // Why the reviewer cannot run, in the hint's place while it cannot.
      match cannot_run {
        Some(said) => row.toned(said, Tone::Warning),
        None => row.hint(ctx.t("project_settings.reviewerHint")),
      }
    }
    Err(why) => row.info(detail).toned(message_text(ctx, why), Tone::Warning),
  }
}

fn locked_connection(
  ctx: &mut Ctx,
  tab: &AutoMode,
  options: &project_settings::ReviewOptions,
  default: Arc<str>,
) -> Element {
  let connections = std::iter::once((String::new(), default))
    .chain(
      options
        .connections
        .iter()
        .map(|(id, name)| (id.clone(), name.as_str().into())),
    )
    .chain(
      options
        .decisions
        .iter()
        .map(|c| (c.id().to_string(), c.name().as_str().into())),
    )
    .collect();
  select(
    ctx,
    (
      "settings-reviewer-connection",
      "project_settings.connection",
      CONNECTION_WIDTH,
    ),
    &tab.connection,
    connections,
    true,
  )
}
