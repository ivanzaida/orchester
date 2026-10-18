//! A server project to join, as the join review (`AiCwO`) and the invite landing (`xeFRB`) name it: the team server
//! by its id in the list and the project by its slug there (`?server=…&project=kontur`).

use orchester_core::id::ServerId;

use super::{RouteError, query::Params};

const SERVER: &str = "server";
const PROJECT: &str = "project";

/// The project at `slug` on `server`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct JoinTarget {
  pub server: ServerId,
  slug: String,
}

impl JoinTarget {
  /// The project at `slug` on `server`; `None` for an empty slug.
  pub fn new(server: ServerId, slug: &str) -> Option<Self> {
    (!slug.is_empty()).then(|| Self {
      server,
      slug: slug.to_owned(),
    })
  }

  pub fn slug(&self) -> &str {
    &self.slug
  }

  /// The query's parameters that name it.
  pub(super) fn params(&self) -> [(&'static str, String); 2] {
    [(SERVER, self.server.to_string()), (PROJECT, self.slug.clone())]
  }

  /// The project the query names; `None` when it names neither, an error when it names one only or a bad server.
  pub(super) fn take(params: &mut Params) -> Result<Option<Self>, RouteError> {
    let (server, slug) = (params.take(SERVER)?, params.take(PROJECT)?);
    let (server, slug) = match (server, slug) {
      (None, None) => return Ok(None),
      (Some(server), Some(slug)) => (server, slug),
      (None, Some(_)) => return Err(missing(SERVER)),
      (Some(_), None) => return Err(missing(PROJECT)),
    };
    let server = server.parse().map_err(|_| RouteError::InvalidParam {
      param: SERVER.to_owned(),
      value: server,
    })?;
    Self::new(server, &slug).map(Some).ok_or_else(|| missing(PROJECT))
  }
}

fn missing(param: &str) -> RouteError {
  RouteError::InvalidParam {
    param: param.to_owned(),
    value: String::new(),
  }
}
