#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

pub mod baseline;
mod core;
mod css;
#[rustfmt::skip]
mod encoding;
pub mod etree;
pub mod external;
#[allow(clippy::type_complexity, reason = "Generated Go table layout")]
#[rustfmt::skip]
mod go_unicode;
pub mod html_processing;
mod input;
mod json;
pub mod main_extractor;
pub mod metadata;
mod metadata_json;
mod options;
mod regex_go;
pub mod selector;
mod settings;
mod shared;
pub mod text;
pub mod url;

#[cfg(test)]
mod upstream_tests;

pub use core::{extract_document, extract_node, Error, ExtractResult};
pub use input::extract;
pub use metadata::Metadata;
pub use options::{
    Config, DateOptions, ExtractionFocus, FallbackCandidates, HtmlDateMode, Options, Tree,
};
pub use rust_domdistiller::dom::{Attribute, Document, Kind, Node, NodeId};
pub use rust_htmldate;
pub use rust_readability;
pub use rust_readability::Url;
pub use shared::{parse_shared_bytes, parse_shared_html, SharedDocument};

pub const GO_REFERENCE_COMMIT: &str = "72dce36bfe95502563533cf68a9050370a3d7081";

pub fn extract_shared_document(
    document: &SharedDocument,
    options: &Options,
) -> Result<ExtractResult, Error> {
    extract_document(document.document(), options)
}

pub fn parse_html(source: &str) -> Document {
    let mut output = HtmlOutput(Document { nodes: Vec::new() });
    let root = match source.strip_prefix('\u{feff}') {
        Some(remainder) => {
            rust_readability::parse_html_direct(&format!("&#xfeff;{remainder}"), &mut output)
        }
        None => rust_readability::parse_html_direct(source, &mut output),
    };
    output.into_document(root)
}

fn readability_kind(kind: rust_readability::Kind) -> Kind {
    match kind {
        rust_readability::Kind::Document => Kind::Document,
        rust_readability::Kind::Element => Kind::Element,
        rust_readability::Kind::Text => Kind::Text,
        rust_readability::Kind::Comment => Kind::Comment,
        rust_readability::Kind::Doctype => Kind::Doctype,
    }
}

struct HtmlOutput(Document);

impl HtmlOutput {
    fn into_document(self, root: NodeId) -> Document {
        self.into_document_with(root, |_| {})
    }

    fn into_document_with(mut self, root: NodeId, mut visit: impl FnMut(NodeId)) -> Document {
        let mut nodes = Vec::<Node>::with_capacity(self.0.nodes.len());
        let mut pending = vec![(root, None)];
        while let Some((original, parent)) = pending.pop() {
            visit(original);
            let source = &mut self.0.nodes[original];
            let index = nodes.len();
            pending.extend(
                source
                    .children
                    .iter()
                    .rev()
                    .map(|&child| (child, Some(index))),
            );
            source.children.clear();
            nodes.push(Node {
                kind: source.kind.clone(),
                tag: std::mem::take(&mut source.tag),
                data: std::mem::take(&mut source.data),
                attrs: std::mem::take(&mut source.attrs),
                parent,
                children: std::mem::take(&mut source.children),
            });
            if let Some(parent) = parent {
                nodes[parent].children.push(index);
            }
        }
        Document { nodes }
    }
}

impl rust_readability::HtmlTreeSink for HtmlOutput {
    type Handle = NodeId;

    fn append_node(
        &mut self,
        parent: Option<NodeId>,
        kind: rust_readability::Kind,
        tag: &str,
        _namespace: &str,
        data: &str,
    ) -> NodeId {
        let index = self.0.nodes.len();
        self.0.nodes.push(Node {
            kind: readability_kind(kind),
            tag: tag.into(),
            data: data.into(),
            attrs: Vec::new(),
            parent,
            children: Vec::new(),
        });
        if let Some(parent) = parent {
            self.0.nodes[parent].children.push(index);
        }
        index
    }

    fn append_attribute(&mut self, node: NodeId, namespace: &str, key: &str, value: &str) {
        self.0.nodes[node].attrs.push(Attribute {
            namespace: namespace.into(),
            key: key.into(),
            value: value.into(),
        });
    }
}

impl rust_readability::HtmlTreeStore for HtmlOutput {
    fn parent(&self, node: NodeId) -> Option<NodeId> {
        self.0.nodes[node].parent
    }

    fn last_child(&self, node: NodeId) -> Option<NodeId> {
        self.0.nodes[node].children.last().copied()
    }

    fn previous_sibling(&self, node: NodeId) -> Option<NodeId> {
        let parent = self.0.nodes[node].parent?;
        let children = &self.0.nodes[parent].children;
        children
            .iter()
            .position(|&child| child == node)?
            .checked_sub(1)
            .map(|index| children[index])
    }

    fn append_text(&mut self, node: NodeId, text: &str) -> bool {
        if self.0.nodes[node].kind != Kind::Text {
            return false;
        }
        self.0.nodes[node].data.push_str(text);
        true
    }

    fn append_child(&mut self, parent: NodeId, child: NodeId) {
        self.0.append(parent, child);
    }

    fn insert_before(&mut self, sibling: NodeId, child: NodeId) {
        etree::insert_before(&mut self.0, sibling, child);
    }

    fn detach(&mut self, node: NodeId) {
        self.0.detach(node);
    }

    fn reparent_children(&mut self, node: NodeId, parent: NodeId) {
        let children = std::mem::take(&mut self.0.nodes[node].children);
        for &child in &children {
            self.0.nodes[child].parent = Some(parent);
        }
        self.0.nodes[parent].children.extend(children);
    }

    fn has_attribute(&self, node: NodeId, namespace: &str, key: &str) -> bool {
        self.0.nodes[node]
            .attrs
            .iter()
            .any(|attribute| attribute.namespace == namespace && attribute.key == key)
    }

    fn sort_attributes(&mut self, node: NodeId) {
        self.0.nodes[node].attrs.sort_unstable_by(|first, second| {
            (&first.key, &first.value).cmp(&(&second.key, &second.value))
        });
    }
}

pub(crate) fn from_readability(parsed: rust_readability::Document) -> Document {
    Document {
        nodes: parsed
            .nodes
            .into_iter()
            .map(|mut node| Node {
                kind: readability_kind(node.kind),
                tag: node.tag.as_str().to_owned(),
                data: node.data.as_str().to_owned(),
                attrs: std::mem::take(&mut node.attrs)
                    .into_iter()
                    .map(|attribute| Attribute {
                        namespace: attribute.namespace.into(),
                        key: attribute.key.into(),
                        value: attribute.value.into(),
                    })
                    .collect(),
                parent: node.parent,
                children: node.children.into_iter().collect(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod html_tests {
    #[test]
    fn shared_input_is_independent_of_extractor_order() {
        let html = format!("<html lang='en'><head><title>Shared article</title></head><body><article><h1>Shared article</h1><p>{}</p></article></body></html>", "A substantial article sentence, with additional context and detailed evidence. ".repeat(30));
        let document = super::parse_shared_bytes(html.as_bytes()).unwrap();
        let snapshot = document.clone();
        let options = super::Options {
            enable_fallback: false,
            fallback_candidates: None,
            exclude_comments: true,
            ..Default::default()
        };
        let expected_trafilatura = super::extract_document(document.document(), &options).unwrap();
        let distiller_options = rust_domdistiller::Options {
            skip_pagination: true,
            ..Default::default()
        };
        let expected_distiller =
            rust_domdistiller::apply(document.document(), &distiller_options).unwrap();
        let expected_readability = rust_readability::Parser::new()
            .parse_dom(&rust_readability::parse_dom(&html), None)
            .unwrap();
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            for engine in order {
                match engine {
                    0 => {
                        let actual = rust_readability::Parser::new()
                            .parse_shared_document(&document, None)
                            .unwrap();
                        assert_eq!(actual.text().unwrap(), expected_readability.text().unwrap());
                        assert_eq!(actual.html().unwrap(), expected_readability.html().unwrap());
                    }
                    1 => {
                        let actual =
                            rust_domdistiller::apply_shared_document(&document, &distiller_options)
                                .unwrap();
                        assert_eq!(actual.text, expected_distiller.text);
                        assert_eq!(actual.node, expected_distiller.node);
                    }
                    _ => {
                        let actual = super::extract_shared_document(&document, &options).unwrap();
                        assert_eq!(actual.content_text, expected_trafilatura.content_text);
                        assert_eq!(actual.content_node, expected_trafilatura.content_node);
                    }
                }
                assert_eq!(document, snapshot);
            }
        }
    }

    #[test]
    #[ignore = "requires the local Readability Go HTML corpus"]
    fn direct_parser_html_corpus() {
        use std::io::BufRead;
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rust-readability/target/go-html-corpus.jsonl");
        let input = std::io::BufReader::new(std::fs::File::open(path).unwrap());
        let mut checked = 0;
        for line in input.lines() {
            let case: serde_json::Value = serde_json::from_str(&line.unwrap()).unwrap();
            let source = case["source"].as_str().unwrap();
            let mut output = super::HtmlOutput(super::Document { nodes: Vec::new() });
            let root = rust_readability::parse_html_direct(source, &mut output);
            let expected = super::from_readability(rust_readability::parse_html(source));
            assert_eq!(output.into_document(root), expected, "{}", case["id"]);
            let shared = super::parse_shared_html(source);
            assert_eq!(
                rust_readability::DomSource::copy_for_readability(&shared),
                rust_readability::parse_dom(source),
                "{}",
                case["id"]
            );
            checked += 1;
        }
        assert_eq!(checked, 1793);
    }

    #[test]
    fn direct_parser_preserves_complete_document() {
        for source in [
            "",
            "<p>text",
            "<!--&amp;--><!DOCTYPE html PUBLIC '' ''><p>text</p>",
            "<html a=one><html a=two b=three><body x=one><body x=two y=three>",
            "<p><a z=one a=two href='/a'>text</a><b z=last a=first>bold</b>",
            "<table>before<p><b>bold<tr><td>cell</table>tail</b>",
            "<template><template><p>nested</template><p>outer</template><p>after</p>",
            "<svg><a xml:lang='fr' xlink:href='/x'>text</a><template>stopped</template></svg>",
            "<math><annotation-xml encoding='text/html'><p>html</p></annotation-xml></math>",
            "<pre>\nfirst\r\nsecond\0third</pre><textarea>\ntext</textarea>",
            "<script><!--<script>nested</script>--></script><p>outside</p>",
        ] {
            let expected = super::from_readability(rust_readability::parse_html(source));
            assert_eq!(super::parse_html(source), expected, "{source:?}");
        }
    }
}
