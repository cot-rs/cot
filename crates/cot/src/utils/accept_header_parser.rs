use mime::Mime;

/// Parser for the [`Accept`] HTTP header.
///
/// [`Accept`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Accept
#[derive(Debug, Clone)]
pub(crate) struct AcceptHeaderParser {
    content_types: Vec<ContentType>,
}

impl AcceptHeaderParser {
    #[must_use]
    pub(crate) fn parse(accept_header: &str) -> Self {
        let mut content_types: Vec<ContentType> = accept_header
            .split(',')
            .filter_map(Self::parse_single)
            .collect();
        content_types.sort_by(|a, b| {
            b.weight
                .partial_cmp(&a.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        AcceptHeaderParser { content_types }
    }

    fn parse_single(part: &str) -> Option<ContentType> {
        let part = part.trim();
        if part.is_empty() {
            return None;
        }

        let mut parts = part.split(';');
        let media_type = parts.next()?.trim().parse::<Mime>().ok()?;
        let weight = parts
            .find_map(|p| {
                let p = p.trim();
                if let Some(weight_value) = p.strip_prefix("q=") {
                    weight_value.parse::<f32>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(1.0); // Default weight is 1.0
        let weight = weight.clamp(0.0, 1.0);

        Some(ContentType { media_type, weight })
    }

    pub(crate) fn contains_explicit(&self, media_type: &Mime) -> bool {
        self.content_types
            .iter()
            .any(|ct| ct.media_type == *media_type)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ContentType {
    media_type: Mime,
    weight: f32,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn parse_accept_explicit() {
        let parser = AcceptHeaderParser::parse("text/html, application/json;q=0.9");
        let content_types = parser.content_types;

        assert_eq!(content_types.len(), 2);
        assert_eq!(content_types[0].media_type, mime::TEXT_HTML);
        assert!((content_types[0].weight - 1.0).abs() < 1e-6);
        assert_eq!(content_types[1].media_type, mime::APPLICATION_JSON);
        assert!((content_types[1].weight - 0.9).abs() < 1e-6);
    }

    #[test]
    fn parse_accept_invalid() {
        let parser = AcceptHeaderParser::parse(", , ;q=0.9, text/html;q=-1.0, image/png;q=20");
        let content_types = parser.content_types;

        assert_eq!(content_types.len(), 2);
        assert_eq!(content_types[0].media_type, mime::IMAGE_PNG);
        assert!((content_types[0].weight - 1.0).abs() < 1e-6);
        assert_eq!(content_types[1].media_type, mime::TEXT_HTML);
        assert!((content_types[1].weight - 0.0).abs() < 1e-6);
    }

    #[test]
    fn parse_accept_and_sort() {
        let parser = AcceptHeaderParser::parse(
            "text/html, application/xhtml+xml, application/xml;q=0.9, image/webp, */*;q=0.8",
        );
        let content_types = parser.content_types;

        assert_eq!(content_types.len(), 5);
        assert_eq!(content_types[0].media_type, mime::TEXT_HTML);
        assert!((content_types[0].weight - 1.0).abs() < 1e-6);
        assert_eq!(
            content_types[1].media_type,
            Mime::from_str("application/xhtml+xml").unwrap()
        );
        assert!((content_types[1].weight - 1.0).abs() < 1e-6);
        assert_eq!(
            content_types[2].media_type,
            Mime::from_str("image/webp").unwrap()
        );
        assert!((content_types[2].weight - 1.0).abs() < 1e-6);
        assert_eq!(
            content_types[3].media_type,
            Mime::from_str("application/xml").unwrap()
        );
        assert!((content_types[3].weight - 0.9).abs() < 1e-6);
        assert_eq!(content_types[4].media_type, Mime::from_str("*/*").unwrap());
        assert!((content_types[4].weight - 0.8).abs() < 1e-6);
    }

    #[test]
    fn parse_contains_explicit() {
        let parser = AcceptHeaderParser::parse(
            "text/html, application/xhtml+xml, application/xml;q=0.9, image/webp, */*;q=0.8",
        );

        assert!(parser.contains_explicit(&mime::TEXT_HTML));
        assert!(parser.contains_explicit(&Mime::from_str("application/xhtml+xml").unwrap()));
        assert!(parser.contains_explicit(&Mime::from_str("application/xml").unwrap()));
        assert!(parser.contains_explicit(&Mime::from_str("image/webp").unwrap()));
        // should return false even if we have a wildcard
        assert!(!parser.contains_explicit(&mime::APPLICATION_JAVASCRIPT));
        assert!(!parser.contains_explicit(&mime::IMAGE_PNG));
        assert!(!parser.contains_explicit(&mime::TEXT_XML));
    }
}
