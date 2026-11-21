use crate::messages::DeviceKind;

/// The device kind of a connecting client: what it declared in the query, else a guess from its user agent.
pub fn connecting_device_kind(query: Option<&str>, user_agent: Option<&str>) -> DeviceKind {
    let declared = query
        .and_then(device_query_value)
        .and_then(declared_device_kind);

    match declared {
        Some(kind) => kind,
        None => match user_agent {
            Some(user_agent) => device_kind(user_agent),
            None => DeviceKind::Unknown,
        },
    }
}

/// Guesses the device kind from a `User-Agent` header value.
fn device_kind(user_agent: &str) -> DeviceKind {
    // Order matters: iPhone and iPad user agents also contain "Mac OS X",
    // and Android ones also contain "Linux".
    if user_agent.contains("iPad") {
        DeviceKind::Ipad
    } else if user_agent.contains("iPhone") {
        DeviceKind::Iphone
    } else if user_agent.contains("Android") {
        DeviceKind::Android
    } else if user_agent.contains("Macintosh") {
        DeviceKind::Mac
    } else if user_agent.contains("Windows") {
        DeviceKind::Windows
    } else if user_agent.contains("Linux") {
        DeviceKind::Linux
    } else {
        DeviceKind::Unknown
    }
}

/// The device kind a client declared itself, if it is a known name. `unknown` counts as not declared.
fn declared_device_kind(value: &str) -> Option<DeviceKind> {
    match value {
        "iphone" => Some(DeviceKind::Iphone),
        "ipad" => Some(DeviceKind::Ipad),
        "android" => Some(DeviceKind::Android),
        "mac" => Some(DeviceKind::Mac),
        "windows" => Some(DeviceKind::Windows),
        "linux" => Some(DeviceKind::Linux),
        _ => None,
    }
}

/// The first `device=` value in a raw query string such as `device=iphone&x=1`.
fn device_query_value(query: &str) -> Option<&str> {
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("device="))
}

#[cfg(test)]
mod tests {
    use super::*;

    const IPHONE_SAFARI: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1";
    const LINUX_FIREFOX: &str =
        "Mozilla/5.0 (X11; Linux x86_64; rv:127.0) Gecko/20100101 Firefox/127.0";

    #[test]
    fn device_kind_from_real_user_agents() {
        let cases = [
            (IPHONE_SAFARI, DeviceKind::Iphone),
            (
                "Mozilla/5.0 (iPad; CPU OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1",
                DeviceKind::Ipad,
            ),
            (
                "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36",
                DeviceKind::Android,
            ),
            (
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15",
                DeviceKind::Mac,
            ),
            (
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
                DeviceKind::Mac,
            ),
            (
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0",
                DeviceKind::Windows,
            ),
            (LINUX_FIREFOX, DeviceKind::Linux),
            ("", DeviceKind::Unknown),
        ];
        for (user_agent, expected) in cases {
            assert_eq!(
                device_kind(user_agent),
                expected,
                "user agent: {user_agent}"
            );
        }
    }

    #[test]
    fn declared_device_kind_accepts_only_known_names() {
        assert_eq!(declared_device_kind("iphone"), Some(DeviceKind::Iphone));
        assert_eq!(declared_device_kind("ipad"), Some(DeviceKind::Ipad));
        assert_eq!(declared_device_kind("android"), Some(DeviceKind::Android));
        assert_eq!(declared_device_kind("mac"), Some(DeviceKind::Mac));
        assert_eq!(declared_device_kind("windows"), Some(DeviceKind::Windows));
        assert_eq!(declared_device_kind("linux"), Some(DeviceKind::Linux));
        assert_eq!(declared_device_kind("unknown"), None);
        assert_eq!(declared_device_kind(""), None);
        assert_eq!(declared_device_kind("iPhone"), None);
        assert_eq!(declared_device_kind("toaster"), None);
    }

    #[test]
    fn device_query_value_takes_the_first_device_key() {
        assert_eq!(device_query_value("device=mac"), Some("mac"));
        assert_eq!(device_query_value("x=1&device=mac"), Some("mac"));
        assert_eq!(device_query_value("device=a&device=b"), Some("a"));
        assert_eq!(device_query_value("devices=mac"), None);
        assert_eq!(device_query_value(""), None);
    }

    #[test]
    fn connecting_device_kind_prefers_a_valid_declaration() {
        let kind = connecting_device_kind(Some("device=android"), Some(IPHONE_SAFARI));
        assert_eq!(kind, DeviceKind::Android);
    }

    #[test]
    fn connecting_device_kind_falls_back_to_the_user_agent() {
        let cases = [
            None,
            Some(""),
            Some("device=toaster"),
            Some("device=unknown"),
            Some("device=a&device=b"),
            Some("device=%69phone"),
        ];
        for query in cases {
            let kind = connecting_device_kind(query, Some(LINUX_FIREFOX));
            assert_eq!(kind, DeviceKind::Linux, "query: {query:?}");
        }
    }

    #[test]
    fn connecting_device_kind_with_nothing_to_go_on_is_unknown() {
        assert_eq!(connecting_device_kind(None, None), DeviceKind::Unknown);
    }
}
