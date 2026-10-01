//! One transcript row: a step execution header (`mDnc3`) or an entry (`N4SlQq`) — time, glyph,
//! a one-line header with right-aligned meta, optional body text and an optional output block. A
//! file change entry opens its file in the Changes tab (↗).

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
  connection::stream::{CommandStatus, FileChangeKind, OutputStream, ToolCommandResult},
  tokens::compact_tokens,
};

use super::{
  super::{super::ProjectNav, heading::step_state},
  engine, first_edit,
  mcp::mcp_server_text,
  problem::problem_text,
  rows::{Header, RowKind, TranscriptRow},
};
use crate::{
  display,
  navigation::route::ChangedFile,
  tasks::transcript::{CommandEntry, CommandMark, Entry, EntryKind, ToolCommandEntry},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, UNPARSED_OPACITY},
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
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
      RowKind::Entry { entry, agent, open } => {
        let nav = ctx
          .use_context::<ProjectNav>()
          .expect("the project shell provides ProjectNav");
        entry_row(ctx, &row, entry, (agent, *open), &nav)
      }
    }
  }
}

/// Disclosure, step number, step and agent, model and time span, tokens with those before the
/// first edit, and state (`mDnc3`). A tool step names its tool and runs, and has no model and no
/// tokens.
fn header_row(ctx: &Ctx, row: &TranscriptRow, header: &Header) -> Element {
  let meta = header_meta(ctx, header);
  let title = ctx.t_args(
    "runs.activity.stepTitle",
    [("step", &*header.step), ("agent", &*header.agent)],
  );
  let tokens = (!header.command).then(|| {
    let tokens = ctx.t_args(
      "runs.tokens",
      [("count", compact_tokens(header.usage.counted()).as_str())],
    );
    let tokens = first_edit::header_tokens(ctx, tokens, header.before_edit.as_ref(), header.ended.is_some());
    Text::new(&tokens).variant(TypographyStyle::Mono).nowrap()
  });
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
    .with_children(tokens)
    .child(compact_status_label(glyph, &ctx.t(state)))
    .into()
}

/// "Codex / Sol · 1 invocation · 12:01–12:02", "Command tool · run 1 · started 12:05".
fn header_meta(ctx: &Ctx, header: &Header) -> Arc<str> {
  let start = header.started.map(display::clock).unwrap_or_default();
  let count = header.invocations.to_string();
  let (engine, running, one, other) = if header.command {
    (
      ctx.t("runs.activity.commandTool"),
      "runs.activity.commandHeaderRunning",
      "runs.activity.commandHeaderFinished.one",
      "runs.activity.commandHeaderFinished.other",
    )
  } else {
    (
      engine(ctx, header.provider, &header.model),
      "runs.activity.headerRunning",
      "runs.activity.headerFinished.one",
      "runs.activity.headerFinished.other",
    )
  };
  let Some(ended) = header.ended else {
    return ctx.t_args(running, [("engine", &*engine), ("count", &count), ("start", &start)]);
  };
  let key = if header.invocations == 1 { one } else { other };
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

/// What an entry shows, apart from its time and whether it is open.
struct Shown {
  glyph: Icon,
  title: Arc<str>,
  meta: Arc<str>,
  body: Option<Arc<str>>,
  /// Lines of the output block, shown while the entry is open.
  output: Option<Vec<Arc<str>>>,
  /// What the output block leaves out, said above it.
  omitted: Option<Arc<str>>,
  /// Opens and closes on a press.
  collapsible: bool,
  /// The repository path a press opens in the Changes tab.
  opens: Option<Arc<str>>,
}

fn entry_row(
  ctx: &Ctx,
  row: &TranscriptRow,
  entry: &Entry,
  (agent, open): (&Arc<str>, bool),
  nav: &ProjectNav,
) -> Element {
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
          .map(|lines| output(ctx, lines, shown.omitted.as_deref())),
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
  if let Some(path) = &shown.opens {
    return opens_file(nav, row, path, line.into());
  }
  if !shown.collapsible || (raw && row.raw) {
    return line.into();
  }
  let (toggles, key) = (row.toggles.clone(), row.key.clone());
  pressable(surface(), move || toggles.flip(&key, open))
    .width(Dimension::Pct(100.0))
    .child(line)
    .into()
}

/// `line`, pressed to open the changed file at `path` in the Changes tab (↗).
fn opens_file(nav: &ProjectNav, row: &TranscriptRow, path: &Arc<str>, line: Element) -> Element {
  let file = ChangedFile {
    path: path.to_string(),
    worktree: false,
  };
  nav
    .changes_link(row.task, &file)
    .on(surface())
    .id(format!("activity-open-file:{path}"))
    .width(Dimension::Pct(100.0))
    .child(line)
    .into()
}

fn entry_header(shown: &Shown, open: bool) -> Element {
  let disclosure = if shown.opens.is_some() {
    Some(icon(Icon::ArrowUpRight, IconSize::Small, PaletteColor::TextMuted))
  } else {
    shown.collapsible.then(|| {
      let chevron = if open { Icon::ChevronDown } else { Icon::ChevronRight };
      icon(chevron, IconSize::Small, PaletteColor::TextMuted)
    })
  };
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

/// The output block (`bjcu3`): the last lines when there are many, with how many are left out, and
/// `omitted`, what was not kept of it at all.
fn output(ctx: &Ctx, lines: &[Arc<str>], omitted: Option<&str>) -> Element {
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
    .with_children(omitted.map(|omitted| Text::new(omitted).variant(TypographyStyle::Caption)))
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
    omitted: None,
    collapsible: false,
    opens: None,
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
    EntryKind::FileChange(change) => {
      let (glyph, key) = match change.kind {
        FileChangeKind::Added => (Icon::FilePlus, "runs.activity.fileAdded"),
        FileChangeKind::Deleted => (Icon::FileMinus, "runs.activity.fileDeleted"),
        FileChangeKind::Modified => (Icon::FilePenLine, "runs.activity.fileModified"),
      };
      // Git's paths use `/` whatever the CLI reported.
      let path: Arc<str> = in_worktree(&change.path).replace('\\', "/").into();
      let meta = ctx.t_args("runs.activity.fileMeta", [("status", &*ctx.t(key))]);
      Shown {
        opens: Some(path.clone()),
        ..plain(glyph, path, meta, None)
      }
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
    // A server the agent works without: unplugged (04.G `a2PR7K`).
    EntryKind::McpServer(server, problem) => {
      let (title, body, said) = mcp_server_text(ctx, server.as_deref(), problem);
      Shown {
        output: said.map(|said| vec![said]),
        ..plain(Icon::Unplug, title, "".into(), Some(body))
      }
    }
    EntryKind::Raw(lines) => unparsed(ctx, lines, raw_lines),
    EntryKind::ToolCommand(entry) => tool_command(ctx, entry),
    EntryKind::McpTool(call) => plain(Icon::Plug, call.tool.clone(), mark_text(ctx, &call.mark), None),
  }
}

/// Why nobody was asked about a call, as the decision marks of the command categories design word it.
fn mark_text(ctx: &Ctx, mark: &CommandMark) -> Arc<str> {
  match mark {
    CommandMark::ExemptTool { server } => ctx.t_args("runs.activity.decision.notAskedExempt", [("server", &**server)]),
  }
}

/// Unparsed lines: collapsed and counted, or shown line by line with *Show raw lines* on.
fn unparsed(ctx: &Ctx, lines: &[(OutputStream, Arc<str>)], raw_lines: bool) -> Shown {
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
    glyph: Icon::CodeXml,
    title: ctx.t_args(key, [("count", lines.len().to_string().as_str())]),
    meta: ctx.t("runs.activity.rawOutput"),
    body: None,
    output: Some(text),
    omitted: None,
    collapsible: !raw_lines,
    opens: None,
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
  let meta = match &entry.mark {
    Some(mark) => ctx.t_args(
      "runs.activity.decision.withMeta",
      [("meta", &*meta), ("mark", &*mark_text(ctx, mark))],
    ),
    None => meta,
  };
  Shown {
    glyph,
    title: entry.command.clone(),
    meta,
    body,
    output: Some(entry.output.lines().map(Arc::from).collect()),
    omitted: None,
    collapsible: !entry.output.is_empty(),
    opens: None,
  }
}

/// A tool step's command: its command line, how it ended and the output its tool kept.
fn tool_command(ctx: &Ctx, entry: &ToolCommandEntry) -> Shown {
  let seconds = entry.duration.map(tenths_of_seconds).unwrap_or_default();
  let code = entry.exit_code.map(|code| code.to_string()).unwrap_or_default();
  let meta = match (entry.result, entry.exit_code) {
    (None, _) => ctx.t("runs.activity.commandRunning"),
    (Some(ToolCommandResult::TimedOut), _) => ctx.t_args("runs.activity.timedOut", [("seconds", seconds.as_str())]),
    (Some(ToolCommandResult::Stopped), _) => ctx.t("runs.activity.stopped"),
    (Some(_), Some(_)) => ctx.t_args(
      "runs.activity.exitDuration",
      [("code", code.as_str()), ("seconds", seconds.as_str())],
    ),
    (Some(_), None) => ctx.t_args("runs.activity.noExitCode", [("seconds", seconds.as_str())]),
  };
  let omitted = (entry.omitted_bytes > 0).then(|| {
    let size = message_text(ctx, &display::byte_size(entry.omitted_bytes));
    ctx.t_args("runs.activity.outputOmitted", [("size", &*size)])
  });
  Shown {
    glyph: Icon::Terminal,
    title: entry.command.clone(),
    meta,
    body: None,
    output: Some(entry.output.lines().map(Arc::from).collect()),
    omitted,
    collapsible: !entry.output.is_empty(),
    opens: None,
  }
}

/// `4.2` for 4,213 milliseconds.
fn tenths_of_seconds(millis: i64) -> String {
  let tenths = millis.max(0) / 100;
  format!("{}.{}", tenths / 10, tenths % 10)
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
