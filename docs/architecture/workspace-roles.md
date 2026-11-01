# Workspace role contract (S1)

This is the public core/API prerequisite for platform task OP-144. Server authorization, database migrations and
HTTP enforcement are implemented in `ivanzaida/orchester-platform` after it pins this contract. An existing server
project is a workspace; its `ProjectId` and project routes retain their names.

## Roles and access

`orchester_core::sharing::MemberRole` has two workspace membership roles: `Admin` (`admin` on the wire) and
`Member` (`member`). The previous `Owner` variant and `owner` wire value are removed. `viewer` and `server_admin`
are not workspace membership roles.

Server administration remains a separate authenticated account attribute. Workspace admins inherit member access;
server admins inherit workspace-admin access, including when they have no membership in that workspace. Core
provides `may_read_workspace`, `may_administer_workspace` and `may_change_configuration`. Their role argument
must come from the requested workspace's authoritative membership, and their server-admin flag from authenticated
server identity. Membership in another workspace grants no access. These helpers do not replace identity,
revocation, MFA, claim or action-specific checks.

`check_workspace_admin_change(current, next, admins)` rejects removal (`next = None`) or demotion of the last
workspace admin with `MembershipChangeError::LastAdmin`. The caller must read the workspace-admin count and apply
the membership change in the same transaction. Server admins without workspace membership do not replace the
last workspace-admin membership. A workspace must initially be created with an admin.

## Breaking transport contract

`orchester_api::TEAM_API_VERSION` is **4**. This is a coordinated breaking upgrade; there are no legacy role or
field aliases. Existing `/api/v1` route paths are unchanged. The platform server must advertise this version and
apply its compatibility checks when adopting it; defining the constant alone does not enforce a handshake.

- Member, invite and event role fields use `admin` or `member` through the shared enum.
- `ManagedProject.admins` replaces `owners` and lists workspace-admin memberships.
- `SharingErrorDetails::Admins { admins }` replaces `Owners { owners }`.
- `SharingErrorDetails::Remote` also uses `admins` for the workspace-admin contact list.
- `ErrorCode::LastWorkspaceAdmin` serializes as `last_workspace_admin`. `LastAdmin` / `last_admin` remains the
  separate error for removing the last server admin.
- `Members.members` contains workspace memberships. `Members.admins` continues to contain server admins who
  inherit access; those entries are identities without a workspace role. These two meanings must not be mixed.

Workspace admins configure team pipelines, agent types and execution limits. A member's local credential/profile
selection is not elevated to workspace configuration. Claim, assignment, execution and delivery policies are
separate authority milestones; this contract does not implement them.

## Existing local storage

The local SQLite schema already stores `owner` in checked role columns. The store adapter maps that stored value
to/from `MemberRole::Admin`, preserving existing local databases without a schema migration. This storage encoding
is not a transport alias: an API request containing `owner` is rejected. Local projects and local edits continue
to use SQLite. The mechanical desktop enum updates retain existing presentation; a role-label redesign is not
part of this prerequisite.

## Platform adoption

After this change is pushed, OP-144 must pin both public core and API dependencies to the pushed commit and update
its lockfile. It must use these shared types rather than introducing replacement DTOs or local path overrides.
Its server migration converts persisted workspace `owner` roles to `admin`; authorization must resolve the
authenticated actor's role for the requested workspace and preserve the last-admin invariant transactionally.
Server-admin identity, membership/invite endpoints and events, cross-workspace isolation, web clients and
compatibility enforcement must be updated together in that repository.
