pub mod calc;

/// A response, independent of the HTTP library so routing is testable
/// natively.
pub struct Reply {
    pub status: u16,
    pub content_type: &'static str,
    pub cache: &'static str,
    pub body: String,
}

const PAGE: &str = include_str!("../web/index.html");
const CSS: &str = include_str!("../web/app.css");
const JS: &str = include_str!("../web/app.js");

/// `client_ip` is Cloudflare's `CF-Connecting-IP` header. It is only echoed
/// back to that same visitor, never logged or stored.
pub fn route(method: &str, path: &str, query: Option<&str>, client_ip: Option<&str>) -> Reply {
    if method != "GET" && method != "HEAD" {
        return json(405, "no-store", serde_json::json!({"error": "Only GET is supported."}));
    }
    match path {
        "/" => text(200, "text/html; charset=utf-8", "public, max-age=300", PAGE),
        "/app.css" => text(200, "text/css; charset=utf-8", "public, max-age=300", CSS),
        "/app.js" => text(200, "text/javascript; charset=utf-8", "public, max-age=300", JS),
        "/healthz" => text(200, "text/plain; charset=utf-8", "no-store", "ok\n"),
        "/api/v1/calc" => {
            let q = query.and_then(|q| param(q, "q")).unwrap_or_default();
            let split = query.and_then(|q| param(q, "split"));
            answer(calc::calc(&q, split.as_deref()), "public, max-age=86400")
        }
        "/api/v1/me" => match client_ip {
            Some(ip) => answer(calc::calc(ip, None), "private, no-store"),
            None => json(404, "no-store", serde_json::json!({"error": "Your IP is only known through the public site."})),
        },
        _ => json(404, "public, max-age=300", serde_json::json!({"error": "Not found."})),
    }
}

fn answer(r: Result<serde_json::Value, String>, cache: &'static str) -> Reply {
    match r {
        Ok(v) => json(200, cache, v),
        Err(e) => json(400, "public, max-age=86400", serde_json::json!({"error": e})),
    }
}

fn json(status: u16, cache: &'static str, v: serde_json::Value) -> Reply {
    Reply { status, content_type: "application/json", cache, body: v.to_string() }
}

fn text(status: u16, content_type: &'static str, cache: &'static str, body: &str) -> Reply {
    Reply { status, content_type, cache, body: body.to_string() }
}

/// First value of `key` in a query string, percent-decoded (`+` = space).
fn param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        (k == key).then(|| decode(v))
    })
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() => {
                let hex = |c: u8| (c as char).to_digit(16).unwrap() as u8;
                out.push(hex(b[i + 1]) << 4 | hex(b[i + 2]));
                i += 2;
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes() {
        assert_eq!(route("GET", "/", None, None).status, 200);
        assert_eq!(route("GET", "/healthz", None, None).body, "ok\n");
        assert_eq!(route("POST", "/", None, None).status, 405);
        assert_eq!(route("GET", "/nope", None, None).status, 404);
        let r = route("GET", "/api/v1/calc", Some("q=2001%3Adb8%3A%3A1%2F64"), None);
        assert_eq!(r.status, 200, "{}", r.body);
        assert!(r.body.contains("2001:db8::/64"));
        let r = route("GET", "/api/v1/calc", Some("q=10.0.0.0%2F22&split=24"), None);
        assert!(r.body.contains("10.0.3.0/24"), "{}", r.body);
        assert_eq!(route("GET", "/api/v1/calc", Some("q=10.0.0.0%2F22&split=21"), None).status, 400);
        assert_eq!(route("GET", "/api/v1/calc", Some("q=nope"), None).status, 400);
        assert_eq!(route("GET", "/api/v1/calc", None, None).status, 400);
        assert_eq!(route("GET", "/api/v1/me", None, None).status, 404);
        let r = route("GET", "/api/v1/me", None, Some("2001:db8::1"));
        assert_eq!(r.status, 200);
        assert_eq!(r.cache, "private, no-store");
    }

    #[test]
    fn decoding() {
        assert_eq!(decode("10.0.0.0%2F8"), "10.0.0.0/8");
        assert_eq!(decode("a+b"), "a b");
        assert_eq!(decode("%zz%"), "%zz%");
        assert_eq!(decode("%4"), "%4");
        assert_eq!(param("x=1&q=fe80%3A%3A1", "q").as_deref(), Some("fe80::1"));
        assert_eq!(param("x=1", "q"), None);
    }
}
