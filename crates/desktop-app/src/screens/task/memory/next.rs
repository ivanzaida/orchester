//! *Next agent receives* (`uE0VS` in `a0PLO`): the step whose next invocation it is and who runs it, the records
//! and tokens it gets against the project's limits, the records grouped by why they are given (latest handoff,
//! pinned, decisions and warnings, best matches) with their tokens, what the limits left out, how agents' records
//! are saved, and that ranking is lexical and waiting proposals are never given. With no agent step working or
//! waiting to start, it says nothing is chosen; when task memory cannot be read, that it cannot show it.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ProjectId,
  memory::{InjectionReason, MemoryScope, MemorySettings, WritePolicy},
};
use orchester_workspace::memory::{
  handoff::HandoffParts,
  next::{Given, NextAgent},
  read::TaskMemory,
};

use crate::{
  screens::memory::{group_heading, words},
  theme::{ExtraSpacing, ExtraTypography, IconSize, MemoryText},
  ui::{
    icon::{Icon, icon},
    meter::{MeterTone, meter},
  },
};

/// The panel's width (`uE0VS`), a saving row's label (`R2TxT`).
pub const PANEL_WIDTH: f32 = 320.0;
const SAVING_LABEL_WIDTH: f32 = 110.0;

/// The panel of `project`, as `memory` says it; `None` while task memory cannot be read.
pub fn panel(ctx: &mut Ctx, project: ProjectId, memory: Option<&TaskMemory>) -> Element {
  let mut panel = Column::new()
    .id("memory-next")
    .width(Dimension::Px(PANEL_WIDTH))
    .height(Dimension::Pct(100.0))
    .flex_shrink(0.0)
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Md)
    .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border));
  let Some(memory) = memory else {
    return panel
      .child(head(ctx, None))
      .child(left_out_line(&ctx.t("memory.next.unreadable")))
      .into();
  };
  panel = panel.child(head(ctx, memory.next.as_ref()));
  match &memory.next {
    Some(next) => {
      panel = panel.child(limits(ctx, next)).child(given(ctx, next, memory));
      panel = panel.with_children(left_out(ctx, next).map(|words| left_out_line(&words)));
    }
    None => panel = panel.child(left_out_line(&ctx.t("memory.next.none"))),
  }
  panel
    .child(saving(ctx, &memory.settings))
    .child(crate::screens::memory::settings_link(
      ctx,
      project,
      "memory-next-settings",
    ))
    .child(
      Text::new(&ctx.t("memory.next.footnote"))
        .id("memory-next-footnote")
        .variant(TypographyStyle::Caption),
    )
    .into()
}

/// *Next agent receives*, the step and its agent, and how the records are chosen (`r67ViM`).
fn head(ctx: &Ctx, next: Option<&NextAgent>) -> Column {
  let step = next.map(|next| {
    words::join(
      ctx,
      &[
        words::step(ctx, next.step.number, &next.step.name).to_string(),
        next.step.agent.to_string(),
      ],
    )
  });
  let sub = match next {
    Some(next) => ctx.t_args("memory.next.chosenFor", [("agent", &*next.step.agent)]),
    None => ctx.t("memory.next.chosen"),
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .align_items(Alignment::Center)
        .child(Text::new(&ctx.t("memory.next.title")).variant(ExtraTypography::InspectorSection.role()))
        .child(Spacer::new().flex(1.0))
        .with_children(step.map(|step| {
          Text::new(&step)
            .id("memory-next-step")
            .variant(TypographyStyle::Caption)
            .nowrap()
        })),
    )
    .child(Text::new(&sub).variant(TypographyStyle::Caption))
}

/// *Records 8 of 8* and *Tokens ≈ 2.1k of 3k*, each with its meter (`fP7Lf`).
fn limits(ctx: &Ctx, next: &NextAgent) -> Column {
  let records = next.given.len();
  let limit = |id: &str, label: &str, value: &str, used: (u64, u64)| {
    Column::new()
      .id(id.to_owned())
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .align_items(Alignment::Center)
          .child(Text::new(label).variant(ExtraTypography::Property.role()))
          .child(Spacer::new().flex(1.0))
          .child(
            Text::new(value)
              .id(format!("{id}-value"))
              .variant(ExtraTypography::PropertyMono.role()),
          ),
      )
      .child(meter(&format!("{id}-meter"), used, MeterTone::Plain))
  };
  let records_value = ctx.t_args(
    "memory.next.of",
    [
      ("used", records.to_string()),
      ("limit", next.limits.records().to_string()),
    ],
  );
  let tokens_value = ctx.t_args(
    "memory.next.tokensOf",
    [
      ("used", words::tokens(ctx, next.tokens()).to_string()),
      ("limit", words::tokens(ctx, next.limits.tokens()).to_string()),
    ],
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(limit(
      "memory-next-records",
      &ctx.t("memory.next.records"),
      &records_value,
      (records as u64, u64::from(next.limits.records())),
    ))
    .child(limit(
      "memory-next-tokens",
      &ctx.t("memory.next.tokens"),
      &tokens_value,
      (u64::from(next.tokens()), u64::from(next.limits.tokens())),
    ))
}

/// The records given, grouped by why (`LiDOj`).
fn given(ctx: &Ctx, next: &NextAgent, memory: &TaskMemory) -> Column {
  let groups: [(&str, &str, &[InjectionReason]); 4] = [
    ("handoff", "memory.next.latestHandoff", &[InjectionReason::Handoff]),
    ("pinned", "memory.next.pinned", &[InjectionReason::Pinned]),
    (
      "decisions",
      "memory.next.decisionsAndWarnings",
      &[InjectionReason::Decision, InjectionReason::Warning],
    ),
    ("matches", "memory.next.bestMatches", &[InjectionReason::Match]),
  ];
  let mut column = Column::new()
    .id("memory-next-given")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role());
  for (key, label, reasons) in groups {
    let entries: Vec<&Given> = next
      .given
      .iter()
      .filter(|given| reasons.contains(&given.reason))
      .collect();
    if entries.is_empty() {
      continue;
    }
    column = column.child(group_heading(
      &format!("memory-next-{key}"),
      &ctx.t(label).to_uppercase(),
      entries.len(),
    ));
    for entry in entries {
      column = column.child(given_row(ctx, entry, memory));
    }
  }
  column
}

/// A record given (`Memory / Given row` `Q0BAI`): its glyph, its key or what it says, *Project* for a project's
/// record, and its tokens.
fn given_row(ctx: &Ctx, given: &Given, memory: &TaskMemory) -> Row {
  let content = given.memory.content();
  let shown = match (content.key(), content.kind()) {
    (Some(key), _) => key.as_str().to_owned(),
    (None, orchester_core::memory::MemoryKind::Handoff) => {
      let parts = HandoffParts::of(content.text());
      let step = memory
        .origins
        .written(&given.memory)
        .and_then(|place| place.step)
        .map(|(number, name)| words::step(ctx, number, &name).to_string());
      let summary = parts.summary.lines().next().unwrap_or_default().to_owned();
      step.map_or_else(
        || summary.clone(),
        |step| words::join(ctx, &[step, summary.clone()]).to_string(),
      )
    }
    (None, _) => content.text().lines().next().unwrap_or_default().to_owned(),
  };
  Row::new()
    .id(format!("memory-next-record-{}", given.memory.id()))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, SpacingSize::Xs))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(
      words::glyph(content.kind()),
      IconSize::Base,
      PaletteColor::TextMuted,
    ))
    .child(
      Text::new(&shown)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .variant(TypographyStyle::Description)
        .nowrap()
        .text_overflow(lurq::components::TextOverflow::Elipsis),
    )
    .with_children((given.memory.scope() == MemoryScope::Project).then(|| {
      Text::new(&ctx.t("memory.scope.project"))
        .variant(MemoryText::Scope.role())
        .nowrap()
    }))
    .child(
      Text::new(&words::tokens(ctx, given.tokens))
        .variant(ExtraTypography::MonoNote.role())
        .nowrap(),
    )
}

/// What the limits left out, if anything (`Ezyea`).
fn left_out(ctx: &Ctx, next: &NextAgent) -> Option<std::sync::Arc<str>> {
  if next.given.is_empty() {
    return Some(ctx.t("memory.next.nothing"));
  }
  let count = |count: usize, one: &str, other: &str| match count {
    0 => None,
    1 => Some(ctx.t(one)),
    count => Some(ctx.t_args(other, [("count", count.to_string())])),
  };
  count(
    next.over_records,
    "memory.next.recordLimitOne",
    "memory.next.recordLimit",
  )
  .or_else(|| count(next.over_tokens, "memory.next.tokenLimitOne", "memory.next.tokenLimit"))
}

fn left_out_line(words: &str) -> Element {
  Row::new()
    .id("memory-next-left-out")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(icon(Icon::CircleMinus, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(words)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .variant(TypographyStyle::Caption),
    )
    .into()
}

/// How the task's and the project's agents' records are saved (`WCIMA`).
fn saving(ctx: &Ctx, settings: &MemorySettings) -> Column {
  let policy = |policy: WritePolicy| {
    ctx.t(match policy {
      WritePolicy::Automatic => "memory.next.automatic",
      WritePolicy::AfterApproval => "memory.next.afterApproval",
    })
  };
  let row = |id: &str, label: &str, value: &str| {
    Row::new()
      .id(id.to_owned())
      .width(Dimension::Pct(100.0))
      .padding(Padding::vertical(SpacingSize::Xs))
      .spacing(SpacingSize::Md)
      .child(
        Text::new(label)
          .width(Dimension::Px(SAVING_LABEL_WIDTH))
          .flex_shrink(0.0)
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted),
      )
      .child(
        Text::new(value)
          .flex(1.0)
          .variant(ExtraTypography::Prose.role())
          .color(PaletteColor::TextPrimary),
      )
  };
  Column::new()
    .id("memory-next-saving")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Sm))
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(row(
      "memory-next-task-saving",
      &ctx.t("memory.panel.taskMemory"),
      &policy(settings.task_writes),
    ))
    .child(row(
      "memory-next-project-saving",
      &ctx.t("memory.panel.projectMemory"),
      &policy(settings.project_writes),
    ))
}
