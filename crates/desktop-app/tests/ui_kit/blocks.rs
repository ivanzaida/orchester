//! Notices, cards, tables, empty, loading and failed states, code boxes, Markdown, menus, popovers and
//! tooltips.

use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};

use lurq::{
  app::theme::{RadiusSize, SpacingSize},
  components::{Column, Placement, Row, Text},
  core::Signal,
  node::dimension::Dimension,
};
use orchester_app::{
  theme::{ExtraRadius, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    code_box::CodeBox,
    icon::Icon,
    markdown::MarkdownView,
    menu::{MenuItem, menu, menu_panel},
    notice::{Notice, NoticeTone},
    popover::{popover, popover_panel},
    state::{EmptyState, load_error, loading},
    table::{ColumnWidth, TableRow, table_header},
    tooltip::Truncated,
  },
};

use crate::harness::{Kit, assert_close, assert_inset, gap_after, gap_under};

/// The content column of a list screen at 1440 (`Content · Tasks` less its padding).
const CONTENT: f32 = 1000.0;

#[test]
fn a_notice_is_12_over_and_14_beside_its_icon_and_text_10_apart() {
  let mut kit = Kit::show(|_| {
    Column::new()
      .width(Dimension::Px(CONTENT))
      .spacing(SpacingSize::Md)
      .child(
        Notice::new(
          "interrupted",
          NoticeTone::Warning,
          "2 runs were interrupted when Orchester closed",
        )
        .body("Their worktrees and commits are kept.")
        .action(ActionButton::new("continue", ButtonKind::Primary, "Continue all").on_click(|| {})),
      )
      .child(Notice::new("merged", NoticeTone::Success, "Merged into main").icon(Icon::GitMerge))
      .into()
  });
  let (notice, glyph, text) = (
    kit.bounds("interrupted"),
    kit.bounds("interrupted-icon"),
    kit.bounds("interrupted-text"),
  );
  assert_close(notice.width, CONTENT, "a notice fills its column");
  assert_close(glyph.x - notice.x, 14.0, "side");
  assert_close(glyph.y - notice.y, 12.0, "top");
  assert_close(glyph.height, 16.0, "icon");
  assert_close(gap_after(glyph, text), 10.0, "icon to text");
  let button = kit.bounds("continue");
  assert_close(
    notice.x + notice.width - button.x - button.width,
    14.0,
    "the action at the end",
  );
  assert_close(gap_after(text, button), 10.0, "text to actions");
  assert_eq!(kit.texts("merged")[0], Icon::GitMerge.glyph().to_string());
  assert_eq!(kit.radius(RadiusSize::Lg), 10.0, "a notice's and a card's radius");
}

#[test]
fn a_card_is_14_over_and_16_beside_its_content() {
  let mut kit = Kit::show(|_| {
    Column::new()
      .width(Dimension::Px(CONTENT))
      .child(card("limit", CardSurface::Decision).child(Text::new("Raise the limit").id("limit-title")))
      .into()
  });
  let (frame, title) = (kit.bounds("limit"), kit.bounds("limit-title"));
  assert_eq!(frame.width, CONTENT);
  assert_inset(
    frame,
    title,
    (16.0, 14.0, frame.width - 16.0 - title.width, 14.0),
    "card",
  );
}

#[test]
fn a_table_has_a_32_header_and_44_rows_12_in_their_cells_12_apart() {
  let opened = Arc::new(AtomicUsize::new(0));
  let open = opened.clone();
  let mut kit = Kit::show(move |_| {
    let columns = [
      (ColumnWidth::Fixed(52.0), Some("ID")),
      (ColumnWidth::Fill, Some("TASK / TAGS")),
      (ColumnWidth::Fixed(128.0), Some("PIPELINE")),
    ];
    let open = open.clone();
    let row = TableRow::new("row")
      .cell(
        ColumnWidth::Fixed(52.0),
        Text::new("KON-12").id("key").variant(ExtraTypography::MonoKey.role()),
      )
      .cell(ColumnWidth::Fill, Text::new("Add retry to webhook sender").id("title"))
      .cell(
        ColumnWidth::Fixed(128.0),
        Text::new("Feature delivery v3").id("pipeline"),
      )
      .on_open(move || {
        open.fetch_add(1, Ordering::SeqCst);
      });
    Column::new()
      .width(Dimension::Px(CONTENT))
      .child(table_header("header", &columns, true))
      .child(row)
      .into()
  });
  let (header, row) = (kit.bounds("header"), kit.bounds("row"));
  assert_eq!((header.height, row.height), (32.0, 44.0));
  assert_eq!(row.width, CONTENT);
  let (key, title, pipeline) = (kit.bounds("key"), kit.bounds("title"), kit.bounds("pipeline"));
  assert_close(key.x - row.x, 12.0, "row start");
  assert_close(title.x - key.x, 52.0 + 12.0, "cells 12 apart");
  assert_close(
    row.x + row.width - pipeline.x,
    128.0 + 12.0 + 14.0 + 12.0,
    "the chevron's column at the end",
  );
  assert_eq!(
    kit.texts("header"),
    ["ID", "TASK / TAGS", "PIPELINE"],
    "the headings, and an empty chevron column"
  );
  assert!(kit.texts("row").contains(&Icon::ChevronRight.glyph().to_string()));
  kit.click("title");
  assert_eq!(opened.load(Ordering::SeqCst), 1, "a press on the row opens it");
}

#[test]
fn empty_loading_and_failed_states_never_look_alike() {
  let retried = Arc::new(AtomicUsize::new(0));
  let retry = retried.clone();
  let mut kit = Kit::show(move |ctx| {
    let retry = retry.clone();
    Column::new()
      .width(Dimension::Px(CONTENT))
      .spacing(SpacingSize::Md)
      .child(
        EmptyState::new(
          "empty",
          Icon::ListPlus,
          "No tasks yet",
          "Describe what agents should build as a task.",
        )
        .action(ActionButton::new("empty-new", ButtonKind::Primary, "New task").on_click(|| {})),
      )
      .child(loading("reading", "Reading Kontur's tasks…"))
      .child(load_error(
        ctx,
        "failed",
        (
          "Tasks cannot be read",
          "Reading the task list failed: database is locked.",
        ),
        move || {
          retry.fetch_add(1, Ordering::SeqCst);
        },
      ))
      .into()
  });
  let (empty, glyph, title, action) = (
    kit.bounds("empty"),
    kit.bounds("empty-glyph"),
    kit.bounds("empty-title"),
    kit.bounds("empty-new"),
  );
  assert_close(glyph.x - empty.x, 28.0, "empty side");
  assert_close(glyph.y - empty.y, 28.0, "empty top");
  assert_eq!((glyph.width, glyph.height), (36.0, 36.0));
  assert_close(gap_under(glyph, title), 14.0, "glyph to title");
  assert_close(empty.y + empty.height - action.y - action.height, 28.0, "empty bottom");
  let (line, label) = (kit.bounds("reading"), kit.bounds("reading-label"));
  assert_close(label.y - line.y, 56.0, "loading top");
  assert_close(gap_after(kit.bounds("reading-icon"), label), 8.0, "loader to words");
  assert_eq!(kit.texts("reading")[0], Icon::LoaderCircle.glyph().to_string());
  assert_eq!(
    kit.texts("failed")[..3],
    [
      Icon::CircleAlert.glyph().to_string(),
      "Tasks cannot be read".to_owned(),
      "Reading the task list failed: database is locked.".to_owned()
    ]
  );
  kit.click("failed-retry");
  assert_eq!(retried.load(Ordering::SeqCst), 1, "Try again reads again");
}

#[test]
fn a_code_box_holds_the_whole_value_8_over_and_10_beside_it_with_copy() {
  let command = "cargo add backoff@0.4 --features tokio".to_owned();
  let shown = command.clone();
  let mut kit = Kit::show(move |ctx| {
    Column::new()
      .width(Dimension::Px(600.0))
      .child(CodeBox::new("command", "Command", shown.clone()).render(ctx))
      .into()
  });
  let (frame, value) = (kit.bounds("command-box"), kit.bounds("command"));
  assert_close(value.x - frame.x, 10.0, "side");
  assert_close(value.y - frame.y, 8.0, "top");
  assert_eq!(kit.texts("command"), [command]);
  assert_close(gap_under(kit.bounds("command-copy"), frame), 6.0, "label row to box");
  assert!(kit.texts("command-copy").contains(&"Copy".to_owned()));
}

#[test]
fn markdown_reads_headings_lists_and_links() {
  let mut kit = Kit::show(|ctx| {
    Column::new()
      .width(Dimension::Px(600.0))
      .child(
        MarkdownView::new(
          "plan",
          "## Approach\n\n1. Wrap `send()` in a retry loop\n2. Record each attempt\n\nSee [the docs](https://example.test/retry).",
          |_| {},
        )
        .render(ctx),
      )
      .into()
  });
  let texts = kit.texts("plan").join("|");
  for part in ["Approach", "Wrap ", "send()", "Record each attempt", "the docs"] {
    assert!(texts.contains(part), "{part} in {texts}");
  }
}

#[test]
fn a_menu_row_is_28_high_8_in_and_an_unavailable_row_says_why() {
  let open = Signal::new(true);
  let chosen = Arc::new(AtomicUsize::new(0));
  let choose = chosen.clone();
  let mut kit = Kit::show(move |ctx| {
    let anchor = ctx.element_ref();
    let choose = choose.clone();
    let panel = menu_panel("menu")
      .width(Dimension::Px(300.0))
      .child(
        MenuItem::new("feature", "Feature delivery v3")
          .icon(Icon::Workflow)
          .chosen(true)
          .on_click(move || {
            choose.fetch_add(1, Ordering::SeqCst);
          }),
      )
      .child(
        MenuItem::new("quick", "Quick fix v2")
          .icon(Icon::Workflow)
          .shortcut("Ctrl 2")
          .on_click(|| {}),
      )
      .child(
        MenuItem::new("draft", "Release v1")
          .icon(Icon::Workflow)
          .unavailable("Not published yet"),
      );
    Row::new()
      .child(
        Row::new()
          .id("anchor")
          .ref_element(anchor.clone())
          .child(Text::new("Pipeline")),
      )
      .child(menu(anchor, open.clone(), panel))
      .into()
  });
  let (panel, feature, quick) = (kit.bounds("menu"), kit.bounds("feature"), kit.bounds("quick"));
  assert_close(feature.x - panel.x, 4.0, "panel padding");
  assert_close(gap_under(feature, quick), 2.0, "rows 2 apart");
  assert_eq!(feature.height, 28.0);
  assert_close(kit.bounds("feature-icon").x - feature.x, 8.0, "row side");
  assert_close(
    gap_under(kit.bounds("anchor"), panel),
    6.0,
    "the menu under its control",
  );
  assert!(
    kit.texts("feature").contains(&Icon::Check.glyph().to_string()),
    "the chosen row's check"
  );
  assert!(kit.texts("quick").contains(&"Ctrl 2".to_owned()));
  let draft = kit.texts("draft");
  assert!(draft.contains(&Icon::Lock.glyph().to_string()) && draft.contains(&"Not published yet".to_owned()));
  kit.click("feature");
  kit.click("draft");
  assert_eq!(chosen.load(Ordering::SeqCst), 1, "only an available row is chosen");
  assert_eq!(kit.radius(ExtraRadius::Tile.role()), 8.0, "a menu's radius");
}

#[test]
fn a_popover_opens_6_under_its_control_14_over_and_16_beside_its_content() {
  let open = Signal::new(true);
  let mut kit = Kit::show(move |ctx| {
    let anchor = ctx.element_ref();
    let panel = popover_panel("permissions", 440.0).child(Text::new("Run permissions").id("permissions-title"));
    Row::new()
      .child(
        Row::new()
          .id("chip")
          .ref_element(anchor.clone())
          .child(Text::new("Edit files")),
      )
      .child(popover(anchor, open.clone(), Placement::BottomStart, panel))
      .into()
  });
  let (chip, panel, title) = (
    kit.bounds("chip"),
    kit.bounds("permissions"),
    kit.bounds("permissions-title"),
  );
  assert_eq!(panel.width, 440.0);
  assert_close(gap_under(chip, panel), 6.0, "under its control");
  assert_close(title.x - panel.x, 16.0, "side");
  assert_close(title.y - panel.y, 14.0, "top");
}

#[test]
fn text_cut_to_its_room_keeps_its_whole_value_in_a_tooltip() {
  let title = "Explain an expired GitHub token in the connection inspector and offer to sign in again";
  let mut kit = Kit::show(move |ctx| {
    Row::new()
      .width(Dimension::Px(200.0))
      .child(Truncated::new("title", title, ExtraTypography::Strong.role()).render(ctx))
      .into()
  });
  assert!(kit.bounds("title").width <= 200.0, "the text stays in its room");
  assert!(!kit.exists("title-tooltip"));
  kit.rest_on("title");
  assert_eq!(kit.texts("title-tooltip"), [title]);
}
