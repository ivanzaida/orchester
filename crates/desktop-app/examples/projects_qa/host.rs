//! The window's root with the system's pickers and clipboard replaced: the folder picker hands over the state's
//! folder, the file picker the state's file, and Copy appends to a file in the QA folder. A state can replace the
//! search for agent CLIs too.

use std::{fs::OpenOptions, io::Write as _, path::PathBuf};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  node::Element,
};
use orchester_app::{
  root::{Root, RootProps},
  services::{
    cli_search::CliSearch,
    path_picker::{PathKind, PathPicker},
  },
  ui::code_box::Copier,
};

#[derive(Clone)]
pub struct HostProps {
  root: RootProps,
  picker: PathPicker,
  copier: Copier,
  cli_search: Option<CliSearch>,
}

impl PartialEq for HostProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root
  }
}

impl DevtoolsInspectable for HostProps {}

/// `root` with pickers that hand over `folder` and `file`, Copy into `copied`, and `cli_search` if the state has one.
pub fn props(
  root: RootProps,
  (folder, file): (PathBuf, PathBuf),
  copied: PathBuf,
  cli_search: Option<CliSearch>,
) -> HostProps {
  let picker = PathPicker::new(move |kind, _| {
    let chosen = match kind {
      PathKind::Folder => folder.clone(),
      PathKind::File => file.clone(),
    };
    Box::pin(async move { Some(chosen) })
  });
  let copier = Copier::other(move |text| {
    OpenOptions::new()
      .create(true)
      .append(true)
      .open(&copied)
      .and_then(|mut file| writeln!(file, "{text}"))
      .is_ok()
  });
  HostProps {
    root,
    picker,
    copier,
    cli_search,
  }
}

pub struct Host;

impl Component for Host {
  type Props = HostProps;

  fn create(_: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<HostProps>().clone();
    ctx.provide(props.picker);
    ctx.provide(props.copier);
    if let Some(cli_search) = props.cli_search {
      ctx.provide(cli_search);
    }
    ctx.mount_keyed::<Root>("app", props.root)
  }
}
