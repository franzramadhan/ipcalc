# ipcalc

IPv4 and IPv6 subnet calculator that explains every answer and maps IPv6 back
to what you already know from IPv4. Live at
<https://ipcalc.franzramadhan.dev>.

- Paste an address with an optional prefix (`192.168.1.0/24`,
  `2001:db8::1/64`, `fe80::1%eth0`). Every result row has a short "why".
- **Explain my IP** explains the address you connect from (Cloudflare's
  `CF-Connecting-IP`). It is shown back to you only, never stored or logged.
- **Copy prompt** gives you a prompt for your own AI agent, built from the
  verified values, so the agent explains instead of recomputing.

It runs as a WebAssembly component (`wasi:http/incoming-handler`) in a
Kubernetes pod with the runwasi wasmtime shim: no filesystem, no outbound
network, about 6 MiB of memory.

## API

| Path | Returns |
|------|---------|
| `GET /api/v1/calc?q=<addr[/prefix]>` | JSON: `family`, `canonical`, `kind`, `facts[]` (`label`, `value`, `why`), `prompt` |
| `GET /api/v1/me` | The same for your own IP (only behind Cloudflare) |
| `GET /healthz` | `ok` |

```bash
curl -s 'https://ipcalc.franzramadhan.dev/api/v1/calc?q=2001:db8::1/64' | jq '.facts[] | {label, value}'
```

## Build and run

Needs rustup (the toolchain is pinned in `rust-toolchain.toml`) and
[wasm-tools](https://github.com/bytecodealliance/wasm-tools).

```bash
cargo test                      # calculator and routing, native
./build.sh                      # -> ipcalc.wasm
wasmtime serve -Scli ipcalc.wasm   # http://localhost:8080
```

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

## Image

`ghcr.io/franzramadhan/ipcalc:<version>`: `FROM scratch` with `ipcalc.wasm`
as the entrypoint. The platform is `linux/amd64`, not `wasip1/wasm`: kubelet
pulls images for the node's platform, so a wasm-platform image would never be
unpacked. Run it with a RuntimeClass whose handler is the wasmtime shim.

Releases: push a `v*` tag matching `Cargo.toml`. The release workflow pushes
the image and attaches `ipcalc.wasm` and `SHA256SUMS` to the GitHub release.

## License

MIT
