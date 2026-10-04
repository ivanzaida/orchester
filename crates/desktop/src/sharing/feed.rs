//! The servers as every screen sees them: the list with each one's connection, read again whenever
//! it changes, published by [`ServerFeedLoader`], which the root mounts once for the window. The
//! connection [`monitor`](super::monitor) runs as a task of the application's runtime.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, StreamEmitter},
  },
  core::Signal,
  node::Element,
};
use orchester_core::id::ServerId;

use super::{ServerEntry, Servers};
use crate::{background, messages::Message};

/// The list as last read.
#[derive(Clone, Debug, PartialEq)]
pub enum ServerList {
  Loading,
  Ready(Arc<[ServerEntry]>),
  Failed(Message),
}

impl lurq::app::component::DevtoolsInspectable for ServerList {}

impl ServerList {
  pub fn entries(&self) -> &[ServerEntry] {
    match self {
      Self::Ready(entries) => entries,
      Self::Loading | Self::Failed(_) => &[],
    }
  }

  pub fn entry(&self, server: ServerId) -> Option<&ServerEntry> {
    self.entries().iter().find(|entry| entry.stored.id == server)
  }
}

/// Provided by the root: the servers, their list, and a sign-in the status bar asked Settings ›
/// Servers to open.
#[derive(Clone)]
pub struct ServerFeed {
  pub servers: Servers,
  pub list: Signal<ServerList>,
  /// Set by the status bar's *Sign in*; the servers screen opens the dialog and clears it.
  pub sign_in: Signal<SignInRequest>,
}

/// The server the status bar asked to sign in to, until the servers screen opened the dialog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SignInRequest(pub Option<ServerId>);

impl lurq::app::component::DevtoolsInspectable for SignInRequest {}

impl PartialEq for ServerFeed {
  fn eq(&self, other: &Self) -> bool {
    self.servers == other.servers
  }
}

impl lurq::app::component::DevtoolsInspectable for ServerFeed {}

impl ServerFeed {
  pub fn new(servers: Servers) -> Self {
    Self {
      servers,
      list: Signal::new(ServerList::Loading),
      sign_in: Signal::new(SignInRequest::default()),
    }
  }

  pub fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the root provides the server feed before mounting screens")
  }
}

/// Keeps the [`ServerFeed`] current. Renders nothing.
pub struct ServerFeedLoader;

impl Component for ServerFeedLoader {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let feed = ServerFeed::of(ctx);
    let list = ctx.stream(
      feed.servers.clone(),
      |servers, emitter: StreamEmitter<ServerList, ()>| async move {
        let mut changes = servers.subscribe();
        loop {
          changes.borrow_and_update();
          let read = {
            let servers = servers.clone();
            background::run(move || servers.entries()).await
          };
          let list = match read {
            Ok(Ok(entries)) => ServerList::Ready(entries.into()),
            Ok(Err(error)) => {
              tracing::error!(%error, "cannot read the shared servers");
              ServerList::Failed(Message::new("servers.loadFailed"))
            }
            Err(message) => ServerList::Failed(message),
          };
          if !emitter.emit(list) || changes.changed().await.is_err() {
            return;
          }
        }
      },
    );
    if let Some(list) = list.state().get().data
      && feed.list.get_untracked() != list
    {
      feed.list.set(list);
    }
    Element::new()
  }
}
