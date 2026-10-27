//! The Settings tab's form (`05d Memory settings` `KwGDK`): its note with the save state (`Note` `Xklz7`); *Saving*,
//! a row per memory with its two option cards (`Saving` `d3Dpf`); *Given to each agent*, the record and token limits
//! beside what they mean, read back under them (`qzWNa`); and the two quiet notes under them (`BFkJm`, `MkzqV`). At
//! most 760 wide, so it narrows with the window.

use std::path::Path;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  memory::{MemoryLimits, MemorySettings, WritePolicy},
  tokens::compact_tokens,
};

use super::{Scope, SettingsProps, SettingsTab, Status};
use crate::{
  screens::hub::parts,
  theme::{ExtraSpacing, ExtraTypography, HubSpacing, HubText, IconSize},
  ui::{
    field::{NumberField, field_error},
    icon::{Icon, icon},
    message::message_text,
    option_card::OptionCard,
    tinted_note::person_only,
  },
};

/// The form's width at most (`Pane` `v4enD3`).
const FORM_WIDTH: f32 = 760.0;
/// The limit fields' boxes with their units (`y5lYd`, `DTz3H`).
const RECORDS_WIDTH: f32 = 141.0;
const TOKENS_WIDTH: f32 = 138.0;

pub(super) fn form(ctx: &mut Ctx, tab: &SettingsTab, (props, base): (&SettingsProps, MemorySettings)) -> Element {
  let repo = props.project.open.repo_root.clone();
  let name = props.project.open.project.name().as_str().to_owned();
  let saving = parts::group(
    "memory-settings-saving",
    parts::head(
      "memory-settings-saving",
      &ctx.t("memory.settings.savingSection"),
      None,
      None,
    ),
    parts::boxed(
      "memory-settings-saving-rows",
      [
        memory_row(
          ("memory-settings-task", &ctx.t("memory.settings.taskMemory")),
          &ctx.t("memory.settings.taskMemoryBody"),
          policies(ctx, tab, (Scope::Task, base.task_writes), &repo),
        ),
        memory_row(
          ("memory-settings-project", &ctx.t("memory.settings.projectMemory")),
          &ctx.t_args("memory.settings.projectMemoryBody", [("project", name.as_str())]),
          policies(ctx, tab, (Scope::Project, base.project_writes), &repo),
        ),
      ],
    ),
  );
  let records = records_field(ctx, tab, &repo);
  let tokens = tokens_field(ctx, tab, &repo);
  let limits = parts::group(
    "memory-settings-given",
    parts::head("memory-settings-given", &ctx.t("memory.settings.given"), None, None),
    parts::boxed(
      "memory-settings-given-rows",
      [
        parts::field(
          "memory-settings-records",
          &ctx.t("memory.settings.records"),
          Some(&ctx.t("memory.settings.recordsBody")),
          records,
        )
        .into(),
        parts::field(
          "memory-settings-tokens",
          &ctx.t("memory.settings.tokens"),
          Some(&ctx.t("memory.settings.tokensBody")),
          tokens,
        )
        .into(),
      ],
    ),
  );
  Column::new()
    .id("memory-settings")
    .width(Dimension::Pct(100.0))
    .max_width(Dimension::Px(FORM_WIDTH))
    .spacing(HubSpacing::Sections.role())
    .child(head(ctx, tab.status(base), &name))
    .with_children(failure(ctx, tab))
    .child(saving)
    .child(limits)
    .child(parts::note(
      "memory-settings-next",
      Icon::History,
      &ctx.t("memory.settings.nextNote"),
    ))
    .child(parts::note(
      "memory-settings-people",
      Icon::Users,
      &ctx.t("memory.settings.peopleNote"),
    ))
    .into()
}

/// A memory's row (`Task memory` `DSVQx`): its name and what it holds, then its two cards under them.
fn memory_row((id, label): (&str, &str), hint: &str, cards: Row) -> Element {
  Column::new()
    .id(format!("{id}-row"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
    .spacing(ExtraSpacing::Loose.role())
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(label)
            .id(format!("{id}-label"))
            .variant(ExtraTypography::Strong.role()),
        )
        .child(
          Text::new(hint)
            .id(format!("{id}-hint"))
            .variant(HubText::Hint.role())
            .width(Dimension::Pct(100.0)),
        ),
    )
    .child(cards)
    .into()
}

/// What the tab is, and its save state at the end (`x1fbsp`).
fn head(ctx: &Ctx, status: Status, project: &str) -> Row {
  let (key, glyph, tone) = match status {
    Status::Saved => ("memory.settings.saved", Icon::Check, PaletteColor::TextMuted),
    Status::Saving => (
      "memory.settings.savingNow",
      Icon::LoaderCircle,
      PaletteColor::TextSecondary,
    ),
    Status::Unsaved => (
      "memory.settings.unsaved",
      Icon::CircleAlert,
      PaletteColor::TextSecondary,
    ),
    Status::Invalid => ("memory.settings.invalid", Icon::CircleAlert, PaletteColor::Danger),
    Status::Failed => ("memory.settings.failed", Icon::CircleAlert, PaletteColor::Danger),
  };
  let words = if tone == PaletteColor::TextMuted {
    PaletteColor::TextSecondary
  } else {
    tone.clone()
  };
  Row::new()
    .id("memory-settings-head")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t_args("memory.tabs.settingsNote", [("project", project)]))
        .id("memory-tab-note")
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .variant(ExtraTypography::Prose.role()),
    )
    .child(
      Row::new()
        .id("memory-settings-status")
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Sm, tone))
        .child(
          Text::new(&ctx.t(key))
            .id("memory-settings-status-label")
            .variant(TypographyStyle::Label)
            .color(words)
            .nowrap(),
        ),
    )
}

/// Why the last save did nothing, under the head: synthetic input asked for it, or it failed.
fn failure(ctx: &Ctx, tab: &SettingsTab) -> Option<Row> {
  if tab.refused.get() {
    return Some(person_only(
      "memory-settings-person-only",
      &ctx.t("memory.settings.personOnly"),
    ));
  }
  let state = tab.save.state().get();
  if state.is_pending() {
    return None;
  }
  let error = state.error?;
  Some(field_error("memory-settings-error", &message_text(ctx, &error)))
}

/// A memory's two ways of saving agents' records, the default one saying so (`bJdmg`).
fn policies(ctx: &Ctx, tab: &SettingsTab, (scope, chosen): (Scope, WritePolicy), repo: &Path) -> Row {
  let pending = tab.save.state().get().is_pending();
  let default = match scope {
    Scope::Task => MemorySettings::DEFAULT.task_writes,
    Scope::Project => MemorySettings::DEFAULT.project_writes,
  };
  let card = |policy: WritePolicy| {
    let (glyph, title, title_default, element) = match policy {
      WritePolicy::Automatic => (
        Icon::Zap,
        "memory.settings.automatic",
        "memory.settings.automaticDefault",
        "automatic",
      ),
      WritePolicy::AfterApproval => (
        Icon::UserRoundCheck,
        "memory.settings.afterApproval",
        "memory.settings.afterApprovalDefault",
        "after-approval",
      ),
    };
    let description = match (scope, policy) {
      (Scope::Task, WritePolicy::Automatic) => "memory.settings.taskAutomatic",
      (Scope::Task, WritePolicy::AfterApproval) => "memory.settings.taskAfterApproval",
      (Scope::Project, WritePolicy::Automatic) => "memory.settings.projectAutomatic",
      (Scope::Project, WritePolicy::AfterApproval) => "memory.settings.projectAfterApproval",
    };
    let scope_name = match scope {
      Scope::Task => "task",
      Scope::Project => "project",
    };
    let (choose, repo) = (tab.clone(), repo.to_path_buf());
    let card = OptionCard::new(
      format!("memory-settings-{scope_name}-{element}"),
      (glyph, ctx.t(if policy == default { title_default } else { title })),
      ctx.t(description),
      policy == chosen,
      move || choose.choose(repo.clone(), (scope, policy)),
    );
    let card = if pending { card.waiting() } else { card };
    Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(card)
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Stretch)
    .child(card(WritePolicy::Automatic))
    .child(card(WritePolicy::AfterApproval))
}

/// *Records per invocation*: from 1 to 50, 8 unless changed (`NZ4YJ`).
fn records_field(ctx: &mut Ctx, tab: &SettingsTab, repo: &Path) -> Element {
  let (min, max) = (MemoryLimits::RECORDS.start(), MemoryLimits::RECORDS.end());
  let read_back = ctx.t_args(
    "memory.settings.recordsReadBack",
    [
      ("min", min.to_string()),
      ("max", max.to_string()),
      ("default", MemoryLimits::DEFAULT.records().to_string()),
    ],
  );
  let field = NumberField::new(
    "memory-settings-records-field",
    ctx.t("memory.settings.records"),
    tab.records.clone(),
    ctx.t("memory.settings.recordsUnit"),
    read_back,
  )
  .width(Dimension::Px(RECORDS_WIDTH));
  limit_field(ctx, tab, (field, tab.read_records().err()), repo)
}

/// *Tokens per invocation*: from 500 to 20,000, 3,000 unless changed (`V1eYs`).
fn tokens_field(ctx: &mut Ctx, tab: &SettingsTab, repo: &Path) -> Element {
  let range = [
    ("min", tab.grouped(*MemoryLimits::TOKENS.start())),
    ("max", tab.grouped(*MemoryLimits::TOKENS.end())),
  ];
  let read_back = match tab.read_tokens() {
    Ok(tokens) => {
      let key = if tokens == MemoryLimits::DEFAULT.tokens() {
        "memory.settings.tokensReadBackDefault"
      } else {
        "memory.settings.tokensReadBack"
      };
      let [min, max] = range;
      ctx.t_args(key, [("tokens", compact_tokens(u64::from(tokens))), min, max])
    }
    Err(_) => ctx.t_args("memory.settings.tokensRange", range),
  };
  let field = NumberField::new(
    "memory-settings-tokens-field",
    ctx.t("memory.settings.tokens"),
    tab.tokens.clone(),
    ctx.t("memory.settings.tokensUnit"),
    read_back,
  )
  .grouped(tab.separator.clone())
  .width(Dimension::Px(TOKENS_WIDTH));
  limit_field(ctx, tab, (field, tab.read_tokens().err()), repo)
}

/// A limit's field at its row's end, saved when it is left, saying why its value is not one in place of its read-back.
fn limit_field(
  ctx: &mut Ctx,
  tab: &SettingsTab,
  (field, error): (NumberField, Option<orchester_workspace::messages::Message>),
  repo: &Path,
) -> Element {
  let (commit, repo) = (tab.clone(), repo.to_path_buf());
  let mut field = field
    .read_back_alignment(Alignment::End)
    .on_leave(move || commit.commit_limits(repo.clone()));
  if let Some(error) = error {
    field = field.error(message_text(ctx, &error));
  }
  Column::new()
    .flex_shrink(0.0)
    .align_items(Alignment::End)
    .child(field.render(ctx))
    .into()
}
