# PV2-21 · Tests that drove the old step editor of a local project run against the hub

- **Status:** done
- **Kind:** tests
- **Depends on:** PV2-07
- **Branch:** —
- **Tokens:** —

## Goal

Since PV2-07 a local project's step route opens in the Pipelines hub's panel, so a test that presses the old editor's
ids (`step-gate`, …) for a local project finds nothing. Found: `mcp_domain domain_pipeline_policy_wait…` fails at its
line 103 (*unknown or stale ref "no"*) on the module and not on `master`. Find every such test outside `--test
pipelines` (which PV2-18 already moved), and make it drive the hub's panel (its Save and *Allow* question) — or a team
project's old page where the old page is what it means to test.

## Done when

Each found test passes against what it names; the list is in the report; `tools/check.ps1` passes.
