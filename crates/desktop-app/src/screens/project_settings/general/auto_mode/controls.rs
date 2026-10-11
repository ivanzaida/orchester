//! The auto mode switch, reviewer selects and the domain's effective-reviewer readback.
use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, SelectOption},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::project_settings::{self, Settings};

use super::{AutoMode, Props};
use crate::{
  screens::project_settings::{page, work::Save},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, SelectField, field},
    form_section::{Place, section as form_section},
    icon::Icon,
    message::message_text,
    toggle::{Toggle, ToggleKind},
    unavailable::reason_line,
  },
};

pub fn section(ctx: &mut Ctx, tab: &AutoMode, props: &Props, settings: &Settings, save: &Save) -> Element {
  let options = project_settings::review_options(settings, &tab.connection.get(), &tab.model.get());
  let busy = save.state().get().is_pending() || tab.asking.get().0.is_some();
  let on_row = switch_row(ctx, tab, props, settings, save, (options.resolved.as_ref().err(), busy));
  let reviewer = if options.connections.is_empty() {
    unavailable_reviewer(ctx, props)
  } else {
    reviewer_row(ctx, tab, options, busy)
  };
  form_section(
    "settings-auto",
    &ctx.t("project_settings.auto"),
    None,
    vec![on_row, reviewer],
  )
  .into()
}
fn switch_row(
  ctx: &Ctx,
  tab: &AutoMode,
  props: &Props,
  settings: &Settings,
  save: &Save,
  (unavailable, busy): (Option<&orchester_workspace::messages::Message>, bool),
) -> Element {
  let on = settings.auto_mode.new_runs;
  let mut toggle = Toggle::new(
    "settings-auto-new-runs",
    ToggleKind::Switch,
    on,
    ctx.t(if on {
      "project_settings.on"
    } else {
      "project_settings.off"
    }),
  );
  if !busy && (on || unavailable.is_none()) {
    let (tab, props, save, base) = (tab.clone(), props.clone(), save.clone(), settings.auto_mode.clone());
    toggle = toggle.on_change(move |chosen| {
      let mut asked = base.clone();
      asked.new_runs = chosen;
      tab.request(&props, &save, asked);
    });
  }
  let control = Column::new()
    .width(Dimension::Px(page::CONTROL_WIDTH))
    .spacing(SpacingSize::Sm)
    .child(toggle);
  page::row(
    ctx,
    "settings-auto-on",
    ("project_settings.newRuns", "project_settings.autoDetail"),
    control.into(),
    Place::Within,
  )
}
fn unavailable_reviewer(ctx: &mut Ctx, props: &Props) -> Element {
  let nav = crate::navigation::Navigation::of(ctx);
  let project = props.tab.project.open.project.id();
  let control = Column::new()
    .width(Dimension::Px(page::CONTROL_WIDTH))
    .spacing(crate::theme::ExtraSpacing::Snug.role())
    .child(reason_line(
      "settings-auto-unavailable",
      &ctx.t("project_settings.reviewerUnavailable"),
    ))
    .child(
      ActionButton::new(
        "settings-auto-open-connections",
        ButtonKind::Secondary,
        ctx.t("project_settings.openConnections"),
      )
      .icon(Icon::Plug)
      .in_form()
      .on_click(move || {
        nav.go(crate::routes::Route::Project(
          project,
          crate::routes::ProjectPage::Connections(crate::routes::ConnectionsView::default()),
        ));
      }),
    );
  page::row(
    ctx,
    "settings-auto-reviewer",
    ("project_settings.reviewer", "project_settings.reviewerDetail"),
    control.into(),
    Place::Last,
  )
}
fn reviewer_row(ctx: &mut Ctx, tab: &AutoMode, options: project_settings::ReviewOptions, busy: bool) -> Element {
  let default = default_connection_label(ctx, options.default_connection.as_deref());
  let connections = std::iter::once((String::new(), default))
    .chain(options.connections.into_iter().map(|(id, name)| (id, name.into())))
    .collect();
  let connection = select(
    ctx,
    "settings-reviewer-connection",
    "project_settings.connection",
    &tab.connection,
    connections,
    busy,
  );
  let model = select(
    ctx,
    "settings-reviewer-model",
    "project_settings.model",
    &tab.model,
    options
      .models
      .into_iter()
      .map(|model| (model.clone(), model_label(ctx, &model)))
      .collect(),
    busy,
  );
  let effort = select(
    ctx,
    "settings-reviewer-effort",
    "project_settings.effort",
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
  );
  let note = match options.resolved {
    Ok(shown) => ctx.t_args(
      "project_settings.reviewedBy",
      [
        ("model", model_label(ctx, shown.model.as_str()).as_ref()),
        ("connection", &shown.connection),
      ],
    ),
    Err(why) => message_text(ctx, &why),
  };
  let controls = Column::new()
    .width(Dimension::Px(page::CONTROL_WIDTH))
    .spacing(SpacingSize::Sm)
    .child(field(
      "settings-reviewer-connection-label",
      Some(&ctx.t("project_settings.connection")),
      connection,
      None,
    ))
    .child(field(
      "settings-reviewer-model-label",
      Some(&ctx.t("project_settings.model")),
      model,
      None,
    ))
    .child(field(
      "settings-reviewer-effort-label",
      Some(&ctx.t("project_settings.effort")),
      effort,
      None,
    ))
    .child(page::note("settings-reviewer-readback", &note));
  page::row(
    ctx,
    "settings-auto-reviewer",
    ("project_settings.reviewer", "project_settings.reviewerDetail"),
    controls.into(),
    Place::Last,
  )
}

fn default_connection_label(ctx: &Ctx, name: Option<&str>) -> Arc<str> {
  name
    .map(|name| ctx.t_args("project_settings.defaultConnection", [("connection", name)]))
    .unwrap_or_else(|| ctx.t("project_settings.default"))
}

fn model_label(ctx: &Ctx, model: &str) -> Arc<str> {
  let key = match model {
    "claude-haiku-4-5-20251001" => "project_settings.haiku45",
    "claude-sonnet-5-5" => "project_settings.sonnet55",
    "claude-opus-5-5" => "project_settings.opus55",
    "claude-fable-5-1" => "project_settings.fable51",
    _ => return model.into(),
  };
  ctx.t(key)
}

fn select(
  ctx: &mut Ctx,
  id: &str,
  key: &str,
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
      .render(ctx)
  } else {
    SelectField::new(
      id.to_owned(),
      ctx.t(key),
      value.clone(),
      options
        .into_iter()
        .map(|(value, label)| SelectOption::new(value, label))
        .collect(),
    )
    .render(ctx)
  }
}
