use crate::{Attribute, Document, Kind, Node, NodeId};
use rust_readability::{DomSource, HtmlTreeSink, HtmlTreeStore};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedDocument {
    document: Document,
    namespaces: Vec<String>,
}

impl SharedDocument {
    pub fn document(&self) -> &Document {
        &self.document
    }
}

impl AsRef<Document> for SharedDocument {
    fn as_ref(&self) -> &Document {
        &self.document
    }
}

pub fn parse_shared_bytes(source: &[u8]) -> Result<SharedDocument, crate::Error> {
    Ok(parse_shared_html(&crate::encoding::decode(source)?))
}

pub fn parse_shared_html(source: &str) -> SharedDocument {
    let mut output = SharedOutput {
        tree: crate::HtmlOutput(Document { nodes: Vec::new() }),
        namespaces: Vec::new(),
    };
    let root = rust_readability::parse_html_direct(source, &mut output);
    let mut namespaces = Vec::with_capacity(output.namespaces.len());
    let document = output.tree.into_document_with(root, |original| {
        namespaces.push(std::mem::take(&mut output.namespaces[original]));
    });
    SharedDocument {
        document,
        namespaces,
    }
}

struct SharedOutput {
    tree: crate::HtmlOutput,
    namespaces: Vec<String>,
}

impl HtmlTreeSink for SharedOutput {
    type Handle = NodeId;

    fn append_node(
        &mut self,
        parent: Option<NodeId>,
        kind: rust_readability::Kind,
        tag: &str,
        namespace: &str,
        data: &str,
    ) -> NodeId {
        self.namespaces.push(namespace.into());
        self.tree.append_node(parent, kind, tag, namespace, data)
    }

    fn append_attribute(&mut self, node: NodeId, namespace: &str, key: &str, value: &str) {
        self.tree.append_attribute(node, namespace, key, value);
    }
}

impl HtmlTreeStore for SharedOutput {
    fn parent(&self, node: NodeId) -> Option<NodeId> {
        self.tree.parent(node)
    }

    fn last_child(&self, node: NodeId) -> Option<NodeId> {
        self.tree.last_child(node)
    }

    fn previous_sibling(&self, node: NodeId) -> Option<NodeId> {
        self.tree.previous_sibling(node)
    }

    fn append_text(&mut self, node: NodeId, text: &str) -> bool {
        self.tree.append_text(node, text)
    }

    fn append_child(&mut self, parent: NodeId, child: NodeId) {
        self.tree.append_child(parent, child);
    }

    fn insert_before(&mut self, sibling: NodeId, child: NodeId) {
        self.tree.insert_before(sibling, child);
    }

    fn detach(&mut self, node: NodeId) {
        self.tree.detach(node);
    }

    fn reparent_children(&mut self, node: NodeId, parent: NodeId) {
        self.tree.reparent_children(node, parent);
    }

    fn has_attribute(&self, node: NodeId, namespace: &str, key: &str) -> bool {
        self.tree.has_attribute(node, namespace, key)
    }

    fn sort_attributes(&mut self, node: NodeId) {
        self.tree.sort_attributes(node);
    }
}

impl HtmlTreeSink for SharedDocument {
    type Handle = NodeId;

    fn append_node(
        &mut self,
        parent: Option<NodeId>,
        kind: rust_readability::Kind,
        tag: &str,
        namespace: &str,
        data: &str,
    ) -> NodeId {
        let index = self.document.nodes.len();
        self.document.nodes.push(Node {
            kind: match kind {
                rust_readability::Kind::Document => Kind::Document,
                rust_readability::Kind::Element => Kind::Element,
                rust_readability::Kind::Text => Kind::Text,
                rust_readability::Kind::Comment => Kind::Comment,
                rust_readability::Kind::Doctype => Kind::Doctype,
            },
            tag: tag.into(),
            data: data.into(),
            attrs: Vec::new(),
            parent,
            children: Vec::new(),
        });
        self.namespaces.push(namespace.into());
        if let Some(parent) = parent {
            self.document.nodes[parent].children.push(index);
        }
        index
    }

    fn append_attribute(&mut self, node: NodeId, namespace: &str, key: &str, value: &str) {
        self.document.nodes[node].attrs.push(Attribute {
            namespace: namespace.into(),
            key: key.into(),
            value: value.into(),
        });
    }
}

impl DomSource for SharedDocument {
    fn node_count(&self) -> usize {
        self.document.nodes.len()
    }

    fn kind(&self, node: NodeId) -> rust_readability::Kind {
        match self.document.nodes[node].kind {
            Kind::Document => rust_readability::Kind::Document,
            Kind::Element => rust_readability::Kind::Element,
            Kind::Text => rust_readability::Kind::Text,
            Kind::Comment => rust_readability::Kind::Comment,
            Kind::Doctype => rust_readability::Kind::Doctype,
        }
    }

    fn tag(&self, node: NodeId) -> &str {
        &self.document.nodes[node].tag
    }
    fn data(&self, node: NodeId) -> &str {
        &self.document.nodes[node].data
    }
    fn parent(&self, node: NodeId) -> Option<NodeId> {
        self.document.nodes[node].parent
    }
    fn children(&self, node: NodeId) -> &[NodeId] {
        &self.document.nodes[node].children
    }
    fn attribute_count(&self, node: NodeId) -> usize {
        self.document.nodes[node].attrs.len()
    }
    fn attribute(&self, node: NodeId, attribute: usize) -> (&str, &str, &str) {
        let attribute = &self.document.nodes[node].attrs[attribute];
        (&attribute.namespace, &attribute.key, &attribute.value)
    }
    fn namespace(&self, node: NodeId) -> &str {
        &self.namespaces[node]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_shared_html(source: &str) -> SharedDocument {
        let mut output = SharedDocument {
            document: Document { nodes: Vec::new() },
            namespaces: Vec::new(),
        };
        rust_readability::parse_html_into(source, &mut output);
        output
    }

    #[test]
    fn shared_tree_preserves_parser_metadata() {
        for source in [
            "<!doctype html><svg><a xlink:href='url'><title>SVG title</title></a></svg><math><mi>x</mi></math><template><p>Inside</p></template><b z='2' a='1'>one<i>two</b>three",
            "<html><title>Empty article</title><p>Short.</p></html>",
            "\u{feff}<!DOCTYPE html><title>With BOM</title><p>Article.</p>",
            "<table>before<p><b z='last' a='first'>bold<tr><td>cell</table>tail</b>",
            "<template><template><p>nested</template><p>outer</template><p>after</p>",
            "<svg><a xml:lang='fr' xlink:href='/x'>text</a><template>stopped</template></svg>",
            "<math><annotation-xml encoding='text/html'><p>html</p></annotation-xml></math>",
            "<html a=one><html a=two b=three><body x=one><body x=two y=three>",
        ] {
            let shared = parse_shared_html(source);
            assert_eq!(shared, reference_shared_html(source));
            assert_eq!(
                shared.copy_for_readability(),
                rust_readability::parse_dom(source)
            );
        }
    }
}
