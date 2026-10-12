use std::ops::{Deref, DerefMut};

use lurq::app::component::DevtoolsInspectable;
use orchester_core::integration::local::LocalPath;

/// App signal wrappers keep core independent of either lurq version's DevTools trait.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Selection(pub Vec<LocalPath>);
impl DevtoolsInspectable for Selection {}
impl Deref for Selection {
  type Target = Vec<LocalPath>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
impl DerefMut for Selection {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ShownFile(pub Option<LocalPath>);
impl DevtoolsInspectable for ShownFile {}
