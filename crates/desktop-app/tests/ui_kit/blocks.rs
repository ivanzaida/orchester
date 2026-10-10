//! Notices, cards, tables, empty, loading and failed states, code boxes, Markdown, menus, popovers and
//! tooltips.

use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize},
  components::{Column, Row, Text},
  core::Signal,
  node::{dimension::Dimension, padding::Padding},
};
use orchester_app::{
  theme::{ExtraRadius, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    chip::TagChip,
    code_box::CodeBox,
    icon::Icon,
    markdown::MarkdownView,
    menu::{MenuItem, menu, menu_panel},
    notice::{Notice, NoticeTone},
    popover::{PopoverSide, popover, popover_panel},
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
    let still = TableRow::new("still")
      .cell(ColumnWidth::Fixed(52.0), Text::new("KON-13"))
      .cell(ColumnWidth::Fill, Text::new("Unreadable run"))
      .cell(
        ColumnWidth::Fixed(128.0),
        Text::new("Quick fix v2").id("still-pipeline"),
      )
      .in_openable_table();
    Column::new()
      .width(Dimension::Px(CONTENT))
      .child(table_header("header", &columns, true))
      .child(row)
      .child(still)
      .into()
  });
  assert_eq!(
    kit.bounds("still-pipeline").x,
    kit.bounds("pipeline").x,
    "a row that does not open keeps the chevron's column, so its cells line up"
  );
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
fn a_highlighted_row_is_tinted_and_keeps_its_cells_where_they_were() {
  let mut kit = Kit::show(|_| {
    let row = |id: &str, key: &str| {
      TableRow::new(id)
        .cell(ColumnWidth::Fixed(52.0), Text::new(key).id(format!("{id}-key")))
        .cell(ColumnWidth::Fill, Text::new("Show token spend per step"))
        .on_open(|| {})
    };
    Column::new()
      .width(Dimension::Px(CONTENT))
      .child(row("new", "KON-37").highlighted())
      .child(row("old", "KON-36"))
      .into()
  });
  let tinted = |kit: &mut Kit, id: &str| {
    let element = kit.tree.get_element_by_id(id).expect("the row is shown");
    element.children().iter().any(|child| child.has_class("highlighted"))
  };
  assert!(tinted(&mut kit, "new"));
  assert!(!tinted(&mut kit, "old"));
  assert_eq!(kit.bounds("new-key").x, kit.bounds("old-key").x);
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
          Arc::new(|_: &str| {}),
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
fn a_menu_row_is_28_high_8_in_an_unavailable_row_says_why_and_a_choice_closes_it() {
  let open = Signal::new(true);
  let shown = open.clone();
  let chosen = Arc::new(AtomicUsize::new(0));
  let choose = chosen.clone();
  let mut kit = Kit::show(move |ctx| {
    let anchor = ctx.element_ref();
    let choose = choose.clone();
    let rows = [
      MenuItem::new("feature", "Feature delivery v3", move || {
        choose.fetch_add(1, Ordering::SeqCst);
      })
      .icon(Icon::Workflow)
      .chosen(true)
      .render(ctx, &shown),
      MenuItem::new("quick", "Quick fix v2", || {})
        .icon(Icon::Workflow)
        .shortcut("Ctrl 2")
        .render(ctx, &shown),
      MenuItem::unavailable("draft", "Release v1", "Not published yet")
        .icon(Icon::Workflow)
        .render(ctx, &shown),
    ];
    let panel = menu_panel("menu").width(Dimension::Px(300.0)).with_children(rows);
    Row::new()
      .child(
        Row::new()
          .id("anchor")
          .ref_element(anchor.clone())
          .child(Text::new("Pipeline")),
      )
      .child(menu(anchor, shown.clone(), panel))
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
  kit.click("draft");
  assert!(open.get(), "an unavailable row keeps the menu open");
  kit.click("feature");
  assert!(!open.get(), "a choice closes the menu");
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
      .child(popover(anchor, open.clone(), PopoverSide::Below, panel))
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
fn text_cut_to_its_room_keeps_its_whole_value_in_a_tooltip_that_wraps() {
  let title =
    "Explain an expired GitHub token in the connection inspector, offer to sign in again and say what changed";
  let mut kit = Kit::show(move |ctx| {
    Column::new()
      .child(
        Row::new()
          .id("cell")
          .width(Dimension::Px(200.0))
          .child(Truncated::new("title", title, ExtraTypography::Strong.role()).render(ctx)),
      )
      .child(Truncated::new("short", "Import contacts", ExtraTypography::Strong.role()).render(ctx))
      .into()
  });
  assert!(kit.bounds("title").width <= 200.0, "the text stays in its room");
  assert!(!kit.exists("title-tooltip"));
  kit.rest_on("short");
  assert!(!kit.exists("short-tooltip"), "text that fits gets no tooltip");
  kit.rest_on("title");
  assert_eq!(kit.texts("title-tooltip"), [title]);
  assert_eq!(
    kit.texts("cell"),
    [title],
    "the row says the title once, also while it is measured"
  );
  let panel = kit.bounds("title-tooltip");
  assert!(
    panel.width <= 400.0 + 16.0 + 0.5,
    "the tooltip keeps to its width: {}",
    panel.width
  );
  assert!(panel.height > 30.0, "its words wrap onto more lines: {}", panel.height);
}

#[test]
fn cut_text_with_a_hint_shows_it_whether_or_not_it_is_cut() {
  let name = "Kontur platform, billing and the partner portal";
  let mut kit = Kit::show(move |ctx| {
    let text =
      |id: &str| Truncated::new(id.to_owned(), name, ExtraTypography::Strong.role()).hint("H:/projects/kontur", "both");
    Column::new()
      .child(Row::new().width(Dimension::Px(120.0)).child(text("cut").render(ctx)))
      .child(text("fits").render(ctx))
      .into()
  });
  kit.rest_on("fits");
  assert_eq!(
    kit.texts("fits-tooltip"),
    ["H:/projects/kontur"],
    "text that fits says its hint"
  );
  kit.rest_on("cut");
  assert_eq!(kit.texts("cut-tooltip"), ["both"], "cut text says the hint for its cut");
}

#[test]
fn framed_cut_text_hangs_its_tooltip_from_the_control_it_names() {
  let mut kit = Kit::show(|ctx| {
    Truncated::new("name", "Kontur", ExtraTypography::Strong.role())
      .hint("H:/projects/kontur", "Kontur · H:/projects/kontur")
      .mono()
      .framed(PaletteColor::TextPrimary, |row, _| {
        row
          .id("control")
          .padding(Padding::horizontal(SpacingSize::Sm))
          .child(Text::new("before").id("before"))
      })
      .render(ctx)
  });
  let (control, before) = (kit.bounds("control"), kit.bounds("before"));
  assert_eq!(
    before.x - control.x,
    8.0,
    "the frame's room and parts come before the text"
  );
  kit.rest_on("before");
  assert_eq!(kit.texts("name-tooltip"), ["H:/projects/kontur"]);
  assert_eq!(kit.bounds("name-tooltip").x, control.x, "from the control's start");
}

#[test]
fn resting_on_cut_text_does_not_move_its_row() {
  let mut kit = Kit::show(|ctx| {
    Row::new()
      .width(Dimension::Px(900.0))
      .spacing(SpacingSize::Sm)
      .child(Truncated::new("fits", "Add retry to webhook sender", ExtraTypography::Strong.role()).render(ctx))
      .child(TagChip::new("tag", "webhooks"))
      .into()
  });
  let before = kit.bounds("tag");
  kit.rest_on("fits");
  assert_eq!(kit.bounds("tag"), before, "the tag stays where it was");
  assert!(!kit.exists("fits-tooltip"));
}
