use std::{collections::HashMap, sync::Arc};

use orchester_core::id::{SessionId, UserId};
use parking_lot::Mutex;

use crate::error::ServerError;

pub const HEARTBEAT_TASK_LIMIT: usize = 256;
const HEARTBEAT_WINDOW_MS: i64 = 60_000;
const HEARTBEATS_PER_WINDOW: usize = 12;
const SESSION_STREAMS: usize = 8;
const USER_STREAMS: usize = 16;

struct Heartbeats {
  until: i64,
  count: usize,
}

#[derive(Default)]
struct Counters {
  heartbeats: HashMap<SessionId, Heartbeats>,
  sessions: HashMap<SessionId, usize>,
  users: HashMap<UserId, usize>,
}

#[derive(Default)]
pub(crate) struct SharingLimits(Arc<Mutex<Counters>>);

pub(crate) struct StreamPermit {
  counters: Arc<Mutex<Counters>>,
  session: SessionId,
  user: UserId,
}

impl SharingLimits {
  pub fn heartbeat(&self, session: SessionId, now: i64) -> Result<(), ServerError> {
    let mut counters = self.0.lock();
    counters.heartbeats.retain(|_, value| value.until > now);
    let value = counters.heartbeats.entry(session).or_insert(Heartbeats {
      until: now.saturating_add(HEARTBEAT_WINDOW_MS),
      count: 0,
    });
    if value.count >= HEARTBEATS_PER_WINDOW {
      return Err(ServerError::RateLimited(((value.until - now + 999) / 1000) as u64));
    }
    value.count += 1;
    Ok(())
  }

  pub fn stream(&self, session: SessionId, user: UserId) -> Result<StreamPermit, ServerError> {
    let mut counters = self.0.lock();
    if counters.sessions.get(&session).copied().unwrap_or(0) >= SESSION_STREAMS
      || counters.users.get(&user).copied().unwrap_or(0) >= USER_STREAMS
    {
      return Err(ServerError::RateLimited(1));
    }
    *counters.sessions.entry(session).or_default() += 1;
    *counters.users.entry(user).or_default() += 1;
    Ok(StreamPermit {
      counters: self.0.clone(),
      session,
      user,
    })
  }
}

impl Drop for StreamPermit {
  fn drop(&mut self) {
    let mut counters = self.counters.lock();
    let sessions = counters
      .sessions
      .get_mut(&self.session)
      .expect("live stream reserves its session");
    *sessions -= 1;
    if *sessions == 0 {
      counters.sessions.remove(&self.session);
    }
    let users = counters
      .users
      .get_mut(&self.user)
      .expect("live stream reserves its user");
    *users -= 1;
    if *users == 0 {
      counters.users.remove(&self.user);
    }
  }
}
