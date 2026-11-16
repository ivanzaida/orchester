//! A shared project has its server project's key (`sharing.project_key`), under which the server numbers its tasks.
//! A project shared into a project made on the server beforehand under another key kept its own until now (a join
//! always took the server's), so one task read `TQ-1` where this computer named it and `TK-1` in the team's lists.
//! Such a project takes the server project's key. A local project, and a shared one whose keys agree, are not
//! touched. Running it again changes nothing.

pub(super) const MIGRATION: &str = r"
UPDATE project SET key = (SELECT project_key FROM sharing)
WHERE EXISTS (SELECT 1 FROM sharing WHERE project_key IS NOT project.key);
";
