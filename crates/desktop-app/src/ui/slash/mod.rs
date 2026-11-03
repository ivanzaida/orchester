//! The slash menu of a composer (ORC-321, ORC-325; ORC-318's `Slash menu`: `07` `jAiyu`, `07b` `m85qp`, `07c`
//! `f0b717`, `07d` `R8eum` in the Orchestrator, `06` `yKWQx`, `06b` `W4SNj3`, `06c` `DunaJ` in a run's Conversation,
//! the rules in `06d` `lXQi1`), shared by both composers: a `/` as the draft's first character opens the menu of the
//! chat's own commands and the project's published skills ([`menu`]), and what is typed after it filters both by name.
//! The arrow keys move through the rows that can be chosen, Enter or a click chooses one, which clears the typed `/`:
//! a command runs, a skill becomes the composer's chip ([`with_skill_chip`]) in place of the one it held. Esc closes
//! the menu and keeps the text. A row that cannot be chosen now stays in its place, faded, and says why. With nothing
//! that matches, Enter sends the text as typed. Once a space or a new line follows the `/`, or anywhere later in the
//! text, a `/` is just text.

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

pub use self::{
  chip::{SkillReach, plan_held, sent_skill_chip, with_skill_chip},
  menu::{SlashMenuProps, menu},
  skills::SkillsOffer,
};
use super::icon::Icon;

/// What running a command does.
pub type Run = Arc<dyn Fn() + Send + Sync>;

/// One of the chat's commands, as its composer offers it now.
#[derive(Clone)]
pub struct SlashCommand {
  /// What names its row, in tools and tests.
  pub name: &'static str,
  pub icon: Icon,
  pub label: Arc<str>,
  /// What it does or, while it cannot run, why.
  pub detail: Arc<str>,
  /// What it does; `None` while it cannot run.
  pub run: Option<Run>,
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

/// A composer's slash menu: its draft, whether the field has the focus, the skill its chip holds, and the menu's
/// state. Made once with the composer's component, so the menu keeps its state while the draft changes.
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
    }
  }

  /// Takes `event` when the open menu has a use for it, before the field's own keys: whether it did.
  pub fn key(&self, event: &KeyboardEvent) -> bool {
    if event.composing || event.default_prevented() || !self.focused.get_untracked() {
      return false;
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
        let next = step(&shown, event.key == "ArrowDown");
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

  /// Chooses `pick` while it can be chosen: the typed `/` goes, and the menu with it; a command runs, a skill becomes
  /// the chip.
  fn run(&self, pick: &Pick) {
    let run = {
      let offered = self.offered.lock();
      match pick {
        Pick::Command(name) => offered
          .commands
          .iter()
          .find(|command| command.name == *name)
          .and_then(|command| command.run.clone()),
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

/// The row after (`down`) or before the one with the focus that can be chosen, around the ends; none when no row can.
fn step(shown: &Shown, down: bool) -> Option<usize> {
  let count = shown.rows.len();
  let last = count.checked_sub(1)?;
  let from = shown.focused.unwrap_or(if down { last } else { 0 });
  (1..=count)
    .map(|offset| {
      if down {
        (from + offset) % count
      } else {
        (from + count - offset) % count
      }
    })
    .find(|&at| shown.rows[at].1)
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
