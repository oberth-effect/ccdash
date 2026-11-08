# Development

1. [The daemons](#1-the-daemons)
2. [The panel](#2-the-panel)
3. [Checks](#3-checks)
4. [The demo](#4-the-demo)

## 1. The daemons

```sh
direnv allow                                     # or: nix develop
cargo run -- hub --config hub.toml               # http.listen, collect.tokens, store.path
cargo run -- collector --config collector.toml   # hub.url = "ws://127.0.0.1:7837/collect"
```

- `CCDASH_RUNTIME_DIR` (sockets) and `XDG_STATE_HOME` (outbox, sync index)
  give a second collector, or the hooks of a test session, a place next to
  the real one.
- `CCDASH_LOG=info,ccdash=debug` for more log.
- A dev hub with a dummy bootstrap token (`devtoken-not-secret`) on
  `127.0.0.1` is the usual end-to-end setup; feed it hook payloads with
  `printf '{…}' | CCDASH_RUNTIME_DIR=… ccdash emit`.

## 2. The panel

`web/` is SvelteKit with Svelte 5, Tailwind, shadcn-svelte and LayerChart;
Node and npm are in the devshell.

```sh
cd web && npm ci
npm run dev                   # Vite on :5173; /api and /act go to CCDASH_HUB (127.0.0.1:7837)
npm run check && npm run lint
npm run build                 # → web/build, what a `cargo run` hub serves
```

- The hub serves `http.web_dir` (the Nix build bakes in the `ccdash-web`
  package; a `cargo run` hub falls back to `web/build`; `CCDASH_WEB_DIR`
  overrides both), with `index.html` for every route.
- The API's TypeScript types (`web/src/lib/api/types.ts`) are generated
  from the Rust models: `cargo test api` fails while the file is stale,
  `CCDASH_UPDATE_TYPES=1 cargo test api` rewrites it.
- After a change to `web/package-lock.json`, put the hash that
  `prefetch-npm-deps web/package-lock.json` prints into
  `flake-parts/pkgs/ccdash-web.nix`.
- The service worker only registers in the built app, so push and the
  offline shell are checked against `web/build` served by a hub.

## 3. Checks

```sh
cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check
cd web && npm run check && npm run lint
nix flake check     # also runs the pre-commit hooks (editorconfig, markdownlint, typos, …)
                    # and the VM tests
nix build .#ccdash .#ccdash-web .#ccdash-aarch64-linux
nix build .#checks.x86_64-linux.vm-single-node -L    # one VM test, with its log
```

The VM tests (`flake-parts/checks/`) boot NixOS with the modules as a user
would set them up, a stand-in `claude` answering the collector:
`vm-single-node` is one machine as hub and workstation (`localCollector`,
`hub.local`), covering the generated tokens, the link and its feature
report, the panel, the plugin, the knowledge tools in and outside a
workspace, and a hub restart. They need KVM; home-manager is a flake input
for them only.

`nix flake check` sees only files git knows; check a fresh clone or a copy
before pushing new files.

## 4. The demo

```sh
nix run .#demo                               # or: cargo run -- demo
nix run .#demo -- --listen 127.0.0.1:7841 --dir /tmp/ccdash-demo-2
```

`ccdash demo` writes a fresh database (`--dir`, by default a `ccdash-demo`
temporary directory, emptied only when an earlier demo made it) and serves
it at `http://127.0.0.1:7840/#token=demo`:

- a week of sessions on two made-up machines, `desk` and `laptop`, in two
  workspaces, `acme` and `personal`, written through the hub's own reducer,
  so the chats, History and Analytics read as they would for real ones;
- memories, issues, progress, documents and a report;
- while it runs, two stand-in collectors keep four sessions open (working,
  waiting on a permission prompt, the owner's turn) and report the machines'
  numbers, the hub's own included, all made up.

The content is `src/demo/script.rs`. Screenshots come from the demo, never
from a real hub.

### Screenshots

`docs/images/`, taken through a headless browser (Playwright) on the demo
with the light colour scheme unless named `-dark`, at CSS pixel scale, then
`oxipng -o 4 --strip safe`; each under 150 kB. Reload after switching the
colour scheme, and wait for a page's data (Projects and a report's frame
take a few seconds) before shooting.

| File            | Page                                                              | Viewport  |
| --------------- | ----------------------------------------------------------------- | --------- |
| `now.png`       | `/` (after "Recent progress" shows)                               | 1280×800  |
| `now-dark.png`  | `/`, dark                                                         | 1280×800  |
| `session.png`   | `/sessions/5f1c2a90-…` (the checkout session), its 2nd group open | 1280×1000 |
| `projects.png`  | `/projects`                                                       | 1280×800  |
| `history.png`   | `/history?range=1h`                                               | 1280×800  |
| `analytics.png` | `/analytics/overview?range=7d`                                    | 1280×800  |
| `machines.png`  | `/machines`                                                       | 1280×500  |
| `report.png`    | `/reports/acme/dependency-audit--2026-11`                         | 1280×800  |
| `phone.png`     | `/`                                                               | 390×844   |
