use std::net::IpAddr;

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
        // Dual-stack listeners report IPv4 peers as IPv4-mapped IPv6 addresses
        let peer_ip = peer_addr.ip().to_canonical();
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
            .flat_map(|value| split_unquoted(value.unwrap_or_default(), ','))
            .map(parse_forwarded_element)
            .collect(),
        ClientIpHeader::XForwardedFor => values
            .flat_map(|value| value.unwrap_or_default().split(','))
            .map(parse_node)
            .collect(),
        ClientIpHeader::Custom(_) => values.map(|value| value.and_then(parse_node)).collect(),
    }
}

/// Splits a header value on a delimiter, except inside quoted strings.
fn split_unquoted(value: &str, delimiter: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    let mut escaped = false;
    for (i, c) in value.char_indices() {
        if escaped {
            escaped = false;
        } else if in_quotes && c == '\\' {
            escaped = true;
        } else if c == '"' {
            in_quotes = !in_quotes;
        } else if !in_quotes && c == delimiter {
            parts.push(&value[start..i]);
            start = i + c.len_utf8();
        }
    }
    parts.push(&value[start..]);
    parts
}

/// Returns the `for` address of a `Forwarded` element; `None` if the element
/// is malformed, as it might be the client's prefix of a proxy-set value.
fn parse_forwarded_element(element: &str) -> Option<IpAddr> {
    let mut node = None;
    for pair in split_unquoted(element, ';') {
        if pair.trim().is_empty() {
            continue;
        }
        let (key, value) = pair.split_once('=')?;
        let key = key.trim();
        let value = unquote(value.trim())?;
        if !is_token(key) {
            return None;
        }
        // Parameters must not be repeated within an element
        if key.eq_ignore_ascii_case("for") && node.replace(value).is_some() {
            return None;
        }
    }
    parse_node(&node?)
}

/// Parses a parameter value: either a quoted string, with escapes resolved, or
/// an unquoted value.
fn unquote(value: &str) -> Option<String> {
    let Some(quoted) = value.strip_prefix('"') else {
        // Lenient, as some proxies don't quote values containing `:` or `[]`
        let is_valid = !value.is_empty()
            && !value
                .chars()
                .any(|c| c == '"' || c == '\\' || c.is_whitespace() || c.is_control());
        return is_valid.then(|| value.to_owned());
    };

    let mut result = String::new();
    let mut chars = quoted.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => result.push(chars.next()?),
            '"' => return chars.as_str().is_empty().then_some(result),
            _ => result.push(c),
        }
    }
    None
}

/// Checks if a string is a valid HTTP token, as defined by RFC 9110.
fn is_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(c))
}

/// Parses an IP address, optionally with a port and/or IPv6 brackets, and
/// converts IPv4-mapped IPv6 addresses to IPv4.
fn parse_node(node: &str) -> Option<IpAddr> {
    let node = node.trim();
    let ip = if let Some(rest) = node.strip_prefix('[') {
        let (ip, port) = rest.split_once(']')?;
        if !port.is_empty() && !is_valid_port(port.strip_prefix(':')?) {
            return None;
        }
        IpAddr::V6(ip.parse().ok()?)
    } else if let Ok(ip) = node.parse() {
        // Bare IPv6 addresses are common in `X-Forwarded-For`
        ip
    } else {
        let (ip, port) = node.rsplit_once(':')?;
        if !is_valid_port(port) {
            return None;
        }
        IpAddr::V4(ip.parse().ok()?)
    };
    Some(ip.to_canonical())
}

/// Checks if a port is either numeric or obfuscated, as defined by RFC 7239.
fn is_valid_port(port: &str) -> bool {
    if let Some(obfuscated) = port.strip_prefix('_') {
        !obfuscated.is_empty()
            && obfuscated
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    } else {
        port.len() <= 5 && port.chars().all(|c| c.is_ascii_digit()) && port.parse::<u16>().is_ok()
    }
}

#[cfg(test)]
mod tests {
    use http::{HeaderName, HeaderValue};

    use super::*;
    use crate::config::ProjectConfig;
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
            (PEER, &["::ffff:203.0.113.1"], "203.0.113.1"),
            (PEER, &["203.0.113.1, ::ffff:10.0.0.2"], "203.0.113.1"),
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
        let cases: &[(&[&str], &str)] = &[
            (&["for=203.0.113.1;proto=http;by=10.0.0.1"], "203.0.113.1"),
            (&["for=6.6.6.6, For=\"[2001:db8::1]:4711\""], "2001:db8::1"),
            (&["proto=http; for=203.0.113.1:80"], "203.0.113.1"),
            (&["for=\"203.0.113.1\""], "203.0.113.1"),
            (&["for=203.0.113.1, for=unknown"], PEER),
            (&["for=_hidden"], PEER),
            (&["proto=http"], PEER),
            // Delimiters inside quoted strings
            (&["for=203.0.113.1;ext=\"a,for=6.6.6.6\""], "203.0.113.1"),
            (&["ext=\"a;for=6.6.6.6\";for=203.0.113.1"], "203.0.113.1"),
            (
                &["for=203.0.113.1;ext=\"a\\\",for=6.6.6.6\""],
                "203.0.113.1",
            ),
            // Malformed elements
            (&["for=6.6.6.6;ext=\", for=203.0.113.1"], PEER),
            (&["for=\"6.6.6.6, for=203.0.113.1"], PEER),
            (&["for=\"203.0.113.1"], PEER),
            (&["for=6.6.6.6;for=203.0.113.1"], PEER),
            (&["for=6.6.6.6;ext=\"", "for=203.0.113.1"], "203.0.113.1"),
            // Obfuscated ports
            (&["for=\"203.0.113.1:_hidden\""], "203.0.113.1"),
            (&["for=\"[2001:db8::1]:_hidden\""], "2001:db8::1"),
            (&["for=\"[2001:db8::1]:_a.b-c_1\""], "2001:db8::1"),
            // Malformed nodes
            (&["for=\"203.0.113.1:_\""], PEER),
            (&["for=\"203.0.113.1:_bad!\""], PEER),
            (&["for=\"203.0.113.1:99999\""], PEER),
            (&["for=\"203.0.113.1:\""], PEER),
            (&["for=\"[203.0.113.1]\""], PEER),
            (&["for=\"[2001:db8::1\""], PEER),
            (&["for=\"[2001:db8::1]x\""], PEER),
        ];

        for (values, expected) in cases {
            let headers: Vec<_> = values.iter().map(|v| ("forwarded", *v)).collect();
            assert_eq!(
                client_ip_for(PEER, &config, &headers),
                *expected,
                "{values:?}"
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
    async fn extract_ipv4_mapped_peer() {
        let mut request = TestRequestBuilder::get("/")
            .remote_addr("[::ffff:203.0.113.1]:4711".parse().unwrap())
            .build();

        let client_ip: ClientIpAddr = request.extract_from_head().await.unwrap();
        assert_eq!(client_ip.ip().to_string(), "203.0.113.1");
        assert_eq!(client_ip.peer_ip().to_string(), "203.0.113.1");
    }

    #[cot::test]
    async fn extract_ipv4_mapped_trusted_peer() {
        let mut request = TestRequestBuilder::get("/")
            .config(
                ProjectConfig::builder()
                    .client_ip(config(vec![ClientIpHeader::XForwardedFor]))
                    .build(),
            )
            .remote_addr("[::ffff:10.0.0.1]:4711".parse().unwrap())
            .build();
        request
            .headers_mut()
            .insert("x-forwarded-for", HeaderValue::from_static("203.0.113.1"));

        let client_ip: ClientIpAddr = request.extract_from_head().await.unwrap();
        assert_eq!(client_ip.ip().to_string(), "203.0.113.1");
        assert_eq!(client_ip.peer_ip().to_string(), PEER);
    }

    #[cot::test]
    async fn extract_without_remote_addr() {
        let mut request = TestRequestBuilder::get("/").build();

        assert!(request.extract_from_head::<ClientIpAddr>().await.is_err());
    }
}
