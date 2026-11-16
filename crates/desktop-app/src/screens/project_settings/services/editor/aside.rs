//! The side column's cards after *Start order*: *Values in a run* (`FJny2`), the placeholders this service can use with
//! what a run fills them with, and *Agents use it with* (`o62dBM`), the tools an agent reaches it through. Both follow
//! the fields as they are now. The page has no run, so a value is said in words where the design shows an example.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  orchester_tools::OrchesterTool,
  service::{RunData, Seed, Service, UiDriver},
};

use super::View;
use crate::theme::{ExtraSpacing, ExtraTypography, ServicesText, SettingsText};

/// The tools that see and use a service's UI, on one line (`sQkuE`).
const UI_TOOLS: [OrchesterTool; 3] = [
  OrchesterTool::UiSnapshot,
  OrchesterTool::UiAct,
  OrchesterTool::UiScreenshot,
];
/// What stands between tools on one line: a sign, not words.
const BETWEEN: &str = " · ";

/// The service as the fields make it, or as stored while they hold a mistake.
fn service(view: &View) -> Option<&Service> {
  view.built.as_ref().ok().or_else(|| view.base())
}

/// A card of the side column: its title over its lines, 10 apart, in a `border` outline, radius 10, padding 16; a
/// stack's *Dependencies* and a suite's *Last runs* are cards too (`z1aSH`, `vMRvk`).
pub(in crate::screens::project_settings::services) fn card(
  id: &str,
  title: &str,
  lines: impl IntoIterator<Item = Element>,
) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::all(SpacingSize::Lg))
    .spacing(ExtraSpacing::Loose.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(SettingsText::SectionTitle.role()),
    )
    .with_children(lines)
    .into()
}

/// *Values in a run*; none while the fields make no service.
pub(super) fn values_card(ctx: &Ctx, view: &View) -> Option<Element> {
  let service = service(view)?;
  let mut values: Vec<(String, Arc<str>)> = vec![
    (
      "{{port}}".to_owned(),
      ctx.t("project_settings.services.editor.values.port"),
    ),
    (
      "{{url}}".to_owned(),
      ctx.t("project_settings.services.editor.values.url"),
    ),
  ];
  values.extend(service.needs().iter().map(|need| {
    (
      format!("{{{{services.{need}.url}}}}"),
      ctx.t_args(
        "project_settings.services.editor.values.serviceUrl",
        [("name", need.as_str())],
      ),
    )
  }));
  values.push((
    "{{run}}".to_owned(),
    ctx.t("project_settings.services.editor.values.run"),
  ));
  if matches!(service.run_data(), RunData::RunFolder(_)) {
    values.push((
      "{{data}}".to_owned(),
      ctx.t("project_settings.services.editor.values.data"),
    ));
  }
  if let Seed::Requests(requests) = service.seed() {
    for (at, request) in requests.as_slice().iter().enumerate() {
      values.extend(request.captures().iter().map(|capture| {
        (
          format!("{{{{seed.{}}}}}", capture.name().as_str()),
          ctx.t_args(
            "project_settings.services.editor.values.captured",
            [("number", (at + 1).to_string())],
          ),
        )
      }));
    }
  }
  let lines = values
    .iter()
    .enumerate()
    .map(|(at, (key, words))| value_line(at, key, words));
  let note = Text::new(&ctx.t("project_settings.services.editor.values.note"))
    .id("settings-service-values-note")
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0));
  Some(card(
    "settings-service-values",
    &ctx.t("project_settings.services.editor.values.title"),
    lines.chain([Element::from(note)]),
  ))
}

/// A placeholder in Geist Mono 11.5 `text_secondary` and what fills it at the line's end (`Uouss`).
fn value_line(at: usize, key: &str, words: &str) -> Element {
  let id = format!("settings-service-value-{at}");
  Row::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(key)
        .id(format!("{id}-key"))
        .variant(ServicesText::SideLine.role())
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(words)
        .id(format!("{id}-words"))
        .variant(ExtraTypography::Note.role())
        .nowrap(),
    )
    .into()
}

/// *Agents use it with*: starting it, its logs, its UI while it has a driver, and stopping it.
pub(super) fn tools_card(ctx: &Ctx, view: &View) -> Option<Element> {
  let service = service(view)?;
  let name = service.name().as_str();
  let call = |tool: OrchesterTool| format!("{} {name}", tool.as_str());
  let mut lines = vec![call(OrchesterTool::StartService), call(OrchesterTool::ServiceLogs)];
  if service.ui_driver() != UiDriver::None {
    lines.push(call(OrchesterTool::UiOpen));
    lines.push(UI_TOOLS.map(OrchesterTool::as_str).join(BETWEEN));
  }
  lines.push(call(OrchesterTool::StopService));
  let lines = lines.into_iter().enumerate().map(|(at, line)| {
    Element::from(
      Text::new(&line)
        .id(format!("settings-service-tool-{at}"))
        .variant(ServicesText::SideLine.role()),
    )
  });
  Some(card(
    "settings-service-tools",
    &ctx.t("project_settings.services.editor.tools.title"),
    lines,
  ))
}
