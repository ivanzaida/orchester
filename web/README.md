# Orchester web apps

An npm workspace (`docs/architecture/server.md`, "The web workspace"): one `package-lock.json` for

| Package | What |
| --- | --- |
| `ui/` (`@orchester/ui`) | The UI kit of the team server's web admin, also copied into Orchester Cloud's private repository (`ivanzaida/orchester-cloud`): components, design tokens, icons, fonts, i18n helpers and the checks' helpers. No words of its own: components take every string from the app |
| `server/` (`@orchester/server-admin`) | The team server's web admin (`server/README.md`), served behind the team server |

```sh
npm ci --ignore-scripts      # from web/
npm run check                # lint, type check and unit tests of every package (also: npm --prefix web run check)
npm run test:pages           # the admin's page tests (next build + Playwright in the installed Edge)
npm run tokens               # regenerate ui/theme/variables.css from ui/theme/pencil-variables.json
```

**Design tokens.** `ui/theme/pencil-variables.json` is a snapshot of `data/orchester2.pen`'s variables (read through the
Pencil MCP, accent graphite). `ui/scripts/tokens.ts` turns it into `ui/theme/variables.css`; a test fails when the
two differ. Values the design uses that are not variables are mapped by hand, with their Pencil node ids, in
`ui/theme/scale.css` (shared) and the app's own `theme/` (`server/theme/admin.css`). Those files are the only places
with colours, sizes or fonts; each package's `literals` test fails on one anywhere else, and ESLint refuses JSX text,
literal labels, inline styles and hex colours.

**Words.** The app has its own `locales/<locale>.json`; `@orchester/ui/i18n` gives the typed translator and the parity
check the app's tests run.

**Image.** `Dockerfile.server` builds the admin's standalone output from this folder. Orchester Cloud's web front and
the orchester.cloud website live in the private repository `ivanzaida/orchester-cloud`.
