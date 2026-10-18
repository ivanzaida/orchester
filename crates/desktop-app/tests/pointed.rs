//! The card a navigation points at (`Needs you · where each item leads` `icGHG`) takes the pointer in its render,
//! once, without writing a signal there: the render that takes it is not made to run again, and a new pointing at
//! the same card points again.

use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};

use lurq::{
  app::{
    App, Tree,
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  node::Element,
};
use orchester_app::{
  navigation::{
    Navigation,
    pointed::{Pointed, PointedCard},
    press::OpenIn,
    tabs::TabSet,
  },
  routes::{Route, routes},
};
use orchester_core::project::TaskKey;
use parking_lot::Mutex;

/// What the card saw: its renders, and the renders that took the pointer.
#[derive(Clone, Default)]
struct Seen {
  renders: Arc<AtomicUsize>,
  taken: Arc<AtomicUsize>,
  navigation: Arc<Mutex<Option<Navigation>>>,
}

impl PartialEq for Seen {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.renders, &other.renders)
  }
}

impl DevtoolsInspectable for Seen {}

fn card() -> Pointed {
  Pointed {
    project: "01a131b9-63a9-7180-9330-12a5ab91a2e5".parse().expect("a project id"),
    task: "KON-12".parse::<TaskKey>().expect("a task key"),
    card: PointedCard::Plan,
  }
}

struct PointedRoot {
  navigation: Navigation,
}

impl Component for PointedRoot {
  type Props = Seen;

  fn create(ctx: &mut Ctx) -> Self {
    let router = ctx.router(routes());
    let navigation = Navigation::new(ctx.signal(TabSet::default()), router);
    *ctx.props::<Seen>().navigation.lock() = Some(navigation.clone());
    Self { navigation }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    ctx.provide(self.navigation.clone());
    let seen = ctx.props::<Seen>().clone();
    Column::new().child(ctx.mount_keyed::<Card>("card", seen))
  }
}

struct Card;

impl Component for Card {
  type Props = Seen;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let seen = ctx.props::<Seen>().clone();
    seen.renders.fetch_add(1, Ordering::SeqCst);
    if Navigation::of(ctx).take_pointed(&card()) {
      seen.taken.fetch_add(1, Ordering::SeqCst);
    }
    Column::new()
  }
}

#[test]
fn pointed_card_takes_the_pointer_once_in_one_render() {
  let mut app = App::new();
  let mut tree = Tree::new();
  let seen = Seen::default();
  tree.mount_root::<PointedRoot>(&mut app, seen.clone());
  tree.pass_headless(&mut app);
  let navigation = seen.navigation.lock().clone().expect("the root made its navigation");
  let route = navigation.route_now();
  let before = seen.renders.load(Ordering::SeqCst);
  navigation.open_to_card(&route, OpenIn::ActiveTab, card());
  for _ in 0..3 {
    tree.pass_headless(&mut app);
  }
  assert_eq!(seen.taken.load(Ordering::SeqCst), 1, "the card takes the pointer");
  assert_eq!(
    seen.renders.load(Ordering::SeqCst) - before,
    1,
    "taking it does not make the card render again"
  );
  navigation.open_to_card(&route, OpenIn::ActiveTab, card());
  for _ in 0..3 {
    tree.pass_headless(&mut app);
  }
  assert_eq!(
    seen.taken.load(Ordering::SeqCst),
    2,
    "pointing at it again points again"
  );
  navigation.go(Route::AllProjects);
  tree.pass_headless(&mut app);
  assert_eq!(seen.taken.load(Ordering::SeqCst), 2);
}
