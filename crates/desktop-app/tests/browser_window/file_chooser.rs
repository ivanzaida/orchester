//! A file field of the person's tab asks for files (`03b · File chooser · dialog open` `TWpyp`, `03c` `x0N8d`): the
//! window opens the system's dialog once, with the single or multiple choice and the types the page gave, shows its
//! hint over the page while the dialog is open, and sends the choice or the cancel back to the tab. The dialog is the
//! test's stand-in, which the test answers.

use std::{path::PathBuf, sync::Arc};

use orchester_app::services::page_file_picker::{PageFiles, extensions};
use orchester_ipc::browser::{
  BrowserCommand, BrowserEvent, BrowserMode, BrowserTabId, FileChoice, FileChooser, FileChooserId,
};
use parking_lot::Mutex;
use tokio::sync::oneshot;

use super::support::{Browser, browser_installed, state};

/// The stand-in dialogs opened, each with what it was asked and where its answer goes.
type Opened = Arc<Mutex<Vec<(PageFiles, oneshot::Sender<Option<Vec<PathBuf>>>)>>>;

fn asked(id: u64, label: Option<&str>) -> BrowserEvent {
  BrowserEvent::FileChooserOpened {
    tab: BrowserTabId::new("P1"),
    chooser: FileChooser {
      id: FileChooserId::new(id),
      multiple: label.is_none(),
      accept: vec![".PEM".to_owned(), "application/x-pem-file".to_owned()],
      label: label.map(str::to_owned),
    },
  }
}

/// The answers the window sent the tabs since the last look.
fn answers(browser: &Browser) -> Vec<BrowserCommand> {
  browser
    .state
    .recorded_commands_for_test()
    .into_iter()
    .map(|(_, command)| command)
    .filter(|command| matches!(command, BrowserCommand::ChooseFiles { .. }))
    .collect()
}

#[test]
fn browser_file_chooser_opens_the_dialog_once_hints_over_the_page_and_sends_the_answer() {
  if !browser_installed() {
    return;
  }
  let mut browser = Browser::open(false);
  let opened = Opened::default();
  let dialogs = opened.clone();
  browser.link.services().page_files.stand_in(move |asked| {
    let (answer, chosen) = oneshot::channel();
    dialogs.lock().push((asked, answer));
    Box::pin(async move { chosen.await.ok().flatten() })
  });
  browser.state.record_commands_for_test();
  browser.feed(state("P1", BrowserMode::Headless).0);
  browser.until_shown("browser-page");

  browser.feed(asked(1, Some("Signing certificate")));
  browser.until_shown("browser-file-hint");
  assert_eq!(
    browser.text("browser-file-hint-title"),
    "Choose a file in the dialog that opened"
  );
  assert_eq!(
    browser.text("browser-file-hint-line"),
    "localhost:5173 asks for a file for Signing certificate. Cancel there to leave the page as it is."
  );
  browser.pass();
  let (files, answer) = opened.lock().pop().expect("the dialog opened");
  assert!(opened.lock().is_empty(), "once for the request");
  assert!(!files.multiple);
  assert_eq!(files.all_files.as_ref(), "All files");
  assert_eq!(extensions(&files.accept), ["pem"], "a MIME type names no extension");
  let chosen = vec![PathBuf::from("kontur-hooks.pem")];
  answer.send(Some(chosen.clone())).expect("the window waits for it");
  browser.until("the hint to go", |browser| !browser.shows("browser-file-hint"));
  let sent = wait_for_answer(&mut browser);
  assert_eq!(
    sent,
    BrowserCommand::ChooseFiles {
      tab: BrowserTabId::new("P1"),
      chooser: FileChooserId::new(1),
      choice: FileChoice::Chosen { paths: chosen },
    }
  );

  browser.feed(asked(2, None));
  browser.until_shown("browser-file-hint");
  assert_eq!(
    browser.text("browser-file-hint-line"),
    "localhost:5173 asks for a file. Cancel there to leave the page as it is."
  );
  let (files, answer) = opened.lock().pop().expect("the dialog opened");
  assert!(files.multiple);
  answer.send(None).expect("the window waits for it");
  browser.until("the hint to go", |browser| !browser.shows("browser-file-hint"));
  assert_eq!(
    wait_for_answer(&mut browser),
    BrowserCommand::ChooseFiles {
      tab: BrowserTabId::new("P1"),
      chooser: FileChooserId::new(2),
      choice: FileChoice::Cancelled,
    }
  );
}

/// The one answer the window sends next.
fn wait_for_answer(browser: &mut Browser) -> BrowserCommand {
  let sent = Mutex::new(Vec::new());
  browser.until("the answer to be sent", |browser| {
    sent.lock().extend(answers(browser));
    !sent.lock().is_empty()
  });
  let mut sent = sent.into_inner();
  assert_eq!(sent.len(), 1, "{sent:?}");
  sent.remove(0)
}
