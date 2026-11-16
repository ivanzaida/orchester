//! *Keep the computer awake while agents work* (`orchester_app::awake`, `docs/plan/orchestrator-autonomy/OA-08-…`):
//! the app holds the system awake while work is under way in a project and the person's setting is on, and lets go
//! at once when either ends. The hold itself is the system's (`orchester_exec::awake`): on Windows and macOS it is
//! really taken here, and released.
//! `cargo test -p orchester-app --test awake awake`.

use std::path::Path;

use orchester_app::awake::KeepAwake;

#[cfg(any(windows, target_os = "macos"))]
#[test]
fn awake_holds_only_while_work_is_under_way_and_the_setting_is_on() {
  let awake = KeepAwake::default();
  let (kontur, billing) = (Path::new("kontur"), Path::new("billing"));
  assert!(!awake.holds());

  // Work with the setting off holds nothing.
  awake.look(kontur, true, false);
  assert!(!awake.holds());
  // The setting on, and work under way: held.
  awake.look(kontur, true, true);
  assert!(awake.holds());
  // Another project works too; the first one ending changes nothing.
  awake.look(billing, true, true);
  awake.look(kontur, false, true);
  assert!(awake.holds(), "billing still works");
  awake.look(billing, false, true);
  assert!(!awake.holds(), "nothing works: the computer may sleep");

  // The person turns the setting off while an agent works: let go at once.
  awake.look(kontur, true, true);
  assert!(awake.holds());
  awake.release();
  assert!(!awake.holds());
}
