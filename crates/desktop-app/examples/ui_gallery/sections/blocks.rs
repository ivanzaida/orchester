//! Notices (`Notice` `R3Pyx` in each tone), cards, a table (`Task list header` `FjOYw`, `Task list row`
//! `XEQuo`), the empty, loading and failed states of a list (`LeIOm`, `O8pgq`, `kXWCj`), the command box
//! (`MOPAN`) and Markdown.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  node::{Element, dimension::Dimension},
};
use orchester_app::{
  theme::ExtraTypography,
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    chip::TagChip,
    code_box::CodeBox,
    detail_row::{DetailValue, detail_row},
    icon::Icon,
    markdown::MarkdownView,
    notice::{Notice, NoticeTone},
    state::{EmptyState, load_error, loading},
    status::{Status, StatusChip},
    table::{ColumnWidth, TableRow, table_header},
    tooltip::Truncated,
  },
};

use super::{line, stack, wide_cell};
use crate::gallery::State;

/// The blocks' width: the Tasks screen's content at 1440.
const CONTENT: f32 = 1100.0;
const HALF: f32 = 540.0;
/// The table's fixed columns (`FjOYw`): key, pipeline, run.
const KEY: f32 = 52.0;
const PIPELINE: f32 = 128.0;
const RUN: f32 = 168.0;

const PLAN: &str = "## Approach\n\n1. Wrap `send()` in a retry loop: exponential backoff from 500 ms, at most 5 attempts.\n2. Honor **Retry-After** for 429 and 503.\n3. Record each attempt in `webhook_attempts`.\n\n> Ask before adding a dependency.\n\nSee [the retry notes](https://example.test/retry).";

pub fn section(ctx: &mut Ctx, state: &State) -> Element {
  let press = |what: &'static str| {
    let pressed = state.pressed.clone();
    move || pressed.set(what.to_owned())
  };
  stack([
    line([wide_cell(
      "Notice · R3Pyx: info, warning, danger, success",
      CONTENT,
      notices(state),
    )])
    .into(),
    line([wide_cell("Card surfaces", CONTENT, cards())]).into(),
    line([wide_cell(
      "Task list header · FjOYw, Task list row · XEQuo",
      CONTENT,
      table(ctx, state),
    )])
    .into(),
    line([
      wide_cell("Empty project · LeIOm", HALF, empty(press("New task"))),
      wide_cell("Reading · O8pgq", HALF, loading("g-loading", "Reading Kontur's tasks…")),
    ])
    .into(),
    line([wide_cell(
      "Notice · Tasks cannot be read · kXWCj",
      CONTENT,
      load_error(
        ctx,
        "g-load-error",
        (
          "Tasks cannot be read",
          "Reading the task list from the project database failed: database is locked. No task was changed.",
        ),
        press("Try again"),
      ),
    )])
    .into(),
    line([
      wide_cell(
        "Command · MOPAN",
        HALF,
        CodeBox::new("g-command", "Command", "cargo add backoff@0.4 --features tokio").render(ctx),
      ),
      wide_cell(
        "Markdown · plan text (Q5J0r)",
        HALF,
        MarkdownView::new("g-markdown", PLAN, {
          let pressed = state.pressed.clone();
          move |link: &str| pressed.set(format!("link {link}"))
        })
        .style(ExtraTypography::Lead.role(), PaletteColor::TextSecondary)
        .render(ctx),
      ),
    ])
    .into(),
  ])
}

fn notices(state: &State) -> Column {
  let (first, second) = (state.pressed.clone(), state.pressed.clone());
  let continue_all = ActionButton::new("g-notice-continue", ButtonKind::Primary, "Continue all")
    .icon(Icon::Play)
    .on_click(move || first.set("Continue all".to_owned()));
  let review = ActionButton::new("g-notice-review", ButtonKind::Secondary, "Review runs")
    .on_click(move || second.set("Review runs".to_owned()));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      Notice::new("g-notice-info", NoticeTone::Info, "Implement is running · attempt 2")
        .icon(Icon::LoaderCircle)
        .body("Builder works in kon-12-webhook-retry. The Summary updates as steps finish."),
    )
    .child(
      Notice::new(
        "g-notice-warning",
        NoticeTone::Warning,
        "2 runs were interrupted when Orchester closed",
      )
      .body("Their worktrees and commits are kept. Continue them from their last checkpoint or leave them stopped.")
      .action(review)
      .action(continue_all),
    )
    .child(
      Notice::new("g-notice-danger", NoticeTone::Danger, "The runner could not restart")
        .body("It stopped 5 times in 2 minutes. Open its log to see why."),
    )
    .child(Notice::new("g-notice-success", NoticeTone::Success, "Merged into main").icon(Icon::GitMerge))
}

fn cards() -> Row {
  let sample = |id: &str, surface: CardSurface, title: &str| {
    card(id, surface).width(Dimension::Px(200.0)).child(
      Text::new(title)
        .variant(ExtraTypography::Strong.role())
        .color(PaletteColor::TextPrimary),
    )
  };
  Row::new()
    .spacing(SpacingSize::Md)
    .child(sample("g-card-panel", CardSurface::Panel, "Panel · result"))
    .child(sample("g-card-raised", CardSurface::Raised, "Raised · metric"))
    .child(sample("g-card-outline", CardSurface::Outline, "Outline · list"))
    .child(sample("g-card-decision", CardSurface::Decision, "Decision"))
    .child(sample("g-card-attention", CardSurface::Attention, "Attention · waits"))
}

fn table(ctx: &mut Ctx, state: &State) -> Column {
  let columns = [
    (ColumnWidth::Fixed(KEY), Some("ID")),
    (ColumnWidth::Fill, Some("TASK / TAGS")),
    (ColumnWidth::Fixed(PIPELINE), Some("PIPELINE")),
    (ColumnWidth::Fixed(RUN), Some("WORKFLOW / RUN")),
  ];
  let rows = [
    ("KON-12", "Add retry to webhook sender", "webhooks", Status::NeedsInput),
    (
      "KON-23",
      "Explain an expired GitHub token in the connection inspector and offer to sign in again",
      "auth",
      Status::StoppedAtLimit,
    ),
    ("KON-24", "Import contacts from CSV", "import", Status::Unreadable),
  ];
  let mut table = Column::new()
    .width(Dimension::Pct(100.0))
    .child(table_header("g-table-header", &columns, true));
  for (index, (key, title, tag, status)) in rows.into_iter().enumerate() {
    let task = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(Truncated::new(format!("g-row-{index}-title"), title, ExtraTypography::Strong.role()).render(ctx))
      .child(TagChip::new(format!("g-row-{index}-tag"), tag));
    let row = TableRow::new(format!("g-row-{index}"))
      .cell(
        ColumnWidth::Fixed(KEY),
        Text::new(key).variant(ExtraTypography::MonoKey.role()),
      )
      .cell(ColumnWidth::Fill, task)
      .cell(
        ColumnWidth::Fixed(PIPELINE),
        Text::new("Feature delivery v3").variant(TypographyStyle::Description),
      )
      .cell(
        ColumnWidth::Fixed(RUN),
        StatusChip::new(format!("g-row-{index}-run"), status)
          .compact()
          .render(ctx),
      )
      .on_open({
        let pressed = state.pressed.clone();
        move || pressed.set(format!("open {key}"))
      });
    table = table.child(row);
  }
  table
}

fn empty(new_task: impl Fn() + Send + Sync + 'static) -> Element {
  EmptyState::new(
    "g-empty",
    Icon::ListPlus,
    "No tasks yet",
    "Describe what agents should build as a task. Drafts can be saved before every detail is known.",
  )
  .action(
    ActionButton::new("g-empty-new", ButtonKind::Primary, "New task")
      .icon(Icon::Plus)
      .on_click(new_task),
  )
  .child(
    Column::new()
      .width(Dimension::Pct(100.0))
      .child(Text::new("Ready to run").variant(TypographyStyle::Caption))
      .child(detail_row(
        "g-empty-agents",
        "Agents",
        "Explorer, Planner, Builder, Reviewer",
        DetailValue::Text,
      ))
      .child(detail_row(
        "g-empty-pipeline",
        "Pipeline",
        "Feature delivery v3 published",
        DetailValue::Text,
      )),
  )
  .into()
}
