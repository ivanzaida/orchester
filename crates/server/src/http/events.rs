use std::{
  convert::Infallible,
  pin::Pin,
  task::{Context, Poll},
  time::Duration,
};

use axum::{
  Extension,
  body::{Body, Bytes},
  extract::State,
  http::{HeaderMap, HeaderValue},
  response::{IntoResponse, Response},
};
use hyper::body::{Body as HttpBody, Frame};
use orchester_api::ErrorCode;
use orchester_core::id::ProjectId;
use tokio::{
  sync::{mpsc, watch},
  task::JoinHandle,
  time::Interval,
};

use super::extract::{Auth, Id};
use crate::{
  Server,
  db::{self, Authenticated},
  error::ServerError,
  sharing::{access, events, members},
  transport::ConnectionStop,
};

struct EventBody {
  receiver: mpsc::Receiver<Bytes>,
  worker: JoinHandle<()>,
  _permit: crate::sharing::limits::StreamPermit,
}
impl HttpBody for EventBody {
  type Data = Bytes;
  type Error = Infallible;
  fn poll_frame(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
    self
      .get_mut()
      .receiver
      .poll_recv(cx)
      .map(|value| value.map(|bytes| Ok(Frame::data(bytes))))
  }
}
impl Drop for EventBody {
  fn drop(&mut self) {
    self.worker.abort();
  }
}

pub async fn subscribe(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(project): Id<ProjectId>,
  Extension(stop): Extension<ConnectionStop>,
  headers: HeaderMap,
) -> Result<Response, ServerError> {
  let mut values = headers.get_all("Last-Event-ID").iter();
  let after = values
    .next()
    .map(|value| {
      value
        .to_str()
        .map_err(|_| ErrorCode::BadRequest)?
        .parse::<u64>()
        .map_err(|_| ErrorCode::BadRequest)
    })
    .transpose()?
    .unwrap_or(0);
  if values.next().is_some() || after > i64::MAX as u64 {
    return Err(ErrorCode::BadRequest.into());
  }
  let changes = server.inner.db.subscribe();
  let initial = auth.clone();
  let generation = server
    .snapshot(move |db, now| {
      db::checked(db, &initial, now, false, false)?;
      if access::role(db, project, initial.user.id)?.is_none() {
        return Err(ErrorCode::Forbidden.into());
      }
      members::generation(db, project, initial.user.id)?.ok_or_else(|| ErrorCode::Forbidden.into())
    })
    .await?;
  let permit = server.inner.sharing_limits.stream(auth.session, auth.user.id)?;
  let (sender, receiver) = mpsc::channel(1);
  let worker = tokio::spawn(async move {
    let stream = EventStream {
      server,
      auth,
      project,
      generation,
      changes,
      stop: stop.0,
      sender,
      keepalive: tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_secs(20),
        Duration::from_secs(20),
      ),
    };
    if let Err(error) = stream.run(after).await {
      error.log();
    }
  });
  let mut response = Body::new(EventBody {
    receiver,
    worker,
    _permit: permit,
  })
  .into_response();
  response
    .headers_mut()
    .insert("Content-Type", HeaderValue::from_static("text/event-stream"));
  response
    .headers_mut()
    .insert("X-Accel-Buffering", HeaderValue::from_static("no"));
  Ok(response)
}

struct EventStream {
  server: Server,
  auth: Authenticated,
  project: ProjectId,
  generation: String,
  changes: watch::Receiver<u64>,
  stop: watch::Receiver<bool>,
  sender: mpsc::Sender<Bytes>,
  keepalive: Interval,
}

enum Delivery {
  Sent,
  Revoked,
  Closed,
}

impl EventStream {
  async fn valid(&self) -> Result<bool, ServerError> {
    let auth = self.auth.clone();
    let project = self.project;
    let generation = self.generation.clone();
    let (current, now) = self
      .server
      .snapshot(move |db, now| stream_auth(db, &auth, project, &generation, now).map(|current| (current, now)))
      .await?;
    let Some(current) = current else {
      return Ok(false);
    };
    if now - current.last_used_at <= 60_000 {
      return Ok(true);
    }
    let generation = self.generation.clone();
    self
      .server
      .write(move |db, now| {
        let Some(current) = stream_auth(db, &current, project, &generation, now)? else {
          return Ok(false);
        };
        db::touch_session(db, &current, now)?;
        Ok(true)
      })
      .await
  }

  async fn send(&mut self, data: Bytes) -> Result<Delivery, ServerError> {
    loop {
      if *self.stop.borrow() {
        return Ok(Delivery::Closed);
      }
      if !self.valid().await? {
        return Ok(Delivery::Revoked);
      }
      tokio::select! {
        permit=self.sender.reserve() => match permit {Ok(permit)=>{permit.send(data);return Ok(Delivery::Sent);},Err(_)=>return Ok(Delivery::Closed)},
        changed=self.changes.changed() => {if changed.is_err() {return Ok(Delivery::Closed);}},
        changed=self.stop.changed() => {if changed.is_err() || *self.stop.borrow() {return Ok(Delivery::Closed);}},
        _tick=self.keepalive.tick() => {},
      }
    }
  }

  async fn revoked(&mut self) {
    tokio::select! {
      _result=self.sender.send(Bytes::from_static(b"event: revoked\ndata: {}\n\n")) => {
        // A disconnected recipient needs no terminal event.
      },
      _changed=self.stop.changed() => {},
    }
  }

  async fn deliver(&mut self, data: Bytes) -> Result<bool, ServerError> {
    match self.send(data).await? {
      Delivery::Sent => Ok(true),
      Delivery::Closed => Ok(false),
      Delivery::Revoked => {
        self.revoked().await;
        Ok(false)
      }
    }
  }

  async fn run(mut self, mut after: u64) -> Result<(), ServerError> {
    // Flushing an initial comment also sends the headers for a project without events.
    if !self.deliver(Bytes::from_static(b": connected\n\n")).await? {
      return Ok(());
    }
    loop {
      self.changes.borrow_and_update();
      if !self.valid().await? {
        self.revoked().await;
        return Ok(());
      }
      let project = self.project;
      let replay = self
        .server
        .snapshot(move |db, now| events::replay(db, project, after, now))
        .await?;
      if replay.reset {
        self.deliver(Bytes::from_static(b"event: reset\ndata: {}\n\n")).await?;
        return Ok(());
      }
      if let Some(last) = replay.events.last() {
        let next = last.id;
        let data: String = replay
          .events
          .into_iter()
          .map(|event| {
            format!(
              "id: {}\nevent: {}\ndata: {}\n\n",
              event.id,
              event.kind.as_str(),
              event.payload
            )
          })
          .collect();
        if !self.deliver(Bytes::from(data)).await? {
          return Ok(());
        }
        after = next;
        continue;
      }
      tokio::select! {
        changed=self.changes.changed() => {if changed.is_err() {return Ok(());}},
        changed=self.stop.changed() => {if changed.is_err() || *self.stop.borrow() {return Ok(());}},
        _closed=self.sender.closed() => return Ok(()),
        _tick=self.keepalive.tick() => {if !self.deliver(Bytes::from_static(b": keep-alive\n\n")).await? {return Ok(());}},
      }
    }
  }
}

fn stream_auth(
  db: &rusqlite::Connection,
  auth: &Authenticated,
  project: ProjectId,
  generation: &str,
  now: i64,
) -> Result<Option<Authenticated>, ServerError> {
  let current = match db::checked(db, auth, now, false, false) {
    Ok(current) => current,
    Err(ServerError::Api(ErrorCode::Unauthorized | ErrorCode::PasswordChangeRequired)) => return Ok(None),
    Err(error) => return Err(error),
  };
  if members::generation(db, project, current.user.id)?.as_deref() != Some(generation) {
    return Ok(None);
  }
  Ok(Some(current))
}
