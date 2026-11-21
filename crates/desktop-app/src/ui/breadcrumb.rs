//! Breadcrumb (`Crumbs` `A7q3o` on the task page, `VpX0E` on New task): where a page sits, lurq's `ui::Breadcrumb` in
//! the theme's recipe (`theme::kit::breadcrumb`). The task page separates crumbs with a `/`, New task with a
//! `chevron-right`: the boards differ, so the caller picks. Its links are pointer-only.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::{MouseButton, MouseEvent},
  },
  node::Element,
  ui::{Breadcrumb, BreadcrumbSeparator, BreadcrumbStep},
};

use super::{icon::Icon, pressable::Press};
use crate::{
  navigation::{
    link::{Open, follow},
    press::OpenIn,
  },
  theme::kit::breadcrumb::{ANCESTOR, LAST, MONO, TITLE},
};

/// The most room a link takes: a longer name is cut there with an ellipsis, said whole in a tooltip. Links keep their
/// room and the page's own crumb gives up what the row lacks, so no crumb is squeezed away.
const LINK_MAX_WIDTH: f32 = 160.0;

/// The task page's separator: a mark, the same in every language.
pub(crate) const SLASH: &str = "/";

/// What stands between two crumbs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrumbSeparator {
  Slash,
  Chevron,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Words {
  Plain,
  Mono,
  Title,
}

pub struct Crumb {
  label: Arc<str>,
  words: Words,
  open: Option<Press>,
  ancestor: bool,
}

impl Crumb {
  /// A crumb that opens its page, a place of the window, with `open` by a browser's rules (`navigation::link`).
  pub fn to(label: impl Into<Arc<str>>, open: Open) -> Self {
    Self::pressed(label, Some(Press::Open(open)))
  }

  /// A crumb that goes back to a panel of its page, which is no place of its own: the page shows it in place.
  pub fn link(label: impl Into<Arc<str>>, open: impl Fn() + Send + Sync + 'static) -> Self {
    Self::pressed(label, Some(Press::run(open)))
  }

  /// The page itself, the last crumb; before the last, a place on the way that cannot be gone to.
  pub fn here(label: impl Into<Arc<str>>) -> Self {
    Self::pressed(label, None)
  }

  fn pressed(label: impl Into<Arc<str>>, open: Option<Press>) -> Self {
    Self {
      label: label.into(),
      words: Words::Plain,
      open,
      ancestor: false,
    }
  }

  /// The label is an identifier, e.g. a task's key.
  pub fn mono(mut self) -> Self {
    self.words = Words::Mono;
    self
  }

  /// An ancestor of the page even when it ends the trail (the code viewer's folders, whose page names the file
  /// in its title): in `text_muted` like the others (`gxF9b` in `o1Uea4`).
  pub fn ancestor(mut self) -> Self {
    self.ancestor = true;
    self
  }

  /// The page title in the last crumb (Effective instructions, c3yZoo).
  pub fn title(mut self) -> Self {
    self.words = Words::Title;
    self
  }

  /// The step `index` with the id `id`; a link that ends the trail stays a link.
  fn step(self, id: String, index: usize, last: bool) -> BreadcrumbStep {
    let step = BreadcrumbStep::new(&self.label)
      .value(&index.to_string())
      .id(id)
      .state(MONO, self.words == Words::Mono)
      .state(TITLE, self.words == Words::Title)
      .state(ANCESTOR, self.ancestor);
    match self.open {
      None => step.link(false),
      Some(press) => pressed(step.link(true).state(LAST, last), press),
    }
  }
}

/// `step` followed by `press`: an action on a click and a click with Ctrl (Cmd on macOS), a place by a browser's rules.
fn pressed(step: BreadcrumbStep, press: Press) -> BreadcrumbStep {
  match press {
    Press::Run(action) => {
      let aside = action.clone();
      step
        .on_select(move || action())
        .on_open_aside(move |event: MouseEvent| {
          if event.button != MouseButton::Middle {
            aside();
          }
        })
    }
    Press::Open(open) => {
      let aside = open.clone();
      step
        .on_select(move || open(OpenIn::ActiveTab))
        .on_open_aside(move |event: MouseEvent| follow(&aside, &event))
    }
  }
}

/// `crumbs` in order, the page itself last, mounted so the tooltips of cut names keep what they read; crumb `n` has the
/// element id `<id>-<n>`, the separators lurq's `<id>-separator`, `<id>-separator-2`, ...
pub fn breadcrumb(ctx: &mut Ctx, id: &str, separator: CrumbSeparator, crumbs: Vec<Crumb>) -> Element {
  let count = crumbs.len();
  let steps = crumbs
    .into_iter()
    .enumerate()
    .map(|(index, crumb)| crumb.step(format!("{id}-{index}"), index, index + 1 == count));
  let separator = match separator {
    CrumbSeparator::Slash => BreadcrumbSeparator::Text(Arc::from(SLASH)),
    CrumbSeparator::Chevron => BreadcrumbSeparator::Icon(Icon::ChevronRight.name().into()),
  };
  Breadcrumb::new(steps)
    .separator(separator)
    .link_max_width(LINK_MAX_WIDTH)
    .tab_stop(false)
    .id(id.to_owned())
    .mount(ctx)
}
