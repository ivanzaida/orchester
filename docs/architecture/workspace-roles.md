# Additive workspace role contracts (S1)

This is the public core/API prerequisite for platform task OP-144. Existing contracts remain intact; new workspace
authority contracts are available separately. Server routes, authorization and persistence live in
`ivanzaida/orchester-platform`. Publishing the types does not implement or enable those routes.

## Existing contracts remain unchanged

`orchester_core::sharing::MemberRole` and the root API role remain `Owner` / `Member`, serialized as `owner` /
`member`. Existing root DTOs, request/response fields, events, invites, signed cloud payloads, errors and the HTTP
client keep their current shapes. In particular, `ManagedProject.owners`, `SharingErrorDetails::Owners`,
`Remote.owners` and `ErrorCode::LastOwner` / `last_owner` are unchanged. Existing server-info versioning and
`/api/v1` behavior are unchanged. SQLite encodings and desktop presentation are unchanged too.

No existing endpoint may silently switch to the new role enum or emit the new contact/error shape. Existing
clients and servers must continue to work during platform adoption.

## New core policy

`orchester_core::sharing::WorkspaceRole` is `Admin` / `Member`, serialized as `admin` / `member`. It is distinct
from legacy `MemberRole` and rejects `owner`, `viewer` and `server_admin` wire values. Explicit `From` conversions
map legacy Owner to workspace Admin and Member to Member, in both directions, at compatibility boundaries.

Workspace admins inherit member access. Server administration is a separate authenticated account attribute;
server admins inherit workspace-admin access even without workspace membership. `may_read_workspace` and
`may_administer_workspace` take the authoritative role for the requested workspace plus that server-admin flag.
Membership in another workspace grants no access. These helpers do not replace identity, revocation, MFA, claims
or action-specific checks. The existing `may_change_configuration` signature and behavior remain unchanged.

`check_workspace_admin_change(current, next, admins)` rejects removing (`next = None`) or demoting the last
workspace admin with `MembershipChangeError::LastAdmin`. The server must read the workspace-admin count and apply
the write in the same transaction. Global server admins without membership do not replace the last workspace-admin
membership. New workspaces must initially have an admin. An existing server project retains its `ProjectId`.

## Separate API contracts

`orchester_api::v2` defines the role-bearing workspace contracts for a separately versioned server surface:

- `MemberRole` reexports core's `WorkspaceRole`; `Member`, `Members`, `MemberCreate`, `MemberChanges` and
  `MemberResponse` use it. `Members.admins` lists server-admin identities separately from workspace memberships.
- `SharedProject` and its response/list wrappers use the new role. `ManagedProject.admins` lists workspace admins.
- `InviteCreate`, `Invite`, `InviteCreated` and `Invites` use the new role; signed cloud-invite contracts remain
  unchanged and need explicit legacy-role adaptation at that boundary.
- `MembershipEvent` covers membership added, removed and role-changed deltas with existing attribution metadata.
  It does not replace the existing full event stream.
- `MembershipErrorCode::LastWorkspaceAdmin` serializes as `last_workspace_admin`.
  `MembershipErrorDetails::{Admins, Remote}` name workspace contacts with `admins`. Existing common errors,
  server-admin `last_admin` and v1 `last_owner` are unaffected.

These are shared contracts for OP-144, not a complete v2 client or server. Platform must keep the existing surface
working and explicitly add the new surface rather than change existing handlers' wire types. An API version bump
on the existing server-info response is not part of this prerequisite.

## Platform adoption

Pin public core and API to the final compatibility commit and update the lockfile; do not copy domain types or use
local path overrides. Resolve authoritative workspace roles for access checks. Legacy endpoints and persisted
roles can use the explicit role adapter; migration to a new internal encoding must preserve v1 boundary behavior.
Add tests proving old and new membership/invite/event shapes remain separate and that old clients still work,
alongside transactional last-admin protection and cross-workspace/server-admin authorization tests. Claim,
assignment, task/module mutation and execution policies remain their separate authority milestones.
