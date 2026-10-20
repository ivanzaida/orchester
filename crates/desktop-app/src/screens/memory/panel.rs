//! The panel at the right of the project's page: *Project memory* (`PFa0R`) — its records by status, how agents'
//! records are saved, what each agent is given, and what agents may and may not do — or, once a press chose a record,
//! that record's inspector (`record`).

mod record;

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::memory::{MemorySettings, WritePolicy};
use orchester_workspace::memory::read::ProjectMemory;

use super::{parts, state::MemoryState};
use crate::{
  theme::{ExtraSpacing, IconSize, MemoryText},
  ui::{
    icon::{Icon, icon},
    inspector::{self, PropertyValue},
  },
};

/// The glyph tile of the panel's header (`Cxf4D`).
const TILE: f32 = 32.0;

/// The project's panel, or the inspector of the chosen record while it is listed.
pub(super) fn side(ctx: &mut Ctx, state: &MemoryState, memory: &Arc<ProjectMemory>) -> Element {
  let chosen = state
    .chosen()
    .filter(|id| memory.records.iter().any(|record| record.id() == *id));
  match chosen {
    Some(record) => ctx.mount_keyed::<record::RecordInspector>(
      &format!("memory-inspector-{record}"),
      record::InspectorProps {
        memory: memory.clone(),
        record,
      },
    ),
    None => project_panel(ctx, state, memory),
  }
}

/// A panel's frame: its header, its body scrolling, and its footer if it has one.
pub(super) fn frame(id: &str, header: Row, body: Column, footer: Option<Row>) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(lurq::app::theme::RadiusSize::Lg)
    .child(header)
    .child(ScrollVertical::new(body).width(Dimension::Pct(100.0)).flex(1.0))
    .with_children(footer)
    .into()
}

/// A panel's header: a glyph in its tile, the title and what it names (`QTwkL`).
pub(super) fn header(glyph: Icon, title: &str, name: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Row::new()
        .width(Dimension::Px(TILE))
        .height(Dimension::Px(TILE))
        .flex_shrink(0.0)
        .align_items(Alignment::Center)
        .justify(Justify::Center)
        .background(PaletteColor::SurfaceRaised)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(lurq::app::theme::RadiusSize::Md)
        .child(icon(glyph, IconSize::Nav, PaletteColor::TextSecondary)),
    )
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(title).variant(MemoryText::PanelTitle.role()))
        .child(Text::new(name).variant(TypographyStyle::Description)),
    )
}

/// A panel's body: its sections 14 apart (`fxvCL`).
pub(super) fn body() -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Inset.role())
}

fn project_panel(ctx: &Ctx, state: &MemoryState, memory: &ProjectMemory) -> Element {
  let counts = memory.counts();
  let (project_waiting, task_waiting) = memory.waiting_by_scope();
  let row = |id: &str, label: &str, value: String| inspector::property(id, &ctx.t(label), &value, PropertyValue::Plain);
  let records = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(inspector::section_title(
      "memory-panel-records",
      &ctx.t("memory.panel.records"),
      Some(&ctx.t_args("memory.panel.inAll", [("count", counts.total().to_string())])),
    ))
    .child(row(
      "memory-panel-active",
      "memory.panel.active",
      ctx
        .t_args("memory.panel.activeValue", [("count", counts.active.to_string())])
        .to_string(),
    ))
    .child(row(
      "memory-panel-waiting",
      "memory.panel.waiting",
      ctx
        .t_args(
          "memory.panel.waitingValue",
          [
            ("count", counts.waiting.to_string()),
            ("project", project_waiting.to_string()),
            ("task", task_waiting.to_string()),
          ],
        )
        .to_string(),
    ))
    .child(row(
      "memory-panel-deprecated",
      "memory.panel.deprecated",
      counts.deprecated.to_string(),
    ))
    .child(row(
      "memory-panel-superseded",
      "memory.panel.superseded",
      counts.superseded.to_string(),
    ))
    .child(row(
      "memory-panel-rejected",
      "memory.panel.rejected",
      counts.rejected.to_string(),
    ));
  let body = body()
    .child(records)
    .child(saving(ctx, &memory.settings))
    .child(limits(ctx, &memory.settings))
    .child(
      inspector::section("memory-panel-notes", SpacingSize::Sm)
        .child(inspector::note(
          "memory-panel-given",
          Icon::Info,
          &ctx.t("memory.panel.givenNote"),
        ))
        .child(inspector::note(
          "memory-panel-people",
          Icon::Shield,
          &ctx.t("memory.panel.peopleNote"),
        )),
    );
  let name = ctx.t_args("memory.panel.name", [("project", &*state.project)]);
  frame(
    "memory-panel",
    header(Icon::Brain, &ctx.t("memory.panel.title"), &name),
    body,
    None,
  )
}

/// *Saving agents' records*: how each memory keeps what agents write (`IBIQR`).
fn saving(ctx: &Ctx, settings: &MemorySettings) -> Column {
  let policy = |policy: WritePolicy| {
    ctx
      .t(match policy {
        WritePolicy::Automatic => "memory.panel.automatic",
        WritePolicy::AfterApproval => "memory.panel.afterApproval",
      })
      .to_string()
  };
  inspector::section("memory-panel-saving", SpacingSize::Sm)
    .child(inspector::section_title(
      "memory-panel-saving-title",
      &ctx.t("memory.panel.saving"),
      None,
    ))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .child(inspector::property(
          "memory-panel-project-writes",
          &ctx.t("memory.panel.projectMemory"),
          &policy(settings.project_writes),
          PropertyValue::Plain,
        ))
        .child(inspector::property(
          "memory-panel-task-writes",
          &ctx.t("memory.panel.taskMemory"),
          &policy(settings.task_writes),
          PropertyValue::Plain,
        )),
    )
}

/// *Given to each agent*: the record and token limits, and where they change (`t6KII`).
fn limits(ctx: &Ctx, settings: &MemorySettings) -> Column {
  inspector::section("memory-panel-limits", SpacingSize::Sm)
    .child(inspector::section_title(
      "memory-panel-limits-title",
      &ctx.t("memory.panel.limits"),
      None,
    ))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .child(inspector::property(
          "memory-panel-records-limit",
          &ctx.t("memory.panel.recordsLimit"),
          &ctx.t_args(
            "memory.panel.atMost",
            [("count", settings.limits.records().to_string())],
          ),
          PropertyValue::Plain,
        ))
        .child(inspector::property(
          "memory-panel-tokens-limit",
          &ctx.t("memory.panel.tokensLimit"),
          &ctx.t_args(
            "memory.panel.atMost",
            [("count", grouped(ctx, settings.limits.tokens()))],
          ),
          PropertyValue::Plain,
        )),
    )
    .child(settings_link(ctx, "memory-panel-settings-link"))
}

/// *Change in Project settings › Memory ↗*: that page is not built yet, so the link says so instead of opening it.
pub(crate) fn settings_link(ctx: &Ctx, id: &str) -> Element {
  parts::unbuilt_link(id, &ctx.t("memory.settingsLink"), &ctx.t("memory.settingsUnbuilt"))
}

/// How agents' records are saved and what each gets, in one line (`eNKSL` in the empty state).
pub(super) fn saving_summary(ctx: &Ctx, settings: &MemorySettings) -> Arc<str> {
  let key = match (settings.project_writes, settings.task_writes) {
    (WritePolicy::AfterApproval, WritePolicy::AfterApproval) => "memory.empty.savingBoth",
    (WritePolicy::Automatic, WritePolicy::Automatic) => "memory.empty.savingNeither",
    (WritePolicy::AfterApproval, WritePolicy::Automatic) => "memory.empty.savingProject",
    (WritePolicy::Automatic, WritePolicy::AfterApproval) => "memory.empty.savingTask",
  };
  ctx.t_args(
    key,
    [
      ("records", settings.limits.records().to_string()),
      ("tokens", grouped(ctx, settings.limits.tokens())),
    ],
  )
}

/// A count with its thousands grouped as the panels show them (*3,000*).
pub(crate) fn grouped(ctx: &Ctx, count: u32) -> String {
  let digits = count.to_string();
  orchester_core::tokens::regroup(&digits, &ctx.t("memory.groupSeparator")).unwrap_or(digits)
}
