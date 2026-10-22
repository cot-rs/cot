use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use http::HeaderValue;

fn ip_or_socket_string_to_ip(s: &str) -> Option<IpAddr> {
    if let Ok(ip) = IpAddr::from_str(
        s.strip_prefix('[')
            .unwrap_or(s)
            .strip_suffix(']')
            .unwrap_or(s),
    ) {
        return Some(ip);
    }
    if let Ok(socket_addr) = SocketAddr::from_str(s) {
        return Some(socket_addr.ip());
    }
    None
}

/// Extracts the remote addresses from an HTTP `Forwarded` header, in order.
/// Invalid or obfuscated addresses are returned as `None`.
pub fn extract_forwarded(header: &HeaderValue) -> Vec<Option<IpAddr>> {
    header
        .to_str()
        .unwrap_or_default()
        .split(',')
        .map(|element| {
            element
                .split(';')
                .filter_map(|pair| pair.split_once('='))
                .find(|(key, _)| key.trim().eq_ignore_ascii_case("for"))
                .and_then(|(_, node)| ip_or_socket_string_to_ip(node.trim().trim_matches('"')))
        })
        .collect()
}

/// Extracts the remote addresses from an HTTP `X-Forwarded-For` header, in
/// order. Invalid addresses are returned as `None`.
pub fn extract_x_forwarded_for(header: &HeaderValue) -> Vec<Option<IpAddr>> {
    header
        .to_str()
        .unwrap_or_default()
        .split(',')
        .map(|ip| IpAddr::from_str(ip.trim()).ok())
        .collect()
}

/// Extracts a single IP address from a header value.
pub fn extract_single_ip(header: &HeaderValue) -> Option<IpAddr> {
    IpAddr::from_str(header.to_str().ok()?.trim()).ok()
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use super::*;

    const IP_V6_CAFE_17: Ipv6Addr =
        Ipv6Addr::from_bits(42_540_766_474_105_631_998_997_804_644_434_968_599);

    #[cot::test]
    async fn forwarded_valid_v4() {
        assert_eq!(
            extract_forwarded(
                &HeaderValue::from_str("proto=http;for=1.2.3.4;by=32.4.2.5").unwrap()
            )[0]
            .unwrap(),
            Ipv4Addr::new(1, 2, 3, 4)
        );
    }

    #[cot::test]
    async fn forwarded_valid_v4_alternative() {
        assert_eq!(
            extract_forwarded(
                &HeaderValue::from_str("by=10.0.0.1;for=203.0.113.95, by=10.0.0.2;for=10.0.0.1")
                    .unwrap()
            )[0]
            .unwrap(),
            Ipv4Addr::new(203, 0, 113, 95)
        );
    }

    #[cot::test]
    async fn forwarded_v4_whitespace() {
        assert_eq!(
            extract_forwarded(&HeaderValue::from_str("by=10.0.0.1; for=203.0.113.95").unwrap())[0]
                .unwrap(),
            Ipv4Addr::new(203, 0, 113, 95)
        );
    }

    #[cot::test]
    async fn forwarded_v4_element_without_for() {
        assert!(
            extract_forwarded(&HeaderValue::from_str("by=10.0.0.1, for=203.0.113.95").unwrap())[0]
                .is_none()
        );
    }

    #[cot::test]
    async fn forwarded_valid_v4_with_port() {
        assert_eq!(
            extract_forwarded(
                &HeaderValue::from_str("proto=http;for=1.2.3.4:3489;by=32.4.2.5").unwrap()
            )[0]
            .unwrap(),
            Ipv4Addr::new(1, 2, 3, 4)
        );
    }

    #[cot::test]
    async fn forwarded_valid_v6() {
        assert_eq!(
            extract_forwarded(&HeaderValue::from_str("For=\"[2001:db8:cafe::17]:4711\"").unwrap())
                [0]
            .unwrap(),
            IP_V6_CAFE_17
        );
    }

    #[cot::test]
    async fn forwarded_valid_v6_no_port() {
        assert_eq!(
            extract_forwarded(&HeaderValue::from_str("For=\"[2001:db8:cafe::17]\"").unwrap())[0]
                .unwrap(),
            IP_V6_CAFE_17
        );
    }

    #[cot::test]
    async fn forwarded_valid_v6_no_port_brackets() {
        assert_eq!(
            extract_forwarded(&HeaderValue::from_str("For=\"2001:db8:cafe::17\"").unwrap())[0]
                .unwrap(),
            IP_V6_CAFE_17
        );
    }

    #[cot::test]
    async fn multiple_forwarded_header_valid_v6() {
        assert_eq!(extract_forwarded(&HeaderValue::from_str("BY=32.234.40.5;For=\"[2001:db8:cafe::17]:4711\", \"[2003:db8:caff::17]:4711\";proto=http").unwrap())[0].unwrap(), IP_V6_CAFE_17);
    }

    #[cot::test]
    async fn extract_x_forwarded_for_valid_v4() {
        assert_eq!(
            extract_x_forwarded_for(
                &HeaderValue::from_str("1.2.3.4,2003:db8:caff::17, 3.45.3.2").unwrap()
            )[0]
            .unwrap(),
            Ipv4Addr::new(1, 2, 3, 4)
        );
    }

    #[cot::test]
    async fn extract_x_forwarded_for_valid_v6() {
        assert_eq!(
            extract_x_forwarded_for(
                &HeaderValue::from_str("2001:db8:cafe::17,1.2.3.4,3.45.3.2").unwrap()
            )[0]
            .unwrap(),
            IP_V6_CAFE_17
        );
    }

    #[cot::test]
    async fn extract_cf_connecting_ip_valid_v4() {
        assert_eq!(
            extract_single_ip(&HeaderValue::from_str("1.2.3.4").unwrap()).unwrap(),
            Ipv4Addr::new(1, 2, 3, 4)
        );
    }

    #[cot::test]
    async fn extract_cf_connecting_ip_valid_v6() {
        assert_eq!(
            extract_single_ip(&HeaderValue::from_str("2001:db8:cafe::17").unwrap()).unwrap(),
            IP_V6_CAFE_17
        );
    }
}
