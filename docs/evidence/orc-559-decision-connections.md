# ORC-559 · Decision connection contract

Implementation: branch `orchester/orc-559-run-1`, based on
`1a9f9c8331235d130acad9af0f734e90143bfcd9`. Workspace only; no visual changes or design verification.

## Primary references read on 2026-11-01

- https://docs.typesafe.ai/api/list-models: GET /v1/models, Authorization: Bearer token.
  Successful body: `{"models":[{"name":"jev-latest","description":"...","release_date":"2026-09-15"}]}`.
- https://docs.typesafe.ai/api/system-one: model names from that catalog are the ids supplied to
  POST /v1/systemone. Check never calls that POST.
- https://laya.studio/docs/api and https://laya.studio/v1/openapi: GET /v1/models is public
  (`security: []` in the downloaded OpenAPI). The spec provides no model-list response schema.
- https://api.laya.studio/v1/models: a public, credential-free GET returned an object with
  `object: "list"`, `routing`, and `data` containing `id`, `repo`, `backbone`,
  `parameters`, `context_tokens`, `languages`, `default`, and an optional `note`.
  The ids were english, multilingual and typed-decisions; repos were convaiinnovations/laya
  with checkpoint subfolders.

The fixtures in `crates/workspace/tests/decision_connections.rs` are documentation-derived stand-ins,
not evidence of a live keyed connection or inference. No real credentials or paid inference were used.
The web reader could not download Laya's JSON URLs; Python's standard HTTPS client fetched the public spec
and catalog. Curl's Windows TLS backend failed with SEC_E_NO_CREDENTIALS; no TLS validation was disabled.

## Workspace behavior

`connections::decision` provides typed drafts, caller authority, save, delete and Check.
`ProjectConnections.decisions` includes decision servers alongside the project's other connection kinds.
Automation can save only localhost, 127.0.0.1 and [::1], using core's URL classification.
Hosted saves, including edits, are refused as person_only even when the global permissions switch is on.
A person can save a hosted server through the same workspace API. Tool arguments cannot claim person authority.

The MCP setup tools are `save_decision_connection` (optional connection id/name adds or edits),
`check_decision_connection`, and `remove_decision_connection`; `list_connections` includes
`decision_models`. Only secret references appear in input, read models and results.

Check performs one bodyless GET of base_url/models, with the configured secret's Bearer header.
It sends no project request content, follows no redirects, uses no proxy, and bounds time and response size.
The private transport boundary reads the credential transiently; raw HTTP/vault diagnostics and response
bodies never reach the read model, error or log. Parsed ids containing a credential echo are rejected,
including JSON-escaped echoes.

TypeSafe's required metadata shape and Laya's observed checkpoint metadata are recognized separately.
A generic OpenAI catalog, malformed or ambiguous payload, or unknown format fails closed as
not_system_one_api. An empty Laya/OpenAI-shaped list lacks checkpoint evidence and is also refused.
No model-list check proves that inference works; Laya's public catalog cannot validate its key.
Connected means that the recognized catalog answered. The check timestamp is the answer's receipt time.

401/403 record key_refused; transport failures, 429 and server errors record nothing_answers.
Missing/archived/unreadable configured keys fail closed without making an anonymous request.
A successful recognized catalog replaces the listed ids; when selected_model is no longer listed,
the check records model_gone with that id. Other unsuccessful checks retain the previous catalog as history,
with their unsuccessful check state. A changed URL/key clears models and check/test history.
A check whose URL/key changed in flight is refused as stale; renames are preserved.
Deletion delegates to the store's guard against deleting the project's chosen reviewer connection.

## Validation

Run the filtered integration binary:
`cargo test -p orchester-workspace --test decision_connections decision_connections`.

The machine's compiler-cache wrapper failed with Access is denied before project compilation.
`.tmp/decision-connections/build.py` runs builds in the foreground with RUSTC_WRAPPER and
RUSTC_WORKSPACE_WRAPPER removed from that subprocess environment only.

No macOS cross-check is possible on this Windows host (no Apple SDK/C cross-compiler).
No target_os-specific code was added. No screenshots, app launch, real-key checks or inference calls are needed
for this workspace-only task.

Final validation of the saved Rust sources: 9 filtered integration tests passed; nightly workspace-package
formatting passed; the full quality gate (workspace/all-targets/all-features clippy with warnings denied,
formatting, file sizes and UI literals) finished with `check: OK`. The first integration run exercised an earlier
parser build and found an ambiguous-catalog acceptance; the final run includes the rejection and passes.

## Review corrections

Check persistence now uses an atomic conditional UPDATE matching the loaded name and complete settings.
If another handle edits between the read and write, the check reloads and validates the URL/key again;
renames survive and changed credentials or URLs return CheckStale. The store regression explicitly
interleaves a second handle's rename, URL edit and key edit between that read and conditional write.

Decision-key references now participate in the authoritative secret replacement guard under its
existing write transaction. ProjectSecrets exposes decision_uses (connection id, name and secret id)
and includes them in replacement_needs_person. The regression starts from a stale unused snapshot,
saves a hosted decision connection as a person, and verifies UnusedOnly returns NeedsPerson without
vault or rotation-metadata mutation. Removing the connection makes the credential unused again.

These corrections change no visual layer and add no target_os-specific code.
