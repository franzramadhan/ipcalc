# Platform linux/amd64, not wasip1/wasm: kubelet pulls for the host platform
# (RuntimeClassInImageCriAPI is alpha), so a wasm-platform image never
# unpacks. The shim runs the component regardless of the image platform.
FROM scratch
COPY ipcalc.wasm /ipcalc.wasm
ENTRYPOINT ["/ipcalc.wasm"]
