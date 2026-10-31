//! The update row (`c69Eti`, the first row of *Updates* in `wSMNG`): a 15 glyph, the label in its state's color, one
//! hint, and what the person can do at its end. Up to date with *Check now*; checking; downloading with its share and
//! *Pause*, and a 4 `info` bar under it; paused with *Resume*; ready with *Restart to update*; waiting for the runs at
//! work with *Install now*; critical; installing; a download that failed its check with *Try again*; an install that
//! did not finish with the installer's log; Orchester Cloud unreachable with *Check now*; a copy that does not update
//! itself, or macOS, with *Download ↗* once the cloud named the newest release. *What's new*, while there are release
//! notes to read, opens them under the row: the update's to install or, after an update, the installed one's.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{app_update::Channel, time::Timestamp};
use orchester_workspace::{
  app_updates::{CopyReason, NewestRelease, ReadyUpdate, UpdateFailure, UpdateState, install::InstallStep},
  display,
};

use super::{
  UpdateAction, UpdatesFacts,
  parts::{Parts, link, notes, progress},
};
use crate::{
  theme::IconSize,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    setting_rows::SettingRow,
  },
};

/// The row's element id.
pub const ROW: &str = "settings-update";

/// What the row says and offers in one state.
struct Said {
  state: &'static str,
  glyph: Icon,
  /// The glyph's color, which the label takes too when it says how the update stands.
  tone: PaletteColor,
  toned_label: bool,
  label: String,
  hint: String,
  controls: Vec<Element>,
  /// A download's bytes so far and in all.
  progress: Option<(u64, u64)>,
}

impl Said {
  fn new(state: &'static str, (glyph, tone): (Icon, PaletteColor), label: String, hint: String) -> Self {
    Self {
      state,
      glyph,
      toned_label: tone != PaletteColor::TextMuted,
      tone,
      label,
      hint,
      controls: Vec::new(),
      progress: None,
    }
  }

  fn control(mut self, control: impl Into<Element>) -> Self {
    self.controls.push(control.into());
    self
  }
}

/// The row as `facts` say, its release notes under it while `notes_open`.
pub(super) fn update_row(ctx: &mut Ctx, facts: &UpdatesFacts, notes_open: bool, parts: &Parts) -> Element {
  let mut said = said(ctx, facts, parts);
  let shown_notes = notes_of(facts);
  // *What's new* comes first among the row's actions (`K9AcC` before `sH1MP`).
  if shown_notes.is_some() {
    said.controls.insert(0, parts.notes_toggle(ctx, notes_open));
  }
  let actions = Row::new()
    .id(format!("{ROW}-actions"))
    .flex_shrink(0.0)
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .with_children(said.controls);
  let mut row = SettingRow::new(ROW, said.label)
    .lead(icon(said.glyph, IconSize::Nav, said.tone.clone()))
    .hint(said.hint)
    .control(actions);
  if said.toned_label {
    row = row.label_color(said.tone);
  }
  if let Some(done) = said.progress {
    row = row.below(progress(&format!("{ROW}-progress"), done));
  }
  if let (true, Some((version, text))) = (notes_open, shown_notes) {
    row = row.below(notes(ctx, (&version, &text), parts));
  }
  Column::new()
    .id(format!("{ROW}-state"))
    .width(Dimension::Pct(100.0))
    .describe("state", said.state)
    .child(row.render(ctx))
    .into()
}

/// The release notes *What's new* shows: the update's to install, else, after an update, the installed one's.
fn notes_of(facts: &UpdatesFacts) -> Option<(String, String)> {
  if let Some(update) = facts.state.to_install() {
    return Some((update.version.to_string(), update.notes.clone()));
  }
  facts
    .just_updated
    .as_ref()
    .map(|updated| (updated.version.to_string(), updated.notes.clone()))
}

fn said(ctx: &mut Ctx, facts: &UpdatesFacts, parts: &Parts) -> Said {
  let channel = channel_name(ctx, facts.settings.map(|settings| settings.channel).unwrap_or_default());
  match &facts.state {
    UpdateState::UpToDate { checked_at } => up_to_date(ctx, facts, (&channel, *checked_at), parts),
    UpdateState::Checking => Said::new(
      "checking",
      (Icon::LoaderCircle, PaletteColor::TextMuted),
      ctx.t("settings.general.updates.checking.label").to_string(),
      ctx
        .t_args("settings.general.updates.checking.hint", [("channel", channel)])
        .to_string(),
    ),
    UpdateState::Downloading {
      version,
      received,
      total,
    } => download(ctx, (&version.to_string(), *received, *total), false, parts),
    UpdateState::Paused {
      version,
      received,
      total,
    } => download(ctx, (&version.to_string(), *received, *total), true, parts),
    UpdateState::Ready(update) => ready(ctx, update, parts),
    UpdateState::WaitingForRuns { update, runs }
    | UpdateState::Installing {
      update,
      step: InstallStep::WaitingForRuns(runs),
    } => waiting(ctx, update, runs, parts),
    UpdateState::Installing { update, step } => installing(ctx, update, step),
    UpdateState::Failed { version, failure } => failed(ctx, &version.to_string(), *failure, parts),
    UpdateState::InstallFailed { version, failure } => {
      let said = Said::new(
        "install-failed",
        (Icon::CircleX, PaletteColor::Danger),
        ctx
          .t_args(
            "settings.general.updates.installFailed.label",
            [("version", version.to_string())],
          )
          .to_string(),
        ctx
          .t_args(
            "settings.general.updates.installFailed.hint",
            [("reason", failure.reason.clone())],
          )
          .to_string(),
      );
      match &failure.log {
        Some(log) => {
          let words = ctx.t("settings.general.updates.installerLog");
          said.control(link(
            &format!("{ROW}-log"),
            &words,
            parts.on(UpdateAction::OpenLog(log.clone())),
          ))
        }
        None => said,
      }
    }
    UpdateState::CloudUnreachable { last_checked } => unreachable(ctx, *last_checked, parts),
    UpdateState::NotThisCopy { reason, newest } => not_this_copy(ctx, *reason, newest.as_ref(), parts),
    UpdateState::NotOnThisPlatform { newest } => not_on_this_platform(ctx, newest.as_ref(), parts),
  }
}

fn up_to_date(ctx: &mut Ctx, facts: &UpdatesFacts, (channel, checked_at): (&str, Timestamp), parts: &Parts) -> Said {
  let time = display::clock(checked_at);
  let hint = match &facts.just_updated {
    Some(updated) => ctx.t_args(
      "settings.general.updates.upToDate.updatedHint",
      [
        ("version", updated.version.to_string()),
        ("channel", channel.to_owned()),
        ("time", time),
      ],
    ),
    None => ctx.t_args(
      "settings.general.updates.upToDate.hint",
      [
        ("version", facts.running.clone()),
        ("channel", channel.to_owned()),
        ("time", time),
      ],
    ),
  };
  let check = parts.ghost(
    ctx,
    ("check-now", Icon::RefreshCw, "settings.general.updates.checkNow"),
    UpdateAction::CheckNow,
  );
  Said::new(
    "up-to-date",
    (Icon::CircleCheck, PaletteColor::Success),
    ctx.t("settings.general.updates.upToDate.label").to_string(),
    hint.to_string(),
  )
  .control(check)
}

/// A download under way, or `paused`: its share, its megabytes, and *Pause* or *Resume*.
fn download(ctx: &mut Ctx, (version, received, total): (&str, u64, u64), paused: bool, parts: &Parts) -> Said {
  let percent = (received.min(total) * 100 / total.max(1)).to_string();
  let sizes = [("received", megabytes(received)), ("total", megabytes(total))];
  let (state, label, hint, control) = if paused {
    let resume = parts.ghost(
      ctx,
      ("resume", Icon::Play, "settings.general.updates.resume"),
      UpdateAction::Resume,
    );
    (
      "paused",
      ctx.t_args(
        "settings.general.updates.paused.label",
        [("version", version.to_owned()), ("percent", percent)],
      ),
      ctx.t_args("settings.general.updates.paused.hint", sizes),
      resume,
    )
  } else {
    let pause = parts.ghost(
      ctx,
      ("pause", Icon::Pause, "settings.general.updates.pause"),
      UpdateAction::Pause,
    );
    (
      "downloading",
      ctx.t_args(
        "settings.general.updates.downloading.label",
        [("version", version.to_owned()), ("percent", percent)],
      ),
      ctx.t_args("settings.general.updates.downloading.hint", sizes),
      pause,
    )
  };
  let mut said = Said::new(
    state,
    (Icon::Download, PaletteColor::TextMuted),
    label.to_string(),
    hint.to_string(),
  )
  .control(control);
  said.progress = Some((received, total));
  said
}

fn ready(ctx: &Ctx, update: &ReadyUpdate, parts: &Parts) -> Said {
  let version = update.version.to_string();
  let said = if update.critical {
    critical(ctx, &version)
  } else {
    Said::new(
      "ready",
      (Icon::CircleArrowUp, PaletteColor::Info),
      ctx
        .t_args("settings.general.updates.ready.label", [("version", version)])
        .to_string(),
      ctx.t("settings.general.updates.ready.hint").to_string(),
    )
  };
  said.control(restart(ctx, parts))
}

fn critical(ctx: &Ctx, version: &str) -> Said {
  Said::new(
    "critical",
    (Icon::ShieldAlert, PaletteColor::Warning),
    ctx
      .t_args(
        "settings.general.updates.critical.label",
        [("version", version.to_owned())],
      )
      .to_string(),
    ctx.t("settings.general.updates.critical.hint").to_string(),
  )
}

/// *Restart to update*, the primary action (`Restart to update` `sH1MP`).
fn restart(ctx: &Ctx, parts: &Parts) -> Element {
  ActionButton::new(
    format!("{ROW}-restart"),
    ButtonKind::Primary,
    ctx.t("settings.general.updates.restart").to_string(),
  )
  .icon(Icon::RotateCcw)
  .press(parts.on(UpdateAction::Restart))
  .into()
}

/// The update waits for the runs at work; *Install now* asks whether to stop them (`03 · Install now` `MGILm`).
fn waiting(ctx: &Ctx, update: &ReadyUpdate, runs: &[String], parts: &Parts) -> Said {
  let version = update.version.to_string();
  if update.critical {
    return critical(ctx, &version).control(restart(ctx, parts));
  }
  let count = runs.len().to_string();
  let (label, hint) = if runs.len() == 1 {
    (
      "settings.general.updates.waiting.label.one",
      "settings.general.updates.waiting.hint.one",
    )
  } else {
    (
      "settings.general.updates.waiting.label.other",
      "settings.general.updates.waiting.hint.other",
    )
  };
  let install = ActionButton::new(
    format!("{ROW}-install-now"),
    ButtonKind::Secondary,
    ctx.t("settings.general.updates.installNow").to_string(),
  )
  .icon(Icon::CircleStop)
  .press(parts.on(UpdateAction::Restart));
  Said::new(
    "waiting",
    (Icon::Hourglass, PaletteColor::Info),
    ctx.t_args(label, [("version", version), ("count", count)]).to_string(),
    ctx.t_args(hint, [("runs", listed(ctx, runs))]).to_string(),
  )
  .control(install)
}

fn installing(ctx: &Ctx, update: &ReadyUpdate, step: &InstallStep) -> Said {
  let hint = match step {
    InstallStep::WaitingForRuns(_) | InstallStep::StoppingRuns => "settings.general.updates.installing.stoppingRuns",
    InstallStep::BackingUp => "settings.general.updates.installing.backingUp",
    InstallStep::HandingOver => "settings.general.updates.installing.handingOver",
  };
  Said::new(
    "installing",
    (Icon::LoaderCircle, PaletteColor::Info),
    ctx
      .t_args(
        "settings.general.updates.installing.label",
        [("version", update.version.to_string())],
      )
      .to_string(),
    ctx.t(hint).to_string(),
  )
}

/// A download that could not be made ready: why, that nothing was installed, and *Try again*.
fn failed(ctx: &Ctx, version: &str, failure: UpdateFailure, parts: &Parts) -> Said {
  let deleted = "settings.general.updates.failed.deletedHint";
  let (label, hint) = match failure {
    UpdateFailure::NoInstaller => (
      "settings.general.updates.failed.noInstaller",
      "settings.general.updates.failed.nothingHint",
    ),
    UpdateFailure::WrongSize => ("settings.general.updates.failed.wrongSize", deleted),
    UpdateFailure::WrongDigest => ("settings.general.updates.failed.wrongDigest", deleted),
    UpdateFailure::UnknownKey => ("settings.general.updates.failed.unknownKey", deleted),
    UpdateFailure::BadSignature => ("settings.general.updates.failed.badSignature", deleted),
    UpdateFailure::NotSaved => (
      "settings.general.updates.failed.notSaved",
      "settings.general.updates.failed.notSavedHint",
    ),
  };
  let try_again = ActionButton::new(
    format!("{ROW}-try-again"),
    ButtonKind::Secondary,
    ctx.t("settings.general.updates.tryAgain").to_string(),
  )
  .icon(Icon::RotateCcw)
  .press(parts.on(UpdateAction::CheckNow));
  Said::new(
    "failed",
    (Icon::CircleX, PaletteColor::Danger),
    ctx.t_args(label, [("version", version.to_owned())]).to_string(),
    ctx.t(hint).to_string(),
  )
  .control(try_again)
}

fn unreachable(ctx: &mut Ctx, last_checked: Option<Timestamp>, parts: &Parts) -> Said {
  let hint = match last_checked {
    Some(at) => {
      let ago = message_text(ctx, &display::ago(Timestamp::now(), at));
      ctx.t_args("settings.general.updates.unreachable.hint", [("ago", ago.to_string())])
    }
    None => ctx.t("settings.general.updates.unreachable.neverHint"),
  };
  let check = parts.ghost(
    ctx,
    ("check-now", Icon::RefreshCw, "settings.general.updates.checkNow"),
    UpdateAction::CheckNow,
  );
  Said::new(
    "unreachable",
    (Icon::CloudOff, PaletteColor::TextMuted),
    ctx.t("settings.general.updates.unreachable.label").to_string(),
    hint.to_string(),
  )
  .control(check)
}

fn not_this_copy(ctx: &Ctx, reason: CopyReason, newest: Option<&NewestRelease>, parts: &Parts) -> Said {
  let (alone, with_newest) = match reason {
    CopyReason::Development => (
      "settings.general.updates.notThisCopy.development",
      "settings.general.updates.notThisCopy.developmentNewest",
    ),
    CopyReason::Qa => (
      "settings.general.updates.notThisCopy.qa",
      "settings.general.updates.notThisCopy.qaNewest",
    ),
    CopyReason::ScriptInstall => (
      "settings.general.updates.notThisCopy.scriptInstall",
      "settings.general.updates.notThisCopy.scriptInstallNewest",
    ),
    CopyReason::Portable => (
      "settings.general.updates.notThisCopy.portable",
      "settings.general.updates.notThisCopy.portableNewest",
    ),
    CopyReason::ProgramFolderUnknown => (
      "settings.general.updates.notThisCopy.programFolderUnknown",
      "settings.general.updates.notThisCopy.programFolderUnknownNewest",
    ),
  };
  let hint = match newest {
    Some(newest) => ctx.t_args(with_newest, [("version", newest.version.to_string())]),
    None => ctx.t(alone),
  };
  let said = Said::new(
    "not-this-copy",
    (Icon::Info, PaletteColor::TextMuted),
    ctx.t("settings.general.updates.notThisCopy.label").to_string(),
    hint.to_string(),
  );
  with_download(ctx, said, newest, parts)
}

/// macOS, which does not update itself yet; elsewhere a platform no release ships for.
fn not_on_this_platform(ctx: &Ctx, newest: Option<&NewestRelease>, parts: &Parts) -> Said {
  let (label, hint) = if cfg!(target_os = "macos") {
    let hint = match newest {
      Some(newest) => ctx.t_args(
        "settings.general.updates.macos.newestHint",
        [("version", newest.version.to_string())],
      ),
      None => ctx.t("settings.general.updates.macos.hint"),
    };
    (ctx.t("settings.general.updates.macos.label"), hint)
  } else {
    (
      ctx.t("settings.general.updates.otherPlatform.label"),
      ctx.t("settings.general.updates.otherPlatform.hint"),
    )
  };
  let said = Said::new(
    "not-on-this-platform",
    (Icon::Info, PaletteColor::TextMuted),
    label.to_string(),
    hint.to_string(),
  );
  with_download(ctx, said, newest, parts)
}

/// *Download 0.4.0 ↗*, once the cloud named the newest release (`Link` `S0Jl30`).
fn with_download(ctx: &Ctx, said: Said, newest: Option<&NewestRelease>, parts: &Parts) -> Said {
  let Some(newest) = newest else {
    return said;
  };
  let words = ctx.t_args(
    "settings.general.updates.download",
    [("version", newest.version.to_string())],
  );
  said.control(link(
    &format!("{ROW}-download"),
    &words,
    parts.on(UpdateAction::Open(newest.download.clone())),
  ))
}

/// `Stable` or `Beta`, as Settings names them.
pub fn channel_name(ctx: &Ctx, channel: Channel) -> String {
  match channel {
    Channel::Stable => ctx.t("settings.general.updates.stable"),
    Channel::Beta => ctx.t("settings.general.updates.beta"),
  }
  .to_string()
}

/// Whole megabytes of `bytes`, as the row counts a download (`38 of 91 MB`).
fn megabytes(bytes: u64) -> String {
  (bytes / 1_000_000).to_string()
}

/// `ORC-110, ORC-346 and ORC-405`.
fn listed(ctx: &Ctx, items: &[String]) -> String {
  match items {
    [] => String::new(),
    [one] => one.clone(),
    [rest @ .., last] => {
      let separator = ctx.t("settings.general.updates.listSeparator");
      let and = ctx.t("settings.general.updates.listLast");
      format!("{}{and}{last}", rest.join(&separator))
    }
  }
}
