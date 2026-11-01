//! *Join a team* and the steps after it (`docs/design/startup.md` decisions 4–6; `Startup · redesign` `qzzYw`):
//! `/join-team` opens on the way the window is signed in for, the Orchester Cloud account's teams and invites
//! (`drCQd`), and `/join-team?way=code` on the two ways side by side, a code on the right (`B914L`). Once a team is
//! joined, `?way=projects&server=<id>` is its shared projects (`v5qbWM`, `NaEPI`) and
//! `?way=folder&server=<id>&project=<slug>` links one of them to a folder (`go2aM`, `sN8Pa`, `tDFeg`).

use orchester_core::id::ServerId;

use super::{
  JoinTarget, RouteError,
  query::{Params, with_query},
};

/// The step's segment.
pub(super) const SEGMENT: &str = "join-team";
const WAY: &str = "way";
const CODE: &str = "code";
const PROJECTS: &str = "projects";
const FOLDER: &str = "folder";
const SERVER: &str = "server";

/// Where the start page's way into a team is.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum JoinWay {
  /// The Orchester Cloud account's teams and invites, or its sign-in while signed out.
  #[default]
  Cloud,
  /// A team server's address and a code.
  Code,
  /// The shared projects of a team this computer joined.
  Projects(ServerId),
  /// A folder to link to one of a team's shared projects.
  Folder(JoinTarget),
}

impl JoinWay {
  pub(super) fn path(&self) -> String {
    let path = format!("/{SEGMENT}");
    match self {
      Self::Cloud => path,
      Self::Code => with_query(path, &[(WAY, Some(CODE))]),
      Self::Projects(server) => with_query(path, &[(WAY, Some(PROJECTS)), (SERVER, Some(&server.to_string()))]),
      Self::Folder(target) => {
        let [server, project] = target.params();
        with_query(
          path,
          &[
            (WAY, Some(FOLDER)),
            (server.0, Some(&server.1)),
            (project.0, Some(&project.1)),
          ],
        )
      }
    }
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    let Some(way) = params.take(WAY)? else {
      return Ok(Self::Cloud);
    };
    let missing = |param: &str| RouteError::InvalidParam {
      param: param.to_owned(),
      value: String::new(),
    };
    match way.as_str() {
      CODE => Ok(Self::Code),
      PROJECTS => params
        .take_parsed(SERVER)?
        .map(Self::Projects)
        .ok_or_else(|| missing(SERVER)),
      FOLDER => JoinTarget::take(params)?
        .map(Self::Folder)
        .ok_or_else(|| missing(SERVER)),
      _ => Err(RouteError::InvalidParam {
        param: WAY.to_owned(),
        value: way,
      }),
    }
  }
}
