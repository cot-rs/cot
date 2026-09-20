use axum::{extract::connect_info::Connected, serve::IncomingStream};
use cot_core::request::{RequestHead, extractors::FromRequestHead};

use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

use http::HeaderValue;

use crate::request::RequestExt;

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
fn extract_forwarded(header: &HeaderValue) -> crate::Result<Option<IpAddr>> {
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
fn extract_x_forwarded_for(header: &HeaderValue) -> crate::Result<Option<IpAddr>> {
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
fn extract_cf_connecting_ip(header: &HeaderValue) -> crate::Result<IpAddr> {
    match header.to_str() {
        Ok(v) => IpAddr::from_str(v)
            .map_err(|err| crate::Error::internal(format!("Malformed IP address: {err}"))),
        Err(err) => Err(crate::Error::internal(format!(
            "HTTP header containing non-ASCII characters: {err}"
        ))),
    }
}

/// Extract the IP from the HTTP `X-Real-IP` header.
fn extract_x_real_ip(header: &HeaderValue) -> crate::Result<IpAddr> {
    match header.to_str() {
        Ok(v) => IpAddr::from_str(v)
            .map_err(|err| crate::Error::internal(format!("Malformed IP address: {err}"))),
        Err(err) => Err(crate::Error::internal(format!(
            "HTTP header containing non-ASCII characters: {err}"
        ))),
    }
}

#[derive(Clone, Copy, Debug)]
/// An extractor that extracts the IP address of the remote.
/// This automatically checks for proxy IP headers and contains their IP if one such is specified.
///
/// # Examples
/// ```rust
/// pub async fn example_handler(ip: RemoteAddr) -> cot::Result<Html> {
///     dbg!(ip.ip()); // Prints the IP as a debug statement
///     dbg!(ip.closest_ip()); // Prints the closest IP as a debug statement
///     ///...
/// }
/// ```
pub struct RemoteAddr {
    /// IP of the closest peer
    direct: IpAddr,
    /// IP behind the proxy if such exist
    proxied: Option<IpAddr>,
}

impl RemoteAddr {
    #[must_use]
    /// Get the IP address of the peer.
    /// This automatically handles proxy IP headers.
    pub fn ip(&self) -> IpAddr {
        self.proxied.unwrap_or(self.direct)
    }

    #[must_use]
    /// Get the IP address of the peer closest to this server.
    /// This does **not** handle proxies.
    ///
    /// In most use-cases [`Self::ip`] is more appropriate.
    pub fn direct_peer_ip(&self) -> IpAddr {
        self.direct
    }
}

impl<'a> Connected<IncomingStream<'a, tokio::net::TcpListener>> for RemoteAddr {
    fn connect_info(stream: IncomingStream<'a, tokio::net::TcpListener>) -> Self {
        let closest_ip = stream.remote_addr().ip();
        RemoteAddr {
            direct: closest_ip,
            proxied: None,
        }
    }
}

impl FromRequestHead for RemoteAddr {
    async fn from_request_head(head: &RequestHead) -> crate::Result<Self> {
        let addr = head
            .extensions
            .get::<RemoteAddr>()
            .expect("Missing RemoteAddr extension");

        let closest = addr.direct;

        let config = head.project_config().clone().trusted_proxy;
        let trusted_proxies = config.get_trusted_proxies();
        let trusted_headers = config.get_trusted_headers();

        let mut proxy: Option<IpAddr> = None;

        if trusted_proxies.iter().any(|proxy| proxy.contains(closest)) {
            for h in trusted_headers {
                if let Some(v) = head.headers.get(&h)
                    && proxy.is_none()
                {
                    let h_as_str = h.as_str();
                    if h_as_str == "forwarded" {
                        proxy = extract_forwarded(v)?;
                    } else if h_as_str == "x-forwarded-for" {
                        proxy = extract_x_forwarded_for(v)?;
                    } else if h_as_str == "cf-connecting-ip" {
                        proxy = Some(extract_cf_connecting_ip(v)?);
                    } else if h_as_str == "x-real-ip" {
                        proxy = Some(extract_x_real_ip(v)?);
                    }
                }
            }
        }

        Ok(RemoteAddr {
            direct: addr.direct,
            proxied: proxy,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use crate::{
        config::{IpWithSubnet, ProjectConfig, ProxyConfig},
        request::RequestExt,
        test::TestRequestBuilder,
    };
    use http::HeaderName;

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

    #[cot::test]
    async fn remote_addr() {
        const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));

        let mut request = TestRequestBuilder::get("/").with_default_config().build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP,
            proxied: None,
        });
        let RemoteAddr { direct, proxied: _ } = request.extract_from_head().await.unwrap();

        assert_eq!(direct, IP);
    }

    #[cot::test]
    async fn remote_addr_v6() {
        const IP: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));

        let mut request = TestRequestBuilder::get("/").with_default_config().build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP,
            proxied: None,
        });
        let RemoteAddr { direct, proxied: _ } = request.extract_from_head().await.unwrap();

        assert_eq!(direct, IP);
    }

    #[cot::test]
    async fn remote_addr_proxied_unconfigured() {
        const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));

        let mut request = TestRequestBuilder::get("/").with_default_config().build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP,
            proxied: None,
        });
        let RemoteAddr { direct: _, proxied } = request.extract_from_head().await.unwrap();

        assert_eq!(proxied, None);
    }

    #[cot::test]
    async fn remote_addr_proxied_configured() {
        const IP: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
        const IP_PROXY: IpAddr = IpAddr::V6(Ipv6Addr::new(21, 32, 43, 54, 65, 76, 87, 98));

        let mut config = ProjectConfig::dev_default();

        config.trusted_proxy = ProxyConfig::new(
            vec![IpWithSubnet::new(IP, 64).unwrap()],
            vec!["Forwarded".to_string()],
        );

        let mut request = TestRequestBuilder::get("/").config(config).build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP,
            proxied: None,
        });
        request.headers_mut().insert(
            HeaderName::from_str("Forwarded").unwrap(),
            HeaderValue::from_str(&format!("for={IP_PROXY}")).unwrap(),
        );

        let ip = request
            .extract_from_head::<RemoteAddr>()
            .await
            .unwrap()
            .ip();

        assert_eq!(ip, IP_PROXY);
    }

    #[cot::test]
    async fn remote_addr_proxied_configured_b() {
        const IP: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
        const IP_PROXY: IpAddr = IpAddr::V6(Ipv6Addr::new(21, 32, 43, 54, 65, 76, 87, 98));

        let mut config = ProjectConfig::dev_default();

        config.trusted_proxy = ProxyConfig::new(
            vec![IpWithSubnet::new(IP, 128).unwrap()],
            vec!["X-Forwarded-For".to_string()],
        );

        let mut request = TestRequestBuilder::get("/").config(config).build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP,
            proxied: None,
        });
        request.headers_mut().insert(
            HeaderName::from_str("Forwarded").unwrap(),
            HeaderValue::from_str(&format!("for={IP_PROXY}")).unwrap(),
        );

        let ip = request
            .extract_from_head::<RemoteAddr>()
            .await
            .unwrap()
            .ip();

        assert_eq!(ip, IP);
    }

    #[cot::test]
    async fn remote_addr_proxied_configured_c() {
        const IP: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
        const IP_WRONG: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 9));
        const IP_PROXY: IpAddr = IpAddr::V6(Ipv6Addr::new(21, 32, 43, 54, 65, 76, 87, 98));

        let mut config = ProjectConfig::dev_default();

        config.trusted_proxy = ProxyConfig::new(
            vec![IpWithSubnet::new(IP, 128).unwrap()],
            vec!["Forwarded".to_string()],
        );

        let mut request = TestRequestBuilder::get("/").config(config).build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP_WRONG,
            proxied: None,
        });
        request.headers_mut().insert(
            HeaderName::from_str("Forwarded").unwrap(),
            HeaderValue::from_str(&format!("for={IP_PROXY}")).unwrap(),
        );

        let ip = request
            .extract_from_head::<RemoteAddr>()
            .await
            .unwrap()
            .ip();

        assert_eq!(ip, IP_WRONG);
    }

    #[cot::test]
    async fn remote_addr_proxied_configured_c_in_range() {
        const IP: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
        const IP_WRONG: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 9));
        const IP_PROXY: IpAddr = IpAddr::V6(Ipv6Addr::new(21, 32, 43, 54, 65, 76, 87, 98));

        let mut config = ProjectConfig::dev_default();

        config.trusted_proxy = ProxyConfig::new(
            vec![IpWithSubnet::new(IP, 64).unwrap()],
            vec!["Forwarded".to_string()],
        );

        let mut request = TestRequestBuilder::get("/").config(config).build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP_WRONG,
            proxied: None,
        });
        request.headers_mut().insert(
            HeaderName::from_str("Forwarded").unwrap(),
            HeaderValue::from_str(&format!("for={IP_PROXY}")).unwrap(),
        );

        let ip = request
            .extract_from_head::<RemoteAddr>()
            .await
            .unwrap()
            .ip();

        assert_eq!(ip, IP_PROXY);
    }

    #[cot::test]
    async fn remote_addr_proxied_configured_d() {
        const IP: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
        const IP_PROXY: IpAddr = IpAddr::V6(Ipv6Addr::new(21, 32, 43, 54, 65, 76, 87, 98));

        let mut config = ProjectConfig::dev_default();

        config.trusted_proxy = ProxyConfig::new(
            vec![IpWithSubnet::new(IP, 128).unwrap()],
            vec!["Forwarded".to_string()],
        );

        let mut request = TestRequestBuilder::get("/").config(config).build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP,
            proxied: None,
        });
        request.headers_mut().insert(
            HeaderName::from_str("X-Forwarded-For").unwrap(),
            HeaderValue::from_str(&format!("{IP_PROXY}")).unwrap(),
        );

        let ip = request
            .extract_from_head::<RemoteAddr>()
            .await
            .unwrap()
            .ip();

        assert_eq!(ip, IP);
    }

    #[cot::test]
    async fn remote_addr_proxied_configured_e() {
        const IP: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
        const IP_PROXY: IpAddr = IpAddr::V6(Ipv6Addr::new(21, 32, 43, 54, 65, 76, 87, 98));

        let mut config = ProjectConfig::dev_default();

        config.trusted_proxy = ProxyConfig::new(
            vec![IpWithSubnet::new(IP, 128).unwrap()],
            vec!["X-Forwarded-For".to_string()],
        );

        let mut request = TestRequestBuilder::get("/").config(config).build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP,
            proxied: None,
        });
        request.headers_mut().insert(
            HeaderName::from_str("X-Forwarded-For").unwrap(),
            HeaderValue::from_str(&format!("{IP_PROXY}")).unwrap(),
        );

        let ip = request
            .extract_from_head::<RemoteAddr>()
            .await
            .unwrap()
            .ip();

        assert_eq!(ip, IP_PROXY);
    }

    #[cot::test]
    async fn remote_addr_proxied_configured_f() {
        const IP: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
        const IP_PROXY: IpAddr = IpAddr::V6(Ipv6Addr::new(21, 32, 43, 54, 65, 76, 87, 98));

        let mut config = ProjectConfig::dev_default();

        config.trusted_proxy = ProxyConfig::new(
            vec![IpWithSubnet::new(IP, 128).unwrap()],
            vec![
                "X-Forwarded-For".to_string(),
                "CF-Connecting-IP".to_string(),
            ],
        );

        let mut request = TestRequestBuilder::get("/").config(config).build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP,
            proxied: None,
        });
        request.headers_mut().insert(
            HeaderName::from_str("CF-Connecting-IP").unwrap(),
            HeaderValue::from_str(&format!("{IP_PROXY}")).unwrap(),
        );

        let ip = request
            .extract_from_head::<RemoteAddr>()
            .await
            .unwrap()
            .ip();

        assert_eq!(ip, IP_PROXY);
    }

    #[cot::test]
    async fn remote_addr_proxied_configured_malformed() {
        const IP: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
        const IP_PROXY: IpAddr = IpAddr::V6(Ipv6Addr::new(21, 32, 43, 54, 65, 76, 87, 98));

        let mut config = ProjectConfig::dev_default();

        config.trusted_proxy = ProxyConfig::new(
            vec![IpWithSubnet::new(IP, 128).unwrap()],
            vec![
                "X-Forwarded-For".to_string(),
                "CF-Connecting-IP".to_string(),
            ],
        );

        let mut request = TestRequestBuilder::get("/").config(config).build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP,
            proxied: None,
        });
        request.headers_mut().insert(
            HeaderName::from_str("CF-Connecting-IP").unwrap(),
            HeaderValue::from_str(&format!("AAA{IP_PROXY}")).unwrap(),
        );

        let ip = request.extract_from_head::<RemoteAddr>().await;

        assert!(ip.is_err());
    }
}
