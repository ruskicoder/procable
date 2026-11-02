//! The one HTTP client factory of the engine, and procable's offline guard.
//!
//! procable runs fully offline once installed. Every `ureq` agent the engine
//! builds comes from [`agent_builder`], whose middleware refuses a request to
//! any host that is not loopback (`localhost`, `*.localhost`, `127.0.0.0/8`,
//! `::1`) before a socket is opened. Proxy environment variables are not
//! read, so a loopback request is never routed off the machine either.
//!
//! Upstream Impeccable used this module to build HTTPS agents for update
//! checks, telemetry, the direction roll, image generation and installs.
//! procable removed those callers; what remains is live mode talking to its
//! helper server on the same machine.

use std::net::IpAddr;

/// Status code of the synthetic response a refused request gets.
pub const OFFLINE_STATUS: u16 = 403;

/// Body of the synthetic response a refused request gets.
pub const OFFLINE_MESSAGE: &str = "procable runs offline: only loopback hosts are reachable";

/// `ureq::AgentBuilder::new()` with the offline guard installed. Every call
/// site in the engine builds its agent from this.
pub fn agent_builder() -> ureq::AgentBuilder {
    ureq::AgentBuilder::new().middleware(offline_guard)
}

fn offline_guard(request: ureq::Request, next: ureq::MiddlewareNext) -> Result<ureq::Response, ureq::Error> {
    if is_loopback_url(request.url()) {
        return next.handle(request);
    }
    let refused = ureq::Response::new(OFFLINE_STATUS, "Forbidden", OFFLINE_MESSAGE)?;
    Err(ureq::Error::Status(OFFLINE_STATUS, refused))
}

/// The host of an absolute `http`, `https`, `ws` or `wss` URL, without
/// userinfo, port or IPv6 brackets. `None` for any other shape.
pub fn url_host(url: &str) -> Option<&str> {
    let (scheme, rest) = url.split_once("://")?;
    if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https" | "ws" | "wss") {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host_port = authority.rsplit_once('@').map(|(_, h)| h).unwrap_or(authority);
    if let Some(bracketed) = host_port.strip_prefix('[') {
        return bracketed.split_once(']').map(|(h, _)| h).filter(|h| !h.is_empty());
    }
    let host = host_port.split(':').next().unwrap_or("");
    (!host.is_empty()).then_some(host)
}

/// True for `localhost`, any `*.localhost` name, and loopback IP literals.
pub fn is_loopback_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") {
        return true;
    }
    host.parse::<IpAddr>().map(|ip| ip.is_loopback()).unwrap_or(false)
}

/// True when `url` is an absolute URL whose host is loopback.
pub fn is_loopback_url(url: &str) -> bool {
    url_host(url).map(is_loopback_host).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_hosts_are_recognised() {
        for url in [
            "http://localhost:3000/a",
            "http://LOCALHOST/",
            "http://app.localhost:5173",
            "http://127.0.0.1:9/x?y",
            "http://127.4.5.6/",
            "http://[::1]:8080/",
            "ws://127.0.0.1:9222/devtools/browser/abc",
            "http://user:pw@localhost:1/",
        ] {
            assert!(is_loopback_url(url), "{url}");
        }
    }

    #[test]
    fn remote_and_odd_hosts_are_not_loopback() {
        for url in [
            "https://impeccable.style/api/roll",
            "https://api.openai.com/v1/images",
            "https://github.com/pbakaus/impeccable/releases/download",
            "http://localhost.evil.example/",
            "http://127.0.0.1.evil.example/",
            "http://10.0.0.1/",
            "http://[2001:db8::1]/",
            "http://evil.example/?u=http://localhost/",
            "http://localhost@evil.example/",
            "file:///etc/hosts",
            "localhost:3000",
            "",
        ] {
            assert!(!is_loopback_url(url), "{url}");
        }
    }

    #[test]
    fn remote_request_is_refused_without_a_connection() {
        let err = agent_builder()
            .build()
            .get("http://procable-offline-test.invalid/")
            .call()
            .unwrap_err();
        match err {
            ureq::Error::Status(code, res) => {
                assert_eq!(code, OFFLINE_STATUS);
                assert_eq!(res.into_string().unwrap(), OFFLINE_MESSAGE);
            }
            other => panic!("expected the offline refusal, got {other}"),
        }
    }

    #[test]
    fn loopback_request_goes_through() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
        });
        let res = agent_builder()
            .build()
            .get(&format!("http://127.0.0.1:{port}/"))
            .call()
            .unwrap();
        assert_eq!(res.into_string().unwrap(), "ok");
        server.join().unwrap();
    }
}
