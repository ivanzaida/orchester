//! The system's own file dialog for a file field of the person's tab (`03b · File chooser · dialog open` `TWpyp`):
//! opened once for each request of the project shown, from the app's own process, for one file or many and the types
//! the page gave, while the page shows its hint (`page::file_chooser`). Once it closes the hint goes, and the choice,
//! or the cancel that leaves the field as it is, goes back to the tab (`03c` `x0N8d`).

use std::{
  collections::HashSet,
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::app::ctx::Ctx;
use orchester_ipc::browser::{BrowserCommand, FileChoice, FileChooserId};
use orchester_workspace::browser::BrowserWindow;
use parking_lot::Mutex;

use super::state::Showing;
use crate::services::page_file_picker::{PageFilePicker, PageFiles};

/// The requests whose dialog the window opened, by project.
#[derive(Default)]
pub struct FileDialogs {
  opened: Mutex<HashSet<(PathBuf, FileChooserId)>>,
}

impl FileDialogs {
  /// Opens the dialog for each request of `showing`'s tabs that has none yet; the requests answered are forgotten.
  pub fn open_new(&self, ctx: &Ctx, showing: &Showing, (picker, window): (&PageFilePicker, &BrowserWindow)) {
    let project: &Path = &showing.project;
    let asked: Vec<_> = showing
      .view
      .as_ref()
      .map(|view| {
        view
          .file_choosers()
          .map(|(tab, chooser)| (tab.clone(), chooser.clone()))
          .collect()
      })
      .unwrap_or_default();
    let mut opened = self.opened.lock();
    opened.retain(|(of, id)| of != project || asked.iter().any(|(_, chooser)| chooser.id == *id));
    for (tab, chooser) in asked {
      if !opened.insert((project.to_path_buf(), chooser.id)) {
        continue;
      }
      let chosen = picker.pick(PageFiles {
        multiple: chooser.multiple,
        accept: chooser.accept,
        all_files: Arc::from(ctx.t("browser.fileChooser.allFiles").to_string()),
      });
      let (commands, window, project) = (showing.commands.clone(), window.clone(), project.to_path_buf());
      showing.commands.spawn(async move {
        let choice = match chosen.await {
          Some(paths) => FileChoice::Chosen { paths },
          None => FileChoice::Cancelled,
        };
        window.file_chooser_answered(&project, &tab, chooser.id);
        commands.send(BrowserCommand::ChooseFiles {
          tab,
          chooser: chooser.id,
          choice,
        });
      });
    }
  }
}
