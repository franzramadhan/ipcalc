# CLAUDE.md

ipcalc: an IPv4/IPv6 subnet calculator that explains every answer, served
as one WebAssembly component (`wasi:http/incoming-handler`). Live at
<https://ipcalc.franzramadhan.dev>. `AGENTS.md` is a symlink to this file:
edit `CLAUDE.md` only.

## Layout

| Path | Contents |
|---|---|
| `src/calc.rs` | parsing, IPv4/IPv6 facts, split, the AI prompt (`render_prompt`) |
| `src/tools.rs` | containment check and route summarization |
| `src/lib.rs` | `route()`: HTTP routing as plain functions, testable natively; embeds `web/` with `include_str!` |
| `src/main.rs` | wasm32-only glue (`wstd::http_server`) and response headers (CSP, nosniff, referrer) |
| `web/` | `index.html`, `app.css`, `app.js`: no framework, no build step |
| `build.sh` | wasm32-wasip1 build + wasmtime v36 proxy adapter, refuses WASI newer than 0.2.6 |

## Rules

- **WASI 0.2.6 ceiling.** The runwasi shim (wasmtime 36) serves WASI 0.2.6.
  Do not move to `wasm32-wasip2`, bump `wasip2` past `=1.0.1`, or add a crate
  that pulls newer WASI imports until the shim moves (README, "WASI version").
  `build.sh` fails when an import is newer.
- **No outbound I/O, no state.** The component reads nothing, stores nothing,
  logs nothing and makes no network calls. `/api/v1/me` echoes
  `CF-Connecting-IP` to the same visitor only. Keep it that way.
- **Explain, do not just compute.** Every fact has a `why`. New facts and
  tools add a `why` and questions for the AI prompt.
- **Both families.** A feature works for IPv4 and IPv6, with tests for both,
  including edges (`/0`, `/31`, `/32`, `/127`, `/128`, `::/0`, mixed lists).
- **API is v1.** Add fields freely; renaming or removing a field or route
  needs `/api/v2`. Keep the README API table and the page's API tab
  (`web/index.html`, `#api`) in step with `src/lib.rs`.
- **Web.** `textContent` only, never `innerHTML` (CSP is `default-src 'self'`,
  no inline scripts or styles). Assets are linked as `/app.js?v=VERSION`;
  `route()` fills in the crate version, so every release busts the caches
  (Cloudflare stretches asset max-age to a day). Keep that placeholder.
- **Keep it small.** Stdlib and `serde_json` first; a new dependency needs a
  reason, and must build for wasm32-wasip1.
- **Public repo.** No lab identifiers: LAN IPs, tailnet name, tunnel or
  account ids, tokens, email addresses.

## Writing

No em dashes or en dashes anywhere (docs, comments, UI text, commit
messages). Use a comma, colon, parentheses or a new sentence. Commits are
GPG-signed.

## Checks before a commit

```bash
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
./build.sh                         # needs wasm-tools
wasmtime serve -Scli ipcalc.wasm   # http://localhost:8080, click through the tabs
```

The code is not rustfmt-formatted (wide lines on purpose); do not reformat
whole files.

## CI

- `.github/workflows/ci.yml` (pull requests): tests, clippy, `./build.sh`.
- `.github/workflows/coverage.yml` (pushes to `main` touching Rust or `web/`):
  `cargo llvm-cov` (without `src/main.rs`, wasm-only), then writes the
  test and coverage badge JSON to the orphan branch `badges`. Never merge
  `badges`.
- `.github/workflows/release.yml` (`v*` tags): checks the tag against
  `Cargo.toml`, builds, pushes `ghcr.io/franzramadhan/ipcalc:<version>`
  (`linux/amd64` scratch image, provenance and SBOM), and creates the GitHub
  release with `ipcalc.wasm` and `SHA256SUMS`. Manual run on a branch:
  `gh workflow run release.yml --ref <branch>` pushes `dev-<short sha>` only.
- Actions are pinned by full commit SHA with the version as a comment;
  `actions/checkout` with `persist-credentials: false`; `permissions:
  contents: read` at the top, more per job only where needed.

## Release

1. Bump `version` in `Cargo.toml` (`v0.MINOR.PATCH`; patch for fixes and UI
   polish, minor for a new tool or API field). Run the checks above.
2. Merge to `main` once CI is green, without waiting for a review:
   `gh stack merge <pr> --yes --squash` for a stacked PR; a single PR is
   not a stack, so `gh pr merge <pr> --squash --delete-branch --admin`.
   Pull `main` and check `Cargo.toml` has the new version before tagging,
   then tag and push: `git tag -s v<version> -m v<version>`,
   `git push origin v<version>`.
3. Watch the release run; take the index digest:
   `docker buildx imagetools inspect ghcr.io/franzramadhan/ipcalc:<version> | grep Digest`.
4. Deploy: the homelab infra repo pins `ipcalc:<version>@sha256:<digest>` in
   `kubernetes/apps/ipcalc/deployment.yaml` (and the version in its app table);
   PR, merge, Flux applies. Check the live page loads `app.js?v=<version>`.
