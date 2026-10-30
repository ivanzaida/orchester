# ORC-450 · Network `Hosts(list)` per agent CLI (2026-10-30)

What each agent CLI the runner starts can enforce for a profile whose network access is a host list
(`NetworkAccess::Hosts`, `docs/architecture/server.md` row 75), and what Orchester does with it. Checked on Windows 11
with **Codex CLI 0.159.2** (`codex --version`, npm `C:/nvm4w/nodejs/codex.cmd`) and **Claude Code 2.1.301**
(`claude --version`). macOS was not probed: its rows follow the sandbox behaviour the capability table already
records for network off (`crates/core/src/permission/capability.rs`).

## Codex CLI 0.159.2

- **Commands.** Codex's sandbox has network on or off (`sandbox_workspace_write.network_access`), with no per-host
  allow-list Orchester can pass per launch. `codex app-server generate-json-schema --experimental` lists a domain map
  only under `NetworkRequirements` / `ApplicationNetworkRequirements` (`domains`, `allowedDomains`,
  `managedAllowedDomainsOnly`): the managed requirements of the experimental network proxy (`codex features list`:
  `network_proxy  experimental  false`), not a launch setting. Orchester does not use it.
- **Web search.** The same schema's config (`ConfigReadResponse` › `tools` › `web_search`, `WebSearchToolConfig`) has
  `allowed_domains: [string]`, so a launch can pass `-c tools.web_search.allowed_domains=[…]`.
- **What Orchester passes.** On macOS, where the sandbox keeps commands off the network, a host list runs as network
  off for commands (`sandbox_workspace_write.network_access=false` when writing; the read-only sandbox has no network)
  plus `-c tools.web_search.allowed_domains=[<hosts>]`, each pattern by its domain (`*.example.org` → `example.org`),
  sorted, without duplicates. Mechanism `CodexSandboxNetworkHosts`. The web search filter works by domain, so a listed
  domain covers its subdomains there; commands reach no host at all.
- **Windows.** The sandbox does not keep commands off the network (`CannotBlockNetwork` for network off), so it cannot
  keep them to a list either: `Unavailable::CannotLimitHosts`, and the step is refused.

## Claude Code 2.1.301

- Its commands keep network access: Orchester does not use Claude Code's own sandbox (not supported on Windows), the
  only place its settings take a domain list for commands. `WebFetch(domain:…)` permission rules limit only its fetch
  tool, not Bash, so they cannot enforce a list on their own.
- `Unavailable::CannotLimitHosts` on both platforms; the step is refused.

## Native agents

Orchester has no sandbox for a native agent's commands (`docs/architecture/native-agent.md` §5.4):
`Unavailable::CannotLimitHosts`; refused.

## Refusal and where it shows

- `AgentPermissions::enforced_on` returns `NotEnforceable { option: NetworkHosts, reason: CannotLimitHosts }`; a host
  list is never run with network on instead.
- Preflight turns that into `PreflightProblem::NetworkHostsNotEnforceable { step, cli }`, so the run does not start;
  the person reads *Step “…” limits network access to a host list, which Codex / Claude Code / Orchester’s native
  agent can’t enforce …* (`runs.preflight.networkHostsNotEnforceable.*`).
- The profile's Connection enforcement Network row reads *Not enforced* with why, naming the CLI
  (`agents.cannotLimitHosts.*`); a native connection's *Enforced by Orchester* group in Settings › Connections has an
  *Only listed hosts* row, *Not offered* with why.

Tests: `cargo test -p orchester-runner --features test-agent --test network_hosts network_hosts` (each CLI's
arguments for a list and the refusal) and `cargo test -p orchester-core --test network_hosts network_hosts`.
