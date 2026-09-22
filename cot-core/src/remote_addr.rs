use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

use http::HeaderValue;

fn ip_or_socket_string_to_ip<'a, T>(s: T) -> Option<IpAddr>
where
    T: Into<&'a str> + Clone,
{
    if let Ok(ip) = IpAddr::from_str(s.clone().into()) {
        return Some(ip);
    }
    if let Ok(socket_addr) = SocketAddr::from_str(s.into()) {
        return Some(socket_addr.ip());
    }
    None
}

/// Extracts the left-most remote address from an HTTP `Forwarded` header.
///
/// # Errors
/// The function will fail if the header is malformed.
pub fn extract_forwarded(header: &HeaderValue) -> crate::Result<Option<IpAddr>> {
    match header.to_str() {
        Ok(v) => {
            match v
                .to_lowercase()
                .split(';')
                .find(|seg| seg.starts_with("for="))
                .map(|for_segments| {
                    for ele in for_segments
                        .split(',')
                        .map(|s| s.trim().trim_start_matches("for="))
                        .map(|s| s.trim_matches('"'))
                    {
                        if let Some(addr) = ip_or_socket_string_to_ip(ele) {
                            return Some(addr);
                        }
                    }
                    None
                }) {
                Some(Some(ip)) => Ok(Some(ip)),
                Some(None) | None => Err(crate::Error::internal("No valid IP address was found.")),
            }
        }
        Err(err) => Err(crate::Error::internal(format!(
            "HTTP header containing non-ASCII characters: {err}"
        ))),
    }
}

/// Extract the left-most remote address from an HTTP `X-Forwarded-For` header.
///
/// # Errors
/// The function will fail if the header is malformed.
pub fn extract_x_forwarded_for(header: &HeaderValue) -> crate::Result<Option<IpAddr>> {
    match header.to_str() {
        Ok(v) => match v.split(',').map(str::trim).next().map(IpAddr::from_str) {
            Some(Ok(ip)) => Ok(Some(ip)),
            Some(Err(err)) => Err(crate::Error::internal(format!(
                "Malformed IP address: {err}"
            ))),
            None => Ok(None),
        },
        Err(err) => Err(crate::Error::internal(format!(
            "HTTP header containing non-ASCII characters: {err}"
        ))),
    }
}

/// Extract the IP from the HTTP `CF-Connecting-IP` header.
///
/// # Errors
/// The function will fail if the header is malformed.
pub fn extract_cf_connecting_ip(header: &HeaderValue) -> crate::Result<IpAddr> {
    match header.to_str() {
        Ok(v) => IpAddr::from_str(v)
            .map_err(|err| crate::Error::internal(format!("Malformed IP address: {err}"))),
        Err(err) => Err(crate::Error::internal(format!(
            "HTTP header containing non-ASCII characters: {err}"
        ))),
    }
}

/// Extract the IP from the HTTP `X-Real-IP` header.
///
/// # Errors
/// The function will fail if the header is malformed.
pub fn extract_x_real_ip(header: &HeaderValue) -> crate::Result<IpAddr> {
    match header.to_str() {
        Ok(v) => IpAddr::from_str(v)
            .map_err(|err| crate::Error::internal(format!("Malformed IP address: {err}"))),
        Err(err) => Err(crate::Error::internal(format!(
            "HTTP header containing non-ASCII characters: {err}"
        ))),
    }
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
            )
            .unwrap()
            .unwrap(),
            Ipv4Addr::new(1, 2, 3, 4)
        );
    }

    #[cot::test]
    async fn forwarded_valid_v4_with_port() {
        assert_eq!(
            extract_forwarded(
                &HeaderValue::from_str("proto=http;for=1.2.3.4:3489;by=32.4.2.5").unwrap()
            )
            .unwrap()
            .unwrap(),
            Ipv4Addr::new(1, 2, 3, 4)
        );
    }

    #[cot::test]
    async fn forwarded_valid_v6() {
        assert_eq!(
            extract_forwarded(&HeaderValue::from_str("For=\"[2001:db8:cafe::17]:4711\"").unwrap())
                .unwrap()
                .unwrap(),
            IP_V6_CAFE_17
        );
    }

    #[cot::test]
    async fn multiple_forwarded_header_valid_v6() {
        assert_eq!(extract_forwarded(&HeaderValue::from_str("BY=32.234.40.5;For=\"[2001:db8:cafe::17]:4711\", \"[2003:db8:caff::17]:4711\";proto=http").unwrap()).unwrap().unwrap(), IP_V6_CAFE_17);
    }

    #[cot::test]
    async fn extract_x_forwarded_for_valid_v4() {
        assert_eq!(
            extract_x_forwarded_for(
                &HeaderValue::from_str("1.2.3.4,2003:db8:caff::17, 3.45.3.2").unwrap()
            )
            .unwrap()
            .unwrap(),
            Ipv4Addr::new(1, 2, 3, 4)
        );
    }

    #[cot::test]
    async fn extract_x_forwarded_for_valid_v6() {
        assert_eq!(
            extract_x_forwarded_for(
                &HeaderValue::from_str("2001:db8:cafe::17,1.2.3.4,3.45.3.2").unwrap()
            )
            .unwrap()
            .unwrap(),
            IP_V6_CAFE_17
        );
    }

    #[cot::test]
    async fn extract_cf_connecting_ip_valid_v4() {
        assert_eq!(
            extract_cf_connecting_ip(&HeaderValue::from_str("1.2.3.4").unwrap()).unwrap(),
            Ipv4Addr::new(1, 2, 3, 4)
        );
    }

    #[cot::test]
    async fn extract_cf_connecting_ip_valid_v6() {
        assert_eq!(
            extract_cf_connecting_ip(&HeaderValue::from_str("2001:db8:cafe::17").unwrap()).unwrap(),
            IP_V6_CAFE_17
        );
    }

    #[cot::test]
    async fn extract_x_real_ip_valid_v6() {
        assert_eq!(
            extract_x_real_ip(&HeaderValue::from_str("2001:db8:cafe::17").unwrap()).unwrap(),
            IP_V6_CAFE_17
        );
    }

    #[cot::test]
    async fn extract_x_real_ip_valid_v4() {
        assert_eq!(
            extract_x_real_ip(&HeaderValue::from_str("1.2.3.4").unwrap()).unwrap(),
            Ipv4Addr::new(1, 2, 3, 4)
        );
    }
}
