//! One transcript row: a step execution header (`mDnc3`) or an entry (`N4SlQq`) — time, glyph,
//! a one-line header with right-aligned meta, optional body text and an optional output block.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::stream::{CommandStatus, FileChangeKind, OutputStream, RunnerProblem},
  tokens::compact_tokens,
};

use super::{
  super::heading::step_state,
  engine,
  rows::{Header, RowKind, TranscriptRow},
};
use crate::{
  display,
  tasks::transcript::{CommandEntry, Entry, EntryKind},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, UNPARSED_OPACITY},
  ui::{
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
    status_label::compact_status_label,
  },
};

/// Time column, glyph tile and the most output lines shown (`qcnZs`, `zu2Sx`).
const TIME_WIDTH: f32 = 36.0;
const GLYPH_SIZE: f32 = 24.0;
const OUTPUT_LINES: usize = 200;

pub(super) struct TranscriptRowView;

impl Component for TranscriptRowView {
  type Props = TranscriptRow;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let row = ctx.props::<TranscriptRow>().clone();
    match &row.kind {
      RowKind::Header(header) => header_row(ctx, &row, header),
      RowKind::Entry { entry, agent, open } => entry_row(ctx, &row, entry, agent, *open),
    }
  }
}

/// Disclosure, step number, step and agent, model and time span, tokens and state (`mDnc3`).
fn header_row(ctx: &Ctx, row: &TranscriptRow, header: &Header) -> Element {
  let start = header.started.map(display::clock).unwrap_or_default();
  let count = header.invocations.to_string();
  let engine = engine(ctx, header.provider, &header.model);
  let meta = match header.ended {
    None => ctx.t_args(
      "runs.activity.headerRunning",
      [("engine", &*engine), ("count", &count), ("start", &start)],
    ),
    Some(ended) => {
      let key = if header.invocations == 1 {
        "runs.activity.headerFinished.one"
      } else {
        "runs.activity.headerFinished.other"
      };
      let end = display::clock(ended);
      ctx.t_args(
        key,
        [
          ("engine", &*engine),
          ("count", &count),
          ("start", &start),
          ("end", &end),
        ],
      )
    }
  };
  let title = ctx.t_args(
    "runs.activity.stepTitle",
    [("step", &*header.step), ("agent", &*header.agent)],
  );
  let tokens = ctx.t_args(
    "runs.tokens",
    [("count", compact_tokens(header.usage.counted()).as_str())],
  );
  let (glyph, state) = step_state(header.status);
  let (toggles, key, open) = (row.toggles.clone(), row.key.clone(), header.open);
  let chevron = if open { Icon::ChevronDown } else { Icon::ChevronRight };
  pressable(surface(), move || toggles.flip(&key, open))
    .id("activity-step-header")
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Group.role())
    .padding_bottom(SpacingSize::Sm)
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .describe("execution", header.execution.to_string())
    .describe("open", open.to_string())
    .child(icon(chevron, IconSize::Small, PaletteColor::TextMuted))
    .child(
      Text::new(&format!("{:02}", header.number))
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted),
    )
    .child(Text::new(&title).variant(TypographyStyle::Label).nowrap())
    .child(Text::new(&meta).variant(TypographyStyle::Caption).nowrap())
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&tokens).variant(TypographyStyle::Mono).nowrap())
    .child(compact_status_label(glyph, &ctx.t(state)))
    .into()
}

/// What an entry shows, apart from its time and whether it is open.
struct Shown {
  glyph: Icon,
  title: Arc<str>,
  meta: Arc<str>,
  body: Option<Arc<str>>,
  /// Lines of the output block, shown while the entry is open.
  output: Option<Vec<Arc<str>>>,
  /// Opens and closes on a press.
  collapsible: bool,
}

fn entry_row(ctx: &Ctx, row: &TranscriptRow, entry: &Entry, agent: &Arc<str>, open: bool) -> Element {
  let shown = shown(ctx, entry, agent, row.raw);
  let raw = matches!(entry.kind, EntryKind::Raw(_));
  let open = open || (raw && row.raw);
  let mut content = Column::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Tight.role())
    .child(entry_header(&shown, open));
  if open {
    content = content
      .with_children(shown.body.as_ref().map(|body| {
        Text::new(body)
          .variant(ExtraTypography::Explanation.role())
          .selectable(true)
      }))
      .with_children(
        shown
          .output
          .as_ref()
          .filter(|lines| !lines.is_empty())
          .map(|lines| output(ctx, lines)),
      );
  }
  let line = Row::new()
    .id("activity-entry")
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Sm)
    .spacing(ExtraSpacing::Stack.role())
    .describe("entry", entry.sequence.to_string())
    .child(
      Row::new()
        .size(Dimension::Px(TIME_WIDTH), Dimension::Px(GLYPH_SIZE))
        .align_items(Alignment::Center)
        .child(
          Text::new(&display::clock(entry.at))
            .variant(TypographyStyle::Mono)
            .color(PaletteColor::TextMuted)
            .nowrap(),
        ),
    )
    .child(glyph(shown.glyph))
    .child(content);
  let line = if raw { line.opacity(UNPARSED_OPACITY) } else { line };
  if !shown.collapsible || (raw && row.raw) {
    return line.into();
  }
  let (toggles, key) = (row.toggles.clone(), row.key.clone());
  pressable(surface(), move || toggles.flip(&key, open))
    .width(Dimension::Pct(100.0))
    .child(line)
    .into()
}

fn entry_header(shown: &Shown, open: bool) -> Element {
  let disclosure = shown.collapsible.then(|| {
    let chevron = if open { Icon::ChevronDown } else { Icon::ChevronRight };
    icon(chevron, IconSize::Small, PaletteColor::TextMuted)
  });
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(GLYPH_SIZE))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&shown.title)
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0)
        .selectable(true),
    )
    .child(
      Text::new(&shown.meta)
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted)
        .nowrap()
        .flex_shrink(0.0),
    )
    .with_children(disclosure)
    .into()
}

fn glyph(glyph: Icon) -> Element {
  Row::new()
    .size(Dimension::Px(GLYPH_SIZE), Dimension::Px(GLYPH_SIZE))
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Sm)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(glyph, IconSize::Entry, PaletteColor::TextSecondary))
    .into()
}

/// The output block (`bjcu3`): the last lines when there are many, with how many are left out.
fn output(ctx: &Ctx, lines: &[Arc<str>]) -> Element {
  let hidden = lines.len().saturating_sub(OUTPUT_LINES);
  let text = lines[hidden..].join("\n");
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Inline.role())
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Tight.role())
    .background(PaletteColor::SurfaceBase)
    .border_inside(BorderSize::Sm, ExtraColor::Hairline.role())
    .rounded(RadiusSize::Sm)
    .with_children((hidden > 0).then(|| {
      Text::new(&ctx.t_args("runs.activity.outputTrimmed", [("count", hidden.to_string().as_str())]))
        .variant(TypographyStyle::Caption)
    }))
    .child(
      Text::new(&text)
        .variant(ExtraTypography::Output.role())
        .selectable(true),
    )
    .into()
}

fn shown(ctx: &Ctx, entry: &Entry, agent: &Arc<str>, raw_lines: bool) -> Shown {
  let plain = |glyph, title: Arc<str>, meta: Arc<str>, body| Shown {
    glyph,
    title,
    meta,
    body,
    output: None,
    collapsible: false,
  };
  match &entry.kind {
    EntryKind::Message(text) => plain(
      Icon::MessageSquare,
      agent.clone(),
      ctx.t("runs.activity.agentMessage"),
      Some(text.clone()),
    ),
    EntryKind::Reasoning(text) => Shown {
      collapsible: true,
      ..plain(
        Icon::Lightbulb,
        ctx.t("runs.activity.reasoning"),
        "".into(),
        Some(text.clone()),
      )
    },
    EntryKind::Command(entry) => command(ctx, entry),
    EntryKind::FileChange(changes) => {
      let (glyph, key) = match changes.first().map(|change| change.kind) {
        Some(FileChangeKind::Added) => (Icon::FilePlus, "runs.activity.fileAdded"),
        Some(FileChangeKind::Deleted) => (Icon::FileMinus, "runs.activity.fileDeleted"),
        Some(FileChangeKind::Modified) | None => (Icon::FilePenLine, "runs.activity.fileModified"),
      };
      let paths: Vec<&str> = changes.iter().map(|change| in_worktree(&change.path)).collect();
      plain(glyph, paths.join(", ").into(), ctx.t(key), None)
    }
    EntryKind::Usage(usage) => {
      let meta = ctx.t_args(
        "runs.activity.usageMeta",
        [
          ("input", compact_tokens(usage.input).as_str()),
          ("output", compact_tokens(usage.output).as_str()),
          ("cached", compact_tokens(usage.cached_input).as_str()),
        ],
      );
      plain(Icon::Gauge, ctx.t("runs.activity.usage"), meta, None)
    }
    EntryKind::Error(message) => plain(
      Icon::AlertCircle,
      ctx.t("runs.activity.error"),
      "".into(),
      Some(message.clone()),
    ),
    EntryKind::NotSignedIn(message) => Shown {
      output: Some(vec![message.clone()]),
      ..plain(
        Icon::AlertCircle,
        ctx.t("runs.activity.notSignedIn"),
        "".into(),
        Some(ctx.t("runs.activity.notSignedInDetail")),
      )
    },
    EntryKind::Problem(problem) => {
      let (title, body) = problem_text(ctx, problem);
      plain(Icon::AlertCircle, title, "".into(), body)
    }
    EntryKind::Raw(lines) => {
      let key = if lines.len() == 1 {
        "runs.activity.unparsed.one"
      } else {
        "runs.activity.unparsed.other"
      };
      let text = lines
        .iter()
        .map(|(stream, line)| match stream {
          OutputStream::Stdout => line.clone(),
          OutputStream::Stderr => ctx.t_args("runs.activity.stderrLine", [("line", &**line)]),
        })
        .collect();
      Shown {
        output: Some(text),
        collapsible: !raw_lines,
        ..plain(
          Icon::CodeXml,
          ctx.t_args(key, [("count", lines.len().to_string().as_str())]),
          ctx.t("runs.activity.rawOutput"),
          None,
        )
      }
    }
  }
}

fn command(ctx: &Ctx, entry: &CommandEntry) -> Shown {
  let code = entry.exit_code.map(|code| code.to_string()).unwrap_or_default();
  let seconds = entry.duration.map(tenths_of_seconds);
  let (glyph, meta, body) = match entry.status {
    None => (Icon::Terminal, ctx.t("runs.activity.commandRunning"), None),
    Some(CommandStatus::Declined) => (
      Icon::AlertCircle,
      ctx.t("runs.activity.declined"),
      Some(ctx.t("runs.activity.declinedDetail")),
    ),
    Some(CommandStatus::Completed | CommandStatus::Failed) => {
      let meta = match &seconds {
        Some(seconds) => ctx.t_args(
          "runs.activity.exitDuration",
          [("code", code.as_str()), ("seconds", seconds)],
        ),
        None => ctx.t_args("runs.activity.exit", [("code", code.as_str())]),
      };
      (Icon::Terminal, meta, None)
    }
  };
  Shown {
    glyph,
    title: entry.command.clone(),
    meta,
    body,
    output: Some(entry.output.lines().map(Arc::from).collect()),
    collapsible: !entry.output.is_empty(),
  }
}

/// `4.2` for 4,213 milliseconds.
fn tenths_of_seconds(millis: i64) -> String {
  let tenths = millis.max(0) / 100;
  format!("{}.{}", tenths / 10, tenths % 10)
}

fn problem_text(ctx: &Ctx, problem: &RunnerProblem) -> (Arc<str>, Option<Arc<str>>) {
  match problem {
    RunnerProblem::UnavailableVariable { variable } => (
      ctx.t_args(
        "runs.activity.problem.unavailableVariable",
        [("variable", variable.as_str())],
      ),
      None,
    ),
    RunnerProblem::CouldNotStart { detail } => (
      ctx.t("runs.activity.problem.couldNotStart"),
      Some(detail.as_str().into()),
    ),
    RunnerProblem::CheckpointFailed { detail } => (
      ctx.t("runs.activity.problem.checkpointFailed"),
      Some(detail.as_str().into()),
    ),
    RunnerProblem::MergeConflict { branch, files } => (
      ctx.t_args("runs.activity.problem.mergeConflict", [("branch", branch.as_str())]),
      Some(
        files
          .join(
            "
",
          )
          .into(),
      ),
    ),
    RunnerProblem::StepBudgetReached { budget } => (
      ctx.t_args(
        "runs.activity.problem.stepBudgetReached",
        [("budget", compact_tokens(*budget).as_str())],
      ),
      None,
    ),
  }
}

/// A path the CLI reported inside the run's worktree, relative to the worktree
/// (`<repo>/.orchester/worktrees/<run>/src/a.rs` reads `src/a.rs`); other paths as reported.
fn in_worktree(path: &str) -> &str {
  const WORKTREES: [&str; 2] = [".orchester\\worktrees\\", ".orchester/worktrees/"];
  let Some(start) = WORKTREES
    .iter()
    .find_map(|marker| path.find(marker).map(|at| at + marker.len()))
  else {
    return path;
  };
  let rest = &path[start..];
  rest.find(['\\', '/']).map_or(path, |separator| &rest[separator + 1..])
}
