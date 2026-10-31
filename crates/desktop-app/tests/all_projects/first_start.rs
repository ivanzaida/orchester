//! The first start (01 `H9CZ1`, signed in 01b `Knacr`): the three ways in, signed out as a fresh install starts and
//! signed in to the stand-in Orchester Cloud, where each leads, *Choose folder…* with a stand-in picker, and its roles.
//! A joined team named on the third way is `join_team`'s (`join_team_first_start_names_the_joined_team`).

use std::fs;

use lurq::{
  app::{App, theme::TypographyStyle},
  layout::text_style::FontWeight,
};
use orchester_app::{
  routes::{JoinWay, Route, RouteFolder},
  test_support::stand_ins::StandIns,
  theme::{self, Mode, StartupSpacing, StartupText},
};

use super::{
  SCREEN,
  support::{Data, Window, signed_in::account_cloud::EMAIL},
};

/// The first start of a fresh install, nobody signed in (01 `H9CZ1`).
fn first_start() -> (Window, StandIns) {
  shown(Window::with_stand_ins(Data::new()))
}

/// The first start signed in to the stand-in Orchester Cloud (01b `Knacr`).
fn signed_in_first_start() -> (Window, StandIns) {
  let data = Data::new();
  let launch = data.launch_signed_in();
  shown(Window::launched_with_stand_ins(data, launch))
}

fn shown((mut window, stand_ins): (Window, StandIns)) -> (Window, StandIns) {
  window.until("the first start", |window| {
    window.tree.get_element_by_id("welcome-ways-row").is_some()
  });
  (window, stand_ins)
}

fn bounds(window: &mut Window, id: &str) -> lurq::core::ElementRect {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

#[test]
fn all_projects_first_start_signed_out_needs_no_account() {
  let (mut window, _stand_ins) = first_start();
  let texts = window.text(SCREEN);
  for words in [
    "Sign in with Orchester Cloud to see your teams, or enter a pairing code a teammate gives you.",
    "Use a code",
    "Choose project…",
  ] {
    assert!(texts.contains(words), "{words}: {texts}");
  }
  assert!(window.tree.get_element_by_id("welcome-your-teams").is_none());
  assert_eq!(
    window.text("welcome-local-text"),
    "No account is needed to work on this computer. Orchester Cloud is only for team features."
  );
  window.press("welcome-sign-in");
  assert_eq!(window.navigation().route_now(), Route::JoinTeam(JoinWay::Cloud));
  window.navigation().go(Route::AllProjects);
  window.until("the first start again", |window| {
    window.tree.get_element_by_id("welcome-use-code").is_some()
  });
  window.press("welcome-use-code");
  assert_eq!(window.navigation().route_now(), Route::JoinTeam(JoinWay::Code));
}

#[test]
fn all_projects_first_start_offers_three_equal_ways() {
  let (mut window, _stand_ins) = signed_in_first_start();
  let texts = window.text(SCREEN);
  for words in [
    "WELCOME TO ORCHESTER",
    "How do you want to start?",
    "Add a project from a folder",
    "On this computer · no account",
    "Choose folder…",
    "Join a team",
    "Pick one of your teams, or enter a pairing code a teammate gives you.",
    "Your teams",
    "Use a code",
    "Open a team's shared project",
    "Needs a team · join on the way",
    "Choose project…",
    "Projects, tasks, runs and worktrees stay on this computer.",
  ] {
    assert!(texts.contains(words), "{words}: {texts}");
  }
  assert_eq!(
    window.text("welcome-local-text"),
    format!("Signed in as {EMAIL}. Local projects never need the account.")
  );
  assert!(window.tree.get_element_by_id("sidebar").is_none(), "no sidebar");
  assert!(
    window.tree.get_element_by_id("welcome-invites").is_none(),
    "no invite waits"
  );

  let ways = ["welcome-folder", "welcome-join", "welcome-shared"].map(|id| bounds(&mut window, id));
  for way in &ways[1..] {
    assert!((way.width - ways[0].width).abs() < 0.5, "equal ways: {ways:?}");
    assert!((way.height - 300.0).abs() < 0.5, "as high as the row: {ways:?}");
    assert!((way.y - ways[0].y).abs() < 0.5, "side by side: {ways:?}");
  }
  assert!(
    (ways[1].x - (ways[0].x + ways[0].width) - 16.0).abs() < 0.5,
    "16 apart: {ways:?}"
  );
  let inner = bounds(&mut window, "welcome-inner");
  assert!((inner.width - 1040.0).abs() < 0.5, "{inner:?}");
}

#[test]
fn all_projects_first_start_ways_lead_to_their_steps() {
  let (mut window, _stand_ins) = signed_in_first_start();
  window.press("welcome-your-teams");
  assert_eq!(window.navigation().route_now(), Route::JoinTeam(JoinWay::Cloud));
  window.navigation().go(Route::AllProjects);
  window.until("the first start again", |window| {
    window.tree.get_element_by_id("welcome-use-code").is_some()
  });
  window.press("welcome-use-code");
  assert_eq!(window.navigation().route_now(), Route::JoinTeam(JoinWay::Code));
  window.navigation().go(Route::AllProjects);
  window.until("the first start again", |window| {
    window.tree.get_element_by_id("welcome-choose-project").is_some()
  });
  window.press("welcome-choose-project");
  assert_eq!(
    window.navigation().route_now(),
    Route::JoinTeam(JoinWay::Cloud),
    "before a team is joined, the way goes through Join a team"
  );
}

#[test]
fn all_projects_first_start_choose_folder_sets_up_a_repository() {
  let (mut window, stand_ins) = first_start();
  let plain = window.data.repo("repo-plain");
  stand_ins.choose(Some(plain.clone()));
  window.press("welcome-choose-folder");
  window.until("Add project on the folder", |window| {
    window.navigation().route_now() == Route::AddProject(RouteFolder::new(&plain))
  });
}

#[test]
fn all_projects_first_start_choose_folder_says_why_it_opened_nothing() {
  let (mut window, stand_ins) = first_start();
  let inside = window.data.repo("repo-outer").join("inside");
  fs::create_dir_all(&inside).expect("a folder inside the repository");
  stand_ins.choose(Some(inside.clone()));
  window.press("welcome-choose-folder");
  window.until("the refusal", |window| window.texts("welcome-error").is_some());
  assert!(
    window.text("welcome-error").contains(&inside.display().to_string()),
    "the line names the folder"
  );
  assert_eq!(window.navigation().route_now(), Route::AllProjects);
}

#[test]
fn all_projects_first_start_roles_follow_the_design() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  let check = |role: TypographyStyle, (size, line): (f32, f32), weight: FontWeight, tracking: f32| {
    let style = theme.typography_style(role);
    assert_eq!(
      (
        style.font_size,
        (style.font_size * style.line_height * 100.0).round() / 100.0,
        style.weight,
        style.letter_spacing,
        &*style.font_family,
      ),
      (size, line, weight, tracking, "Geist"),
      "{role:?}"
    );
  };
  check(StartupText::StartTitle.role(), (26.0, 32.5), FontWeight::SemiBold, -0.5);
  check(StartupText::CardTitle.role(), (16.0, 20.8), FontWeight::SemiBold, 0.0);
  check(StartupText::CardBody.role(), (12.5, 18.75), FontWeight::Normal, 0.0);
  check(StartupText::CardMeta.role(), (11.5, 14.95), FontWeight::Normal, 0.0);
  check(StartupText::Footer.role(), (11.5, 16.1), FontWeight::Normal, 0.0);
  assert_eq!(StartupSpacing::Head.value(), 10.0);
}
