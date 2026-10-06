//! One transcript entry (`N4SlQq`): time, glyph, a one-line header with right-aligned meta, optional
//! body text and an optional output block. A file change entry opens its file in the Changes tab
//! (↗). A problem row has no time column: its time is in the meta, its glyph in `$text`.

use std::{borrow::Cow, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::stream::{FileChangeKind, OutputStream},
  id::TaskId,
  native::tools::ToolName,
  tokens::compact_tokens,
};

use super::{
  super::{super::ProjectNav, review_note::review_line},
  command_entry,
  header::header,
  mcp::mcp_server_text,
  mcp_call, native_call,
  problem::{self, problem_text},
  request_link::{RequestLink, request_link},
  toggles::Toggles,
};
use crate::{
  display,
  navigation::route::ChangedFile,
  tasks::transcript::{Entry, EntryKind},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, UNPARSED_OPACITY},
  ui::{
    command_row::{CommandPlace, Copied, command_preview, command_row},
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
  },
};

/// Time column, glyph tile and the most output lines shown (`qcnZs`, `zu2Sx`).
const TIME_WIDTH: f32 = 36.0;
const GLYPH_SIZE: f32 = 24.0;
const OUTPUT_LINES: usize = 200;

/// One transcript entry on its own line: in an open tool group, a problem row or the *Tools* view.
#[derive(Clone)]
pub(in crate::screens::project::task) struct EntryProps {
  pub(in crate::screens::project::task) key: Arc<str>,
  /// The task whose run the transcript is of: its Changes tab shows a changed file.
  pub(in crate::screens::project::task) task: TaskId,
  pub(in crate::screens::project::task) entry: Entry,
  pub(in crate::screens::project::task) agent: Arc<str>,
  /// Whether unparsed lines are shown line by line.
  pub(in crate::screens::project::task) raw: bool,
  /// The time in a column of its own; without it, the time is in the meta (a problem row).
  pub(in crate::screens::project::task) time_column: bool,
  /// A line of a chip's open group (`bdCsu`, `Hn1xv`): it starts closed, a failure too, so the
  /// group reads as plain rows until one is opened.
  pub(in crate::screens::project::task) in_group: bool,
  pub(in crate::screens::project::task) toggles: Toggles,
  /// The card of the request a problem row names, while the run's timeline has it.
  pub(in crate::screens::project::task) request: Option<RequestLink>,
}

impl PartialEq for EntryProps {
  fn eq(&self, other: &Self) -> bool {
    self.key == other.key
      && self.task == other.task
      && self.entry == other.entry
      && self.agent == other.agent
      && self.raw == other.raw
      && self.time_column == other.time_column
      && self.in_group == other.in_group
      && self.request.as_ref().map(|link| &link.target) == other.request.as_ref().map(|link| &link.target)
  }
}

impl lurq::app::component::DevtoolsInspectable for EntryProps {}

pub(in crate::screens::project::task) struct EntryView {
  copied: Signal<Copied>,
}

impl Component for EntryView {
  type Props = EntryProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      copied: ctx.signal(Copied::default()),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<EntryProps>().clone();
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let open = props
      .toggles
      .is_open(&props.key, !props.in_group && entry_starts_open(&props.entry));
    entry_row(ctx, &props, open, &nav, &self.copied)
  }
}

/// Failures start open; commands, reasoning and unparsed lines start closed.
fn entry_starts_open(entry: &Entry) -> bool {
  match entry.kind {
    EntryKind::Command(_) | EntryKind::ToolCommand(_) | EntryKind::Reasoning(_) | EntryKind::Raw(_) => {
      entry.is_failure()
    }
    _ => true,
  }
}

/// What an entry shows, apart from its time and whether it is open.
pub(super) struct Shown {
  pub(super) glyph: Icon,
  pub(super) title: Arc<str>,
  pub(super) meta: Arc<str>,
  /// How a request about it was answered, after the meta; it shows whole or not at all.
  pub(super) mark: Option<Arc<str>>,
  pub(super) body: Option<Arc<str>>,
  /// The body is a command, set in mono (a denied native command, 04.H10 `IcGvI`).
  pub(super) mono_body: bool,
  /// Lines of the output block, shown while the entry is open.
  pub(super) output: Option<Vec<Arc<str>>>,
  /// What the output block leaves out, said above it.
  pub(super) omitted: Option<Arc<str>>,
  /// Opens and closes on a press.
  pub(super) collapsible: bool,
  /// The repository path a press opens in the Changes tab.
  pub(super) opens: Option<Arc<str>>,
}

impl Shown {
  /// One line of `title` and `meta`, with `body` under it.
  pub(super) const fn plain(glyph: Icon, title: Arc<str>, meta: Arc<str>, body: Option<Arc<str>>) -> Self {
    Self {
      glyph,
      title,
      meta,
      mark: None,
      body,
      mono_body: false,
      output: None,
      omitted: None,
      collapsible: false,
      opens: None,
    }
  }

  /// With `mark` after the meta.
  pub(super) fn marked(self, mark: Option<Arc<str>>) -> Self {
    Self { mark, ..self }
  }
}

fn entry_row(ctx: &Ctx, row: &EntryProps, open: bool, nav: &ProjectNav, copied: &Signal<Copied>) -> Element {
  let entry = &row.entry;
  // A problem row a title says better than its transcript entry; it has no command box of its own.
  let titled = if row.time_column {
    None
  } else {
    problem::titled(ctx, entry)
  };
  let command_box = titled.is_none();
  let mut shown = titled.unwrap_or_else(|| shown(ctx, entry, &row.agent, row.raw));
  if !row.time_column {
    let time = display::clock(entry.at);
    shown.meta = if shown.meta.is_empty() {
      time.as_str().into()
    } else {
      ctx.t_args(
        "runs.conversation.problemMeta",
        [("meta", &*shown.meta), ("time", time.as_str())],
      )
    };
  }
  let raw = matches!(entry.kind, EntryKind::Raw(_));
  let open = open || (raw && row.raw);
  let content = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(entry_header(ctx, &shown, (open, !row.time_column), entry.sequence))
    .with_children(
      mcp_call::review_of(entry)
        .map(|review| review_line(ctx, format!("activity-entry-review-{}", entry.sequence), review)),
    )
    .with_children(
      open
        .then(|| entry_details(ctx, (entry, command_box, row.request.as_ref()), &shown, copied))
        .flatten(),
    );
  let line = Row::new()
    .id("activity-entry")
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Sm)
    .spacing(ExtraSpacing::Stack.role())
    .describe("entry", entry.sequence.to_string())
    .with_children(row.time_column.then(|| {
      Row::new()
        .id(format!("activity-entry-time-{}", entry.sequence))
        .size(Dimension::Px(TIME_WIDTH), Dimension::Px(GLYPH_SIZE))
        .flex_shrink(0.0)
        .align_items(Alignment::Center)
        .child(
          Text::new(&display::clock(entry.at))
            .variant(TypographyStyle::Mono)
            .color(PaletteColor::TextMuted)
            .nowrap(),
        )
    }))
    .child(glyph(shown.glyph, entry.sequence, !row.time_column))
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
    .id(format!("activity-entry-toggle-{}", entry.sequence))
    .width(Dimension::Pct(100.0))
    .child(line)
    .into()
}

/// What an open entry shows under its header; `command_box`: its command in a box of its own when
/// the title does not hold it whole; `request`: the link to the card of the request it names.
fn entry_details(
  ctx: &Ctx,
  (entry, command_box, request): (&Entry, bool, Option<&RequestLink>),
  shown: &Shown,
  copied: &Signal<Copied>,
) -> Option<Column> {
  let command = match &entry.kind {
    _ if !command_box => None,
    // A native file tool's row names its target; a declined native command is its row's body.
    EntryKind::Command(command)
      if command
        .native
        .as_ref()
        .is_some_and(|call| call.tool != ToolName::RunCommand || call.declined.is_some()) =>
    {
      None
    }
    EntryKind::Command(command) => Some(&*command.command),
    // A tool step's one-line command is whole in the row's title: its output follows (`hep0l`).
    EntryKind::ToolCommand(command) if matches!(command_preview(&command.command), Cow::Borrowed(_)) => None,
    EntryKind::ToolCommand(command) => Some(&*command.command),
    _ => None,
  };
  if command.is_none() && shown.body.is_none() && shown.output.as_ref().is_none_or(Vec::is_empty) {
    return None;
  }
  Some(
    Column::new()
      .id(format!("activity-entry-details-{}", entry.sequence))
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Tight.role())
      .with_children(command.map(|command| {
        command_row(
          ctx,
          format!("activity-command-detail-{}", entry.sequence),
          format!("activity-command-copy-{}", entry.sequence),
          command,
          CommandPlace::Transcript,
          copied,
        )
      }))
      .with_children(shown.body.as_ref().map(|body| {
        let style = if shown.mono_body {
          ExtraTypography::MonoArea
        } else {
          ExtraTypography::Explanation
        };
        Text::new(body).variant(style.role()).selectable(true)
      }))
      .with_children(request.map(|link| request_link(ctx, link, entry.sequence)))
      .with_children(
        shown
          .output
          .as_ref()
          .filter(|lines| !lines.is_empty())
          .map(|lines| output(ctx, lines, shown.omitted.as_deref())),
      ),
  )
}

/// `line`, pressed to open the changed file at `path` in the Changes tab (↗).
fn opens_file(nav: &ProjectNav, row: &EntryProps, path: &Arc<str>, line: Element) -> Element {
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

/// `problem`: a problem row, whose title says what happened and gives way last.
fn entry_header(ctx: &Ctx, shown: &Shown, (open, problem): (bool, bool), sequence: u64) -> Element {
  let disclosure = if shown.opens.is_some() {
    Some(icon(Icon::ArrowUpRight, IconSize::Small, PaletteColor::TextMuted))
  } else {
    shown.collapsible.then(|| {
      let chevron = if open { Icon::ChevronDown } else { Icon::ChevronRight };
      icon(chevron, IconSize::Small, PaletteColor::TextMuted)
    })
  };
  // After a meta, the mark goes on from it (*exit 0 · 2.0 s* *· allowed by the profile · Worktree*).
  let mark = shown.mark.as_ref().map(|mark| {
    if shown.meta.is_empty() {
      mark.clone()
    } else {
      ctx.t_args("runs.activity.decision.afterMeta", [("mark", &**mark)])
    }
  });
  header(
    (&shown.title, &shown.meta, mark.as_deref()),
    disclosure.map(Element::from),
    sequence,
    (GLYPH_SIZE, problem),
  )
}

/// The entry's glyph on its tile; a problem row's in `$text` (`OlLGD`).
fn glyph(glyph: Icon, sequence: u64, problem: bool) -> Element {
  let color = if problem {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  };
  Row::new()
    .id(format!("activity-entry-icon-{sequence}"))
    .size(Dimension::Px(GLYPH_SIZE), Dimension::Px(GLYPH_SIZE))
    .flex_shrink(0.0)
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Sm)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(glyph, IconSize::Entry, color))
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
  let plain = Shown::plain;
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
    EntryKind::Command(entry) => match &entry.native {
      Some(call) => native_call::call(ctx, entry, call),
      None => command_entry::command(ctx, entry),
    },
    EntryKind::Approval(approval) => native_call::approval(ctx, approval),
    EntryKind::FileChange(change) => {
      let (glyph, key) = file_change_kind(change.kind);
      // Git's paths use `/` whatever the CLI reported.
      let path: Arc<str> = in_worktree(&change.path)
        .unwrap_or(&change.path)
        .replace('\\', "/")
        .into();
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
    EntryKind::ToolCommand(entry) => command_entry::tool_command(ctx, entry),
    EntryKind::McpTool(call) => mcp_call::call(ctx, call),
    EntryKind::ExemptionNotApplied { server, tool, other } => {
      mcp_call::exemption_not_applied(ctx, (server, tool, other))
    }
  }
}

/// A file change's glyph and the key of its status.
pub(super) const fn file_change_kind(kind: FileChangeKind) -> (Icon, &'static str) {
  match kind {
    FileChangeKind::Added => (Icon::FilePlus, "runs.activity.fileAdded"),
    FileChangeKind::Deleted => (Icon::FileMinus, "runs.activity.fileDeleted"),
    FileChangeKind::Modified => (Icon::FilePenLine, "runs.activity.fileModified"),
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
    mark: None,
    body: None,
    mono_body: false,
    output: Some(text),
    omitted: None,
    collapsible: !raw_lines,
    opens: None,
  }
}

/// A path the CLI reported inside the run's worktree, relative to the worktree
/// (`<repo>/.orchester/worktrees/<run>/src/a.rs` reads `src/a.rs`); `None` for other paths. On
/// Windows the worktree's path mixes separators (`H:\repo\.orchester/worktrees\<run>`). The run's
/// worktree is the innermost one: the repository itself may lie in another project's worktree.
pub(super) fn in_worktree(path: &str) -> Option<&str> {
  const WORKTREES: &str = ".orchester/worktrees/";
  // Both separators are one byte: the offsets in the copy are the path's.
  let start = path.replace('\\', "/").rfind(WORKTREES)? + WORKTREES.len();
  let rest = &path[start..];
  rest.find(['\\', '/']).map(|separator| &rest[separator + 1..])
}
