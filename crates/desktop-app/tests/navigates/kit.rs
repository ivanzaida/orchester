//! The UI kit's navigating controls alone, in a window of their own with the app's navigation: a focused button of a
//! form that opens a place (`ActionButton::opens`), which Enter presses with the keys held, and a link inside a row
//! that opens a page of its own, whose middle click is the link's alone.

use std::sync::Arc;

use lurq::{
  app::{
    App, Tree,
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::MouseButton,
  },
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_app::{
  navigation::{Navigation, tabs::TabSet},
  routes::{Route, routes},
  theme::{self, Mode},
  ui::{
    button::{ActionButton, ButtonKind},
    pressable::opening,
    table::{ColumnWidth, TableRow},
  },
};
use parking_lot::Mutex;

use crate::support::link_press::{Presser, enter_in, press_in};

/// Where the button leads.
const BUTTON_PLACE: Route = Route::Settings;
/// Where the link inside the row leads; the row itself opens [`Route::Usage`].
const LINK_PLACE: Route = Route::AddProject(None);
/// The link's room in its row.
const LINK_SIZE: (f32, f32) = (80.0, 20.0);

/// The navigation the root made, for the test to read.
#[derive(Clone, Default)]
struct Made(Arc<Mutex<Option<Navigation>>>);

impl PartialEq for Made {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl DevtoolsInspectable for Made {}

struct KitRoot {
  navigation: Navigation,
}

impl Component for KitRoot {
  type Props = Made;

  fn create(ctx: &mut Ctx) -> Self {
    let router = ctx.router(routes());
    let navigation = Navigation::new(ctx.signal(TabSet::default()), router);
    *ctx.props::<Made>().0.lock() = Some(navigation.clone());
    Self { navigation }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    ctx.provide(self.navigation.clone());
    let button = ActionButton::new("kit-button", ButtonKind::Secondary, "Settings")
      .in_form()
      .opens(self.navigation.to(BUTTON_PLACE));
    let link = opening(self.navigation.to(LINK_PLACE))
      .id("kit-link")
      .width(Dimension::Px(LINK_SIZE.0))
      .height(Dimension::Px(LINK_SIZE.1));
    let row = TableRow::new("kit-row")
      .cell(ColumnWidth::Fixed(LINK_SIZE.0), link)
      .opens(self.navigation.to(Route::Usage(Box::default())));
    Column::new()
      .width(Dimension::Pct(100.0))
      .child(Element::from(button))
      .child(Element::from(row))
  }
}

struct Kit {
  app: App,
  tree: Tree,
  navigation: Navigation,
}

impl Kit {
  fn open() -> Self {
    let mut app = App::new();
    theme::install(&mut app, Mode::Dark);
    let mut tree = Tree::new();
    tree.resize(800, 600);
    let made = Made::default();
    tree.mount_root::<KitRoot>(&mut app, made.clone());
    tree.pass_headless(&mut app);
    let navigation = made.0.lock().clone().expect("the root made its navigation");
    Self { app, tree, navigation }
  }

  fn pass(&mut self) {
    self.tree.pass_headless(&mut self.app);
  }

  fn places(&self) -> Vec<Route> {
    let tabs = self.navigation.tabs_now();
    tabs.tabs().iter().map(|tab| tab.route().clone()).collect()
  }
}

#[test]
fn enter_with_ctrl_on_a_focused_button_that_opens_opens_a_background_tab() {
  let mut kit = Kit::open();
  let home = kit.navigation.tabs_now().active().id();
  kit
    .tree
    .get_element_by_id_mut("kit-button")
    .expect("the button is shown")
    .focus();
  kit.pass();
  enter_in(&mut kit.tree, true);
  kit.pass();
  assert_eq!(
    kit.places(),
    [Route::AllProjects, BUTTON_PLACE],
    "a new tab right of the active one"
  );
  assert_eq!(
    kit.navigation.tabs_now().active().id(),
    home,
    "the active tab stays active"
  );
  assert_eq!(kit.navigation.route_now(), Route::AllProjects);

  enter_in(&mut kit.tree, false);
  kit.pass();
  assert_eq!(
    kit.navigation.route_now(),
    BUTTON_PLACE,
    "Enter alone opens it in the active tab"
  );
  assert_eq!(kit.places().len(), 2, "and no tab");
}

#[test]
fn a_middle_click_on_a_link_inside_a_row_that_opens_opens_one_tab_at_the_link() {
  let mut kit = Kit::open();
  let bounds = kit
    .tree
    .get_element_by_id_mut("kit-link")
    .and_then(|element| element.bounds())
    .expect("the link is laid out");
  let mut expected = vec![Route::AllProjects];
  for presser in [Presser::Person, Presser::Synthetic] {
    press_in(&mut kit.tree, bounds, (presser, MouseButton::Middle), (false, false));
    kit.pass();
    expected.insert(1, LINK_PLACE);
    assert_eq!(
      kit.places(),
      expected,
      "{presser:?}: one new tab, the link's, and not the row's"
    );
    assert_eq!(kit.navigation.route_now(), Route::AllProjects);
  }
}
