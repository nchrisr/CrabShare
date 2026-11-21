use std::net::IpAddr;
use std::net::SocketAddr;

/// The group key for a client address: the first three octets of an IPv4 address (a /24). IPv6 gets `None` (D5).
pub fn subnet_key(addr: SocketAddr) -> Option<String> {
    match addr.ip() {
        IpAddr::V4(ip) => {
            let octets = ip.octets();
            Some(format!("{}_{}_{}", octets[0], octets[1], octets[2]))
        }
        IpAddr::V6(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subnet_key_ipv4() {
        let addr: SocketAddr = "192.168.1.5:3000".parse().unwrap();
        assert_eq!(subnet_key(addr), Some("192_168_1".to_string()));
    }

    #[test]
    fn subnet_key_ipv6() {
        // [::1] is the IPv6 localhost.
        let addr: SocketAddr = "[::1]:3000".parse().unwrap();
        assert_eq!(subnet_key(addr), None);
    }
}
