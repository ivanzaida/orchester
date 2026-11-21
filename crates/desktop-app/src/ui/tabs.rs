//! Page tabs (`Page tab/active` `loUWZ`, `Page tab/default` `NspMt`): lurq's `ui::Tabs` in the theme's recipe
//! (`theme::kit::tabs`), each label in the box the current one needs, so a switch moves no tab (`L00 · The tab row
//! never moves` `g5mqSl`); a count follows the label in its words (`All 24`), or as a number after it (Settings'
//! *Accounts 3*), or, for what waits for the person, in a badge after it (`Count badge` `CiYKk`). A tab not built yet
//! is its label and a lock, with why in its tooltip (`Page tab/locked` `Smb4t`). Tabs are pointer-only; a press of the
//! current tab opens its place again, as a link would.
//!
//! The segmented control is [`segments`].

mod segments;

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::{MouseButton, MouseEvent},
    theme::PaletteColor,
  },
  node::Element,
  ui::{Tab, Tabs},
};

pub use self::segments::{Segment, SegmentFace, face, segmented};
use super::{icon::Icon, pressable::Press, tooltip::Tooltip};
use crate::{
  navigation::{
    link::{Open, follow},
    press::OpenIn,
  },
  theme::kit::tabs::{COMPACT, LOCKED, WAITS},
};

#[derive(Clone)]
enum TabState {
  Open(Press),
  /// The page shown, which a press does not open again: it keeps the panel it shows.
  Here,
  /// Not built yet, for this reason.
  Unavailable(Arc<str>),
}

pub struct PageTab {
  id: Arc<str>,
  /// The label with its count, as the screen words it.
  label: Arc<str>,
  current: bool,
  state: TabState,
  explanation: Option<Arc<str>>,
  /// A count of what waits for the person, in a badge after the label.
  badge: Option<usize>,
  /// How many there are, a number after the label, and whether it counts what waits for the person.
  count: Option<(usize, bool)>,
  /// A dot after the count in the tone of what it counts.
  dot: Option<PaletteColor>,
  /// The count is still read: its bone after the label.
  reading: bool,
  compact: bool,
}

impl PageTab {
  fn of(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>, current: bool, state: TabState) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      current,
      state,
      explanation: None,
      badge: None,
      count: None,
      dot: None,
      reading: false,
      compact: false,
    }
  }

  /// The tab 32 high instead of 34, as the Agents hub draws its tabs and a tool's (`Cc3JP`).
  #[must_use]
  pub const fn compact(mut self) -> Self {
    self.compact = true;
    self
  }

  /// A tab that `on_open` opens; `label` already holds its count, if any ([`counted`]).
  pub fn new(
    id: impl Into<Arc<str>>,
    label: impl Into<Arc<str>>,
    current: bool,
    on_open: impl Fn() + Send + Sync + 'static,
  ) -> Self {
    Self::of(id, label, current, TabState::Open(Press::run(on_open)))
  }

  /// A tab that is a place of its own: a press opens it with `open` by a browser's rules (`navigation::link`).
  pub fn to(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>, current: bool, open: Open) -> Self {
    Self::of(id, label, current, TabState::Open(Press::Open(open)))
  }

  /// A tab whose page is not built yet: its label and a lock, and `reason` in its tooltip; it cannot be
  /// opened.
  pub fn unavailable(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>, reason: impl Into<Arc<str>>) -> Self {
    Self::of(id, label, false, TabState::Unavailable(reason.into()))
  }

  /// The current tab, which a press leaves as it is: no action, and no place to open again, e.g. a page that keeps
  /// the panel it shows.
  pub fn current(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>) -> Self {
    Self::of(id, label, true, TabState::Here)
  }

  /// A lock after the label, and `reason` in the tab's tooltip: what the tab opens is not all there yet.
  pub fn explanation(mut self, reason: impl Into<Arc<str>>) -> Self {
    self.explanation = Some(reason.into());
    self
  }

  /// `count` things waiting for the person, in a badge after the label (`CiYKk`); its element id is `<id>-badge`.
  #[must_use]
  pub const fn badge(mut self, count: usize) -> Self {
    self.badge = Some(count);
    self
  }

  /// How many there are, 6 after the label in 12 `text_muted` (`mIlfH` in Settings' `Tabs`), or in `warning` when
  /// it counts what `waits` for the person (the Agents hub's tabs, `JasTq`, `Wk4e9` in `Cc3JP`); its element id is
  /// `<id>-count`.
  #[must_use]
  pub const fn count(mut self, count: usize, waits: bool) -> Self {
    self.count = Some((count, waits));
    self
  }

  /// A 6 dot in `tone` 6 after the count, the state of what it counts, e.g. a run's services (`Dot` `JrkWf` in
  /// `cjvRr`: `danger` once one crashed); its element id is `<id>-dot`.
  #[must_use]
  pub fn dot(mut self, tone: PaletteColor) -> Self {
    self.dot = Some(tone);
    self
  }

  /// How many there are is still read: a count's bone after the label (`D · Bones` in `13 · Patterns` `AOQZ3`); its
  /// element id is `<id>-count`.
  #[must_use]
  pub const fn count_reading(mut self) -> Self {
    self.reading = true;
    self
  }

  /// lurq's tab of value `index`, and what a press of it does, if anything.
  fn into_tab(self, index: usize) -> (Tab<usize>, Option<Press>) {
    let mut tab = Tab::new(index, &self.label)
      .id(self.id.clone())
      .state(COMPACT, self.compact);
    tab = match (self.badge, self.count) {
      (Some(count), _) => tab.badge(count),
      (None, Some((count, waits))) => tab.count(count).state(WAITS, waits),
      (None, None) => tab.count_pending(self.reading),
    };
    if let Some(tone) = self.dot {
      tab = tab.dot(tone);
    }
    let press = match self.state {
      TabState::Open(press) => {
        tab = opens_aside(tab, press.clone());
        Some(press)
      }
      TabState::Here => {
        tab = tab.inert(true);
        None
      }
      TabState::Unavailable(reason) => {
        tab = locked(tab.disabled_reason(&reason), &self.id, reason);
        None
      }
    };
    if let Some(reason) = self.explanation {
      tab = locked(tab, &self.id, reason);
    }
    (tab, press)
  }
}

/// `tab` with a lock after its label and `reason` in its tooltip, `<id>-tooltip`.
fn locked(tab: Tab<usize>, id: &str, reason: Arc<str>) -> Tab<usize> {
  tab
    .trailing_icon(Icon::Lock.name())
    .state(LOCKED, true)
    .tooltip(Tooltip::new(id, reason).to_ui())
}

/// `tab`, whose press with the middle button or Ctrl (Cmd on macOS) runs `press` too: an action, or its place opened
/// by a browser's rules.
fn opens_aside(tab: Tab<usize>, press: Press) -> Tab<usize> {
  match press {
    Press::Run(action) => tab.on_open_aside(move |event: MouseEvent| {
      if event.button != MouseButton::Middle {
        action();
      }
    }),
    Press::Open(open) => tab.on_open_aside(move |event: MouseEvent| follow(&open, &event)),
  }
}

/// The row of page tabs on its line, as wide as its parent: `tabs` at its start, `end` at its end, with `rule` between
/// the tabs before `end` and those after it, and `element` at the end of the line. Mounted, so each label keeps the
/// room it takes as the current tab, and a line too narrow for its tabs scrolls them, with a More menu at its end.
fn tab_line(
  ctx: &mut Ctx,
  id: &str,
  tabs: Vec<PageTab>,
  end: Vec<PageTab>,
  (rule, element): (Option<usize>, Option<Element>),
) -> Element {
  let at_end = tabs.len();
  let selected = tabs
    .iter()
    .chain(&end)
    .position(|tab| tab.current)
    .unwrap_or(usize::MAX);
  let (shown, presses): (Vec<Tab<usize>>, Vec<Option<Press>>) = tabs
    .into_iter()
    .chain(end)
    .enumerate()
    .map(|(index, tab)| {
      let (tab, press) = tab.into_tab(index);
      (
        tab
          .at_end(rule.is_none() && index >= at_end)
          .separated(rule == Some(index)),
        press,
      )
    })
    .unzip();
  let mut line = Tabs::controlled(selected, shown, move |index: &usize| match presses.get(*index) {
    Some(Some(Press::Run(action))) => action(),
    Some(Some(Press::Open(open))) => open(OpenIn::ActiveTab),
    Some(None) | None => {}
  })
  .reselect(true)
  .reserve_selected_width(true)
  .tab_stop(false)
  .more_label(&ctx.t("ui.tabs.more"))
  .scroll_back_label(&ctx.t("ui.tabs.scrollBack"))
  .scroll_on_label(&ctx.t("ui.tabs.scrollOn"))
  .id(id.to_owned());
  if let Some(element) = element {
    line = line.end(element);
  }
  line.mount(ctx)
}

/// A tab's label followed by its count, as the locale words it (`All 24`).
pub fn counted(ctx: &Ctx, label: &str, count: usize) -> Arc<str> {
  ctx.t_args(
    "ui.tabCount",
    [("label", label.to_owned()), ("count", count.to_string())],
  )
}

/// The row of page tabs on its line, as wide as its parent. The line is drawn under the tabs, so the
/// current tab's underline covers it (`BzK1B`).
pub fn page_tabs(ctx: &mut Ctx, id: &str, tabs: impl IntoIterator<Item = PageTab>) -> Element {
  tab_line(ctx, id, tabs.into_iter().collect(), Vec::new(), (None, None))
}

/// [`page_tabs`] with `end` at the right end of the line, e.g. a page's Settings after its lists (`NestZ` in
/// `W721XP`).
pub fn page_tabs_with_end(
  ctx: &mut Ctx,
  id: &str,
  tabs: impl IntoIterator<Item = PageTab>,
  end: impl IntoIterator<Item = PageTab>,
) -> Element {
  tab_line(
    ctx,
    id,
    tabs.into_iter().collect(),
    end.into_iter().collect(),
    (None, None),
  )
}

/// [`page_tabs`] with `note` at the right end of the line, e.g. why a tab cannot be opened (`NWxZx` in `ptqq0`).
pub fn page_tabs_with_note(
  ctx: &mut Ctx,
  id: &str,
  tabs: impl IntoIterator<Item = PageTab>,
  note: impl Into<Element>,
) -> Element {
  tab_line(
    ctx,
    id,
    tabs.into_iter().collect(),
    Vec::new(),
    (None, Some(note.into())),
  )
}

/// [`page_tabs`] with `after` set apart by a rule (`Divider` `Fbmqe` before *Archived* in `Agents · v2` L13 `moPNo`).
pub fn page_tabs_divided(
  ctx: &mut Ctx,
  id: &str,
  tabs: impl IntoIterator<Item = PageTab>,
  after: Vec<PageTab>,
) -> Element {
  let tabs: Vec<PageTab> = tabs.into_iter().collect();
  let rule = (!after.is_empty()).then_some(tabs.len());
  tab_line(ctx, id, tabs, after, (rule, None))
}
