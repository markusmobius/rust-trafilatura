use crate::{etree, text, Document, Kind, NodeId, Tree};
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

static UNLIKELY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("(?i)combx|comment|community|disqus|extra|foot|header|menu|remark|rss|shoutbox|sidebar|sponsor|ad-break|agegate|pagination|pager|popup|tweet|twitter").unwrap()
});
static POSSIBLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("(?i)and|article|body|column|main|shadow").unwrap());
static POSITIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("(?i)article|body|content|entry|hentry|main|page|pagination|post|text|blog|story")
        .unwrap()
});
static NEGATIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("(?i)button|combx|comment|com-|contact|figure|foot|footer|footnote|form|input|masthead|media|meta|outbrain|promo|related|scroll|shoutbox|sidebar|sponsor|shopping|tags|tool|widget").unwrap()
});
static VIDEO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)https?://(?:www\.)?(?:youtube|vimeo)\.com").unwrap());

fn attribute<'document>(document: &'document Document, node: NodeId, name: &str) -> &'document str {
    document.nodes[node]
        .attrs
        .iter()
        .find(|attribute| attribute.key == name && attribute.namespace.is_empty())
        .map_or("", |attribute| attribute.value.as_str())
}

fn text_length(document: &Document, node: NodeId) -> usize {
    text::trim(&document.text(node)).chars().count()
}

fn link_density(document: &Document, node: NodeId) -> f64 {
    let links: usize = document
        .tagged(node, "a")
        .iter()
        .map(|&link| text_length(document, link))
        .sum();
    links as f64 / text_length(document, node).max(1) as f64
}

fn class_weight(document: &Document, node: NodeId) -> f64 {
    ["class", "id"]
        .iter()
        .map(|name| {
            let value = attribute(document, node, name);
            f64::from(POSITIVE.is_match(value)) * 25.0 - f64::from(NEGATIVE.is_match(value)) * 25.0
        })
        .sum()
}

fn initial_score(document: &Document, node: NodeId) -> f64 {
    class_weight(document, node)
        + match document.nodes[node].tag.as_str() {
            "div" | "article" => 5.0,
            "pre" | "td" | "blockquote" => 3.0,
            "address" | "ol" | "ul" | "dl" | "dd" | "dt" | "li" | "form" | "aside" => -3.0,
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "th" | "header" | "footer" | "nav" => -5.0,
            _ => 0.0,
        }
}

fn remove_unlikely(document: &mut Document, root: NodeId) {
    for node in document.elements(root) {
        let attributes = format!(
            "{} {}",
            attribute(document, node, "class"),
            attribute(document, node, "id")
        );
        if !matches!(document.nodes[node].tag.as_str(), "html" | "body")
            && UNLIKELY.is_match(&attributes)
            && !POSSIBLE.is_match(&attributes)
        {
            etree::remove(document, node, true);
        }
    }
}

fn transform_divs(document: &mut Document, root: NodeId) {
    for node in document.tagged(root, "div") {
        let contains_block = document.elements(node).iter().any(|&child| {
            let tag = document.nodes[child].tag.as_str();
            [
                "a",
                "blockquote",
                "dl",
                "div",
                "img",
                "ol",
                "p",
                "table",
                "ul",
            ]
            .iter()
            .any(|prefix| tag.starts_with(prefix))
        });
        if !contains_block {
            document.nodes[node].tag = "p".into();
        }
    }
    for node in document.tagged(root, "div") {
        let leading = etree::text(document, node);
        if !leading.trim().is_empty() {
            etree::set_text_slot(document, node, None);
            let paragraph = document.create_element("p");
            etree::set_text(document, paragraph, &leading);
            document.nodes[node].children.insert(0, paragraph);
            document.nodes[paragraph].parent = Some(node);
        }
        for child in etree::children(document, node).into_iter().rev() {
            let tail = etree::tail(document, child);
            if !tail.trim().is_empty() {
                etree::set_tail(document, child, "");
                let paragraph = document.create_element("p");
                etree::set_text(document, paragraph, &tail);
                let position = document.nodes[node]
                    .children
                    .iter()
                    .position(|&item| item == child)
                    .unwrap();
                document.nodes[node]
                    .children
                    .insert(position + 1, paragraph);
                document.nodes[paragraph].parent = Some(node);
            }
            if document.nodes[child].tag == "br" {
                etree::remove(document, child, true);
            }
        }
    }
}

fn score_paragraphs(document: &Document, root: NodeId) -> (Vec<Option<f64>>, Option<NodeId>) {
    let mut scores = vec![None; document.nodes.len()];
    let mut order = Vec::new();
    for node in etree::iter(document, root, &["p", "pre", "td"]) {
        let Some(parent) = document.nodes[node]
            .parent
            .filter(|&parent| document.nodes[parent].kind == Kind::Element)
        else {
            continue;
        };
        let content = text::trim(&document.text(node));
        let length = content.chars().count();
        if length < 25 {
            continue;
        }
        let grandparent = document.nodes[parent]
            .parent
            .filter(|&parent| document.nodes[parent].kind == Kind::Element);
        for ancestor in [Some(parent), grandparent].into_iter().flatten() {
            if scores[ancestor].is_none() {
                scores[ancestor] = Some(initial_score(document, ancestor));
                order.push(ancestor);
            }
        }
        let score = 2.0 + content.matches(',').count() as f64 + (length as f64 / 100.0).min(3.0);
        *scores[parent].as_mut().unwrap() += score;
        if let Some(grandparent) = grandparent {
            *scores[grandparent].as_mut().unwrap() += score / 2.0;
        }
    }
    let mut best = None;
    for node in order {
        *scores[node].as_mut().unwrap() *= 1.0 - link_density(document, node);
        if best.is_none_or(|previous| scores[node] > scores[previous]) {
            best = Some(node);
        }
    }
    (scores, best)
}

fn get_article(document: &mut Document, best: NodeId, scores: &[Option<f64>]) -> NodeId {
    let threshold = (scores[best].unwrap() * 0.2).max(10.0);
    let siblings = document.nodes[best]
        .parent
        .map_or_else(|| vec![best], |parent| etree::children(document, parent));
    let frame = document.create_element("html");
    let body = document.create_element("body");
    let output = document.create_element("div");
    document.append(frame, body);
    document.append(body, output);
    for sibling in siblings {
        let mut include =
            sibling == best || scores[sibling].is_some_and(|score| score >= threshold);
        if !include && document.nodes[sibling].tag == "p" {
            let density = link_density(document, sibling);
            let content = etree::text(document, sibling);
            let length = content.chars().count();
            include = (length > 80 && density < 0.25)
                || (length <= 80
                    && density == 0.0
                    && (content.contains(". ") || content.ends_with('.')));
        }
        if include {
            etree::append(document, output, sibling);
        }
    }
    output
}

fn sanitize(document: &mut Document, root: NodeId, scores: &[Option<f64>]) {
    for node in etree::iter(document, root, &["h1", "h2", "h3", "h4", "h5", "h6"]) {
        if class_weight(document, node) < 0.0 || link_density(document, node) > 0.33 {
            etree::remove(document, node, true);
        }
    }
    for node in etree::iter(document, root, &["form", "textarea"]) {
        etree::remove(document, node, true);
    }
    for node in etree::iter(document, root, &["iframe"]) {
        if VIDEO.is_match(attribute(document, node, "src")) {
            etree::set_text(document, node, "VIDEO");
        } else {
            etree::remove(document, node, true);
        }
    }
    let mut top = root;
    while let Some(parent) = document.nodes[top].parent {
        top = parent;
    }
    let mut allowed = HashSet::new();
    for node in etree::iter(
        document,
        top,
        &["table", "ul", "div", "aside", "header", "footer", "section"],
    )
    .into_iter()
    .rev()
    {
        if allowed.contains(&node) {
            continue;
        }
        let weight = class_weight(document, node);
        let score = scores.get(node).copied().flatten().unwrap_or(0.0);
        if weight + score < 0.0 {
            etree::remove(document, node, true);
            continue;
        }
        if document.text(node).matches(',').count() >= 10 {
            continue;
        }
        let count = |tag| document.tagged(node, tag).len() as i64;
        let paragraphs = count("p");
        let images = count("img");
        let items = count("li") - 100;
        let embeds = count("embed");
        let inputs = document
            .tagged(node, "input")
            .iter()
            .filter(|&&input| attribute(document, input, "type") != "hidden")
            .count() as i64;
        let length = text_length(document, node);
        let density = link_density(document, node);
        let remove = (paragraphs != 0 && images as f64 > 1.0 + paragraphs as f64 * 1.3)
            || (items > paragraphs && !matches!(document.nodes[node].tag.as_str(), "ol" | "ul"))
            || inputs as f64 > paragraphs as f64 / 3.0
            || (length < 25 && (images == 0 || images > 2))
            || (weight < 25.0 && density > 0.2)
            || (weight >= 25.0 && density > 0.5)
            || (embeds == 1 && length < 75)
            || embeds > 1;
        if remove {
            etree::remove(document, node, true);
        } else if length == 0 {
            let siblings = document.nodes[node]
                .parent
                .map_or_else(Vec::new, |parent| etree::children(document, parent));
            let position = siblings
                .iter()
                .position(|&sibling| sibling == node)
                .unwrap_or(0);
            let following = siblings
                .iter()
                .skip(position + 1)
                .map(|&sibling| text_length(document, sibling))
                .find(|&length| length > 0)
                .unwrap_or(0);
            let preceding = siblings[..position]
                .iter()
                .rev()
                .map(|&sibling| text_length(document, sibling))
                .find(|&length| length > 0)
                .unwrap_or(0);
            if following + preceding > 1000 {
                allowed.extend(etree::iter(
                    document,
                    node,
                    &["table", "ul", "div", "section"],
                ));
            } else {
                etree::remove(document, node, true);
            }
        }
    }
}

fn xml_length(document: &Document, root: NodeId) -> usize {
    let escaped_length = |value: &str, attribute: bool| {
        value
            .chars()
            .map(|character| match character {
                '&' => 5,
                '<' | '>' => 4,
                '"' if attribute => 6,
                '\n' | '\r' | '\t' if attribute => 5,
                '\r' => 5,
                _ => 1,
            })
            .sum::<usize>()
    };
    let mut length = escaped_length(&etree::tail(document, root), false);
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        let node = &document.nodes[node];
        match node.kind {
            Kind::Text => length += escaped_length(&node.data, false),
            Kind::Comment => length += 7 + node.data.chars().count(),
            Kind::Element => {
                let tag = node.tag.chars().count();
                length += if node.children.is_empty() {
                    tag + 3
                } else {
                    tag * 2 + 5
                };
                for attribute in &node.attrs {
                    length +=
                        4 + attribute.key.chars().count() + escaped_length(&attribute.value, true);
                }
            }
            _ => {}
        }
        pending.extend(node.children.iter().copied());
    }
    length
}

pub(crate) fn extract(source: &Document, source_root: NodeId) -> Tree {
    let mut document = Document { nodes: Vec::new() };
    let mut root = etree::import_tree(&mut document, source, source_root);
    if document.nodes[root].kind == Kind::Document {
        root = etree::children(&document, root)
            .into_iter()
            .next()
            .unwrap_or(root);
    }
    for node in etree::iter(&document, root, &["script", "style", "fencedframe"]) {
        etree::remove(&mut document, node, true);
    }
    for ruthless in [true, false] {
        if ruthless {
            remove_unlikely(&mut document, root);
        }
        transform_divs(&mut document, root);
        let (scores, best) = score_paragraphs(&document, root);
        root = if let Some(best) = best {
            get_article(&mut document, best, &scores)
        } else if ruthless {
            continue;
        } else {
            etree::children(&document, root)
                .into_iter()
                .find(|&child| document.nodes[child].tag == "body")
                .unwrap_or(root)
        };
        sanitize(&mut document, root, &scores);
        if !ruthless || xml_length(&document, root) >= 250 {
            break;
        }
    }
    Tree { document, root }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires exact Python input DOMs from lab/python_fallback.py candidate-fixtures"]
    fn python_candidate_corpus() {
        use serde::Deserialize;
        use std::io::BufRead;

        #[derive(Deserialize)]
        struct ReferenceNode {
            kind: String,
            tag: String,
            data: String,
            attrs: Vec<ReferenceAttribute>,
            parent: Option<NodeId>,
            children: Vec<NodeId>,
        }
        #[derive(Deserialize)]
        struct ReferenceAttribute {
            namespace: String,
            key: String,
            value: String,
        }
        #[derive(Deserialize)]
        struct Case {
            name: String,
            nodes: Vec<ReferenceNode>,
            expected_nodes: Vec<ReferenceNode>,
        }
        fn from_nodes(nodes: Vec<ReferenceNode>) -> Document {
            Document {
                nodes: nodes
                    .into_iter()
                    .map(|node| crate::Node {
                        kind: match node.kind.as_str() {
                            "element" => Kind::Element,
                            "text" => Kind::Text,
                            "comment" => Kind::Comment,
                            _ => panic!("unsupported reference node kind"),
                        },
                        tag: node.tag,
                        data: node.data,
                        parent: node.parent,
                        children: node.children,
                        attrs: node
                            .attrs
                            .into_iter()
                            .map(|attribute| crate::Attribute {
                                namespace: attribute.namespace,
                                key: attribute.key,
                                value: attribute.value,
                            })
                            .collect(),
                    })
                    .collect(),
            }
        }
        let path = std::env::var("READABILITY_LXML_REFERENCE")
            .expect("set READABILITY_LXML_REFERENCE to candidates.jsonl");
        let input = std::io::BufReader::new(std::fs::File::open(path).unwrap());
        let mut checked = 0;
        let mut mismatches = 0;
        for line in input.lines() {
            let case: Case = serde_json::from_str(&line.unwrap()).unwrap();
            let document = from_nodes(case.nodes);
            let actual = extract(&document, 0);
            let expected = from_nodes(case.expected_nodes);
            let expected_html = expected.outer_html(0);
            let observed_html = actual.document.outer_html(actual.root);
            if expected_html != observed_html {
                mismatches += 1;
                if mismatches <= 20 {
                    let offset = expected_html
                        .chars()
                        .zip(observed_html.chars())
                        .position(|(expected, observed)| expected != observed)
                        .unwrap_or_else(|| {
                            expected_html
                                .chars()
                                .count()
                                .min(observed_html.chars().count())
                        });
                    eprintln!(
                        "{}: expected {} chars, actual {} chars; first HTML difference at {}\nexpected: {:?}\nactual: {:?}",
                        case.name,
                        text_length(&expected, 0),
                        text_length(&actual.document, actual.root),
                        offset,
                        expected_html
                            .chars()
                            .skip(offset.saturating_sub(80))
                            .take(240)
                            .collect::<String>(),
                        observed_html
                            .chars()
                            .skip(offset.saturating_sub(80))
                            .take(240)
                            .collect::<String>()
                    );
                }
            }
            checked += 1;
        }
        assert!(checked > 0);
        assert_eq!(
            mismatches, 0,
            "{checked} exact-input Python candidates checked"
        );
    }

    #[test]
    fn matches_python_readability_lxml_controls() {
        let footer = "Legal footer material should remain excluded. ".repeat(15);
        let cases = [
            ("<html><body><div><p>This is a sufficiently long article paragraph, with a little more detail.</p></div></body></html>".to_owned(), "<div><div><p>This is a sufficiently long article paragraph, with a little more detail.</p></div></div>"),
            (format!("<html><body><div class=\"footer\"><p>{footer}</p></div><p>Short article.</p></body></html>"), "<body><p>Short article.</p></body>"),
            ("<html><body><div>Leading article text with enough details.<span>Inline detail.</span>Trailing explanatory text.<br/>Final sentence.</div></body></html>".to_owned(), "<div><div><body><p>Leading article text with enough details.<span>Inline detail.</span>Trailing explanatory text.<br/>Final sentence.</p></body></div></div>"),
        ];
        for (source, expected) in cases {
            let document = crate::parse_html(&source);
            let original = document.clone();
            let result = extract(&document, 0);
            assert_eq!(
                result
                    .document
                    .outer_html(result.root)
                    .replace("<br>", "<br/>"),
                expected
            );
            assert_eq!(document, original);
        }
    }
}
