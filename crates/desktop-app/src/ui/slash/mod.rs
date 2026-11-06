//! The slash menu of a composer (ORC-321, ORC-325; ORC-318's `Slash menu`: `07` `jAiyu`, `07b` `m85qp`, `07c`
//! `f0b717`, `07d` `R8eum` in the Orchestrator, `06` `yKWQx`, `06b` `W4SNj3`, `06c` `DunaJ` in a run's Conversation,
//! the rules in `06d` `lXQi1`), shared by both composers: a `/` as the draft's first character opens the menu of the
//! chat's own commands and the project's published skills ([`menu`]), and what is typed after it filters both by name.
//! The arrow keys move through the rows that can be chosen, Enter or a click chooses one, which clears the typed `/`:
//! a command runs, or, when it takes an argument, becomes the composer's command chip and opens its picker
//! ([`argument`], ORC-771); a skill becomes the composer's chip ([`with_chips`]) in place of the one it held. Esc
//! closes the menu and keeps the text. A row that cannot be chosen now stays in its place, faded, and says why. With
//! nothing that matches, Enter sends the text as typed. Once a space or a new line follows the `/`, or anywhere later
//! in the text, a `/` is just text; while the chip holds a command, the draft is that command's.

mod argument;
mod chip;
mod menu;
mod rows;
mod skills;

use std::{ops::Range, sync::Arc};

use lurq::{
  app::{ctx::Ctx, events::KeyboardEvent},
  core::Signal,
};
use orchester_core::id::ResourceId;
use orchester_workspace::skills::offered::OfferedSkill;
use parking_lot::Mutex;

use self::argument::Arguing;
pub use self::{
  argument::{Applied, Apply, ArgumentPicker, Asked, Choice, Offer, Taking},
  chip::{SkillReach, plan_held, sent_skill_chip, with_chips},
  menu::{SlashMenuProps, menu},
  skills::SkillsOffer,
};
use super::icon::Icon;

/// What running a command does.
pub type Run = Arc<dyn Fn() + Send + Sync>;

/// What choosing a command does: it runs, or it takes an argument first.
#[derive(Clone)]
pub enum Does {
  Run(Run),
  Take(Taking),
}

/// One of the chat's commands, as its composer offers it now.
#[derive(Clone)]
pub struct SlashCommand {
  /// What names its row, in tools and tests, and, for a command with an argument, what its chip shows after a `/`.
  pub name: &'static str,
  pub icon: Icon,
  pub label: Arc<str>,
  /// What it does or, while it cannot run, why.
  pub detail: Arc<str>,
  /// What it does; `None` while it cannot run.
  pub does: Option<Does>,
}

/// What a row chooses: a command by its name, or a skill.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Pick {
  Command(&'static str),
  Skill(ResourceId),
}

/// The rows the menu shows now, as the keys read them: what each chooses and whether it can be chosen, in order, and
/// the one with the focus.
#[derive(Clone, Debug, Default)]
struct Shown {
  rows: Vec<(Pick, bool)>,
  focused: Option<usize>,
}

/// What the composer offers now: its commands, the skills read last, and what sends the draft as typed when nothing
/// matches (`None` where the field's own Enter sends it).
#[derive(Default)]
struct Offered {
  commands: Vec<SlashCommand>,
  skills: Vec<OfferedSkill>,
  send: Option<Run>,
}

/// A composer's slash menu: its draft, whether the field has the focus, the skill its chip holds, the command with an
/// argument it holds and the menu's and the picker's state. Made once with the composer's component, so the menu keeps
/// its state while the draft changes.
#[derive(Clone)]
pub struct SlashField {
  draft: Signal<String>,
  /// The field has the focus: only then is the menu open and does it take keys.
  focused: Signal<bool>,
  /// The skill the composer's chip holds: a message carries one.
  skill: Signal<Option<OfferedSkill>>,
  /// The row with the keyboard's focus, among the rows shown.
  focus: Signal<Option<usize>>,
  /// Esc closed the menu: it stays closed until the draft no longer starts with `/`.
  closed: Signal<bool>,
  offered: Arc<Mutex<Offered>>,
  shown: Arc<Mutex<Option<Shown>>>,
  /// The command with an argument the composer's chip holds, and its choices.
  argument: Signal<Option<Arguing>>,
  /// What that command's picker offers and what applying it does.
  taking: Arc<Mutex<Option<Taking>>>,
  /// The picker's row with the keyboard's focus, and the rows it shows now.
  choice_focus: Signal<Option<usize>>,
  offered_choices: Arc<Mutex<Option<argument::Offered>>>,
}

impl PartialEq for SlashField {
  fn eq(&self, other: &Self) -> bool {
    self.draft.id() == other.draft.id() && Arc::ptr_eq(&self.shown, &other.shown)
  }
}

impl SlashField {
  /// The menu of `draft`, whose composer keeps whether its field has the focus in `focused` and the skill its chip
  /// holds in `skill`.
  pub fn new(
    ctx: &mut Ctx,
    draft: Signal<String>,
    (focused, skill): (Signal<bool>, Signal<Option<OfferedSkill>>),
  ) -> Self {
    Self {
      draft,
      focused,
      skill,
      focus: ctx.signal(None),
      closed: ctx.signal(false),
      offered: Arc::default(),
      shown: Arc::default(),
      argument: ctx.signal(None),
      taking: Arc::default(),
      choice_focus: ctx.signal(None),
      offered_choices: Arc::default(),
    }
  }

  /// Takes `event` when the open menu, or the command the chip holds, has a use for it, before the field's own keys:
  /// whether it did.
  pub fn key(&self, event: &KeyboardEvent) -> bool {
    if event.composing || event.default_prevented() || !self.focused.get_untracked() {
      return false;
    }
    if let Some(held) = self.argument.get_untracked() {
      return self.argument_key(event, &held);
    }
    let Some(shown) = self.shown.lock().clone() else {
      return false;
    };
    match event.key.as_str() {
      "Escape" => {
        self.closed.set(true);
        *self.shown.lock() = None;
      }
      "ArrowDown" | "ArrowUp" => {
        let choosable: Vec<bool> = shown.rows.iter().map(|(_, can)| *can).collect();
        let next = step(&choosable, shown.focused, event.key == "ArrowDown");
        if next != shown.focused {
          self.focus.set(next);
        }
      }
      "Enter" if !event.shift && !event.ctrl && !event.meta => {
        if !self.choose(&shown) {
          return false;
        }
      }
      _ => return false,
    }
    event.prevent_default();
    true
  }

  /// Enter: chooses the row with the focus, or, with nothing that matches, sends the text as typed. Whether the menu
  /// took the key: not when the field's own Enter sends.
  fn choose(&self, shown: &Shown) -> bool {
    if shown.rows.is_empty() {
      let send = self.offered.lock().send.clone();
      return send.is_some_and(|send| {
        send();
        true
      });
    }
    if let Some((pick, true)) = shown.focused.and_then(|at| shown.rows.get(at)) {
      self.run(pick);
    }
    true
  }

  /// Chooses `pick` while it can be chosen: the typed `/` goes, and the menu with it; a command runs or becomes the
  /// command chip, a skill becomes the skill chip.
  fn run(&self, pick: &Pick) {
    let run = {
      let offered = self.offered.lock();
      match pick {
        Pick::Command(name) => offered
          .commands
          .iter()
          .find(|command| command.name == *name)
          .and_then(|command| {
            let does = command.does.clone()?;
            Some(match does {
              Does::Run(run) => run,
              Does::Take(taking) => {
                let (field, held) = (self.clone(), (command.name, command.icon));
                Arc::new(move || field.hold(held, taking.clone())) as Run
              }
            })
          }),
        Pick::Skill(id) => offered
          .skills
          .iter()
          .find(|skill| skill.id == *id)
          .cloned()
          .map(|skill| {
            let chip = self.skill.clone();
            Arc::new(move || chip.set(Some(skill.clone()))) as Run
          }),
      }
    };
    let Some(run) = run else {
      return;
    };
    *self.shown.lock() = None;
    self.focus.set(None);
    self.draft.set(String::new());
    run();
  }
}

/// The row after (`down`) or before `focused` that can be chosen, by `choosable`, around the ends; none when no row
/// can.
fn step(choosable: &[bool], focused: Option<usize>, down: bool) -> Option<usize> {
  let count = choosable.len();
  let last = count.checked_sub(1)?;
  let from = focused.unwrap_or(if down { last } else { 0 });
  (1..=count)
    .map(|offset| {
      if down {
        (from + offset) % count
      } else {
        (from + count - offset) % count
      }
    })
    .find(|&at| choosable[at])
}

/// What filters the menu in `text`, the draft: the words after a `/` that starts it, while no space or new line
/// follows; `None` while the draft does not open the menu.
pub fn query(text: &str) -> Option<&str> {
  let rest = text.strip_prefix('/')?;
  (!rest.contains(char::is_whitespace)).then_some(rest)
}

/// Where `query` is in `label`, by letters and ignoring case: the letters to draw in bold. An empty query matches
/// every label, and none of its letters.
pub fn matched(label: &str, query: &str) -> Option<Range<usize>> {
  if query.is_empty() {
    return Some(0..0);
  }
  label.char_indices().find_map(|(start, _)| {
    let mut rest = label[start..].char_indices();
    let mut end = start;
    for wanted in query.chars() {
      let (at, found) = rest.next()?;
      if !found.to_lowercase().eq(wanted.to_lowercase()) {
        return None;
      }
      end = start + at + found.len_utf8();
    }
    Some(start..end)
  })
}
