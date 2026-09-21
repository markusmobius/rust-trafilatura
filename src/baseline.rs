use crate::{
    etree, json,
    settings::{DEDUPE_SCAN_CAP, MIN_DUPLICATE_LENGTH, TEXT_BLOCK_TAGS},
    text, Document, Kind, Node, NodeId,
};
use regex::Regex;
use serde_json::{Map, Value};
use std::sync::LazyLock;

static COOKIE_CONSENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)cookie[-_]?(?:banner|bar|consent|law|notice|policy|description)|notice[-_]{0,2}cookie|consent[-_]?(?:banner|manager|sdk)|borlabs|cookiebot|cmplz|onetrust|moove[-_]?gdpr").unwrap()
});
static JSON_CONTENT_HOOK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"articleBody|reviewBody|recipeInstructions|acceptedAnswer|"(?:Product|VideoObject|HowTo)""#).unwrap()
});
static EMBEDDED_HTML: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)</(?:a|abbr|address|article|aside|b|blockquote|body|br|caption|cite|code|dd|del|div|dl|dt|em|figcaption|figure|footer|h[1-6]|head|header|hr|html|i|img|ins|kbd|li|main|mark|nav|ol|p|pre|q|quote|s|section|small|span|strong|sub|summary|sup|table|tbody|td|tfoot|th|thead|time|title|tr|u|ul)>|<(?:a|abbr|address|article|aside|b|blockquote|body|br|caption|cite|code|dd|del|div|dl|dt|em|figcaption|figure|footer|h[1-6]|head|header|hr|html|i|img|ins|kbd|li|main|mark|nav|ol|p|pre|q|quote|s|section|small|span|strong|sub|summary|sup|table|tbody|td|tfoot|th|thead|time|title|tr|u|ul)(?:[ \t\n\f\r][^<>]*=[^<>]*)?/?>").unwrap()
});

fn discarded_tag(node: &Node) -> bool {
    matches!(
        node.tag.as_str(),
        "aside" | "footer" | "script" | "style" | "fencedframe" | "svg" | "template"
    ) || (node.tag == "div"
        && (node.attr("id").contains("footer") || node.attr("class").contains("footer")))
}

fn discarded_cookie(node: &Node) -> bool {
    COOKIE_CONSENT.is_match(text::trim_space(node.attr("class")))
        || COOKIE_CONSENT.is_match(text::trim_space(node.attr("id")))
}

pub fn basic_cleaning(document: &mut Document, root: NodeId) {
    let mut discarded: Vec<_> = document
        .elements(root)
        .into_iter()
        .filter(|&element| discarded_tag(&document.nodes[element]))
        .collect();
    discarded.extend(
        document
            .elements(root)
            .into_iter()
            .filter(|&element| discarded_cookie(&document.nodes[element])),
    );
    for element in discarded.into_iter().rev() {
        etree::remove(document, element, true)
    }
}

fn field<'value>(object: &'value Map<String, Value>, key: &str) -> &'value Value {
    object.get(key).unwrap_or(&Value::Null)
}

fn walk_json_content(value: &Value, bodies: &mut Vec<String>, teasers: &mut Vec<String>) {
    let mut pending: Vec<_> = json::items(value).iter().rev().collect();
    while let Some(value) = pending.pop() {
        let Some(object) = value.as_object() else {
            continue;
        };
        for key in ["articleBody", "reviewBody"] {
            if let Some(value) = field(object, key)
                .as_str()
                .filter(|value| !value.is_empty())
            {
                bodies.push(value.into())
            }
        }
        for key in ["recipeInstructions", "step"] {
            for step in json::items(field(object, key)) {
                if let Some(value) = step.as_str() {
                    bodies.push(value.into())
                } else if let Some(step) = step.as_object() {
                    if let Some(value) = field(step, "text").as_str() {
                        bodies.push(value.into())
                    }
                    for sub in json::items(field(step, "itemListElement")) {
                        if let Some(value) =
                            sub.as_object().and_then(|sub| field(sub, "text").as_str())
                        {
                            bodies.push(value.into())
                        }
                    }
                }
            }
        }
        if let Some(answer) = field(object, "acceptedAnswer").as_object() {
            if let Some(value) = field(answer, "text").as_str() {
                bodies.push(value.into())
            }
        }
        if crate::metadata_json::schema_types(object)
            .iter()
            .any(|kind| matches!(kind.as_str(), "product" | "videoobject"))
        {
            if let Some(value) = field(object, "description").as_str() {
                teasers.push(value.into())
            }
        }
        pending.extend(json::items(field(object, "mainEntity")).iter().rev());
        pending.extend(json::items(field(object, "@graph")).iter().rev());
    }
}

pub fn collect_json_content(document: &Document, root: NodeId) -> (Vec<String>, Vec<String>) {
    let mut bodies = Vec::new();
    let mut teasers = Vec::new();
    for script in document.tagged(root, "script") {
        if document.nodes[script].attr("type") != "application/ld+json" {
            continue;
        }
        let value = document.text(script);
        if !JSON_CONTENT_HOOK.is_match(&value) {
            continue;
        }
        if let Some(value) = json::decode(&value) {
            walk_json_content(&value.0, &mut bodies, &mut teasers)
        }
    }
    if let Some(preload) = document
        .tagged(root, "div")
        .into_iter()
        .find(|&element| document.nodes[element].attr("id") == "data-preloaded")
    {
        if let Some(topics) = json::parse(document.nodes[preload].attr("data-preloaded")) {
            if let Some(topics) = topics.0.as_object() {
                for (key, value) in topics {
                    if !key.starts_with("topic_") {
                        continue;
                    }
                    let Some(encoded) = value.as_str() else {
                        continue;
                    };
                    let Some(topic) = json::parse(encoded) else {
                        continue;
                    };
                    let Some(stream) = topic.0.get("post_stream").and_then(Value::as_object) else {
                        continue;
                    };
                    for post in json::items(field(stream, "posts")) {
                        if let Some(cooked) = post.get("cooked").and_then(Value::as_str) {
                            bodies.push(cooked.into())
                        }
                    }
                }
            }
        }
    }
    (bodies, teasers)
}

pub fn render_baseline_text(raw: &str) -> String {
    let raw = text::remove_control_characters(&text::unescape_html(raw));
    if EMBEDDED_HTML.is_match(&raw) {
        let document = crate::parse_html(&raw);
        return document
            .tagged(0, "body")
            .first()
            .map(|&body| text::trim(&document.text(body)))
            .unwrap_or_default();
    }
    text::trim(&raw)
}

fn build_baseline_body(
    document: &mut Document,
    values: &[String],
    deduplicate: bool,
) -> (NodeId, String) {
    let body = document.create_element("body");
    let mut content = String::new();
    let mut content_length = 0;
    for value in values {
        let value = text::remove_control_characters(value);
        let length = value.chars().count();
        if value.is_empty()
            || (deduplicate
                && length > MIN_DUPLICATE_LENGTH
                && content_length <= DEDUPE_SCAN_CAP
                && content.contains(&value))
        {
            continue;
        }
        let paragraph = document.create_element("p");
        document.append(body, paragraph);
        etree::set_text(document, paragraph, &value);
        if !content.is_empty() {
            content.push('\n');
            content_length += 1
        }
        content.push_str(&value);
        content_length += length;
    }
    (body, content)
}

pub fn baseline(document: &mut Document, root: NodeId) -> (NodeId, String) {
    let root = etree::clone_tree(document, root);
    baseline_owned(document, root)
}

pub(crate) fn baseline_owned(document: &mut Document, root: NodeId) -> (NodeId, String) {
    let (bodies, teasers) = collect_json_content(document, root);
    let bodies: Vec<_> = bodies
        .iter()
        .map(|value| render_baseline_text(value))
        .collect();
    let (body, content) = build_baseline_body(document, &bodies, true);
    if content.chars().count() > 100 {
        return (body, content);
    }
    basic_cleaning(document, root);
    let mut articles = Vec::new();
    let mut max_length = 0;
    for article in document.tagged(root, "article") {
        if document.has_ancestor(article, &["article"]) {
            continue;
        }
        let value = text::trim(&document.text(article));
        let length = value.chars().count();
        if length > 100 {
            articles.push(value);
            max_length = max_length.max(length)
        }
    }
    if !articles.is_empty() {
        articles.retain(|value| value.chars().count() * 5 >= max_length);
        return build_baseline_body(document, &articles, false);
    }
    let paragraphs: Vec<_> = etree::iter(
        document,
        root,
        &["blockquote", "code", "p", "pre", "q", "quote"],
    )
    .into_iter()
    .map(|element| text::trim(&document.text(element)))
    .collect();
    let (body, content) = build_baseline_body(document, &paragraphs, true);
    if content.chars().count() > 100 {
        return (body, content);
    }
    let teasers: Vec<_> = teasers
        .iter()
        .map(|value| render_baseline_text(value))
        .collect();
    let (teaser_body, teaser_text) = build_baseline_body(document, &teasers, true);
    let mut parts = Vec::new();
    if let Some(&body) = document.tagged(root, "body").first() {
        for node in std::iter::once(body).chain(document.descendants(body)) {
            if document.nodes[node].kind == Kind::Text {
                let value = text::trim(&document.nodes[node].data);
                if !value.is_empty() {
                    parts.push(value)
                }
            }
        }
    }
    let (body, content) = build_baseline_body(document, &[parts.join("\n")], false);
    let teaser_length = teaser_text.chars().count();
    if teaser_length > 100 && teaser_length > content.chars().count() {
        (teaser_body, teaser_text)
    } else {
        (body, content)
    }
}

fn render_text(
    document: &Document,
    root: NodeId,
    separator: char,
    remove_controls: bool,
) -> String {
    render_text_filtered(document, root, separator, remove_controls, |_| false)
}

fn render_text_filtered(
    document: &Document,
    root: NodeId,
    separator: char,
    remove_controls: bool,
    exclude: impl Fn(NodeId) -> bool,
) -> String {
    let mut result = String::new();
    let mut pending = vec![(root, false)];
    while let Some((element, closing)) = pending.pop() {
        if exclude(element) {
            continue;
        }
        let node = &document.nodes[element];
        let block = TEXT_BLOCK_TAGS.contains(&node.tag.as_str());
        if block {
            result.push(separator)
        }
        if closing {
            continue;
        }
        if node.kind == Kind::Text {
            if remove_controls {
                result.push_str(&text::remove_control_characters(&node.data))
            } else {
                result.push_str(&node.data)
            }
        }
        if block {
            pending.push((element, true))
        }
        pending.extend(node.children.iter().rev().map(|&child| (child, false)));
    }
    result
}

pub fn html_to_text(document: &mut Document, root: NodeId) -> String {
    let root = etree::clone_tree(document, root);
    let root = document
        .tagged(root, "body")
        .first()
        .copied()
        .unwrap_or(root);
    basic_cleaning(document, root);
    text::trim(&render_text(document, root, ' ', true))
}

pub(crate) fn html_to_text_readonly(document: &Document, root: NodeId) -> String {
    let root = document
        .tagged(root, "body")
        .first()
        .copied()
        .unwrap_or(root);
    text::trim(&render_text_filtered(
        document,
        root,
        ' ',
        true,
        |element| {
            let node = &document.nodes[element];
            element != root
                && node.kind == Kind::Element
                && (discarded_tag(node) || discarded_cookie(node))
        },
    ))
}

pub fn plain_text(document: &Document, root: Option<NodeId>) -> String {
    let Some(root) = root else {
        return String::new();
    };
    render_text(document, root, '\n', false)
        .split('\n')
        .map(text::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
