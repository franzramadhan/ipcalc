#[cfg(target_arch = "wasm32")]
use wstd::http::{Error, Method, Request, Response, body::Body};

const CSP: &str = "default-src 'self'; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'; form-action 'self'";

#[cfg(target_arch = "wasm32")]
#[wstd::http_server]
async fn main(request: Request<Body>) -> Result<Response<Body>, Error> {
    let ip = request.headers().get("cf-connecting-ip").and_then(|v| v.to_str().ok());
    let r = ipcalc::route(request.method().as_str(), request.uri().path(), request.uri().query(), ip);
    let body = if request.method() == Method::HEAD { String::new() } else { r.body };
    Ok(Response::builder()
        .status(r.status)
        .header("content-type", r.content_type)
        .header("cache-control", r.cache)
        .header("content-security-policy", CSP)
        .header("x-content-type-options", "nosniff")
        .header("referrer-policy", "no-referrer")
        .body(body.into())?)
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let _ = CSP;
    eprintln!("ipcalc is a wasi:http component: build it with ./build.sh and run it with `wasmtime serve -Scli ipcalc.wasm`");
    std::process::exit(1);
}
