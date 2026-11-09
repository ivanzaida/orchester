//! The comparison of a team MCP server the team changed while an admin edited it (`Compare` in `Agents · MCP server ·
//! admin · draft` A19 `DV8OI`, A20 `CAPhQ`): one row for each field that is not the same in the version the draft was
//! edited from, the team's newer version and the draft, with what *Rebase* makes of it. A field only one side changed
//! says whose it is; one both changed asks, with *Mine* and *Theirs*, and nothing is chosen until the person chooses.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  agents::{
    team_mcp_conflict::Newer,
    team_mcp_merge::{McpField, Pick, Row as Compared, Verdict},
  },
  display,
};

use crate::{
  theme::{ExtraSpacing, HubSpacing, HubText, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// What the person picked for each field both changed.
pub(super) type Picks = Signal<Vec<(McpField, Pick)>>;

/// The element id of a field's row, `team-hub-compare-<field>`.
fn field_id(field: McpField) -> &'static str {
  match field {
    McpField::Name => "name",
    McpField::RunsAs => "runs",
    McpField::Program => "program",
    McpField::Arguments => "arguments",
    McpField::Folder => "folder",
    McpField::Variables => "variables",
    McpField::Url => "url",
    McpField::Header => "header",
    McpField::ExemptTools => "exempt",
  }
}

/// The words of a field's name, as the form calls it.
fn label_key(field: McpField) -> &'static str {
  match field {
    McpField::Name => "team_new.mcp.name",
    McpField::RunsAs => "team_new.mcp.runsAs",
    McpField::Program => "team_new.mcp.program",
    McpField::Arguments => "team_new.mcp.arguments",
    McpField::Folder => "team_new.mcp.folder",
    McpField::Variables => "team_new.mcp.variables",
    McpField::Url => "team_new.mcp.url",
    McpField::Header => "team_new.mcp.header",
    McpField::ExemptTools => "team_new.mcp.exempt",
  }
}

/// A value as a cell says it: arguments in one line, as they run; names apart by commas; *None* where it has nothing.
fn words(ctx: &Ctx, field: McpField, value: &[String]) -> String {
  match field {
    McpField::RunsAs => ctx
      .t(if value.first().is_some_and(|runs| runs == "remote") {
        "team_new.mcp.runsRemote"
      } else {
        "team_new.mcp.runsProgram"
      })
      .to_string(),
    McpField::Folder if value.is_empty() => ctx.t("team_new.mcp.folderWorktree").to_string(),
    _ if value.is_empty() => ctx.t("team_hub.compare.none").to_string(),
    McpField::Arguments => value.join(" "),
    McpField::Variables => value
      .iter()
      .map(|name| {
        ctx
          .t_args("team_hub.mcp.fromTeam.variable", [("name", name.as_str())])
          .to_string()
      })
      .collect::<Vec<_>>()
      .join(", "),
    _ => value.join(", "),
  }
}

/// *· 1 both changed · 1 theirs · 1 yours*: how many fields stand each way, after the head's label.
pub(super) fn counts(ctx: &Ctx, newer: &Newer) -> Arc<str> {
  let comparison = &newer.comparison;
  let mut said: Option<String> = None;
  for (verdict, key) in [
    (Verdict::BothChanged, "team_hub.compare.count.both"),
    (Verdict::TakesTheirs, "team_hub.compare.count.theirs"),
    (Verdict::KeepsMine, "team_hub.compare.count.mine"),
  ] {
    let count = comparison.count(verdict);
    if count == 0 {
      continue;
    }
    let part = ctx.t_args(key, [("count", count.to_string())]).to_string();
    said = Some(match said {
      Some(before) => ctx
        .t_args("team_hub.edit.line.joined", [("lead", before), ("words", part)])
        .to_string(),
      None => part,
    });
  }
  ctx.t_args("team_hub.compare.counts", [("counts", said.unwrap_or_default())])
}

/// The comparison: its head (*Compare* and the counts) over the table.
pub(super) fn section(ctx: &Ctx, newer: &Newer, picks: &Picks) -> Column {
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("team_hub.compare.label"))
        .id("team-hub-compare-label")
        .variant(HubText::PanelLabel.role())
        .nowrap(),
    )
    .child(
      Text::new(&counts(ctx, newer))
        .id("team-hub-compare-counts")
        .variant(HubText::PanelHint.role())
        .nowrap(),
    );
  let rows = newer
    .comparison
    .rows
    .iter()
    .map(|compared| row(ctx, newer, compared, picks));
  let table = Column::new()
    .id("team-hub-compare")
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .clip()
    .child(table_head(ctx, newer))
    .with_children(rows);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(head)
    .child(table)
}

fn cell_row() -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::CompareSide.role())
    .padding(Padding::symmetric(
      HubSpacing::CompareSide.role(),
      HubSpacing::CompareY.role(),
    ))
}

fn table_head(ctx: &Ctx, newer: &Newer) -> Row {
  let version = |number: orchester_core::resource::VersionNumber| number.to_string();
  let theirs = match &newer.published {
    Some(published) => ctx.t_args(
      "team_hub.compare.head.theirs",
      [
        ("version", version(newer.number)),
        ("who", published.by.clone()),
        ("time", display::clock(published.at)),
      ],
    ),
    None => ctx.t_args(
      "team_hub.compare.head.theirsPlain",
      [("version", version(newer.number))],
    ),
  };
  let head = |id: &str, words: &str| {
    Text::new(words)
      .id(format!("team-hub-compare-head-{id}"))
      .variant(HubText::Hint.role())
  };
  cell_row()
    .id("team-hub-compare-head")
    .padding(Padding::symmetric(
      HubSpacing::CompareSide.role(),
      HubSpacing::SubY.role(),
    ))
    .background(PaletteColor::SurfaceRaised)
    .child(
      Column::new()
        .width(Dimension::Px(HubSpacing::CompareKey.value()))
        .child(head(
          "field",
          &ctx.t_args("team_hub.compare.head.field", [("version", version(newer.base_number))]),
        )),
    )
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .child(head("theirs", &theirs)),
    )
    .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(head(
      "yours",
      &ctx.t_args("team_hub.compare.head.yours", [("version", version(newer.base_number))]),
    )))
    .child(
      Column::new()
        .width(Dimension::Px(HubSpacing::CompareResult.value()))
        .child(head("result", &ctx.t("team_hub.compare.head.result"))),
    )
}

fn row(ctx: &Ctx, newer: &Newer, compared: &Compared, picks: &Picks) -> Row {
  let id = format!("team-hub-compare-{}", field_id(compared.field));
  let (theirs_taken, mine_taken) = match compared.verdict {
    Verdict::BothChanged => (true, true),
    Verdict::TakesTheirs => (true, false),
    Verdict::KeepsMine => (false, true),
    Verdict::Same | Verdict::Unchanged => (false, false),
  };
  let value = |side: &str, shown: &[String], taken: bool| {
    Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(
      Text::new(&words(ctx, compared.field, shown))
        .id(format!("{id}-{side}"))
        .variant(HubText::CompareCell.role())
        .color(if taken {
          PaletteColor::TextPrimary
        } else {
          PaletteColor::TextMuted
        })
        .width(Dimension::Pct(100.0)),
    )
  };
  let both = compared.verdict == Verdict::BothChanged;
  let row = cell_row()
    .id(id.clone())
    .describe("verdict", verdict_name(compared.verdict))
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(key_cell(ctx, newer, compared, &id))
    .child(value("theirs", &compared.theirs, theirs_taken))
    .child(value("mine", &compared.mine, mine_taken))
    .child(result_cell(ctx, compared, picks, &id));
  if both {
    row.background(PaletteColor::WarningMuted)
  } else {
    row
  }
}

const fn verdict_name(verdict: Verdict) -> &'static str {
  match verdict {
    Verdict::BothChanged => "both",
    Verdict::TakesTheirs => "theirs",
    Verdict::KeepsMine => "mine",
    Verdict::Same | Verdict::Unchanged => "same",
  }
}

/// The field's name over what the version it was edited from held (`Field` `ht9l0` in A19).
fn key_cell(ctx: &Ctx, newer: &Newer, compared: &Compared, id: &str) -> Column {
  let base = compared.base.as_ref().map(|base| {
    Text::new(&ctx.t_args(
      "team_hub.compare.base",
      [
        ("version", newer.base_number.to_string()),
        ("value", words(ctx, compared.field, base)),
      ],
    ))
    .id(format!("{id}-base"))
    .variant(HubText::CompareBase.role())
    .width(Dimension::Pct(100.0))
  });
  Column::new()
    .width(Dimension::Px(HubSpacing::CompareKey.value()))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Text::new(&ctx.t(label_key(compared.field)))
        .id(format!("{id}-name"))
        .variant(HubText::CompareField.role())
        .width(Dimension::Pct(100.0)),
    )
    .with_children(base)
}

/// What becomes of the field (`Result` `n5P6EH` in A19): a glyph and words, and for a field both changed the choice.
fn result_cell(ctx: &Ctx, compared: &Compared, picks: &Picks, id: &str) -> Column {
  let (glyph, tone, key) = match compared.verdict {
    Verdict::BothChanged => (Icon::CircleAlert, PaletteColor::Warning, "team_hub.compare.result.both"),
    Verdict::TakesTheirs => (Icon::Check, PaletteColor::Success, "team_hub.compare.result.theirs"),
    Verdict::KeepsMine => (Icon::Check, PaletteColor::Success, "team_hub.compare.result.mine"),
    Verdict::Same | Verdict::Unchanged => (Icon::Equal, PaletteColor::TextMuted, "team_hub.compare.result.same"),
  };
  let said = Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Sm, tone))
    .child(
      Text::new(&ctx.t(key))
        .id(format!("{id}-result"))
        .variant(HubText::Status.role())
        .color(if compared.verdict == Verdict::BothChanged {
          PaletteColor::TextPrimary
        } else {
          PaletteColor::TextSecondary
        }),
    );
  Column::new()
    .width(Dimension::Px(HubSpacing::CompareResult.value()))
    .spacing(ExtraSpacing::Hair.role())
    .child(said)
    .with_children((compared.verdict == Verdict::BothChanged).then(|| pick(ctx, compared.field, picks, id)))
}

/// *Mine* and *Theirs* for a field both changed: neither is chosen until the person chooses.
fn pick(ctx: &Ctx, field: McpField, picks: &Picks, id: &str) -> Element {
  let chosen = picks.with(|picks| picks.iter().find(|(picked, _)| *picked == field).map(|(_, side)| *side));
  let segment = |side: Pick, key: &str, name: &str| {
    let picks = picks.clone();
    Segment::new(
      format!("{id}-pick-{name}"),
      SegmentFace::Text(ctx.t(key)),
      chosen == Some(side),
      move || {
        picks.update(|picks| {
          picks.retain(|(picked, _)| *picked != field);
          picks.push((field, side));
        });
      },
    )
  };
  segmented(
    &format!("{id}-pick"),
    [
      segment(Pick::Mine, "team_hub.compare.pick.mine", "mine"),
      segment(Pick::Theirs, "team_hub.compare.pick.theirs", "theirs"),
    ],
    Focus::Keyboard,
  )
  .into()
}

/// The fields both changed that have no pick yet, in the order of the table.
pub(super) fn unresolved(newer: &Newer, picks: &Picks) -> Vec<McpField> {
  picks.with(|picks| {
    newer
      .comparison
      .both_changed()
      .into_iter()
      .filter(|field| !picks.iter().any(|(picked, _)| picked == field))
      .collect()
  })
}

/// The names of `fields` as the form calls them, in a sentence: apart by commas, the last after *and*.
pub(super) fn names(ctx: &Ctx, fields: &[McpField]) -> String {
  let named: Vec<String> = fields
    .iter()
    .map(|field| ctx.t(label_key(*field)).to_string())
    .collect();
  match named.split_last() {
    Some((last, rest)) if !rest.is_empty() => ctx
      .t_args(
        "sharing.config.and",
        [
          ("first", rest.join(&ctx.t("sharing.config.separator"))),
          ("last", last.clone()),
        ],
      )
      .to_string(),
    _ => named.concat(),
  }
}
