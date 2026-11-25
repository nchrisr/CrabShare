//! Which browser pages may open `/ws`: only the server's own page, reached by an address
//! that DNS rebinding can't fake. Requests without an `Origin` header (tools, native
//! clients) are not browsers and pass. See D15 in `docs/DECISIONS.md`.

use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use axum::http::HeaderMap;
use axum::http::HeaderName;
use axum::http::header::HOST;
use axum::http::header::ORIGIN;

/// Why an upgrade was refused. A fixed label, so no client text ever reaches a log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalReason {
    /// `Host` isn't an IP address, `localhost` or a `.local` name (stops DNS rebinding).
    HostNotAllowed,
    /// `Origin` names a different server than `Host` (stops cross-site connections).
    OriginMismatch,
    /// A header is missing, repeated or unreadable.
    MalformedHeaders,
}

impl RefusalReason {
    pub fn label(&self) -> &'static str {
        match self {
            RefusalReason::HostNotAllowed => "host not allowed",
            RefusalReason::OriginMismatch => "origin mismatch",
            RefusalReason::MalformedHeaders => "malformed headers",
        }
    }

    fn index(&self) -> usize {
        match self {
            RefusalReason::HostNotAllowed => 0,
            RefusalReason::OriginMismatch => 1,
            RefusalReason::MalformedHeaders => 2,
        }
    }
}

/// Checks a `/ws` upgrade request's `Origin` and `Host` headers.
pub fn check_upgrade_origin(headers: &HeaderMap) -> Result<(), RefusalReason> {
    let Some(origin) = single_header(headers, ORIGIN)? else {
        // Browsers always send Origin, so this is a tool or a native client.
        return Ok(());
    };
    let Some(host) = single_header(headers, HOST)? else {
        return Err(RefusalReason::MalformedHeaders);
    };
    if origin.is_empty() {
        return Err(RefusalReason::MalformedHeaders);
    }

    let Some((host_name, _port)) = split_host_port(host) else {
        return Err(RefusalReason::MalformedHeaders);
    };
    if !is_allowed_host(host_name) {
        return Err(RefusalReason::HostNotAllowed);
    }

    let Some(origin_authority) = origin_authority(origin) else {
        return Err(RefusalReason::OriginMismatch);
    };
    if !origin_authority.eq_ignore_ascii_case(host) {
        return Err(RefusalReason::OriginMismatch);
    }
    Ok(())
}

/// The header's value if it appears once, `None` if absent. Repeated or not visible
/// ASCII is malformed: an unreadable `Origin` must never count as a missing one.
fn single_header(headers: &HeaderMap, name: HeaderName) -> Result<Option<&str>, RefusalReason> {
    let mut values = headers.get_all(name).iter();
    let Some(first) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(RefusalReason::MalformedHeaders);
    }
    match first.to_str() {
        Ok(text) => Ok(Some(text)),
        Err(_) => Err(RefusalReason::MalformedHeaders),
    }
}

/// Splits `host[:port]`, where host may be a bracketed IPv6 literal. `None` if the
/// host is empty or has characters no hostname has, or the port isn't a valid `u16`.
fn split_host_port(authority: &str) -> Option<(&str, Option<u16>)> {
    let (host, port_text) = if authority.starts_with('[') {
        let end = authority.find(']')?;
        let (host, rest) = authority.split_at(end + 1);
        if rest.is_empty() {
            (host, None)
        } else {
            (host, Some(rest.strip_prefix(':')?))
        }
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };

    let port = match port_text {
        Some(text) => Some(parse_port(text)?),
        None => None,
    };

    let is_bracketed = host.starts_with('[');
    if !is_bracketed && !is_hostname_text(host) {
        return None;
    }
    Some((host, port))
}

/// Digits only, so `+80` or ` 80` don't slip through `u16::from_str`.
fn parse_port(text: &str) -> Option<u16> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<u16>().ok()
}

fn is_hostname_text(host: &str) -> bool {
    let allowed = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-';
    !host.is_empty() && host.bytes().all(allowed)
}

/// An address a real visit uses and DNS rebinding can't: an IP literal, `localhost`,
/// or a `.local` name (resolved by multicast DNS inside the LAN).
fn is_allowed_host(host: &str) -> bool {
    if let Some(inner) = host
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    {
        return inner.parse::<Ipv6Addr>().is_ok();
    }
    if host.parse::<Ipv4Addr>().is_ok() {
        return true;
    }
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    is_local_name(host)
}

/// `name.local`, with at least one character before the dot.
fn is_local_name(host: &str) -> bool {
    const SUFFIX: &str = ".local";
    if host.len() <= SUFFIX.len() {
        return false;
    }
    // Safe to slice by bytes: callers have already checked the host is ASCII.
    let (name, suffix) = host.split_at(host.len() - SUFFIX.len());
    suffix.eq_ignore_ascii_case(SUFFIX) && !name.ends_with('.')
}

/// The `host[:port]` part of an `http://` or `https://` origin. `None` for `null`,
/// another scheme, or anything with a path.
fn origin_authority(origin: &str) -> Option<&str> {
    let (scheme, authority) = origin.split_once("://")?;
    let is_web_scheme = scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https");
    if !is_web_scheme || authority.is_empty() {
        return None;
    }
    if authority.contains(['/', '?', '#']) {
        return None;
    }
    Some(authority)
}

/// Counts refusals per reason, so a hostile page retrying in a loop can't flood the log.
pub struct RefusalCounts {
    counts: [AtomicU64; 3],
}

impl RefusalCounts {
    pub const fn new() -> Self {
        RefusalCounts {
            counts: [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)],
        }
    }

    /// Adds one refusal. Returns the new count when it is worth logging (1, 2, 4, 8, ...),
    /// so a million refusals write about 20 lines.
    pub fn record(&self, reason: RefusalReason) -> Option<u64> {
        // Relaxed is enough: the counter orders nothing else, it only has to add up.
        let count = self.counts[reason.index()].fetch_add(1, Ordering::Relaxed) + 1;
        if count.is_power_of_two() {
            Some(count)
        } else {
            None
        }
    }
}

/// The server's refusal counts since it started.
pub static REFUSALS: RefusalCounts = RefusalCounts::new();

#[cfg(test)]
mod tests {
    use axum::http::HeaderMap;
    use axum::http::HeaderValue;
    use axum::http::header::HOST;
    use axum::http::header::ORIGIN;

    use super::RefusalCounts;
    use super::RefusalReason;
    use super::check_upgrade_origin;

    fn headers(host: Option<&str>, origin: Option<&str>) -> HeaderMap {
        let mut map = HeaderMap::new();
        if let Some(host) = host {
            map.insert(
                HOST,
                HeaderValue::from_str(host).expect("test host is valid"),
            );
        }
        if let Some(origin) = origin {
            map.insert(
                ORIGIN,
                HeaderValue::from_str(origin).expect("test origin is valid"),
            );
        }
        map
    }

    fn check(host: Option<&str>, origin: Option<&str>) -> Result<(), RefusalReason> {
        check_upgrade_origin(&headers(host, origin))
    }

    #[test]
    fn allows_requests_without_origin() {
        assert_eq!(check(Some("evil.com:3000"), None), Ok(()));
        assert_eq!(check(None, None), Ok(()));
    }

    #[test]
    fn allows_the_servers_own_page() {
        let cases = [
            ("localhost:3000", "http://localhost:3000"),
            ("localhost", "http://localhost"),
            ("192.168.1.20:3000", "http://192.168.1.20:3000"),
            ("pi.local", "http://pi.local"),
            ("Pi.Local:3000", "http://pi.local:3000"),
            ("localhost:3000", "HTTP://localhost:3000"),
            ("[::1]:3000", "http://[::1]:3000"),
            ("192.168.1.20", "https://192.168.1.20"),
            ("0.0.0.0:3000", "http://0.0.0.0:3000"),
        ];
        for (host, origin) in cases {
            assert_eq!(check(Some(host), Some(origin)), Ok(()), "{host} / {origin}");
        }
    }

    #[test]
    fn refuses_hosts_dns_rebinding_could_use() {
        let hosts = [
            "evil.com:3000",
            "192.168.1.20.evil.com",
            "xlocal",
            ".local",
            "pi..local",
            "pi.local.",
            "localhost.evil.com",
            "foo.localhost",
            "raspberrypi:3000",
        ];
        for host in hosts {
            let origin = format!("http://{host}");
            assert_eq!(
                check(Some(host), Some(&origin)),
                Err(RefusalReason::HostNotAllowed),
                "{host}"
            );
        }
    }

    #[test]
    fn refuses_other_sites_and_other_ports() {
        let cases = [
            ("192.168.1.20:3000", "https://evil.com"),
            ("192.168.1.20:3000", "http://192.168.1.20:8080"),
            ("192.168.1.20", "http://192.168.1.20:80"),
            ("localhost:3000", "null"),
            ("localhost:3000", "http://localhost:3000/path"),
            ("localhost:3000", "tauri://localhost:3000"),
            ("localhost:3000", "http://"),
        ];
        for (host, origin) in cases {
            assert_eq!(
                check(Some(host), Some(origin)),
                Err(RefusalReason::OriginMismatch),
                "{host} / {origin}"
            );
        }
    }

    #[test]
    fn refuses_malformed_headers() {
        let cases = [
            (Some("localhost:3000"), ""),
            (None, "http://localhost:3000"),
            (Some("localhost:"), "http://localhost:"),
            (Some("localhost:abc"), "http://localhost:abc"),
            (Some("localhost:99999"), "http://localhost:99999"),
            (Some("localhost:+80"), "http://localhost:+80"),
            (Some("user@localhost"), "http://user@localhost"),
            (Some("::1"), "http://::1"),
            (Some("[::1"), "http://[::1"),
        ];
        for (host, origin) in cases {
            assert_eq!(
                check(host, Some(origin)),
                Err(RefusalReason::MalformedHeaders),
                "{host:?} / {origin}"
            );
        }
    }

    #[test]
    fn refuses_repeated_headers() {
        let mut two_origins = headers(Some("localhost:3000"), Some("http://localhost:3000"));
        two_origins.append(ORIGIN, HeaderValue::from_static("http://localhost:3000"));
        assert_eq!(
            check_upgrade_origin(&two_origins),
            Err(RefusalReason::MalformedHeaders)
        );

        let mut two_hosts = headers(Some("localhost:3000"), Some("http://localhost:3000"));
        two_hosts.append(HOST, HeaderValue::from_static("localhost:3000"));
        assert_eq!(
            check_upgrade_origin(&two_hosts),
            Err(RefusalReason::MalformedHeaders)
        );
    }

    #[test]
    fn refuses_unreadable_origin() {
        let mut map = headers(Some("localhost:3000"), None);
        let non_ascii = HeaderValue::from_bytes("http://lócalhost:3000".as_bytes())
            .expect("header values may hold non-ASCII bytes");
        map.insert(ORIGIN, non_ascii);
        assert_eq!(
            check_upgrade_origin(&map),
            Err(RefusalReason::MalformedHeaders)
        );
    }

    #[test]
    fn logs_refusals_at_powers_of_two() {
        let counts = RefusalCounts::new();
        let mut logged = Vec::new();
        for _ in 0..20 {
            if let Some(count) = counts.record(RefusalReason::OriginMismatch) {
                logged.push(count);
            }
        }
        assert_eq!(logged, vec![1, 2, 4, 8, 16]);
        assert_eq!(counts.record(RefusalReason::HostNotAllowed), Some(1));
    }
}
