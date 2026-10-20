//! The notifier's rules: once per event and not for what stood at the first reading, a group's events within 30 s as
//! one, nothing while the window shows the place, in quiet hours or for a group switched off, and an interruption and
//! a merge held 30 s.

use super::*;

/// A plain event of `kind` keyed `key`, about task ORC-`number` of project `project`.
fn event(project: ProjectId, key: &str, kind: Kind, number: u32) -> Event {
  let task: orchester_core::project::TaskKey = format!("ORC-{number}").parse().expect("a key");
  Event {
    key: key.into(),
    batch: None,
    kind,
    origin: Origin::Project {
      id: project,
      name: "Kontur".into(),
    },
    task: Some(EventTask {
      key: task.clone(),
      title: "A task".into(),
    }),
    step: None,
    target: Target::Task {
      project,
      key: task,
      page: TaskPage::Conversation,
      card: None,
    },
  }
}

fn question(project: ProjectId, key: &str, number: u32) -> Event {
  event(
    project,
    key,
    Kind::Question {
      agent: "Planner".into(),
    },
    number,
  )
}

#[test]
fn notifier_tells_nothing_of_the_first_reading_and_each_new_event_once() {
  let project = ProjectId::generate();
  let scope = Scope::Project(project);
  let settings = NotificationSettings::default();
  let mut notifier = Notifier::default();
  notifier.observe(scope, vec![question(project, "a", 1)], at(0));
  assert!(
    notifier.due(&view(at(0), false, &settings)).is_empty(),
    "it stood already"
  );
  notifier.observe(scope, vec![question(project, "a", 1), question(project, "b", 2)], at(1));
  let shown = notifier.due(&view(at(1), false, &settings));
  assert_eq!(shown.len(), 1);
  assert_eq!(shown[0].title.key, "notifications.title.question");
  notifier.observe(
    scope,
    vec![question(project, "a", 1), question(project, "b", 2)],
    at(100_000),
  );
  assert!(
    notifier.due(&view(at(100_000), false, &settings)).is_empty(),
    "no repeat"
  );
  notifier.observe(scope, vec![question(project, "a", 1)], at(100_001));
  notifier.observe(
    scope,
    vec![question(project, "a", 1), question(project, "b", 2)],
    at(100_002),
  );
  assert_eq!(
    notifier.due(&view(at(100_002), false, &settings)).len(),
    1,
    "a wait that came back is new"
  );
}

#[test]
fn notifier_coalesces_a_groups_events_within_30_seconds() {
  let project = ProjectId::generate();
  let scope = Scope::Project(project);
  let settings = NotificationSettings::default();
  let mut notifier = Notifier::default();
  notifier.observe(scope, Vec::new(), at(0));
  notifier.observe(scope, vec![question(project, "a", 18)], at(1_000));
  assert_eq!(
    notifier.due(&view(at(1_000), false, &settings)).len(),
    1,
    "the first at once"
  );
  let three = vec![
    question(project, "a", 18),
    question(project, "b", 55),
    question(project, "c", 56),
    question(project, "d", 57),
  ];
  notifier.observe(scope, three, at(5_000));
  assert!(
    notifier.due(&view(at(5_000), false, &settings)).is_empty(),
    "within 30 s they wait"
  );
  assert_eq!(
    notifier.next_due(at(5_000)),
    Some(COALESCE - std::time::Duration::from_secs(4))
  );
  let shown = notifier.due(&view(at(31_000), false, &settings));
  assert_eq!(shown.len(), 1, "one for the group");
  assert_eq!(shown[0].title.key, "notifications.many.needsYou");
  assert_eq!(shown[0].title.args, [("count", "3".to_owned())]);
  assert_eq!(shown[0].body.key, "notifications.list.project.more");
  assert_eq!(
    shown[0].body.args,
    [
      ("first", "ORC-55".to_owned()),
      ("second", "ORC-56".to_owned()),
      ("count", "1".to_owned()),
      ("project", "Kontur".to_owned()),
    ]
  );
}

#[test]
fn notifier_shows_only_while_the_window_is_unfocused_or_shows_elsewhere() {
  let project = ProjectId::generate();
  let scope = Scope::Project(project);
  let settings = NotificationSettings::default();
  let mut notifier = Notifier::default();
  notifier.observe(scope, Vec::new(), at(0));
  notifier.observe(scope, vec![question(project, "a", 1)], at(0));
  assert!(
    notifier.due(&view(at(0), true, &settings)).is_empty(),
    "the person looks at it"
  );
  notifier.observe(scope, vec![question(project, "a", 1), question(project, "b", 2)], at(1));
  let elsewhere = View {
    on_screen: &|_| false,
    ..view(at(1), true, &settings)
  };
  assert_eq!(notifier.due(&elsewhere).len(), 1, "focused, but another place shows");
}

#[test]
fn notifier_keeps_quiet_hours_and_switched_off_groups_but_the_badge_counts() {
  let project = ProjectId::generate();
  let scope = Scope::Project(project);
  let quiet = NotificationSettings {
    quiet: Some(QuietHours::new(11 * 60, 13 * 60).expect("quiet")),
    ..NotificationSettings::default()
  };
  let mut notifier = Notifier::default();
  notifier.observe(scope, Vec::new(), at(0));
  notifier.set_needs_you(project, 2);
  notifier.observe(scope, vec![question(project, "a", 1)], at(0));
  assert!(notifier.due(&view(at(0), false, &quiet)).is_empty(), "quiet at noon");
  assert_eq!(notifier.badge(), 2, "the badge still counts");

  let ready = event(project, "ready", Kind::ReadyToMerge, 3);
  notifier.observe(scope, vec![question(project, "a", 1), ready.clone()], at(1));
  let settings = NotificationSettings::default();
  assert!(
    notifier.due(&view(at(1), false, &settings)).is_empty(),
    "Finished and info is off until turned on"
  );
  notifier.observe(scope, vec![question(project, "a", 1)], at(2));
  notifier.observe(scope, vec![question(project, "a", 1), ready], at(3));
  let on = settings.with(NotificationGroup::Finished, true);
  assert_eq!(notifier.due(&view(at(3), false, &on)).len(), 1);
  notifier.forget(scope);
  assert_eq!(notifier.badge(), 0);
}

#[test]
fn notifier_drops_what_was_handled_and_holds_an_interruption() {
  let project = ProjectId::generate();
  let scope = Scope::Project(project);
  let settings = NotificationSettings::default();
  let mut notifier = Notifier::default();
  notifier.observe(scope, Vec::new(), at(0));
  notifier.observe(scope, vec![question(project, "a", 1)], at(0));
  assert_eq!(notifier.due(&view(at(0), false, &settings)).len(), 1);
  notifier.observe(
    scope,
    vec![question(project, "a", 1), question(project, "b", 2)],
    at(1_000),
  );
  notifier.observe(scope, vec![question(project, "a", 1)], at(2_000));
  assert!(
    notifier.due(&view(at(40_000), false, &settings)).is_empty(),
    "answered before it showed"
  );

  let interrupted = event(project, "int", Kind::Interrupted, 3);
  notifier.observe(scope, vec![interrupted.clone()], at(50_000));
  assert!(notifier.due(&view(at(50_000), false, &settings)).is_empty(), "held");
  notifier.observe(scope, Vec::new(), at(60_000));
  assert!(
    notifier.due(&view(at(90_000), false, &settings)).is_empty(),
    "the Orchestrator continued it"
  );
  notifier.observe(scope, vec![interrupted], at(100_000));
  let shown: Vec<Notification> = notifier.due(&view(at(130_000), false, &settings));
  assert_eq!(shown.len(), 1, "told once it stood 30 s");
  assert_eq!(shown[0].group, NotificationGroup::Stopped);
}

#[test]
fn notifier_holds_a_merge_so_the_chats_record_of_its_own_can_drop_it() {
  let project = ProjectId::generate();
  let scope = Scope::Project(project);
  let finished = NotificationSettings::default().with(NotificationGroup::Finished, true);
  let mut notifier = Notifier::default();
  notifier.observe(scope, Vec::new(), at(0));
  let merged = event(project, "merged:1", Kind::Merged, 3);
  notifier.observe(scope, vec![merged.clone()], at(1_000));
  assert!(notifier.due(&view(at(1_000), false, &finished)).is_empty(), "held");
  notifier.observe(scope, Vec::new(), at(5_000));
  assert!(
    notifier.due(&view(at(40_000), false, &finished)).is_empty(),
    "the chat's record came: the Orchestrator merged it"
  );
  notifier.observe(scope, vec![merged], at(50_000));
  let shown = notifier.due(&view(at(80_000), false, &finished));
  assert_eq!(shown.len(), 1, "a person's merge is told once it stood 30 s");
  assert_eq!(shown[0].title.key, "notifications.title.merged");
}
