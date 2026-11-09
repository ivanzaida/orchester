//! Report (`Report` `a6k4l`): the format as segments (`Segmented` `vb45L`) and where it is read from, the standard
//! output or a file whose path follows (`Oy4Qd`); choosing a format that is only printed or only written moves *Read
//! from* with it. Layer and needs (`Layer and needs` `yTX7L`): the layer as segments (`nX1Mm`), and what is started
//! before the suite — nothing, services (their chips and *Add a service*) or a stack (its select, `I2meD`) — with the
//! services that start, in order, and the slots they take (`Chain` `kNqkC`). A choice saves at once.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, SelectOption, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{ReportFormat, TestLayer};
use orchester_workspace::agent_testing::suite_editor::form::{NeedsKind, ReadFrom, SuiteForm};

use super::{
  super::{
    super::kit::Group,
    editor::fields::{field, input},
    words,
  },
  Commit, View,
  fields::{Choices, SuiteFields},
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, ServicesText},
  ui::{
    chip::TagChip,
    field::{SelectField, field_error},
    icon::{Icon, icon},
    message::message_text,
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// *A stack*'s select (`Select` `I2meD`) and *Add a service*'s.
const SELECT_WIDTH: f32 = 140.0;
/// A chain chip (`postgres` `z5Gr0`).
const CHAIN_HEIGHT: f32 = 26.0;

/// A segment that makes the choice `change` and saves.
fn segment(
  (id, label): (String, Arc<str>),
  chosen: bool,
  (fields, commit): (&SuiteFields, &Commit),
  change: impl Fn(&mut Choices) + Send + Sync + 'static,
) -> Segment {
  let (fields, commit) = (fields.clone(), commit.clone());
  Segment::new(id, SegmentFace::Text(label), chosen, move || {
    fields.choose(|choices| change(choices));
    commit();
  })
}

/// The Report group.
pub(super) fn report_group(ctx: &mut Ctx, fields: &SuiteFields, view: &View, commit: &Commit) -> Element {
  let choices = fields.choices.get();
  let formats = ReportFormat::ALL.map(|format| {
    segment(
      (
        format!("settings-suite-format-{}", format.as_str()),
        words::report(ctx, format),
      ),
      choices.format == format,
      (fields, commit),
      move |choices| {
        choices.read_from = SuiteForm::read_from_for(format, choices.read_from);
        choices.format = format;
      },
    )
  });
  let format = field(
    "settings-suite-format",
    ctx.t("project_settings.services.suite.format"),
    ctx.t("project_settings.services.suite.formatHint"),
  )
  .render(ctx, segmented("settings-suite-format-choice", formats, Focus::Keyboard));
  let sources = [
    (ReadFrom::Printed, "printed", "project_settings.services.suite.printed"),
    (ReadFrom::File, "file", "project_settings.services.suite.file"),
  ]
  .map(|(read_from, name, key)| {
    segment(
      (format!("settings-suite-read-{name}"), ctx.t(key)),
      choices.read_from == read_from,
      (fields, commit),
      move |choices| choices.read_from = read_from,
    )
  });
  let path = (choices.read_from == ReadFrom::File).then(|| {
    let label = ctx.t("project_settings.services.suite.reportPath");
    input(("settings-suite-report-path", label), &fields.report_path, commit)
      .placeholder(ctx.t("project_settings.services.suite.reportPathExample"))
      .render(ctx)
  });
  let error = view
    .problems()
    .and_then(|problems| problems.report.as_ref())
    .map(|error| Element::from(field_error("settings-suite-report-error", &message_text(ctx, error))));
  let read = field(
    "settings-suite-read",
    ctx.t("project_settings.services.suite.readFrom"),
    ctx.t("project_settings.services.suite.readFromHint"),
  )
  .render(
    ctx,
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(segmented("settings-suite-read-choice", sources, Focus::Keyboard))
      .with_children(path)
      .with_children(error),
  );
  Group::new(
    "settings-suite-report-group",
    ctx.t("project_settings.services.suite.report"),
  )
  .row(format)
  .row(read)
  .into()
}

/// The Layer and needs group.
pub(super) fn needs_group(ctx: &mut Ctx, fields: &SuiteFields, view: &View, commit: &Commit) -> Element {
  let choices = fields.choices.get();
  let layers = TestLayer::ALL.map(|layer| {
    segment(
      (
        format!("settings-suite-layer-{}", layer.as_str()),
        words::layer(ctx, layer),
      ),
      choices.layer == layer,
      (fields, commit),
      move |choices| choices.layer = layer,
    )
  });
  let layer = field(
    "settings-suite-layer",
    ctx.t("project_settings.services.suite.layer"),
    ctx.t("project_settings.services.suite.layerHint"),
  )
  .render(ctx, segmented("settings-suite-layer-choice", layers, Focus::Keyboard));
  let needs = needs_row(ctx, fields, view, commit);
  Group::new(
    "settings-suite-needs-group",
    ctx.t("project_settings.services.suite.layerAndNeeds"),
  )
  .row(layer)
  .row(needs)
  .into()
}

/// *Needs*: nothing, services or a stack, what is chosen of it, and the services that start, in order.
fn needs_row(ctx: &mut Ctx, fields: &SuiteFields, view: &View, commit: &Commit) -> Element {
  let choices = fields.choices.get();
  let kinds = [
    (
      NeedsKind::Nothing,
      "nothing",
      "project_settings.services.suite.needsNothing",
    ),
    (
      NeedsKind::Services,
      "services",
      "project_settings.services.suite.needsServices",
    ),
    (NeedsKind::Stack, "stack", "project_settings.services.suite.needsStack"),
  ]
  .map(|(kind, name, key)| {
    segment(
      (format!("settings-suite-needs-{name}"), ctx.t(key)),
      choices.needs == kind,
      (fields, commit),
      move |choices| choices.needs = kind,
    )
  });
  let pick = match choices.needs {
    NeedsKind::Nothing => None,
    NeedsKind::Stack => Some(stack_select(ctx, fields, view)),
    NeedsKind::Services => Some(service_picks(ctx, fields, view, commit)),
  };
  let line = Row::new()
    .id("settings-suite-needs-pick")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(segmented("settings-suite-needs-choice", kinds, Focus::Keyboard))
    .with_children(pick);
  let error = view.problems().and_then(|problems| problems.needs.clone());
  let started = (error.is_none() && choices.needs != NeedsKind::Nothing).then(|| {
    let form = fields.form();
    view.stored.started(&form.needed())
  });
  let under: Option<Element> = match (error, started) {
    (Some(error), _) | (None, Some(Err(error))) => {
      Some(field_error("settings-suite-needs-error", &message_text(ctx, &error)).into())
    }
    (None, Some(Ok(started))) => Some(chain(ctx, &started)),
    (None, None) => None,
  };
  field(
    "settings-suite-needs",
    ctx.t("project_settings.services.editor.needs"),
    ctx.t("project_settings.services.suite.needsHint"),
  )
  .render(
    ctx,
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(line)
      .with_children(under),
  )
}

/// *A stack*'s select: the project's stacks; choosing one saves (the page follows [`SuiteFields::stack`]).
fn stack_select(ctx: &mut Ctx, fields: &SuiteFields, view: &View) -> Element {
  let offered: Vec<SelectOption<String>> = view
    .stored
    .stacks
    .iter()
    .map(|stack| SelectOption::new(stack.name.to_string(), stack.name.to_string()))
    .collect();
  let placeholder = if offered.is_empty() {
    "project_settings.services.suite.noStacks"
  } else {
    "project_settings.services.suite.chooseStack"
  };
  let select = SelectField::new(
    "settings-suite-stack",
    ctx.t("project_settings.services.suite.chooseStack"),
    fields.stack.clone(),
    offered.clone(),
  )
  .placeholder(ctx.t(placeholder))
  .width(Dimension::Px(SELECT_WIDTH));
  if offered.is_empty() { select.locked() } else { select }.render(ctx)
}

/// The services it needs as chips with their ×, then *Add a service* offering the others.
fn service_picks(ctx: &mut Ctx, fields: &SuiteFields, view: &View, commit: &Commit) -> Element {
  let chosen = fields.choices.get().services;
  let chips = chosen.iter().map(|name| {
    let (fields, commit, removed) = (fields.clone(), commit.clone(), name.clone());
    Element::from(
      TagChip::new(format!("settings-suite-need-{name}"), name.clone())
        .mono()
        .pill()
        .removable(
          ctx.t_args("project_settings.services.editor.removeNeed", [("name", name)]),
          move || {
            fields.choose(|choices| choices.services.retain(|service| *service != removed));
            commit();
          },
        ),
    )
  });
  let chips: Vec<Element> = chips.collect();
  let offered: Vec<SelectOption<String>> = view
    .stored
    .services
    .iter()
    .map(|entry| entry.name.to_string())
    .filter(|name| !chosen.contains(name))
    .map(|name| SelectOption::new(name.clone(), name))
    .collect();
  let add = SelectField::new(
    "settings-suite-add-need",
    ctx.t("project_settings.services.editor.addNeed"),
    fields.adding.clone(),
    offered.clone(),
  )
  .placeholder(ctx.t("project_settings.services.editor.addNeed"))
  .width(Dimension::Px(SELECT_WIDTH));
  let add = if offered.is_empty() { add.locked() } else { add }.render(ctx);
  Row::new()
    .id("settings-suite-needs")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .with_children(chips)
    .child(add)
    .into()
}

/// The services that start before the suite, numbered, an arrow between them, and the slots they take (`kNqkC`).
fn chain(ctx: &Ctx, started: &[Arc<str>]) -> Element {
  let mut row = Row::new()
    .id("settings-suite-chain")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center);
  for (at, name) in started.iter().enumerate() {
    if at > 0 {
      row = row.child(icon(Icon::ArrowRight, IconSize::Sm, PaletteColor::TextMuted));
    }
    row = row.child(
      Row::new()
        .id(format!("settings-suite-chain-{name}"))
        .height(Dimension::Px(CHAIN_HEIGHT))
        .padding(Padding::horizontal(SpacingSize::Sm))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .background(PaletteColor::SurfaceRaised)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Md)
        .child(Text::new(&(at + 1).to_string()).variant(ServicesText::ChainNumber.role()))
        .child(Text::new(name).variant(ServicesText::ChainName.role())),
    );
  }
  let slots = if started.len() == 1 {
    ctx.t("project_settings.services.suite.slots.one")
  } else {
    ctx.t_args(
      "project_settings.services.suite.slots.other",
      [("count", started.len().to_string())],
    )
  };
  row
    .child(
      Text::new(&slots)
        .id("settings-suite-slots")
        .variant(ExtraTypography::Note.role()),
    )
    .into()
}
