//! The confirmation itself (`Dialog` `MdjVb` in `A23` `Pnpbe`): the question, what
//! archiving does, the resources that name it and the runs that will stop in two lists, what stays not ready, and
//! *Cancel* and the danger *Archive and stop N runs*.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_api::{SharedResourceKind, UsedBy, UsedByResource, UsedByRun};
use orchester_workspace::{
  messages::Message,
  used_by::{RunLine, RunState},
};

use super::{Failed, ReadFailed, words};
use crate::{
  theme::{ARCHIVE_DIALOG_WIDTH, ArchiveText, DialogSpacing, ExtraRadius, ExtraSpacing, LOCAL_ARCHIVE_DIALOG_WIDTH},
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::over_window,
    field::field_error,
    icon::Icon,
    message::message_text,
    state::loading,
  },
};

/// What the confirmation is about.
pub(in crate::screens) struct DialogProps {
  pub name: Arc<str>,
  /// Whether a profile that names the resource stays not ready once it is archived.
  pub blocks_profiles: bool,
  /// A local project's confirmation: its own words, no admins and no other people's runs.
  pub local: bool,
  /// Whether the resource has versions, which stay readable once it is archived; a local MCP server has none.
  pub versioned: bool,
}

/// What *Archive and stop N runs* does.
pub(in crate::screens) type Confirm = Arc<dyn Fn() + Send + Sync>;

/// What the confirmation shows now.
pub(in crate::screens) struct State {
  /// What uses the resource: none while it is read; the failure when it could not be.
  pub read: Option<Result<UsedBy, ReadFailed>>,
  /// How each run that will stop stands: only a local project's confirmation says it.
  pub lines: Vec<RunLine>,
  /// The archive was asked for and has not answered.
  pub archiving: bool,
  /// Why the archive did nothing.
  pub failed: Option<Failed>,
  /// A local archive happened, and these runs of it could not be stopped: the confirmation says so, in place of the
  /// question.
  pub stopped: Option<Message>,
  /// Why a press by automation did nothing.
  pub person_refused: Option<Element>,
}

/// The confirmation over the window while `open` holds. `retry` reads what uses the resource again; `confirm` asks for
/// the archive.
pub(in crate::screens) fn dialog(
  ctx: &mut Ctx,
  props: &DialogProps,
  state: State,
  (open, retry, confirm): (&Signal<bool>, &Signal<u64>, Confirm),
) -> Element {
  let name = props.name.as_ref();
  if let Some(message) = &state.stopped {
    return left_running(ctx, props, message, open);
  }
  let mut dialog = frame(props)
    .child(
      Text::new(&ctx.t_args("team_archive.title", [("name", name)]))
        .id("team-hub-archive-title")
        .variant(ArchiveText::Title.role()),
    )
    .child(
      Text::new(&ctx.t_args(
        match (props.local, props.versioned) {
          (false, _) => "team_archive.body",
          (true, true) => "local_archive.body",
          (true, false) => "local_archive.bodyUnversioned",
        },
        [("name", name)],
      ))
      .id("team-hub-archive-body")
      .variant(ArchiveText::Body.role())
      .width(Dimension::Pct(100.0)),
    );
  let stops = match &state.read {
    Some(Ok(used)) => {
      dialog = dialog
        .child(used_by(ctx, &used.resources))
        .child(runs(ctx, props.local, (&used.runs, &state.lines)))
        .with_children(
          props
            .blocks_profiles
            .then(|| note(ctx, props.local, (name, &used.resources)))
            .flatten(),
        );
      Some(used.runs.len())
    }
    Some(Err(failed)) => {
      dialog = dialog.child(read_failed(ctx, *failed, retry));
      None
    }
    None => {
      dialog = dialog.child(loading("team-hub-archive-reading", &ctx.t("team_archive.reading")));
      None
    }
  };
  let failure = state.failed.as_ref().map(|failed| failure(ctx, failed, name));
  let dialog = dialog
    .with_children(failure)
    .with_children(state.person_refused)
    .child(buttons(ctx, stops, state.archiving, (open, confirm)));
  over_window("team-hub-archive-dialog", dialog, open, None)
}

/// The dialog's box, which the question and the notice share.
fn frame(props: &DialogProps) -> Column {
  Column::new()
    .id("team-hub-archive-dialog")
    .width(Dimension::Px(if props.local {
      LOCAL_ARCHIVE_DIALOG_WIDTH
    } else {
      ARCHIVE_DIALOG_WIDTH
    }))
    .padding(Padding::all(DialogSpacing::Side.role()))
    .spacing(DialogSpacing::Parts.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(ExtraRadius::Tile.role())
    .box_shadow(ShadowStyle::Lg)
}

/// A local resource was archived, but some of the runs that use it could not be stopped and keep working on it: said
/// in the confirmation's place, before the lists read the archive and the resource's panel goes. *Close* ends it.
fn left_running(ctx: &Ctx, props: &DialogProps, message: &Message, open: &Signal<bool>) -> Element {
  let shut = open.clone();
  let close = ActionButton::new(
    "team-hub-archive-close",
    ButtonKind::Secondary,
    ctx.t("local_archive.close"),
  )
  .in_form()
  .on_click(move || shut.set(false));
  let dialog = frame(props)
    .child(
      Text::new(&ctx.t_args("local_archive.doneTitle", [("name", props.name.as_ref())]))
        .id("team-hub-archive-title")
        .variant(ArchiveText::Title.role()),
    )
    .child(
      Text::new(&message_text(ctx, message))
        .id("team-hub-archive-left-running")
        .variant(ArchiveText::Body.role())
        .width(Dimension::Pct(100.0)),
    )
    .child(
      Row::new()
        .id("team-hub-archive-buttons")
        .width(Dimension::Pct(100.0))
        .justify(Justify::End)
        .child(close),
    );
  over_window("team-hub-archive-dialog", dialog, open, None)
}

/// A list's heading over its rows in one outline (`Used by` `pC90U`, `Runs that stop` `SWl1b`).
fn group(ctx: &Ctx, id: &str, heading: &str, rows: Vec<Element>, nothing: &str) -> Column {
  let list = Column::new()
    .id(format!("{id}-list"))
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .clip();
  let list = if rows.is_empty() {
    list.child(row(
      format!("{id}-none"),
      Text::new(&ctx.t(nothing)).variant(ArchiveText::Holder.role()),
      None,
    ))
  } else {
    list.with_children(rows)
  };
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Text::new(heading)
        .id(format!("{id}-heading"))
        .variant(ArchiveText::Heading.role()),
    )
    .child(list)
}

/// One row: what it names at its start and `detail` at its end (`Row` `y7PyW`).
fn row(id: String, name: Text, detail: Option<Text>) -> Row {
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .justify(Justify::SpaceBetween)
    .child(name)
    .with_children(detail)
}

fn used_by(ctx: &Ctx, resources: &[UsedByResource]) -> Column {
  let rows = resources
    .iter()
    .enumerate()
    .map(|(index, used)| {
      let kind = ctx.t(words::kind_key(used.kind));
      let id = format!("team-hub-archive-used-{index}");
      row(
        id.clone(),
        Text::new(&ctx.t_args("team_archive.row", [("name", used.name.as_str()), ("kind", &kind)]))
          .id(format!("{id}-name"))
          .variant(ArchiveText::Name.role()),
        Some(
          // A version number reads as the team's own pins say it: `v4`.
          Text::new(&used.version.to_string())
            .id(format!("{id}-version"))
            .variant(ArchiveText::Version.role()),
        ),
      )
      .into()
    })
    .collect();
  let heading = ctx.t_args("team_archive.usedBy", [("count", resources.len().to_string())]);
  group(ctx, "team-hub-archive-used", &heading, rows, "team_archive.nothingUses")
}

/// What a run that will stop says of how it stands.
const fn state_key(state: RunState) -> &'static str {
  match state {
    RunState::Pending => "local_archive.runState.pending",
    RunState::Running => "local_archive.runState.running",
    RunState::Paused => "local_archive.runState.paused",
    RunState::Waiting => "local_archive.runState.waiting",
    RunState::Interrupted | RunState::Ended => "local_archive.runState.interrupted",
  }
}

fn runs(ctx: &Ctx, local: bool, (runs, lines): (&[UsedByRun], &[RunLine])) -> Column {
  let rows = runs
    .iter()
    .enumerate()
    .map(|(index, run)| {
      let id = format!("team-hub-archive-run-{index}");
      // The server names who holds each run's task, who is told it stopped.
      let holder = run.holder.as_ref().map(|holder| {
        Text::new(&holder.display_name)
          .id(format!("{id}-holder"))
          .variant(ArchiveText::Holder.role())
      });
      // A local run is named by its number among its task's runs and says how it stands.
      let line = lines.iter().find(|line| line.run == run.run);
      let name = match line {
        Some(line) => ctx.t_args(
          "local_archive.runRow",
          [("key", run.task_key.clone()), ("number", line.number.to_string())],
        ),
        None => run.task_key.as_str().into(),
      };
      let holder = holder.or_else(|| {
        line.map(|line| {
          Text::new(&ctx.t(state_key(line.state)))
            .id(format!("{id}-state"))
            .variant(ArchiveText::Holder.role())
        })
      });
      row(
        id.clone(),
        Text::new(&name)
          .id(format!("{id}-name"))
          .variant(ArchiveText::Name.role()),
        holder,
      )
      .into()
    })
    .collect();
  let heading = ctx.t_args(
    if local {
      "local_archive.runs"
    } else {
      "team_archive.runs"
    },
    [("count", runs.len().to_string())],
  );
  group(ctx, "team-hub-archive-runs", &heading, rows, "team_archive.noRuns")
}

/// What stays not ready: the profiles that name the resource, until it is taken out of them or restored; none when no
/// profile does.
fn note(ctx: &Ctx, local: bool, (name, used): (&str, &[UsedByResource])) -> Option<Text> {
  let profiles: Vec<&str> = used
    .iter()
    .filter(|used| used.kind == SharedResourceKind::AgentProfile)
    .map(|used| used.name.as_str())
    .collect();
  if profiles.is_empty() {
    return None;
  }
  let key = match (local, profiles.len() == 1) {
    (false, true) => "team_archive.note.one",
    (false, false) => "team_archive.note.many",
    (true, true) => "local_archive.note.one",
    (true, false) => "local_archive.note.many",
  };
  let names = words::listed(ctx, &profiles);
  Some(
    Text::new(&ctx.t_args(key, [("names", &*names), ("name", name)]))
      .id("team-hub-archive-note")
      .variant(ArchiveText::Note.role())
      .width(Dimension::Pct(100.0)),
  )
}

/// What uses the resource could not be read: nothing is archived without it, and the person may read it again.
fn read_failed(ctx: &Ctx, failed: ReadFailed, retry: &Signal<u64>) -> Column {
  let words = if failed.not_reached {
    "team_archive.notReached"
  } else {
    "team_archive.readFailed"
  };
  let again = retry.clone();
  Column::new()
    .id("team-hub-archive-read-failed")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(field_error("team-hub-archive-read-error", &ctx.t(words)))
    .child(
      ActionButton::new(
        "team-hub-archive-retry",
        ButtonKind::Secondary,
        ctx.t("team_archive.retry"),
      )
      .icon(Icon::RotateCcw)
      .small()
      .on_click(move || again.update(|count| *count += 1)),
    )
}

/// Why the team server did not archive it.
fn failure(ctx: &Ctx, failed: &Failed, name: &str) -> Element {
  let words = match failed {
    Failed::Refused(code) => match code.as_str() {
      "not_admin" => ctx.t("team_archive.refused.notAdmin"),
      "revision_conflict" => ctx.t_args("team_archive.refused.conflict", [("name", name)]),
      other => ctx.t_args("team_archive.refused.other", [("code", other)]),
    },
    Failed::Said(message) => message_text(ctx, message),
    Failed::Unread(read) => ctx.t(if read.not_reached {
      "team_archive.notReached"
    } else {
      "team_archive.readFailed"
    }),
    Failed::RunsChanged => ctx.t("team_archive.refused.runsChanged"),
  };
  field_error("team-hub-archive-error", &words).into()
}

/// *Cancel* and *Archive and stop N runs* (`Buttons` `P2WoD`); the action is not offered before what uses the resource
/// is known, which says how many runs it stops.
fn buttons(ctx: &Ctx, stops: Option<usize>, archiving: bool, (open, confirm): (&Signal<bool>, Confirm)) -> Row {
  let shut = open.clone();
  let cancel = ActionButton::new(
    "team-hub-archive-cancel",
    ButtonKind::Secondary,
    ctx.t("team_archive.cancel"),
  )
  .in_form()
  .on_click(move || shut.set(false));
  let label = match stops {
    Some(0) | None => ctx.t("team_archive.confirm"),
    Some(1) => ctx.t("team_archive.confirmOne"),
    Some(count) => ctx.t_args("team_archive.confirmMany", [("count", count.to_string())]),
  };
  let archive = ActionButton::new("team-hub-archive-confirm", ButtonKind::Danger, label)
    .icon(Icon::Archive)
    .in_form();
  let archive = if archiving {
    archive.loading()
  } else if stops.is_none() {
    archive.disabled()
  } else {
    archive.on_click(move || confirm())
  };
  Row::new()
    .id("team-hub-archive-buttons")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .justify(Justify::End)
    .child(cancel)
    .child(archive)
}
