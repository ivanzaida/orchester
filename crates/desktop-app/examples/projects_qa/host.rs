//! The window's root with the system's pickers and clipboard replaced: the folder picker hands over the state's
//! folder, the file picker the stand-in `gh`, and Copy appends to a file in the QA folder.

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
  services::path_picker::{PathKind, PathPicker},
  ui::code_box::Copier,
};

#[derive(Clone)]
pub struct HostProps {
  root: RootProps,
  picker: PathPicker,
  copier: Copier,
}

impl PartialEq for HostProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root
  }
}

impl DevtoolsInspectable for HostProps {}

/// `root` with pickers that hand over `folder` and `program`, and Copy into `copied`.
pub fn props(root: RootProps, folder: PathBuf, program: PathBuf, copied: PathBuf) -> HostProps {
  let picker = PathPicker::new(move |kind, _| {
    let chosen = match kind {
      PathKind::Folder => folder.clone(),
      PathKind::File => program.clone(),
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
  HostProps { root, picker, copier }
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
    ctx.mount_keyed::<Root>("app", props.root)
  }
}
