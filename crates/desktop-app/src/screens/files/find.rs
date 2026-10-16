//! Search within the file (`Files · find in file` `b7RGL`, `File head · find` `RxJ3Z`): a 260 wide field with
//! where the current place is among those found ("14 of 41"), *Match case*, the previous and next place and
//! a cross that closes it; Enter goes to the next place, Shift+Enter to the previous one, Esc closes it.
//! The places are found by the workspace ([`orchester_workspace::files::FileText::find`], off the UI thread:
//! [`super::search`]); the lines show them, the current one apart. Only a person's typing is searched: what automation
//! types finds nothing, since counting places would let it read the file a character at a time.

use lurq::{
  app::{
    ctx::Ctx,
    events::{KeyboardEvent, is_synthetic_input},
  },
  components::Row,
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::files::MAX_MATCHES;

use crate::{
  theme::ExtraSpacing,
  ui::{field::FieldInput, icon::Icon, icon_button::IconButton},
};

/// The width of the find field (`kWlG3`).
const FIELD_WIDTH: f32 = 260.0;

/// The search within the file: whether it is open, what it looks for and which place is current.
#[derive(Clone)]
pub struct FindState {
  pub open: Signal<bool>,
  pub query: Signal<String>,
  pub match_case: Signal<bool>,
  /// The current place, an index into what the search found.
  pub at: Signal<usize>,
  /// What a person last typed. Only that is searched: text automation sets or types finds nothing, so that
  /// counting what it finds cannot read the file.
  typed: Signal<String>,
  focus: Signal<bool>,
  input: ElementRef,
}

impl FindState {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      query: ctx.signal(String::new()),
      // *Match case* is on until a person turns it off, as `b7RGL` draws the search.
      match_case: ctx.signal(true),
      at: ctx.signal(0),
      typed: ctx.signal(String::new()),
      focus: ctx.signal(false),
      input: ElementRef::new(),
    }
  }

  /// Opens the search with the focus in its field.
  pub fn show(&self) {
    self.open.set(true);
    self.focus.set(true);
  }

  fn close(&self) {
    self.open.set(false);
  }

  /// What the search looks for: its text and whether case counts; `None` while it is closed or empty.
  pub fn looking_for(&self) -> Option<(String, bool)> {
    let query = self.query.get();
    let typed = self.typed.get() == query;
    (self.open.get() && typed && !query.is_empty()).then(|| (query, self.match_case.get()))
  }

  /// The current place among `count`, kept within them.
  pub fn current(&self, count: usize) -> Option<usize> {
    (count > 0).then(|| self.at.get().min(count - 1))
  }

  /// Moves the current place to the next (or previous) of `count`, around the ends.
  fn step(at: &Signal<usize>, count: usize, forward: bool) {
    if count == 0 {
      return;
    }
    at.update(|at| {
      let now = (*at).min(count - 1);
      *at = if forward {
        (now + 1) % count
      } else {
        (now + count - 1) % count
      };
    });
  }

  /// The search's controls for `count` places found, or while a new query is `searching` for.
  pub fn bar(&self, ctx: &mut Ctx, (count, searching): (usize, bool)) -> Element {
    if self.focus.get() {
      ctx.focus(&self.input);
      self.focus.set(false);
    }
    let field = FieldInput::new("files-find-field", ctx.t("files.find.placeholder"), self.query.clone())
      .search()
      .placeholder(ctx.t("files.find.placeholder"))
      .width(Dimension::Px(FIELD_WIDTH))
      .element(self.input.clone())
      .on_typed({
        let typed = self.typed.clone();
        move |event| {
          if !is_synthetic_input() {
            typed.set(event.new_value().to_owned());
          }
        }
      });
    let field = match self.count_text(ctx, count, searching) {
      Some(text) => field.unit(text),
      None => field,
    };
    let (at, at_next, state) = (self.at.clone(), self.at.clone(), self.clone());
    let match_case = self.match_case.clone();
    let mut case = IconButton::new("files-find-case", Icon::CaseSensitive, ctx.t("files.find.matchCase"))
      .toggle()
      .on_click(move || match_case.update(|on| *on = !*on));
    if self.match_case.get() {
      case = case.active();
    }
    let keys = {
      let (at, state) = (self.at.clone(), self.clone());
      move |event: KeyboardEvent| match event.key.as_str() {
        "Enter" => {
          Self::step(&at, count, !event.shift);
          event.prevent_default();
        }
        "Escape" => {
          state.close();
          event.prevent_default();
        }
        _ => {}
      }
    };
    Row::new()
      .id("files-find")
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .on_key_down(keys)
      .child(field.render(ctx))
      .child(case.render(ctx))
      .child(
        IconButton::new("files-find-previous", Icon::ChevronUp, ctx.t("files.find.previous"))
          .small()
          .on_click(move || Self::step(&at, count, false))
          .render(ctx),
      )
      .child(
        IconButton::new("files-find-next", Icon::ChevronDown, ctx.t("files.find.next"))
          .small()
          .on_click(move || Self::step(&at_next, count, true))
          .render(ctx),
      )
      .child(
        IconButton::new("files-find-close", Icon::X, ctx.t("files.find.close"))
          .small()
          .on_click(move || state.close())
          .render(ctx),
      )
      .into()
  }

  /// "14 of 41", "No results", "Searching…" while a new query is searched for, or nothing before anything is
  /// typed.
  fn count_text(&self, ctx: &Ctx, count: usize, searching: bool) -> Option<String> {
    let query = self.query.get();
    if query.is_empty() || self.typed.get() != query {
      return None;
    }
    if searching {
      return Some(ctx.t("files.find.searching").to_string());
    }
    let Some(current) = self.current(count) else {
      return Some(ctx.t("files.find.none").to_string());
    };
    let key = if count >= MAX_MATCHES {
      "files.find.many"
    } else {
      "files.find.count"
    };
    let at = (current + 1).to_string();
    Some(
      ctx
        .t_args(key, [("at", at.as_str()), ("count", &count.to_string())])
        .to_string(),
    )
  }
}
