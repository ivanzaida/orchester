//! `/unwatch` (ORC-774, `Parts · /unwatch` `NtsXc` in `Orchestrator watch · redesign`; its badge P5 `ijspu`, ORC-890):
//! *Stop watching* in the slash menu, offered while the conversation leads something. Chosen, it becomes the
//! composer's *Stop watching* badge and opens its picker ([`panel`]), which lists only what this conversation watches,
//! each with what is still open there, then *All of them*. Enter adds the focused row to the badge; the composer's
//! send stops the badge's subscriptions (`orchester_workspace::orchestrator::watch::stop_in`, kept as the person's
//! `/unwatch` with its stop line), off the UI thread and with no turn of its own; *All of them* in the badge stops
//! everything here. It cancels and pauses nothing. A message written with it is optional and goes as any other. The
//! picker follows what the screen last read the conversation to lead.

mod panel;

use std::{path::PathBuf, sync::Arc};

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::orchestrator::{WatchTarget, watch::NamedTarget};
use orchester_workspace::orchestrator::{
  watch::{StopBy, StopWhat},
  watching::WatchedSubscription,
};

use self::panel::Shown;
use super::{Held, WhatIWatch, answer::target_icon};
use crate::{
  screens::orchestrator::data::Read,
  ui::{
    icon::Icon,
    message::message_text,
    slash::{Applied, ArgumentPicker, Asked, Badge, Choice, Does, Offer, SlashCommand, Taking, matched},
  },
};

/// The command's name, which its picker's elements are named after.
const COMMAND: &str = "unwatch";
/// The keys of the picker's choices: *All of them*, the two targets that are no module or task, and the prefixes of a
/// module's and a task's id.
const ALL: &str = "all";
const WITHOUT_MODULE: &str = "without-module";
const PROJECT: &str = "project";
const MODULE: &str = "module-";
const TASK: &str = "task-";

impl WhatIWatch {
  /// *Stop watching* in the slash menu of the project at `root`: it takes what is chosen in its picker; faded, with
  /// why, while the conversation leads nothing.
  pub(in crate::screens::orchestrator) fn unwatch_command(&self, ctx: &Ctx, root: PathBuf) -> SlashCommand {
    let label = ctx.t("composer.slash.command.unwatch.label");
    if self.watches().is_none() {
      return SlashCommand {
        name: COMMAND,
        icon: Icon::EyeOff,
        label,
        detail: ctx.t("composer.slash.command.watch.nothing"),
        does: None,
      };
    }
    let this = self.clone();
    SlashCommand {
      name: COMMAND,
      icon: Icon::EyeOff,
      label,
      detail: ctx.t("composer.slash.command.unwatch.detail"),
      does: Some(Does::Take(Taking {
        picker: Arc::new(UnwatchPicker {
          held: self.held.clone(),
        }),
        apply: Arc::new(move |applied: Applied| this.unwatch(root.clone(), &applied)),
        badge: Badge {
          icon: Icon::EyeOff,
          verb: ctx.t("composer.slash.command.unwatch.badge"),
        },
      })),
    }
  }

  /// The composer's send with the badge: the conversation shown stops watching what it holds, or everything once *All
  /// of them* is among them, by the person's own keys only ([`WhatIWatch::stop_by_person`]); whether it took them.
  fn unwatch(&self, root: PathBuf, applied: &Applied) -> bool {
    let Held(Some((chat, _))) = self.held.get_untracked() else {
      return false;
    };
    let what = if applied.choices.iter().any(|choice| &*choice.key == ALL) {
      StopWhat::All
    } else {
      let targets: Vec<NamedTarget> = applied.choices.iter().filter_map(named).collect();
      if targets.is_empty() {
        return false;
      }
      StopWhat::Targets(targets)
    };
    self.stop_by_person((root, chat), what, StopBy::Unwatch)
  }
}

/// What the picker offers: the subscriptions of the conversation shown, as the screen last read them.
struct UnwatchPicker {
  held: Signal<Held>,
}

impl ArgumentPicker for UnwatchPicker {
  fn offer(&self, ctx: &mut Ctx, asked: &Asked<'_>) -> Offer {
    let Held(held) = self.held.get();
    let watching = match held.map(|(_, read)| read) {
      Some(Read::Ready(watching)) => watching,
      Some(Read::Failed(why)) => {
        return Offer {
          rows: Vec::new(),
          panel: panel::unshown(ctx, asked.id, Some(&why)),
        };
      }
      Some(Read::Loading) | None => {
        return Offer {
          rows: Vec::new(),
          panel: panel::unshown(ctx, asked.id, None),
        };
      }
    };
    let filter = asked.filter.trim();
    let offered: Vec<(&WatchedSubscription, Choice)> = watching
      .subscriptions
      .iter()
      .map(|watched| (watched, choice(ctx, watched)))
      .filter(|(_, choice)| matched(&choice.label, filter).is_some())
      .collect();
    let all = Choice {
      key: ALL.into(),
      icon: Icon::EyeOff,
      label: ctx.t("orchestrator.watch.unwatch.all"),
    };
    let taken: Vec<Arc<str>> = asked.taken.iter().map(|choice| choice.key.clone()).collect();
    let rows = offered
      .iter()
      .map(|(_, choice)| choice)
      .chain([&all])
      .map(|choice| (choice.clone(), !taken.contains(&choice.key)))
      .collect();
    let panel = panel::panel(
      ctx,
      &Shown {
        id: asked.id,
        filter,
        offered: &offered,
        all: &all,
        taken: &taken,
        focused: asked.focused,
        total: watching.subscriptions.len(),
      },
    );
    Offer { rows, panel }
  }
}

/// The chip `watched` becomes: its target's icon and name.
fn choice(ctx: &Ctx, watched: &WatchedSubscription) -> Choice {
  let target = watched.target.target;
  Choice {
    key: key_of(target),
    icon: target_icon(target),
    label: message_text(ctx, &watched.target.name),
  }
}

/// The key of the chip of `target`.
fn key_of(target: WatchTarget) -> Arc<str> {
  match target {
    WatchTarget::Module(id) => format!("{MODULE}{id}").into(),
    WatchTarget::Task(id) => format!("{TASK}{id}").into(),
    WatchTarget::WithoutModule => WITHOUT_MODULE.into(),
    WatchTarget::Project => PROJECT.into(),
  }
}

/// The target a chip names, with the name it shows for a module or a task; `None` for *All of them* and a key the
/// picker never makes.
fn named(choice: &Choice) -> Option<NamedTarget> {
  let target = match &*choice.key {
    WITHOUT_MODULE => WatchTarget::WithoutModule,
    PROJECT => WatchTarget::Project,
    key => {
      if let Some(module) = key.strip_prefix(MODULE) {
        WatchTarget::Module(module.parse().ok()?)
      } else {
        WatchTarget::Task(key.strip_prefix(TASK)?.parse().ok()?)
      }
    }
  };
  let name = matches!(target, WatchTarget::Module(_) | WatchTarget::Task(_)).then(|| choice.label.to_string());
  Some(NamedTarget { target, name })
}
