//! The rows of *Link to a server project* (`bWD9q`): the offer of `orchester.toml`'s address, the server, the
//! person's projects there or an address, and the repository compared with the project's hint.

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

use super::{Chosen, Link, Listed, Picked, Props};
use crate::{
  navigation::Navigation,
  routes::{OrchesterView, Route},
  screens::{
    project_settings::page::{self, CONTROL_WIDTH},
    settings::orchester::{Said, moment, shown_address},
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, OrchesterText},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::{FieldInput, SelectField, field, field_error},
    form_section::{Label, Place, section, side_row},
    icon::{Icon, icon},
    pressable::pressable,
    toggle::{Toggle, ToggleKind},
  },
};

impl Link {
  pub(super) fn link_section(&self, ctx: &mut Ctx, props: &Props, listed: Option<&Result<Listed, Said>>) -> Element {
    let mut rows: Vec<Element> = Vec::new();
    rows.extend(self.discovery_offer(ctx, props));
    rows.push(self.server_row(ctx, props));
    rows.push(self.project_row(ctx, props, listed));
    rows.push(self.repository_row(ctx, props, listed));
    rows.push(self.link_row(ctx, props, listed));
    section(
      "project-settings-sharing-link",
      &ctx.t("sharing.link.section"),
      None,
      rows,
    )
    .into()
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

  fn server_row(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let keys = ("sharing.link.server", "sharing.link.serverDescription");
    if props.facts.servers.is_empty() {
      let navigation = Navigation::of(ctx);
      let connect = ActionButton::new(
        "sharing-connect-server",
        ButtonKind::Secondary,
        ctx.t("settings.connectServer"),
      )
      .icon(Icon::Plus)
      .on_click(move || navigation.go(Route::Orchester(OrchesterView::connect())));
      let control = Column::new()
        .width(Dimension::Px(CONTROL_WIDTH))
        .spacing(SpacingSize::Sm)
        .child(page::note("sharing-no-server", &ctx.t("sharing.link.noServer")))
        .child(Row::new().child(connect));
      return page::row(ctx, "sharing-server", keys, control.into(), Place::Within);
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
    let control = Column::new()
      .width(Dimension::Px(CONTROL_WIDTH))
      .spacing(SpacingSize::Sm)
      .child(select);
    let control = match self.chosen_entry(props) {
      Some(entry) => {
        let (glyph, words) = server_hint(ctx, entry);
        control.child(page::icon_note("sharing-server-state", glyph, &words))
      }
      None => control,
    };
    page::row(ctx, "sharing-server", keys, control.into(), Place::Within)
  }

  fn project_row(&self, ctx: &mut Ctx, props: &Props, listed: Option<&Result<Listed, Said>>) -> Element {
    let mut control = Column::new()
      .width(Dimension::Px(CONTROL_WIDTH))
      .spacing(SpacingSize::Sm);
    match listed {
      None => control = control.child(page::note("sharing-projects-reading", &ctx.t("sharing.link.reading"))),
      Some(Err(reason)) => control = control.child(field_error("sharing-projects-error", &reason.text(ctx))),
      Some(Ok(listed)) if listed.0.is_empty() => {
        control = control.child(page::note("sharing-projects-none", &ctx.t("sharing.link.noProjects")));
      }
      Some(Ok(listed)) => {
        for project in &listed.0 {
          control = control.child(self.project_choice(ctx, project));
        }
      }
    }
    let address = FieldInput::new("sharing-address", ctx.t("sharing.link.address"), self.address.clone())
      .mono()
      .placeholder(ctx.t("sharing.link.addressPlaceholder"))
      .render(ctx);
    control = control.child(field(
      "sharing-address-field",
      Some(&ctx.t("sharing.link.address")),
      address,
      None,
    ));
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
    side_row(
      Label::new("sharing-project", ctx.t("sharing.link.project")).description(description),
      control,
      Place::Within,
      true,
    )
  }

  /// A project to choose (`Project · Kontur` in `bWD9q`): the radio, and 10 from it the name in 12.5/500, the address
  /// in mono and whether it is empty with its repository hint, 2 apart; 8 over and under and 10 at the sides, and
  /// the chosen one on `hover` in a `border_strong` stroke, radius 6.
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
          .variant(ExtraTypography::Note.role())
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
      .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Start)
      .rounded(RadiusSize::Md)
      .child(radio)
      .child(lines);
    if chosen {
      row
        .background(ExtraColor::Hover.role())
        .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
        .into()
    } else {
      row.into()
    }
  }

  /// The checkout's `origin` against the chosen project's hint (`Repository` in `bWD9q`).
  fn repository_row(&self, ctx: &mut Ctx, props: &Props, listed: Option<&Result<Listed, Said>>) -> Element {
    let picked = self.picked_project(listed);
    let keys = ("sharing.link.repository", "sharing.link.repositoryDescription");
    if props.facts.origin.is_none() {
      let control = Column::new()
        .width(Dimension::Px(CONTROL_WIDTH))
        .child(no_origin(ctx, picked));
      return page::row(ctx, "sharing-repository", keys, control.into(), Place::Within);
    }
    let (glyph, color, words) = remote_words(ctx, props, picked);
    let control = Row::new()
      .id("sharing-remote")
      .width(Dimension::Px(CONTROL_WIDTH))
      .spacing(ExtraSpacing::Pair.role())
      .align_items(Alignment::Start)
      .child(icon(glyph, IconSize::Sm, color.clone()))
      .child(
        Text::new(&words)
          .id("sharing-remote-text")
          .variant(ExtraTypography::Note.role())
          .color(color)
          .flex(1.0),
      );
    page::row(ctx, "sharing-repository", keys, control.into(), Place::Within)
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
fn server_hint(ctx: &Ctx, entry: &ServerEntry) -> (Icon, String) {
  let (glyph, state) = if entry.certificate_changed {
    (Icon::ShieldAlert, ctx.t("sharing.link.serverState.certificateChanged"))
  } else {
    match entry.connection {
      Some(Connection::Connected) => (Icon::CircleCheck, ctx.t("sharing.link.serverState.connected")),
      Some(Connection::Checking) | None => (Icon::RefreshCw, ctx.t("sharing.link.serverState.checking")),
      Some(Connection::Reconnecting { attempt, .. }) => (
        Icon::RefreshCw,
        ctx.t_args(
          "sharing.link.serverState.reconnecting",
          [("attempt", attempt.to_string())],
        ),
      ),
      Some(Connection::Offline { since, .. }) => (
        Icon::CloudOff,
        ctx.t_args("sharing.link.serverState.offline", [("time", moment(ctx, since))]),
      ),
      Some(Connection::SignedOut) => (Icon::CloudOff, ctx.t("sharing.link.serverState.signedOut")),
    }
  };
  (
    glyph,
    ctx
      .t_args("sharing.link.serverHint", [("state", state.to_string())])
      .to_string(),
  )
}
