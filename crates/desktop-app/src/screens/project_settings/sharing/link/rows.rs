//! The groups of the Share page (`04c Share` `JeXKt`): the offer of `orchester.toml`'s address, *Server* (the server
//! with its state), *Project on the server* (the person's projects there, each a radio row, or an address), and the
//! repository compared with the project's hint (`Repository check` `n3lMzy`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, SelectOption, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_api::SharedProject;
use orchester_workspace::sharing::{
  Connection, RemoteMatch, ServerEntry, ServerProject, project::discovery::ProjectDiscovery, remote_match,
};

use super::{Chosen, Link, Listed, Picked, Problem, Props};
use crate::{
  navigation::Navigation,
  routes::{OrchesterView, Route},
  screens::{
    project_settings::{
      kit::{Group, SettingRow, Tone},
      page::CONTROL_WIDTH,
    },
    settings::orchester::{Said, moment, shown_address},
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, OrchesterText},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::{FieldInput, SelectField},
    icon::{Icon, icon},
    pressable::pressable,
    toggle::{Toggle, ToggleKind},
  },
};

impl Link {
  /// The page's groups and what linking says under them.
  pub(super) fn link_section(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    listed: Option<&Result<Listed, Said>>,
  ) -> Vec<Element> {
    let mut parts: Vec<Element> = Vec::new();
    parts.extend(self.discovery_offer(ctx, props));
    let server = self.server_row(ctx, props);
    parts.push(
      Group::new("project-settings-sharing-link", ctx.t("sharing.share.server"))
        .row(server)
        .into(),
    );
    parts.push(self.project_group(ctx, props, listed));
    parts.push(self.repository_line(ctx, props, listed));
    parts.push(self.link_row(ctx, props, listed));
    let resolving = self.resolve.state().get();
    let pending = resolving.is_pending();
    if let Some(Problem::NoProject(address, checked)) = resolving.error.filter(|_| !pending) {
      let again = self.choice(props, listed);
      parts.push(self.not_member(ctx, props, (&address, checked), again));
    }
    parts
  }

  /// *orchester.toml names a project* on a connected server, with *Use this address* and *Not now*; nothing is
  /// contacted before. A file that names a server this computer isn't connected to asks step by step instead.
  fn discovery_offer(&self, ctx: &mut Ctx, props: &Props) -> Option<Element> {
    if let Some(offer) = self.unconnected_offer(ctx, props) {
      return Some(offer);
    }
    let ProjectDiscovery::Project(address) = &props.facts.discovery else {
      return None;
    };
    if !props
      .facts
      .servers
      .iter()
      .any(|entry| entry.address == *address.server())
    {
      return None;
    }
    if self.discovery_answered.get() {
      return None;
    }
    let text = address.to_text();
    let (field_value, answered) = (self.address.clone(), self.discovery_answered.clone());
    let use_it = ActionButton::new(
      "sharing-discovery-use",
      ButtonKind::Primary,
      ctx.t("sharing.link.discovery.use"),
    )
    .on_click(move || {
      field_value.set(text.clone());
      answered.set(true);
    });
    let answered = self.discovery_answered.clone();
    let not_now = ActionButton::new(
      "sharing-discovery-not-now",
      ButtonKind::Ghost,
      ctx.t("sharing.link.discovery.notNow"),
    )
    .on_click(move || answered.set(true));
    // The answers stand under the notice, *Use this address* first (`Discovery file · offered` in `N9NFio`).
    Some(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Md)
        .child(
          Callout::new(
            "sharing-discovery",
            CalloutTone::Info,
            ctx.t("sharing.link.discovery.title"),
          )
          .icon(Icon::FileText)
          .body(ctx.t_args("sharing.link.discovery.body", [("address", address.to_text())])),
        )
        .child(
          Row::new()
            .id("sharing-discovery-answers")
            .spacing(SpacingSize::Sm)
            .child(use_it)
            .child(not_now),
        )
        .into(),
    )
  }

  /// The server the project goes to, with its state; *Connect a server* when this computer has none.
  fn server_row(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let row =
      SettingRow::new("sharing-server", ctx.t("sharing.link.server")).info(ctx.t("sharing.link.serverDescription"));
    if props.facts.servers.is_empty() {
      let navigation = Navigation::of(ctx);
      let connect = ActionButton::new(
        "sharing-connect-server",
        ButtonKind::Secondary,
        ctx.t("settings.connectServer"),
      )
      .icon(Icon::Plus)
      .in_form()
      .opens(navigation.to(Route::Orchester(OrchesterView::connect())));
      return row.hint(ctx.t("sharing.link.noServer")).control(connect).render(ctx);
    }
    let options = props
      .facts
      .servers
      .iter()
      .map(|entry| {
        let label = ctx.t_args(
          "sharing.link.serverOption",
          [
            ("server", entry.stored.name.clone()),
            ("host", entry.address.authority()),
          ],
        );
        SelectOption::new(Chosen(Some(entry.stored.id)), label)
      })
      .collect();
    let select = SelectField::new(
      "sharing-server-select",
      ctx.t("sharing.link.server"),
      self.server.clone(),
      options,
    )
    .width(Dimension::Px(CONTROL_WIDTH))
    .render(ctx);
    let row = match self.chosen_entry(props) {
      Some(entry) => row.hint(server_hint(ctx, entry)),
      None => row,
    };
    row.control(select).render(ctx)
  }

  /// *Project on the server*: the person's projects there, each a radio row, and an address to type instead.
  fn project_group(&self, ctx: &mut Ctx, props: &Props, listed: Option<&Result<Listed, Said>>) -> Element {
    let server = self.chosen_entry(props).map_or_else(
      || ctx.t("sharing.connect.theServer").to_string(),
      |entry| entry.stored.name.clone(),
    );
    let description = ctx.t_args(
      "sharing.link.projectDescription",
      [
        ("server", server),
        ("project", props.project.open.project.name().as_str().to_owned()),
        ("count", props.facts.tasks.to_string()),
      ],
    );
    let mut rows: Vec<Element> = match listed {
      None => vec![
        SettingRow::new("sharing-projects-reading", ctx.t("sharing.link.reading"))
          .label_color(PaletteColor::TextSecondary)
          .render(ctx),
      ],
      Some(Err(reason)) => vec![
        SettingRow::new("sharing-projects-error", ctx.t("sharing.link.project"))
          .toned(reason.text(ctx), Tone::Danger)
          .render(ctx),
      ],
      Some(Ok(listed)) if listed.0.is_empty() => vec![
        SettingRow::new("sharing-projects-none", ctx.t("sharing.link.noProjects"))
          .label_color(PaletteColor::TextSecondary)
          .render(ctx),
      ],
      Some(Ok(listed)) => listed
        .0
        .iter()
        .map(|project| self.project_choice(ctx, project))
        .collect(),
    };
    let address = FieldInput::new("sharing-address", ctx.t("sharing.link.address"), self.address.clone())
      .mono()
      .placeholder(ctx.t("sharing.link.addressPlaceholder"))
      .width(Dimension::Px(CONTROL_WIDTH))
      .render(ctx);
    rows.push(
      SettingRow::new("sharing-address-field", ctx.t("sharing.link.address"))
        .hint(ctx.t("sharing.share.addressHint"))
        .control(address)
        .render(ctx),
    );
    Group::new("sharing-project", ctx.t("sharing.share.projectOnServer"))
      .rows(rows)
      .after(
        Text::new(&description)
          .id("sharing-project-description")
          .variant(crate::theme::ProjectSettingsText::Hint.role())
          .width(Dimension::Pct(100.0)),
      )
      .into()
  }

  /// A project to choose (`Create kontur` `nGNTo`): the radio, its name over its address, whether it is empty and its
  /// repository hint, the chosen one on `hover`.
  fn project_choice(&self, ctx: &Ctx, project: &ServerProject) -> Element {
    let id = project.project.id;
    let chosen = self.picked.get().0 == Some(id) && self.address.get().trim().is_empty();
    let hint = project.project.remote_hint.as_ref().map_or_else(
      || ctx.t("sharing.link.noHint").to_string(),
      |hint| {
        ctx
          .t_args("sharing.link.setUpFor", [("hint", hint.as_str())])
          .to_string()
      },
    );
    // The API names no task count; a project that never numbered a task is empty.
    let tasks = if project.project.next_number <= 1 {
      ctx.t("sharing.link.empty")
    } else {
      ctx.t("sharing.link.withTasks")
    };
    let line = ctx.t_args(
      "sharing.link.projectLine",
      [("tasks", tasks.to_string()), ("hint", hint)],
    );
    let pick = {
      let (picked, address) = (self.picked.clone(), self.address.clone());
      move || {
        picked.set(Picked(Some(id)));
        address.set(String::new());
      }
    };
    let on_radio = pick.clone();
    let radio = Toggle::new(
      format!("sharing-project-{}", project.project.slug),
      ToggleKind::Radio,
      chosen,
      project.project.name.clone(),
    )
    .hide_label()
    .on_change(move |_| on_radio());
    let mut lines = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Tight.role())
      .child(Text::new(&project.project.name).variant(ExtraTypography::OptionTitle.role()))
      .child(Text::new(shown_address(&project.project.address)).variant(OrchesterText::Address.role()))
      .child(
        Text::new(&line)
          .variant(crate::theme::ProjectSettingsText::Hint.role())
          .width(Dimension::Pct(100.0)),
      );
    if project.checkout.is_some() {
      lines = lines.child(
        Text::new(&ctx.t("sharing.link.linkedElsewhere"))
          .variant(ExtraTypography::Note.role())
          .color(PaletteColor::Warning),
      );
    }
    let row = pressable(pick)
      .id(format!("sharing-project-{}-option", project.project.slug))
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(
        crate::theme::ProjectSettingsSpacing::RowX.role(),
        ExtraSpacing::Loose.role(),
      ))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Start)
      .child(radio)
      .child(lines);
    if chosen {
      row.background(ExtraColor::Hover.role()).into()
    } else {
      row.into()
    }
  }

  /// The checkout's `origin` against the chosen project's hint, as a line in its tone; a checkout without one says so.
  fn repository_line(&self, ctx: &mut Ctx, props: &Props, listed: Option<&Result<Listed, Said>>) -> Element {
    let picked = self.picked_project(listed);
    if props.facts.origin.is_none() {
      return no_origin(ctx, picked);
    }
    let (glyph, color, words) = remote_words(ctx, props, picked);
    let fill = match color {
      PaletteColor::Success => Some(PaletteColor::SuccessMuted),
      PaletteColor::Warning => Some(PaletteColor::WarningMuted),
      _ => None,
    };
    let line = Row::new()
      .id("sharing-remote")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Md)
      .child(icon(glyph, IconSize::Base, color))
      .child(
        Text::new(&words)
          .id("sharing-remote-text")
          .variant(crate::theme::ExtraTypography::Prose.role())
          .flex(1.0),
      );
    match fill {
      Some(fill) => line.background(fill).into(),
      None => line.border(Border::inside(BorderSize::Sm, PaletteColor::Border)).into(),
    }
  }

  pub(super) fn picked_project<'a>(&self, listed: Option<&'a Result<Listed, Said>>) -> Option<&'a SharedProject> {
    let id = self.picked.get().0?;
    let Some(Ok(listed)) = listed else {
      return None;
    };
    listed
      .0
      .iter()
      .map(|project| &project.project)
      .find(|project| project.id == id)
  }
}

/// *This checkout has no origin remote* (`No origin to compare` in `N9NFio`): nothing to compare, and linking is still
/// possible.
fn no_origin(ctx: &Ctx, picked: Option<&SharedProject>) -> Element {
  let body = match picked.and_then(|project| project.remote_hint.as_ref().map(|hint| (project, hint))) {
    Some((project, hint)) => ctx.t_args(
      "sharing.link.remote.noOriginHint",
      [("project", project.name.as_str()), ("hint", hint.as_str())],
    ),
    None => ctx.t("sharing.link.remote.noOriginBody"),
  };
  Callout::new(
    "sharing-no-origin",
    CalloutTone::Neutral,
    ctx.t("sharing.link.remote.noOrigin"),
  )
  .icon(Icon::GitBranch)
  .body(body)
  .into()
}

/// How the checkout's `origin` compares with `picked`'s hint, as an icon, its color and the words.
fn remote_words(ctx: &Ctx, props: &Props, picked: Option<&SharedProject>) -> (Icon, PaletteColor, String) {
  let origin = props.facts.origin.as_ref();
  let Some(project) = picked else {
    return (
      Icon::Info,
      PaletteColor::TextMuted,
      ctx.t("sharing.link.remote.choose").to_string(),
    );
  };
  match remote_match(origin, project.remote_hint.as_ref()) {
    RemoteMatch::Same => (
      Icon::CircleCheck,
      PaletteColor::Success,
      ctx
        .t_args(
          "sharing.link.remote.same",
          [
            (
              "origin",
              origin.map(|origin| origin.as_str().to_owned()).unwrap_or_default(),
            ),
            ("project", project.name.clone()),
          ],
        )
        .to_string(),
    ),
    RemoteMatch::Different { checkout, hint } => (
      Icon::TriangleAlert,
      PaletteColor::Warning,
      ctx
        .t_args(
          "sharing.link.remote.different",
          [
            ("origin", checkout.as_str().to_owned()),
            ("project", project.name.clone()),
            ("hint", hint.as_str().to_owned()),
          ],
        )
        .to_string(),
    ),
    RemoteMatch::NoOrigin => (
      Icon::GitBranch,
      PaletteColor::TextMuted,
      ctx.t("sharing.link.remote.noOriginBody").to_string(),
    ),
    RemoteMatch::NoHint => (
      Icon::Info,
      PaletteColor::TextMuted,
      ctx.t("sharing.link.remote.noHint").to_string(),
    ),
  }
}

/// The chosen server's state as the status bar says it (*Offline since 14:31* in `aocsw`), and where another server is
/// added.
fn server_hint(ctx: &Ctx, entry: &ServerEntry) -> String {
  let state = if entry.certificate_changed {
    ctx.t("sharing.link.serverState.certificateChanged")
  } else {
    match entry.connection {
      Some(Connection::Connected) => ctx.t("sharing.link.serverState.connected"),
      Some(Connection::Checking) | None => ctx.t("sharing.link.serverState.checking"),
      Some(Connection::Reconnecting { attempt, .. }) => ctx.t_args(
        "sharing.link.serverState.reconnecting",
        [("attempt", attempt.to_string())],
      ),
      Some(Connection::Offline { since, .. }) => {
        ctx.t_args("sharing.link.serverState.offline", [("time", moment(ctx, since))])
      }
      Some(Connection::SignedOut) => ctx.t("sharing.link.serverState.signedOut"),
    }
  };
  ctx
    .t_args("sharing.link.serverHint", [("state", state.to_string())])
    .to_string()
}
