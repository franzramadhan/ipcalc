# ipcalc

[![tests](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2Ffranzramadhan%2Fipcalc%2Fbadges%2Ftests.json)](https://github.com/franzramadhan/ipcalc/actions/workflows/coverage.yml)
[![coverage](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2Ffranzramadhan%2Fipcalc%2Fbadges%2Fcoverage.json&cacheSeconds=300)](https://github.com/franzramadhan/ipcalc/actions/workflows/coverage.yml)
[![release](https://img.shields.io/github/v/release/franzramadhan/ipcalc)](https://github.com/franzramadhan/ipcalc/releases/latest)
[![license](https://img.shields.io/github/license/franzramadhan/ipcalc)](LICENSE)

IPv4 and IPv6 subnet calculator that explains every answer and maps IPv6 back
to what you already know from IPv4. Live at
<https://ipcalc.franzramadhan.dev>.

It runs as a single WebAssembly component (`wasi:http/incoming-handler`,
about 350 KB) in a Kubernetes pod with the runwasi wasmtime shim: no
filesystem, no outbound network, about 6 MiB of memory.

## Features

| Tab | What it does |
|-----|--------------|
| **Explain** | Any address with an optional prefix (`192.168.1.0/24`, `2001:db8::1/64`, `fe80::1%eth0`): network, range, mask and wildcard, address type, IPv6 compression rules, EUI-64, multicast scope, embedded IPv4. Optionally splits the block into smaller subnets (`10.0.0.0/22` into /24s, a /56 into /64s). **Explain my IP** does the same for the address you connect from. |
| **Inside?** | Whether an address or subnet is inside a network, with the masking step a router does. Explains bigger prefixes and why IPv4 is never inside IPv6 (IPv4-mapped, NAT64). |
| **Summarize** | Up to 64 subnets, IPv4, IPv6 or both: the exact minimal set of prefixes, the single supernet, and how many extra addresses the supernet would also cover. |
| **IPv4 vs IPv6** | Cheat sheet: each IPv4 habit and its IPv6 version. |
| **API** | The JSON API behind the page (below). |

Every result row carries a short "why", the answer sits in a card on top,
and **Ask your own AI** builds a prompt from the verified values, so an agent
explains instead of recomputing (agents often get `::` and prefix
boundaries wrong). Each result also shows the API call that produced it.

Nothing you type is stored or logged. **Explain my IP** reads Cloudflare's
`CF-Connecting-IP` and shows it back to you only.

## API

`GET` only, JSON, no key. Base URL `https://ipcalc.franzramadhan.dev`.

| Request | Returns |
|---------|---------|
| `/api/v1/calc?q=<addr[/prefix]>[&split=<n>]` | Explain an address or prefix; `split` lists the first 256 subnets of size /n |
| `/api/v1/contains?net=<cidr>&ip=<addr\|cidr>` | `Result` fact `inside` or `outside` |
| `/api/v1/summarize?q=<list>` | `Exact summary`, `Single supernet`, `Extra addresses in the supernet` (per family when mixed); list separated by commas, spaces or new lines |
| `/api/v1/me` | `calc` for your own IP (only through Cloudflare, else 404) |
| `/healthz` | `ok` |

Every answer has `title`, `facts[]` (`label`, `value`, `why`) and `prompt`.
`calc` adds `family`, `canonical`, `kind` and `split` (`block`, `prefix`,
`total`, `subnets[]` or null). Bad input returns HTTP 400 with
`{"error": "..."}`.

```bash
curl -s 'https://ipcalc.franzramadhan.dev/api/v1/calc?q=2001:db8::1/64' | jq '.facts[] | {label, value}'
curl -s 'https://ipcalc.franzramadhan.dev/api/v1/contains?net=10.0.0.0/22&ip=10.0.1.77' | jq -r '.facts[] | select(.label == "Result").value'
curl -s 'https://ipcalc.franzramadhan.dev/api/v1/summarize?q=10.0.0.0/24,10.0.1.0/24' | jq '.facts[1].value'
```

The public instance allows 60 API requests per 10 seconds per IP and sleeps
from 23:30 to 06:00 WIB (UTC+7).

## Build and run

Needs rustup (the toolchain is pinned in `rust-toolchain.toml`),
[wasm-tools](https://github.com/bytecodealliance/wasm-tools) and, to run it
locally, [wasmtime](https://wasmtime.dev).

```bash
cargo test                         # calculator and routing, native
./build.sh                         # -> ipcalc.wasm
wasmtime serve -Scli ipcalc.wasm   # http://localhost:8080
```

Contributing: the rules, the checks and the release steps are in
[CLAUDE.md](CLAUDE.md) (also read by coding agents as `AGENTS.md`).

## WASI version

The runwasi `containerd-shim-wasmtime` v0.6.1 is built on wasmtime 36, which
serves WASI 0.2.6. A component that imports a newer WASI fails to start:

```
component imports instance `wasi:http/types@0.2.12`, but a matching implementation was not found in the linker
```

Rust's own `wasm32-wasip2` std already imports 0.2.12, so pinning crates is not
enough. `build.sh` therefore builds for `wasm32-wasip1` (with `wasip2` pinned
to WASI 0.2.4 for the HTTP bindings) and turns the module into a component
with the wasmtime v36.0.10 proxy adapter (pinned by sha256). It fails if any
import is newer than 0.2.6. Once the shim moves to a newer wasmtime, switch to
`wasm32-wasip2` and drop the adapter step.

## Image and releases

`ghcr.io/franzramadhan/ipcalc:<version>`: `FROM scratch` with `ipcalc.wasm`
as the entrypoint. The platform is `linux/amd64`, not `wasip1/wasm`: kubelet
pulls images for the node's platform, so a wasm-platform image would never be
unpacked. Run it with a RuntimeClass whose handler is the wasmtime shim:

```yaml
spec:
  runtimeClassName: wasmtime
  containers:
    - name: ipcalc
      image: ghcr.io/franzramadhan/ipcalc:<version>
      ports: [{name: http, containerPort: 8080}]
      readinessProbe: {httpGet: {path: /healthz, port: http}}
```

A `v*` tag matching `Cargo.toml` runs the release workflow: it pushes the
image (with provenance and SBOM) and attaches `ipcalc.wasm` and `SHA256SUMS`
to the GitHub release.

## License

MIT
