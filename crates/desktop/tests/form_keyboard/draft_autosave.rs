//! Typing into a step while its draft is being saved: the app keeps running and the newest text
//! is what ends up saved.

use std::{sync::mpsc, thread, time::Duration};

use orchester_desktop::pipelines::{data::OpenedPipeline, library::library_in};

use crate::harness::Desktop;

const NAME: &str = "step-name";
/// Longer than the whole scenario takes; a hang fails the test instead of the run.
const DEADLINE: Duration = Duration::from_secs(60);

/// Runs `scenario` on its own thread and fails when it does not finish in time. A hung thread
/// is left behind: nothing can stop it, and the process ends with the test run.
fn within_deadline(scenario: impl FnOnce() + Send + 'static) {
  let (done, finished) = mpsc::channel();
  let worker = thread::spawn(move || {
    scenario();
    // The receiver is gone only after the deadline passed, which already failed the test.
    let _ = done.send(());
  });
  match finished.recv_timeout(DEADLINE) {
    Ok(()) => worker.join().expect("the scenario thread finished"),
    Err(mpsc::RecvTimeoutError::Disconnected) => {
      if let Err(panic) = worker.join() {
        std::panic::resume_unwind(panic);
      }
    }
    Err(mpsc::RecvTimeoutError::Timeout) => panic!("the app stopped responding within {DEADLINE:?}"),
  }
}

#[test]
fn typing_into_a_step_name_while_its_draft_saves_keeps_the_app_running() {
  within_deadline(|| {
    let mut desktop = Desktop::with_project();
    desktop.wait_for("nav-pipelines");
    desktop.click("nav-pipelines");
    desktop.wait_for("pipelines-create-starter");
    desktop.click("pipelines-create-starter");
    desktop.wait_for(NAME);
    desktop.click(NAME);
    // Every key is an edit; the first starts a save, the others arrive while it runs.
    desktop.type_text("xyz");
    desktop.tree.tick_futures();
    desktop.type_text("uvw");
    let typed = desktop.value(NAME);
    assert!(typed.ends_with("xyzuvw"), "the field keeps every key: {typed}");
    let repo = desktop.repo().to_path_buf();
    let stored_name = move || {
      let pipeline = library_in(&repo).expect("the library loads")[0].id;
      let opened = OpenedPipeline::load_in(&repo, pipeline).expect("the pipeline opens");
      opened.body.steps()[0].name.as_str().to_owned()
    };
    desktop.settle("the newest name saved", |_| stored_name() == typed);
  });
}
