//! A small reader for `dioxus-ssr`'s output, and the two questions the
//! accessibility checks ask of it: what is this element's role, and what is its
//! accessible name (RFC-050 slice 1, level 2).
//!
//! It is not a browser. It reads well-formed markup the renderer emits (elements,
//! quoted or bare attributes, text, comments, void elements), and implements the
//! parts of ARIA's role and accessible-name rules that bekoedit's own markup
//! relies on: an explicit `role`, else the tag's implicit role; `aria-labelledby`,
//! then `aria-label`, then a wrapping `<label>`, then the element's own text, then
//! `title`; `aria-hidden` subtrees contribute nothing. Anything it does not know,
//! it does not guess: an unknown tag has no role.

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Node {
    Element(Element),
    Text(String),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Element {
    pub(crate) tag: String,
    pub(crate) attrs: Vec<(String, String)>,
    pub(crate) children: Vec<Node>,
}

const VOID: &[&str] = &["input", "br", "hr", "img", "meta", "link", "source", "wbr"];

fn unescape(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Parses `html` into its top-level nodes. Panics, naming the offset, on markup it
/// cannot read: a test must not silently check half a page.
pub(crate) fn parse(html: &str) -> Vec<Node> {
    let bytes = html.as_bytes();
    let mut stack: Vec<Element> = vec![Element {
        tag: "#root".into(),
        attrs: vec![],
        children: vec![],
    }];
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            let end = html[i..].find('<').map_or(bytes.len(), |n| i + n);
            let text = unescape(&html[i..end]);
            if !text.is_empty() {
                stack.last_mut().unwrap().children.push(Node::Text(text));
            }
            i = end;
        } else if html[i..].starts_with("<!--") {
            let close = html[i..]
                .find("-->")
                .unwrap_or_else(|| panic!("unterminated comment at {i}"));
            i += close + 3;
        } else if html[i..].starts_with("</") {
            let end = html[i..]
                .find('>')
                .unwrap_or_else(|| panic!("unterminated end tag at {i}"))
                + i;
            let name = html[i + 2..end].trim().to_ascii_lowercase();
            let closed = stack
                .pop()
                .unwrap_or_else(|| panic!("stray </{name}> at {i}"));
            assert_eq!(
                closed.tag, name,
                "mismatched </{name}> at {i}, open element was <{}>",
                closed.tag
            );
            stack
                .last_mut()
                .unwrap()
                .children
                .push(Node::Element(closed));
            i = end + 1;
        } else {
            let (tag, attrs, self_closed, next) = read_start_tag(html, i);
            let element = Element {
                tag: tag.clone(),
                attrs,
                children: vec![],
            };
            if self_closed || VOID.contains(&tag.as_str()) {
                stack
                    .last_mut()
                    .unwrap()
                    .children
                    .push(Node::Element(element));
            } else {
                stack.push(element);
            }
            i = next;
        }
    }
    assert_eq!(stack.len(), 1, "unclosed <{}>", stack.last().unwrap().tag);
    stack.pop().unwrap().children
}

fn read_start_tag(html: &str, start: usize) -> (String, Vec<(String, String)>, bool, usize) {
    let bytes = html.as_bytes();
    let mut i = start + 1;
    let name_end = html[i..]
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .map_or(html.len(), |n| i + n);
    let tag = html[i..name_end].to_ascii_lowercase();
    assert!(!tag.is_empty(), "empty tag name at {start}");
    i = name_end;
    let mut attrs = Vec::new();
    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        assert!(i < bytes.len(), "unterminated <{tag}> at {start}");
        match bytes[i] {
            b'>' => return (tag, attrs, false, i + 1),
            b'/' if bytes.get(i + 1) == Some(&b'>') => return (tag, attrs, true, i + 2),
            _ => {
                let end = html[i..]
                    .find(|c: char| c.is_whitespace() || c == '=' || c == '>' || c == '/')
                    .map_or(html.len(), |n| i + n);
                let name = html[i..end].to_ascii_lowercase();
                assert!(
                    !name.is_empty(),
                    "cannot read an attribute of <{tag}> at {i}"
                );
                i = end;
                let mut value = String::new();
                if bytes.get(i) == Some(&b'=') {
                    i += 1;
                    if matches!(bytes.get(i), Some(b'"') | Some(b'\'')) {
                        let quote = bytes[i] as char;
                        let close = html[i + 1..]
                            .find(quote)
                            .unwrap_or_else(|| panic!("unterminated attribute {name} at {i}"))
                            + i
                            + 1;
                        value = unescape(&html[i + 1..close]);
                        i = close + 1;
                    } else {
                        let end = html[i..]
                            .find(|c: char| c.is_whitespace() || c == '>')
                            .map_or(html.len(), |n| i + n);
                        value = unescape(&html[i..end]);
                        i = end;
                    }
                }
                attrs.push((name, value));
            }
        }
    }
}

impl Element {
    pub(crate) fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    pub(crate) fn has_class(&self, class: &str) -> bool {
        self.attr("class")
            .is_some_and(|c| c.split_whitespace().any(|x| x == class))
    }

    fn hidden(&self) -> bool {
        self.attr("aria-hidden") == Some("true") || self.attr("hidden").is_some()
    }

    /// This element and every descendant, in document order.
    pub(crate) fn descendants(&self) -> Vec<&Element> {
        let mut out = vec![self];
        for child in &self.children {
            if let Node::Element(e) = child {
                out.extend(e.descendants());
            }
        }
        out
    }

    /// The ARIA role: the first token of an explicit `role`, else the tag's implicit one.
    pub(crate) fn role(&self) -> Option<String> {
        if let Some(role) = self.attr("role").and_then(|r| r.split_whitespace().next()) {
            return Some(role.to_string());
        }
        let implicit = match self.tag.as_str() {
            "button" => "button",
            "nav" => "navigation",
            "main" => "main",
            "aside" => "complementary",
            "footer" => "contentinfo",
            "header" => "banner",
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => "heading",
            "textarea" => "textbox",
            "select" => "combobox",
            "table" => "table",
            "tr" => "row",
            "td" => "cell",
            "th" => "columnheader",
            "ul" | "ol" => "list",
            "li" => "listitem",
            "a" if self.attr("href").is_some() => "link",
            "input" => match self.attr("type").unwrap_or("text") {
                "checkbox" => "checkbox",
                "number" => "spinbutton",
                "button" | "submit" | "reset" => "button",
                _ => "textbox",
            },
            _ => return None,
        };
        Some(implicit.to_string())
    }
}

/// Text contributed by `node` to a name computed from content: hidden subtrees
/// give nothing, and a nested form control is skipped (a label's text is not its
/// control's value).
fn content_text(node: &Node, out: &mut String) {
    match node {
        Node::Text(t) => out.push_str(t),
        Node::Element(e)
            if e.hidden() || matches!(e.tag.as_str(), "input" | "select" | "textarea") => {}
        Node::Element(e) => {
            if let Some(label) = e.attr("aria-label").filter(|l| !l.trim().is_empty()) {
                out.push_str(label);
            } else {
                for child in &e.children {
                    content_text(child, out);
                }
            }
        }
    }
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The roles whose name may come from their own content.
const NAME_FROM_CONTENT: &[&str] = &[
    "button",
    "tab",
    "treeitem",
    "menuitem",
    "link",
    "heading",
    "cell",
    "columnheader",
    "option",
    "checkbox",
    "row",
];

/// The accessible name of `element` within `page` (the whole parsed document, for
/// `aria-labelledby` and wrapping `<label>`s), or `None` if it has none.
pub(crate) fn accessible_name(page: &[Node], element: &Element) -> Option<String> {
    if element.hidden() {
        return None;
    }
    let all: Vec<&Element> = page
        .iter()
        .filter_map(|n| {
            if let Node::Element(e) = n {
                Some(e)
            } else {
                None
            }
        })
        .flat_map(Element::descendants)
        .collect();
    let non_empty = |s: String| (!s.trim().is_empty()).then(|| collapse(&s));
    if let Some(ids) = element.attr("aria-labelledby") {
        let mut text = String::new();
        for id in ids.split_whitespace() {
            if let Some(target) = all.iter().find(|e| e.attr("id") == Some(id)) {
                for child in &target.children {
                    content_text(child, &mut text);
                }
                text.push(' ');
            }
        }
        if let Some(name) = non_empty(text) {
            return Some(name);
        }
    }
    if let Some(name) = element
        .attr("aria-label")
        .map(str::to_string)
        .and_then(non_empty)
    {
        return Some(name);
    }
    if matches!(element.tag.as_str(), "input" | "select" | "textarea") {
        let wrapping = all.iter().find(|candidate| {
            candidate.tag == "label"
                && candidate
                    .descendants()
                    .iter()
                    .any(|d| std::ptr::eq(*d, element))
        });
        if let Some(label) = wrapping {
            let mut text = String::new();
            for child in &label.children {
                content_text(child, &mut text);
            }
            if let Some(name) = non_empty(text) {
                return Some(name);
            }
        }
    } else if element
        .role()
        .is_some_and(|r| NAME_FROM_CONTENT.contains(&r.as_str()))
    {
        let mut text = String::new();
        for child in &element.children {
            content_text(child, &mut text);
        }
        if let Some(name) = non_empty(text) {
            return Some(name);
        }
    }
    element
        .attr("title")
        .map(str::to_string)
        .and_then(non_empty)
}

/// Every element of `page`, in document order.
pub(crate) fn elements(page: &[Node]) -> Vec<&Element> {
    page.iter()
        .filter_map(|n| {
            if let Node::Element(e) = n {
                Some(e)
            } else {
                None
            }
        })
        .flat_map(Element::descendants)
        .collect()
}
