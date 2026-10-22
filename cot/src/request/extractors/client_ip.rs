use std::net::{IpAddr, SocketAddr};

use http::HeaderMap;
use tracing::debug;

use crate::config::{ClientIpConfig, ClientIpHeader};
use crate::request::extractors::FromRequestHead;
use crate::request::{RemoteAddr, RequestExt, RequestHead};

/// An extractor for the IP address of the client.
///
/// If the request comes from one of the trusted proxies configured in
/// [`ClientIpConfig`], the client IP is read from the configured headers.
/// Otherwise, it's the address of the peer that connected to the server.
///
/// # Examples
///
/// ```
/// use cot::html::Html;
/// use cot::request::extractors::ClientIpAddr;
///
/// async fn handler(client_ip: ClientIpAddr) -> Html {
///     Html::new(format!("Your IP is {}", client_ip.ip()))
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientIpAddr {
    ip: IpAddr,
    peer_ip: IpAddr,
}

impl ClientIpAddr {
    /// Returns the IP address of the client, taking trusted proxies into
    /// account.
    #[must_use]
    pub fn ip(&self) -> IpAddr {
        self.ip
    }

    /// Returns the IP address of the peer that connected to the server, which
    /// might be a proxy.
    #[must_use]
    pub fn peer_ip(&self) -> IpAddr {
        self.peer_ip
    }
}

impl FromRequestHead for ClientIpAddr {
    #[expect(clippy::unused_async_trait_impl)]
    async fn from_request_head(head: &RequestHead) -> crate::Result<Self> {
        let RemoteAddr(peer_addr) = head.extensions.get::<RemoteAddr>().ok_or_else(|| {
            crate::Error::internal("the remote address of the request is unknown")
        })?;
        let peer_ip = peer_addr.ip();
        let config = &head.project_config().client_ip;

        Ok(Self {
            ip: client_ip(peer_ip, &head.headers, config),
            peer_ip,
        })
    }
}

fn client_ip(peer_ip: IpAddr, headers: &HeaderMap, config: &ClientIpConfig) -> IpAddr {
    let is_trusted = |ip: &IpAddr| config.proxies.iter().any(|proxy| proxy.contains(ip));
    if !is_trusted(&peer_ip) {
        return peer_ip;
    }

    for header in &config.headers {
        let hops = hops(header, headers);
        if hops.is_empty() {
            continue;
        }

        // Proxies append to the list, so only the right-most entries can be
        // trusted
        let mut ip = peer_ip;
        for hop in hops.into_iter().rev() {
            let Some(hop) = hop else { break };
            ip = hop;
            if !is_trusted(&hop) {
                break;
            }
        }
        return ip;
    }

    debug!("No client IP header in a request from trusted proxy {peer_ip}");
    peer_ip
}

/// Returns the addresses from all instances of a header, in order; `None` if
/// an address is invalid or obfuscated.
fn hops(header: &ClientIpHeader, headers: &HeaderMap) -> Vec<Option<IpAddr>> {
    let values = headers
        .get_all(header.as_str())
        .iter()
        .map(|value| value.to_str().ok());

    match header {
        ClientIpHeader::Forwarded => values
            .flat_map(|value| value.unwrap_or_default().split(','))
            .map(|element| {
                element
                    .split(';')
                    .filter_map(|pair| pair.split_once('='))
                    .find(|(key, _)| key.trim().eq_ignore_ascii_case("for"))
                    .and_then(|(_, node)| parse_node(node.trim().trim_matches('"')))
            })
            .collect(),
        ClientIpHeader::XForwardedFor => values
            .flat_map(|value| value.unwrap_or_default().split(','))
            .map(parse_node)
            .collect(),
        ClientIpHeader::Custom(_) => values.map(|value| value.and_then(parse_node)).collect(),
    }
}

/// Parses an IP address, optionally with a port and/or IPv6 brackets.
fn parse_node(node: &str) -> Option<IpAddr> {
    let node = node.trim();
    node.parse()
        .or_else(|_| node.parse::<SocketAddr>().map(|addr| addr.ip()))
        .or_else(|_| node.trim_start_matches('[').trim_end_matches(']').parse())
        .ok()
}

#[cfg(test)]
mod tests {
    use http::{HeaderName, HeaderValue};

    use super::*;
    use crate::test::TestRequestBuilder;

    const PEER: &str = "10.0.0.1";

    fn config(headers: Vec<ClientIpHeader>) -> ClientIpConfig {
        ClientIpConfig::builder()
            .proxies(vec!["10.0.0.0/8".parse().unwrap()])
            .headers(headers)
            .build()
    }

    fn client_ip_for(peer: &str, config: &ClientIpConfig, headers: &[(&str, &str)]) -> String {
        let mut header_map = HeaderMap::new();
        for (name, value) in headers {
            header_map.append(
                HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        client_ip(peer.parse().unwrap(), &header_map, config).to_string()
    }

    #[test]
    fn x_forwarded_for() {
        let config = config(vec![ClientIpHeader::XForwardedFor]);
        let cases: &[(&str, &[&str], &str)] = &[
            ("203.0.113.1", &["6.6.6.6"], "203.0.113.1"),
            (PEER, &[], PEER),
            (PEER, &["203.0.113.1"], "203.0.113.1"),
            (PEER, &["6.6.6.6, 203.0.113.1"], "203.0.113.1"),
            (PEER, &["6.6.6.6", "203.0.113.1"], "203.0.113.1"),
            (PEER, &["203.0.113.1, 10.0.0.2"], "203.0.113.1"),
            (PEER, &["10.0.0.3, 10.0.0.2"], "10.0.0.3"),
            (PEER, &["garbage, 203.0.113.1"], "203.0.113.1"),
            (PEER, &["203.0.113.1, garbage"], PEER),
            (PEER, &[""], PEER),
            (PEER, &["[2001:db8::1]:4711"], "2001:db8::1"),
        ];

        for (peer, values, expected) in cases {
            let headers: Vec<_> = values.iter().map(|v| ("x-forwarded-for", *v)).collect();
            assert_eq!(
                client_ip_for(peer, &config, &headers),
                *expected,
                "{values:?}"
            );
        }
    }

    #[test]
    fn forwarded() {
        let config = config(vec![ClientIpHeader::Forwarded]);
        let cases: &[(&str, &str)] = &[
            ("for=203.0.113.1;proto=http;by=10.0.0.1", "203.0.113.1"),
            ("for=6.6.6.6, For=\"[2001:db8::1]:4711\"", "2001:db8::1"),
            ("proto=http; for=203.0.113.1:80", "203.0.113.1"),
            ("for=203.0.113.1, for=unknown", PEER),
            ("for=_hidden", PEER),
            ("proto=http", PEER),
        ];

        for (header, expected) in cases {
            assert_eq!(
                client_ip_for(PEER, &config, &[("forwarded", header)]),
                *expected,
                "{header}"
            );
        }
    }

    #[test]
    fn custom_header() {
        let config = config(vec![ClientIpHeader::Custom(HeaderName::from_static(
            "x-real-ip",
        ))]);

        assert_eq!(
            client_ip_for(PEER, &config, &[("x-real-ip", "203.0.113.1")]),
            "203.0.113.1"
        );
    }

    #[test]
    fn first_present_header_is_used() {
        let config = config(vec![
            ClientIpHeader::Forwarded,
            ClientIpHeader::XForwardedFor,
        ]);

        assert_eq!(
            client_ip_for(PEER, &config, &[("x-forwarded-for", "203.0.113.1")]),
            "203.0.113.1"
        );
    }

    #[test]
    fn no_trusted_proxies() {
        let config = ClientIpConfig::default();

        assert_eq!(
            client_ip_for(PEER, &config, &[("x-forwarded-for", "203.0.113.1")]),
            PEER
        );
    }

    #[cot::test]
    async fn extract() {
        let mut request = TestRequestBuilder::get("/")
            .remote_addr("203.0.113.1:4711".parse().unwrap())
            .build();

        let client_ip: ClientIpAddr = request.extract_from_head().await.unwrap();
        assert_eq!(client_ip.ip().to_string(), "203.0.113.1");
        assert_eq!(client_ip.peer_ip().to_string(), "203.0.113.1");
    }

    #[cot::test]
    async fn extract_without_remote_addr() {
        let mut request = TestRequestBuilder::get("/").build();

        assert!(request.extract_from_head::<ClientIpAddr>().await.is_err());
    }
}
