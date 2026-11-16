//! Seed (`Seed` `F2EhK`): what runs once the service is ready, before agents use it — *HTTP requests*, *A command* or
//! *None* in a segmented control (`tynvK`). Requests are listed in a box in the order they are sent, each numbered
//! ([`request`]), with *Add request* and how later requests and agents read what they capture under it (`qF3lb`). A
//! command is a program and its arguments, as the service's own (the design leaves it undrawn, `d5FheK`). A kind chosen
//! saves; the requests and the command each stay as typed while the other is chosen.

mod capture;
mod request;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::Column,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_workspace::{
  agent_testing::editor::{
    form::{RequestLine, SeedKind},
    order::Field,
  },
  messages::Message,
};

use self::request::RequestProps;
use super::{
  super::super::kit::{Group, form_note},
  Commit, View,
  fields::{field, input, placeholder_in},
  sections::{Sections, Seeds},
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    message::message_text,
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// How a later request writes what an earlier one captured: a placeholder, not words.
const CAPTURED: &str = "{{seed.<name>}}";

/// The Seed group.
pub(super) fn seed_group(ctx: &mut Ctx, sections: &Sections, view: &View, commit: &Commit) -> Element {
  let kind = sections.seeds.get().kind;
  let body = match kind {
    SeedKind::Requests => Some(requests(ctx, sections, view, commit)),
    SeedKind::Command => Some(command(ctx, sections, view, commit)),
    SeedKind::None => None,
  };
  let control = Column::new()
    .id("settings-service-seed")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(kinds(ctx, sections, kind, commit))
    .with_children(body);
  let row = field(
    "settings-service-seed",
    ctx.t("project_settings.services.editor.seed.title"),
    ctx.t("project_settings.services.editor.seed.hint"),
  )
  .render(ctx, control);
  Group::new(
    "settings-service-seed-group",
    ctx.t("project_settings.services.editor.seed.title"),
  )
  .row(row)
  .into()
}

/// *HTTP requests · A command · None*; choosing requests while there is none opens a new one.
fn kinds(ctx: &Ctx, sections: &Sections, chosen: SeedKind, commit: &Commit) -> Element {
  let segments = [
    (
      SeedKind::Requests,
      "requests",
      "project_settings.services.editor.seed.requests",
    ),
    (
      SeedKind::Command,
      "command",
      "project_settings.services.editor.seed.command",
    ),
    (SeedKind::None, "none", "project_settings.services.editor.seed.none"),
  ]
  .map(|(kind, name, key)| {
    let (seeds, open, commit) = (sections.seeds.clone(), sections.open_request.clone(), commit.clone());
    Segment::new(
      format!("settings-service-seed-{name}"),
      SegmentFace::Text(ctx.t(key)),
      chosen == kind,
      move || {
        let mut opened = false;
        seeds.update(|seeds| {
          seeds.kind = kind;
          if kind == SeedKind::Requests && seeds.requests.is_empty() {
            seeds.requests.push(RequestLine::default());
            opened = true;
          }
        });
        if opened {
          open.set(Some(0));
        }
        commit();
      },
    )
  });
  segmented("settings-service-seed-kind", segments, Focus::Keyboard).into()
}

/// The requests' box, *Add request*, the note and why the requests make no seed.
fn requests(ctx: &mut Ctx, sections: &Sections, view: &View, commit: &Commit) -> Element {
  let Seeds { requests, .. } = sections.seeds.get();
  let open = sections.open_request.get();
  let problem = view.problems().and_then(|problems| problems.seed.as_ref());
  let count = requests.len();
  let rows: Vec<Element> = requests
    .into_iter()
    .enumerate()
    .map(|(at, line)| {
      let said = problem
        .filter(|problem| problem.request == Some(at))
        .map(|problem| problem.message.clone())
        .or_else(|| placeholder_in(view, Field::Seed(Some(at))))
        .map(|message| message_text(ctx, &message));
      ctx.mount_keyed::<request::RequestRow>(
        &format!("settings-service-seed-request-{at}"),
        RequestProps {
          at,
          count,
          line,
          open: open == Some(at),
          said,
          sections: sections.clone(),
          commit: commit.clone(),
        },
      )
    })
    .collect();
  let listed = (!rows.is_empty()).then(|| {
    Column::new()
      .id("settings-service-seed-requests")
      .width(Dimension::Pct(100.0))
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .clip()
      .with_children(rows.into_iter().enumerate().map(|(at, row)| {
        let line = Column::new().width(Dimension::Pct(100.0)).child(row);
        if at == 0 {
          line
        } else {
          line.border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
        }
      }))
  });
  let whole = problem.filter(|problem| problem.request.is_none()).map(|problem| {
    Element::from(field_error(
      "settings-service-seed-error",
      &message_text(ctx, &problem.message),
    ))
  });
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .with_children(listed)
    .with_children(whole)
    .child(foot(ctx, sections))
    .into()
}

/// *Add request* over how later requests and agents read what is captured (`Seed foot` `qF3lb`).
fn foot(ctx: &Ctx, sections: &Sections) -> Element {
  let (seeds, open) = (sections.seeds.clone(), sections.open_request.clone());
  let add = ActionButton::new(
    "settings-service-seed-add",
    ButtonKind::Ghost,
    ctx.t("project_settings.services.editor.seed.add"),
  )
  .icon(Icon::Plus)
  .small()
  .on_click(move || {
    let mut at = 0;
    seeds.update(|seeds| {
      seeds.requests.push(RequestLine::default());
      at = seeds.requests.len() - 1;
    });
    open.set(Some(at));
  });
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(Element::from(add))
    .child(form_note(
      "settings-service-seed-note",
      &ctx.t_args("project_settings.services.editor.seed.note", [("captured", CAPTURED)]),
    ))
    .into()
}

/// The seed command: its program, then its arguments one per line, and why they make no command.
fn command(ctx: &mut Ctx, sections: &Sections, view: &View, commit: &Commit) -> Element {
  let problem: Option<Message> = view
    .problems()
    .and_then(|problems| problems.seed.as_ref())
    .map(|problem| problem.message.clone())
    .or_else(|| placeholder_in(view, Field::Seed(None)));
  let program = input(
    (
      "settings-service-seed-program",
      ctx.t("project_settings.services.editor.seed.program"),
    ),
    &sections.seed_program,
    commit,
  )
  .placeholder(ctx.t("project_settings.services.editor.seed.program"));
  let arguments = input(
    (
      "settings-service-seed-arguments",
      ctx.t("project_settings.services.editor.seed.arguments"),
    ),
    &sections.seed_arguments,
    commit,
  )
  .compact_text_area()
  .text_style(ExtraTypography::MonoArea.role())
  .placeholder(ctx.t("project_settings.services.editor.seed.argumentsHint"));
  let (program, arguments) = match &problem {
    Some(_) => (program.invalid(), arguments.invalid()),
    None => (program, arguments),
  };
  let said =
    problem.map(|problem| Element::from(field_error("settings-service-seed-error", &message_text(ctx, &problem))));
  Column::new()
    .id("settings-service-seed-command")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(program.render(ctx))
    .child(arguments.render(ctx))
    .with_children(said)
    .into()
}
