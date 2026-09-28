//! Unsaved edits of a tab, and the question closing such a tab asks (design doc, "Tabs": closing
//! asks only when the tab's page would ask on navigating away). A page that knows it holds edits
//! not saved yet — an agent profile or a command tool before *Save draft* — marks them through
//! [`TabEdits`]; pages whose drafts save themselves never do. The question is not drawn yet: it is
//! built from the confirmation dialog's frame (`P8IN7Z`, as Remove from list uses it).

use std::{
  collections::{HashMap, HashSet},
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
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    backdrop::backdrop,
    button::ActionButton,
    form::{Submit, form},
    icon::Icon,
  },
};

/// Dialog width, action bar and button heights, as the confirmation dialog's (`P8IN7Z`, `cDtbR`).
const DIALOG_WIDTH: f32 = 600.0;
const ACTIONS_HEIGHT: f32 = 76.0;
const BUTTON_HEIGHT: f32 = 36.0;

/// Which pages of which tabs hold unsaved edits.
#[derive(Default)]
pub struct Unsaved {
  marks: Mutex<HashMap<TabId, HashSet<u64>>>,
}

impl Unsaved {
  /// Whether a page of `tab` holds unsaved edits.
  pub fn any(&self, tab: TabId) -> bool {
    self.marks.lock().get(&tab).is_some_and(|marks| !marks.is_empty())
  }

  fn set(&self, tab: TabId, mark: u64, unsaved: bool) {
    let mut marks = self.marks.lock();
    let marks = marks.entry(tab).or_default();
    if unsaved {
      marks.insert(mark);
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
    self.edits.tabs.unsaved().set(self.edits.tab, self.id, unsaved);
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

/// "Discard changes?" for the tab being closed: keep it open, or discard the edits and close it.
/// Escape keeps it open.
pub struct DiscardChanges;

impl Component for DiscardChanges {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let tabs = tabs(ctx);
    match tabs.discarding().get() {
      Some(tab) => ctx.mount_keyed::<DiscardDialog>(&format!("discard-{tab}"), tab),
      None => Column::new().into(),
    }
  }
}

impl lurq::app::component::DevtoolsInspectable for TabId {}

/// The question about one tab; a new one is mounted each time it is asked.
struct DiscardDialog {
  open: Signal<bool>,
}

impl Component for DiscardDialog {
  type Props = TabId;

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
    let tab = *ctx.props::<TabId>();
    let tabs = tabs(ctx);
    let (discarding, close) = (tabs.discarding().clone(), tabs.clone());
    let confirm: Submit = Some(Arc::new(move || {
      discarding.set(None);
      close.close(tab);
    }));
    let content = Column::new()
      .id("discard-changes-dialog")
      .width(Dimension::Px(DIALOG_WIDTH))
      .background(ExtraColor::Surface.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .child(header(ctx))
      .child(actions(ctx, &tabs, confirm.clone()));
    let dialog = backdrop(ctx, form(confirm, content));
    Modal::new(dialog).target(ModalTarget::Root).open(self.open.clone())
  }
}

fn tabs(ctx: &mut Ctx) -> Tabs {
  ctx
    .use_context::<Tabs>()
    .expect("the window provides Tabs before mounting the discard question")
}

fn header(ctx: &Ctx) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t("tabs.discard.title")).variant(ExtraTypography::DialogTitle.role()))
    .child(Text::new(&ctx.t("tabs.discard.explanation")).variant(ExtraTypography::Explanation.role()))
    .into()
}

fn actions(ctx: &Ctx, tabs: &Tabs, confirm: Submit) -> Element {
  let keep = tabs.discarding().clone();
  let discard = ActionButton::new(ctx.t("tabs.discard.confirm"))
    .primary()
    .icon(Icon::X)
    .id("discard-changes-confirm")
    .in_form()
    .height(Dimension::Px(BUTTON_HEIGHT));
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
      ActionButton::new(ctx.t("tabs.discard.keep"))
        .id("discard-changes-keep")
        .in_form()
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(move || keep.set(None))
        .render(ctx),
    )
    .child(discard.render(ctx))
    .into()
}
