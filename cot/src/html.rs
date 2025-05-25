//! HTML rendering utilities.
//!
//! This module provides structures and methods for creating and rendering HTML
//! content with support for nested elements and text nodes.
//!
//! # Examples
//!
//! ## Creating and rendering an HTML Tag
//!
//! ```
//! use cot::html::HtmlTag;
//!
//! let tag = HtmlTag::new("br");
//! let html = tag.render();
//! assert_eq!(html.as_str(), "<br/>");
//! ```
//!
//! ## Adding Attributes to an HTML Tag
//!
//! ```
//! use cot::html::HtmlTag;
//!
//! let mut tag = HtmlTag::new("input");
//! tag.attr("type", "text").attr("placeholder", "Enter text");
//! tag.bool_attr("disabled");
//! assert_eq!(
//!     tag.render().as_str(),
//!     "<input type=\"text\" placeholder=\"Enter text\" disabled/>"
//! );
//! ```
//!
//! ## Creating nested HTML elements
//!
//! ```
//! use cot::html::{Html, HtmlNode, HtmlTag, HtmlText};
//!
//! let mut div = HtmlTag::new("div");
//! div.attr("class", "container");
//! div.child(HtmlNode::Text(HtmlText::new("Hello, ")));
//!
//! let mut span = HtmlTag::new("span");
//! span.attr("class", "highlight");
//! span.child(HtmlNode::Text(HtmlText::new("world!")));
//! div.child(HtmlNode::Tag(span));
//!
//! let html = div.render();
//! assert_eq!(
//!     html.as_str(),
//!     "<div class=\"container\">Hello, <span class=\"highlight\">world!</span></div>"
//! );
//! ```

use std::fmt::Write;

use askama::filters::Escaper;
use derive_more::{Deref, Display, From};

/// A type that represents HTML content as a string.
///
/// This type can contain nested HTML elements and text nodes, providing a
/// hierarchical structure for HTML content. It automatically handles HTML
/// escaping for text content while preserving the structure of nested elements.
///
/// # Examples
///
/// ```
/// use cot::html::Html;
///
/// let html = Html::new("<div>Hello</div>");
/// assert_eq!(html.as_str(), "<div>Hello</div>");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Deref, From, Display)]
pub struct Html(pub String);

impl Html {
    /// Creates a new `Html` instance from a string.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::Html;
    ///
    /// let html = Html::new("<div>Hello</div>");
    /// assert_eq!(html.as_str(), "<div>Hello</div>");
    /// ```
    #[must_use]
    pub fn new<T: Into<String>>(html: T) -> Self {
        Self(html.into())
    }

    /// Returns the inner string as a `&str`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::Html;
    ///
    /// let html = Html::new("<div>Hello</div>");
    /// assert_eq!(html.as_str(), "<div>Hello</div>");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.as_ref()
    }
}

impl AsRef<str> for Html {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Represents a text node in HTML content.
///
/// Text nodes contain plain text that will be automatically escaped when
/// rendered to prevent XSS attacks. This ensures that any special HTML
/// characters in the text are properly encoded.
///
/// # Examples
///
/// ```
/// use cot::html::HtmlText;
///
/// let text = HtmlText::new("Hello & welcome to <our> site!");
/// assert_eq!(
///     text.render().as_str(),
///     "Hello &#38; welcome to &#60;our&#62; site!"
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlText {
    content: String,
}

impl HtmlText {
    /// Creates a new `HtmlText` instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::HtmlText;
    ///
    /// let text = HtmlText::new("Hello, world!");
    /// assert_eq!(text.content(), "Hello, world!");
    /// ```
    #[must_use]
    pub fn new<T: Into<String>>(content: T) -> Self {
        Self {
            content: content.into(),
        }
    }

    /// Returns the text content.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::HtmlText;
    ///
    /// let text = HtmlText::new("Hello, world!");
    /// assert_eq!(text.content(), "Hello, world!");
    /// ```
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Renders the text node as escaped HTML.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::HtmlText;
    ///
    /// let text = HtmlText::new("Hello & <world>");
    /// assert_eq!(text.render().as_str(), "Hello &#38; &#60;world&#62;");
    /// ```
    #[must_use]
    pub fn render(&self) -> Html {
        let mut result = String::new();
        askama::filters::Html
            .write_escaped_str(&mut result, &self.content)
            .expect("Failed to escape HTML text");
        Html(result)
    }
}

/// Represents a node in the HTML tree structure.
///
/// An HTML node can be either an HTML tag with attributes and children,
/// or a text node containing plain text content.
///
/// # Examples
///
/// ```
/// use cot::html::{HtmlNode, HtmlTag, HtmlText};
///
/// let text_node = HtmlNode::Text(HtmlText::new("Hello"));
/// let tag_node = HtmlNode::Tag(HtmlTag::new("div"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlNode {
    /// An HTML tag with attributes and potential children.
    Tag(HtmlTag),
    /// A text node containing plain text content.
    Text(HtmlText),
}

impl HtmlNode {
    /// Renders the HTML node to a string.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::{HtmlNode, HtmlText};
    ///
    /// let node = HtmlNode::Text(HtmlText::new("Hello"));
    /// assert_eq!(node.render().as_str(), "Hello");
    /// ```
    #[must_use]
    pub fn render(&self) -> Html {
        match self {
            HtmlNode::Tag(tag) => tag.render(),
            HtmlNode::Text(text) => text.render(),
        }
    }
}

impl From<HtmlTag> for HtmlNode {
    fn from(tag: HtmlTag) -> Self {
        HtmlNode::Tag(tag)
    }
}

impl From<HtmlText> for HtmlNode {
    fn from(text: HtmlText) -> Self {
        HtmlNode::Text(text)
    }
}

/// A helper struct for rendering HTML tags with support for nested content.
///
/// This struct is used to build HTML tags with attributes, boolean attributes,
/// and child nodes. It automatically escapes all attribute values and properly
/// renders nested content.
///
/// # Examples
///
/// ```
/// use cot::html::{HtmlNode, HtmlTag, HtmlText};
///
/// let mut tag = HtmlTag::new("div");
/// tag.attr("class", "container");
/// tag.child(HtmlNode::Text(HtmlText::new("Hello, world!")));
/// assert_eq!(
///     tag.render().as_str(),
///     "<div class=\"container\">Hello, world!</div>"
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlTag {
    tag: String,
    attributes: Vec<(String, String)>,
    boolean_attributes: Vec<String>,
    children: Vec<HtmlNode>,
}

impl HtmlTag {
    /// Creates a new `HtmlTag` instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::HtmlTag;
    ///
    /// let tag = HtmlTag::new("div");
    /// assert_eq!(tag.render().as_str(), "<div/>");
    /// ```
    #[must_use]
    pub fn new(tag: &str) -> Self {
        Self {
            tag: tag.to_string(),
            attributes: Vec::new(),
            boolean_attributes: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Creates a new `HtmlTag` instance for an input element.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::HtmlTag;
    ///
    /// let input = HtmlTag::input("text");
    /// assert_eq!(input.render().as_str(), "<input type=\"text\"/>");
    /// ```
    #[must_use]
    pub fn input(input_type: &str) -> Self {
        let mut input = Self::new("input");
        input.attr("type", input_type);
        input
    }

    /// Adds an attribute to the HTML tag.
    ///
    /// # Safety
    ///
    /// This function will escape the attribute value. Note that it does not
    /// escape the attribute name.
    ///
    /// # Panics
    ///
    /// This function will panic if the attribute already exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::HtmlTag;
    ///
    /// let mut tag = HtmlTag::new("input");
    /// tag.attr("type", "text").attr("placeholder", "Enter text");
    /// assert_eq!(
    ///     tag.render().as_str(),
    ///     "<input type=\"text\" placeholder=\"Enter text\"/>"
    /// );
    /// ```
    pub fn attr<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) -> &mut Self {
        let key = key.into();
        assert!(
            !self.attributes.iter().any(|(k, _)| k == &key),
            "Attribute already exists: {key}"
        );
        self.attributes.push((key, value.into()));
        self
    }

    /// Adds a boolean attribute to the HTML tag.
    ///
    /// # Safety
    ///
    /// This function will not escape the attribute name.
    ///
    /// # Panics
    ///
    /// This function will panic if the boolean attribute already exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::HtmlTag;
    ///
    /// let mut tag = HtmlTag::new("input");
    /// tag.bool_attr("disabled");
    /// assert_eq!(tag.render().as_str(), "<input disabled/>");
    /// ```
    pub fn bool_attr(&mut self, key: &str) -> &mut Self {
        assert!(
            !self.boolean_attributes.contains(&key.to_string()),
            "Boolean attribute already exists: {key}"
        );
        self.boolean_attributes.push(key.to_string());
        self
    }

    /// Adds a child node to the HTML tag.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::{HtmlNode, HtmlTag, HtmlText};
    ///
    /// let mut div = HtmlTag::new("div");
    /// div.child(HtmlNode::Text(HtmlText::new("Hello")));
    /// assert_eq!(div.render().as_str(), "<div>Hello</div>");
    /// ```
    pub fn child(&mut self, node: HtmlNode) -> &mut Self {
        self.children.push(node);
        self
    }

    /// Adds multiple child nodes to the HTML tag.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::{HtmlNode, HtmlTag, HtmlText};
    ///
    /// let mut div = HtmlTag::new("div");
    /// div.children(vec![
    ///     HtmlNode::Text(HtmlText::new("Hello")),
    ///     HtmlNode::Text(HtmlText::new(" world")),
    /// ]);
    /// assert_eq!(div.render().as_str(), "<div>Hello world</div>");
    /// ```
    pub fn children(&mut self, nodes: Vec<HtmlNode>) -> &mut Self {
        self.children.extend(nodes);
        self
    }

    /// Adds a text child to the HTML tag.
    ///
    /// This is a convenience method for adding text content without
    /// manually creating an `HtmlText` and `HtmlNode`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::HtmlTag;
    ///
    /// let mut div = HtmlTag::new("div");
    /// div.push_str("Hello, world!");
    /// assert_eq!(div.render().as_str(), "<div>Hello, world!</div>");
    /// ```
    pub fn push_str<T: Into<String>>(&mut self, content: T) -> &mut Self {
        self.child(HtmlNode::Text(HtmlText::new(content)))
    }

    pub fn push_tag<T: Into<HtmlTag>>(&mut self, tag: T) -> &mut Self {
        self.child(HtmlNode::Tag(tag.into()))
    }

    /// Renders the HTML tag.
    ///
    /// # Panics
    ///
    /// Panics if the [`String`] writer fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::HtmlTag;
    ///
    /// let tag = HtmlTag::new("div");
    /// assert_eq!(tag.render().as_str(), "<div></div>");
    /// ```
    #[must_use]
    pub fn render(&self) -> Html {
        const FAIL_MSG: &str = "Failed to write HTML tag";

        let mut result = String::new();
        write!(&mut result, "<{}", self.tag).expect(FAIL_MSG);

        for (key, value) in &self.attributes {
            write!(&mut result, " {key}=\"").expect(FAIL_MSG);
            askama::filters::Html
                .write_escaped_str(&mut result, value)
                .expect(FAIL_MSG);
            write!(&mut result, "\"").expect(FAIL_MSG);
        }
        for key in &self.boolean_attributes {
            write!(&mut result, " {key}").expect(FAIL_MSG);
        }

        if self.children.is_empty() {
            write!(&mut result, "/>").expect(FAIL_MSG);
        } else {
            write!(&mut result, ">").expect(FAIL_MSG);

            for child in &self.children {
                write!(&mut result, "{}", child.render().as_str()).expect(FAIL_MSG);
            }

            write!(&mut result, "</{}>", self.tag).expect(FAIL_MSG);
        }

        result.into()
    }
}

impl From<&HtmlTag> for HtmlTag {
    fn from(value: &HtmlTag) -> Self {
        value.clone()
    }
}

impl From<&mut HtmlTag> for HtmlTag {
    fn from(value: &mut HtmlTag) -> Self {
        value.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_new() {
        let html = Html::new("<div>Hello</div>");
        assert_eq!(html.as_str(), "<div>Hello</div>");
    }

    #[test]
    fn test_html_text_new() {
        let text = HtmlText::new("Hello, world!");
        assert_eq!(text.content(), "Hello, world!");
    }

    #[test]
    fn test_html_text_render() {
        let text = HtmlText::new("Hello, world!");
        assert_eq!(text.render().as_str(), "Hello, world!");
    }

    #[test]
    fn test_html_text_escaping() {
        let text = HtmlText::new("Hello & <world> \"test\"");
        assert_eq!(
            text.render().as_str(),
            "Hello &#38; &#60;world&#62; &#34;test&#34;"
        );
    }

    #[test]
    fn test_html_node_text() {
        let node = HtmlNode::Text(HtmlText::new("Hello"));
        assert_eq!(node.render().as_str(), "Hello");
    }

    #[test]
    fn test_html_node_tag() {
        let tag = HtmlTag::new("div");
        let node = HtmlNode::Tag(tag);
        assert_eq!(node.render().as_str(), "<div/>");
    }

    #[test]
    fn test_html_node_from_text() {
        let text = HtmlText::new("Hello");
        let node: HtmlNode = text.into();
        assert_eq!(node.render().as_str(), "Hello");
    }

    #[test]
    fn test_html_node_from_tag() {
        let tag = HtmlTag::new("div");
        let node: HtmlNode = tag.into();
        assert_eq!(node.render().as_str(), "<div/>");
    }

    #[test]
    fn test_html_tag_new() {
        let tag = HtmlTag::new("div");
        assert_eq!(tag.render().as_str(), "<div/>");
    }

    #[test]
    fn test_html_tag_with_attributes() {
        let mut tag = HtmlTag::new("input");
        tag.attr("type", "text").attr("placeholder", "Enter text");
        assert_eq!(
            tag.render().as_str(),
            "<input type=\"text\" placeholder=\"Enter text\"/>"
        );
    }

    #[test]
    fn test_html_tag_escaping() {
        let mut tag = HtmlTag::new("input");
        tag.attr("type", "text").attr("placeholder", "<>&\"'");
        assert_eq!(
            tag.render().as_str(),
            "<input type=\"text\" placeholder=\"&#60;&#62;&#38;&#34;&#39;\"/>"
        );
    }

    #[test]
    fn test_html_tag_with_boolean_attributes() {
        let mut tag = HtmlTag::new("input");
        tag.bool_attr("disabled");
        assert_eq!(tag.render().as_str(), "<input disabled/>");
    }

    #[test]
    fn test_html_tag_input() {
        let mut input = HtmlTag::input("text");
        input.attr("name", "username");
        assert_eq!(
            input.render().as_str(),
            "<input type=\"text\" name=\"username\"/>"
        );
    }

    #[test]
    fn test_html_tag_children() {
        let mut div = HtmlTag::new("div");
        div.child(HtmlNode::Text(HtmlText::new("Hello")));
        assert_eq!(div.render().as_str(), "<div>Hello</div>");
    }

    #[test]
    fn test_html_tag_children_multiple() {
        let mut div = HtmlTag::new("div");
        div.children(vec![
            HtmlNode::Text(HtmlText::new("Hello")),
            HtmlNode::Text(HtmlText::new(" world")),
        ]);
        assert_eq!(div.render().as_str(), "<div>Hello world</div>");
    }

    #[test]
    fn test_html_tag_text() {
        let mut div = HtmlTag::new("div");
        div.push_str("Hello, world!");
        assert_eq!(div.render().as_str(), "<div>Hello, world!</div>");
    }

    #[test]
    fn test_html_tag_nested_structure() {
        let mut div = HtmlTag::new("div");
        div.attr("class", "container");
        div.push_str("Hello, ");

        let mut span = HtmlTag::new("span");
        span.attr("class", "highlight");
        span.push_str("world!");
        div.child(HtmlNode::Tag(span));

        assert_eq!(
            div.render().as_str(),
            "<div class=\"container\">Hello, <span class=\"highlight\">world!</span></div>"
        );
    }

    #[test]
    fn test_html_tag_deeply_nested() {
        let mut outer = HtmlTag::new("div");
        outer.attr("id", "outer");

        let mut middle = HtmlTag::new("div");
        middle.attr("id", "middle");

        let mut inner = HtmlTag::new("span");
        inner.attr("id", "inner");
        inner.push_str("Deep content");

        middle.child(HtmlNode::Tag(inner));
        outer.child(HtmlNode::Tag(middle));

        assert_eq!(
            outer.render().as_str(),
            "<div id=\"outer\"><div id=\"middle\"><span id=\"inner\">Deep content</span></div></div>"
        );
    }

    #[test]
    fn test_html_tag_mixed_content() {
        let mut div = HtmlTag::new("div");
        div.push_str("Start ");

        let mut em = HtmlTag::new("em");
        em.push_str("emphasized");
        div.child(HtmlNode::Tag(em));

        div.push_str(" middle ");

        let mut strong = HtmlTag::new("strong");
        strong.push_str("bold");
        div.child(HtmlNode::Tag(strong));

        div.push_str(" end");

        assert_eq!(
            div.render().as_str(),
            "<div>Start <em>emphasized</em> middle <strong>bold</strong> end</div>"
        );
    }

    #[test]
    fn test_html_tag_text_escaping_in_children() {
        let mut div = HtmlTag::new("div");
        div.push_str("Safe content & <unsafe> content");
        assert_eq!(
            div.render().as_str(),
            "<div>Safe content &#38; &#60;unsafe&#62; content</div>"
        );
    }

    #[test]
    fn test_self_closing_tags() {
        // self-closing tags are not handled differently in this implementation
        // to be conformant both with HTML and XHTML
        let br = HtmlTag::new("br");
        assert_eq!(br.render().as_str(), "<br/>");

        let mut img = HtmlTag::new("img");
        img.attr("src", "image.jpg").attr("alt", "An image");
        assert_eq!(
            img.render().as_str(),
            "<img src=\"image.jpg\" alt=\"An image\"/>"
        );

        let mut input = HtmlTag::new("input");
        input.attr("type", "text").bool_attr("required");
        assert_eq!(input.render().as_str(), "<input type=\"text\" required/>");
    }
}
