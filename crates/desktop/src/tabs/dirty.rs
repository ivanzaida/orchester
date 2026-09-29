//! Unsaved edits of a tab, and the question closing such a tab or leaving such a page asks (design
//! doc, "Tabs": closing asks only when the tab's page would ask on navigating away). A page that
//! knows it holds edits not saved yet — an agent profile or a command tool before *Save draft*, a
//! draft task being edited — marks them through [`TabEdits`]; pages whose drafts save themselves
//! never do. A page that also asks before the tab leaves it (Edit task, `kDmo9`) marks its edits
//! with what the question says; every user navigation of the active tab asks first
//! ([`Tabs::leave_active`](super::Tabs::leave_active)). Both questions use the confirmation
//! dialog's frame (`P8IN7Z`, as Remove from list uses it).

use std::{
  collections::HashMap,
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Modal, ModalTarget, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use parking_lot::Mutex;

use super::{Tabs, model::TabId};
use crate::{
  messages::Message,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    backdrop::backdrop,
    button::ActionButton,
    form::{Submit, form},
    icon::Icon,
    message_text,
  },
};

/// Dialog width, action bar and button heights, as the confirmation dialog's (`P8IN7Z`, `cDtbR`).
const DIALOG_WIDTH: f32 = 600.0;
const ACTIONS_HEIGHT: f32 = 76.0;
const BUTTON_HEIGHT: f32 = 36.0;

/// Which pages of which tabs hold unsaved edits, each with what leaving it asks, if it asks.
#[derive(Default)]
pub struct Unsaved {
  marks: Mutex<HashMap<TabId, HashMap<u64, Option<Message>>>>,
}

impl Unsaved {
  /// Whether a page of `tab` holds unsaved edits.
  pub fn any(&self, tab: TabId) -> bool {
    self.marks.lock().get(&tab).is_some_and(|marks| !marks.is_empty())
  }

  /// What leaving the page of `tab` asks, when its edits ask before it is left.
  pub fn leave_question(&self, tab: TabId) -> Option<Message> {
    self.marks.lock().get(&tab)?.values().flatten().next().cloned()
  }

  /// `leave`: `None` for edits that do not ask before the page is left.
  fn set(&self, tab: TabId, mark: u64, unsaved: bool, leave: Option<Message>) {
    let mut marks = self.marks.lock();
    let marks = marks.entry(tab).or_default();
    if unsaved {
      marks.insert(mark, leave);
    } else {
      marks.remove(&mark);
    }
  }

  pub(super) fn forget(&self, tab: TabId) {
    self.marks.lock().remove(&tab);
  }
}

/// Provided by each tab's host to the pages in it.
#[derive(Clone)]
pub struct TabEdits {
  tabs: Tabs,
  tab: TabId,
}

impl TabEdits {
  pub(super) fn new(tabs: Tabs, tab: TabId) -> Self {
    Self { tabs, tab }
  }

  /// A mark for one page, which it keeps while mounted: dropping it clears the page's edits.
  pub fn mark(&self) -> EditMark {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    EditMark {
      edits: self.clone(),
      id: NEXT.fetch_add(1, Ordering::Relaxed),
    }
  }
}

/// A page's unsaved edits in its tab.
pub struct EditMark {
  edits: TabEdits,
  id: u64,
}

impl EditMark {
  /// Records whether the page holds edits that are not saved.
  pub fn set(&self, unsaved: bool) {
    self.edits.tabs.unsaved().set(self.edits.tab, self.id, unsaved, None);
  }

  /// Records edits that are not saved and that the tab asks about before it leaves the page:
  /// `explanation` says what leaving discards. `None` once nothing is unsaved.
  pub fn set_asking(&self, explanation: Option<Message>) {
    let unsaved = explanation.is_some();
    self
      .edits
      .tabs
      .unsaved()
      .set(self.edits.tab, self.id, unsaved, explanation);
  }

  /// The page's mark in the tab it is in; `None` outside a tab.
  pub fn of(ctx: &mut Ctx) -> Option<Self> {
    ctx.use_context::<TabEdits>().map(|edits| edits.mark())
  }
}

impl Drop for EditMark {
  fn drop(&mut self) {
    self.set(false);
  }
}

/// What the discard question asks about.
#[derive(Clone)]
pub enum Discard {
  /// Closing the tab.
  Close(TabId),
  /// Leaving the page of the active tab for where the user asked to go.
  Leave(Leaving),
}

/// A navigation the active tab holds back until its page's edits may be discarded.
#[derive(Clone)]
pub struct Leaving {
  /// Tells one question from the next, for the dialog's key and props.
  id: u64,
  explanation: Message,
  go: Arc<dyn Fn() + Send + Sync>,
}

impl Leaving {
  pub(super) fn new(explanation: Message, go: Arc<dyn Fn() + Send + Sync>) -> Self {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    Self {
      id: NEXT.fetch_add(1, Ordering::Relaxed),
      explanation,
      go,
    }
  }
}

impl PartialEq for Discard {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Close(tab), Self::Close(other)) => tab == other,
      (Self::Leave(leaving), Self::Leave(other)) => leaving.id == other.id,
      _ => false,
    }
  }
}

impl lurq::app::component::DevtoolsInspectable for Discard {}

impl Discard {
  fn key(&self) -> String {
    match self {
      Self::Close(tab) => format!("discard-{tab}"),
      Self::Leave(leaving) => format!("leave-{}", leaving.id),
    }
  }
}

/// "Discard changes?" for the tab being closed (keep it open, or discard the edits and close it)
/// or the page being left (keep editing, or discard the edits and go). Escape keeps them.
pub struct DiscardChanges;

impl Component for DiscardChanges {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let tabs = tabs(ctx);
    match tabs.discarding().get() {
      Some(question) => ctx.mount_keyed::<DiscardDialog>(&question.key(), question),
      None => Column::new().into(),
    }
  }
}

impl lurq::app::component::DevtoolsInspectable for TabId {}

/// One question; a new one is mounted each time it is asked.
struct DiscardDialog {
  open: Signal<bool>,
}

impl Component for DiscardDialog {
  type Props = Discard;

  fn create(ctx: &mut Ctx) -> Self {
    let open = ctx.signal(true);
    let discarding = tabs(ctx).discarding().clone();
    ctx.watch(&open, move |open| {
      if !open {
        discarding.set(None);
      }
    });
    Self { open }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let question = ctx.props::<Discard>().clone();
    let tabs = tabs(ctx);
    let discarding = tabs.discarding().clone();
    let texts = Texts::of(ctx, &question);
    let confirm: Submit = Some(Arc::new(move || {
      discarding.set(None);
      match &question {
        Discard::Close(tab) => tabs.close(*tab),
        Discard::Leave(leaving) => (leaving.go)(),
      }
    }));
    let content = Column::new()
      .id("discard-changes-dialog")
      .width(Dimension::Px(DIALOG_WIDTH))
      .background(ExtraColor::Surface.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .child(header(&texts))
      .child(actions(ctx, &texts, confirm.clone()));
    let dialog = backdrop(ctx, form(confirm, content));
    Modal::new(dialog).target(ModalTarget::Root).open(self.open.clone())
  }
}

/// What the question says: closing uses the tab's wording, leaving the page's (`b3VIZo`, whose
/// buttons have no icons).
struct Texts {
  explanation: Arc<str>,
  keep: Arc<str>,
  confirm: Arc<str>,
  confirm_icon: Option<Icon>,
  title: Arc<str>,
}

impl Texts {
  fn of(ctx: &Ctx, question: &Discard) -> Self {
    match question {
      Discard::Close(_) => Self {
        title: ctx.t("tabs.discard.title"),
        explanation: ctx.t("tabs.discard.explanation"),
        keep: ctx.t("tabs.discard.keep"),
        confirm: ctx.t("tabs.discard.confirm"),
        confirm_icon: Some(Icon::X),
      },
      Discard::Leave(leaving) => Self {
        title: ctx.t("tabs.discard.title"),
        explanation: message_text(ctx, &leaving.explanation),
        keep: ctx.t("tabs.discard.keepEditing"),
        confirm: ctx.t("tabs.discard.confirmLeave"),
        confirm_icon: None,
      },
    }
  }
}

fn tabs(ctx: &mut Ctx) -> Tabs {
  ctx
    .use_context::<Tabs>()
    .expect("the window provides Tabs before mounting the discard question")
}

fn header(texts: &Texts) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Sm)
    .child(Text::new(&texts.title).variant(ExtraTypography::DialogTitle.role()))
    .child(Text::new(&texts.explanation).variant(ExtraTypography::Explanation.role()))
    .into()
}

fn actions(ctx: &mut Ctx, texts: &Texts, confirm: Submit) -> Element {
  let keep = tabs(ctx).discarding().clone();
  let discard = ActionButton::new(texts.confirm.clone())
    .primary()
    .id("discard-changes-confirm")
    .in_form()
    .height(Dimension::Px(BUTTON_HEIGHT));
  let discard = match texts.confirm_icon {
    Some(glyph) => discard.icon(glyph),
    None => discard,
  };
  let discard = match confirm {
    Some(confirm) => discard.on_click(move || confirm()),
    None => discard.on_click(|| {}).enabled(false),
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ACTIONS_HEIGHT))
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Spacer::new().flex(1.0))
    .child(
      ActionButton::new(texts.keep.clone())
        .id("discard-changes-keep")
        .in_form()
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(move || keep.set(None))
        .render(ctx),
    )
    .child(discard.render(ctx))
    .into()
}
