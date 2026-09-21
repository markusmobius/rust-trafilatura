use crate::{text::trim_space, Document, Kind, Node, NodeId};

pub(crate) struct ElementIterator<'a> {
    root: NodeId,
    next: Option<NodeId>,
    tags: &'a [&'a str],
}

impl<'a> ElementIterator<'a> {
    pub(crate) fn including(root: NodeId) -> Self {
        Self {
            root,
            next: Some(root),
            tags: &[],
        }
    }

    pub(crate) fn descendants(document: &Document, root: NodeId, tags: &'a [&'a str]) -> Self {
        let mut iterator = Self {
            root,
            next: Some(root),
            tags,
        };
        iterator.advance(document, root);
        iterator
    }

    fn advance(&mut self, document: &Document, mut current: NodeId) {
        self.next = loop {
            let mut next = children(document, current).first().copied();
            if next.is_none() {
                while current != self.root {
                    next = next_element(document, current);
                    if next.is_some() {
                        break;
                    }
                    let Some(parent) = document.nodes[current].parent else {
                        break;
                    };
                    current = parent;
                }
            }
            let Some(next) = next else {
                break None;
            };
            if self.tags.is_empty() || self.tags.contains(&document.nodes[next].tag.as_str()) {
                break Some(next);
            }
            current = next;
        };
    }

    pub(crate) fn next(&mut self, document: &Document) -> Option<NodeId> {
        let current = self.next?;
        self.advance(document, current);
        Some(current)
    }
}

pub fn is_void(document: &Document, element: NodeId) -> bool {
    document.nodes[element].kind == Kind::Element
        && matches!(
            document.nodes[element].tag.as_str(),
            "area"
                | "base"
                | "br"
                | "col"
                | "embed"
                | "hr"
                | "img"
                | "input"
                | "keygen"
                | "link"
                | "meta"
                | "param"
                | "source"
                | "track"
                | "wbr"
        )
}

pub fn iter(document: &Document, element: NodeId, tags: &[&str]) -> Vec<NodeId> {
    let mut elements = vec![element];
    elements.extend(document.elements(element));
    if !tags.is_empty() {
        elements.retain(|&node| tags.contains(&document.nodes[node].tag.as_str()));
    }
    elements
}

pub fn iter_descendants(document: &Document, element: NodeId, tags: &[&str]) -> Vec<NodeId> {
    let mut elements = iter(document, element, tags);
    if elements.first() == Some(&element) {
        elements.remove(0);
    }
    elements
}

pub fn text(document: &Document, element: NodeId) -> String {
    document.nodes[element]
        .children
        .iter()
        .take_while(|&&child| document.nodes[child].kind != Kind::Element)
        .filter(|&&child| document.nodes[child].kind == Kind::Text)
        .map(|&child| document.nodes[child].data.as_str())
        .collect()
}

pub fn children(document: &Document, element: NodeId) -> Vec<NodeId> {
    document.nodes[element]
        .children
        .iter()
        .copied()
        .filter(|&child| document.nodes[child].kind == Kind::Element)
        .collect()
}

pub(crate) fn text_slot(document: &Document, element: NodeId) -> Option<String> {
    document.nodes[element]
        .children
        .iter()
        .take_while(|&&child| document.nodes[child].kind != Kind::Element)
        .any(|&child| document.nodes[child].kind == Kind::Text)
        .then(|| text(document, element))
}

pub(crate) fn tail_slot(document: &Document, element: NodeId) -> Option<String> {
    (!tail_nodes(document, element).is_empty()).then(|| tail(document, element))
}

pub(crate) fn set_text_slot(document: &mut Document, element: NodeId, value: Option<&str>) {
    if is_void(document, element) {
        return;
    }
    set_text(document, element, value.unwrap_or_default());
    if value.is_none() {
        let empty = document.nodes[element].children[0];
        document.detach(empty);
    }
}

pub(crate) fn set_tail_slot(document: &mut Document, element: NodeId, value: Option<&str>) {
    set_tail(document, element, value.unwrap_or_default());
    if value == Some("") {
        let Some(parent) = document.nodes[element].parent else {
            return;
        };
        if is_void(document, parent) {
            return;
        }
        let offset = document.nodes[parent]
            .children
            .iter()
            .position(|&child| child == element)
            .unwrap()
            + 1;
        let empty = create_text(document, "");
        document.nodes[parent].children.insert(offset, empty);
        document.nodes[empty].parent = Some(parent);
    }
}

pub fn next_element(document: &Document, element: NodeId) -> Option<NodeId> {
    let parent = document.nodes[element].parent?;
    document.nodes[parent]
        .children
        .iter()
        .copied()
        .skip_while(|&child| child != element)
        .skip(1)
        .find(|&child| document.nodes[child].kind == Kind::Element)
}

pub fn create_text(document: &mut Document, text: &str) -> NodeId {
    let index = document.nodes.len();
    document.nodes.push(Node {
        kind: Kind::Text,
        tag: String::new(),
        data: text.into(),
        attrs: Vec::new(),
        parent: None,
        children: Vec::new(),
    });
    index
}

pub fn insert_before(document: &mut Document, sibling: NodeId, child: NodeId) {
    let Some(parent) = document.nodes[sibling].parent else {
        return;
    };
    document.detach(child);
    let position = document.nodes[parent]
        .children
        .iter()
        .position(|&node| node == sibling)
        .unwrap();
    document.nodes[parent].children.insert(position, child);
    document.nodes[child].parent = Some(parent);
}

pub fn set_text(document: &mut Document, element: NodeId, text: &str) {
    if is_void(document, element) {
        return;
    }
    let previous = document.nodes[element]
        .children
        .iter()
        .copied()
        .take_while(|&child| document.nodes[child].kind != Kind::Element)
        .filter(|&child| document.nodes[child].kind == Kind::Text)
        .collect::<Vec<_>>();
    for child in previous {
        document.detach(child);
    }
    let text = create_text(document, text);
    document.nodes[element].children.insert(0, text);
    document.nodes[text].parent = Some(element);
}

pub fn tail_nodes(document: &Document, element: NodeId) -> Vec<NodeId> {
    let Some(parent) = document.nodes[element].parent else {
        return Vec::new();
    };
    document.nodes[parent]
        .children
        .iter()
        .copied()
        .skip_while(|&node| node != element)
        .skip(1)
        .take_while(|&node| document.nodes[node].kind != Kind::Element)
        .filter(|&node| document.nodes[node].kind == Kind::Text)
        .collect()
}

pub fn tail(document: &Document, element: NodeId) -> String {
    tail_nodes(document, element)
        .iter()
        .map(|&node| document.nodes[node].data.as_str())
        .collect()
}

pub fn set_tail(document: &mut Document, element: NodeId, tail: &str) {
    let Some(parent) = document.nodes[element].parent else {
        return;
    };
    if is_void(document, parent) {
        return;
    }
    for node in tail_nodes(document, element) {
        document.detach(node);
    }
    if tail.is_empty() {
        return;
    }
    let node = create_text(document, tail);
    let position = document.nodes[parent]
        .children
        .iter()
        .position(|&child| child == element)
        .unwrap();
    document.nodes[parent].children.insert(position + 1, node);
    document.nodes[node].parent = Some(parent);
}

pub fn append(document: &mut Document, parent: NodeId, child: NodeId) {
    let tails = tail_nodes(document, child);
    document.append(parent, child);
    for tail in tails {
        document.append(parent, tail);
    }
}

pub fn extend(document: &mut Document, parent: NodeId, children: &[NodeId]) {
    for &child in children {
        append(document, parent, child);
    }
}

pub fn clone_tree(document: &mut Document, root: NodeId) -> NodeId {
    let cloned_root = document.nodes.len();
    let mut pending = vec![(root, None)];
    while let Some((original, parent)) = pending.pop() {
        let mut node = document.nodes[original].clone();
        let index = document.nodes.len();
        pending.extend(
            node.children
                .iter()
                .rev()
                .map(|&child| (child, Some(index))),
        );
        node.children.clear();
        node.parent = parent;
        document.nodes.push(node);
        if let Some(parent) = parent {
            document.nodes[parent].children.push(index);
        }
    }
    cloned_root
}

pub fn import_tree(document: &mut Document, source: &Document, root: NodeId) -> NodeId {
    let imported_root = document.nodes.len();
    let mut pending = vec![(root, None)];
    while let Some((original, parent)) = pending.pop() {
        let mut node = source.nodes[original].clone();
        let index = document.nodes.len();
        pending.extend(
            node.children
                .iter()
                .rev()
                .map(|&child| (child, Some(index))),
        );
        node.children.clear();
        node.parent = parent;
        document.nodes.push(node);
        if let Some(parent) = parent {
            document.nodes[parent].children.push(index)
        }
    }
    imported_root
}

pub fn remove(document: &mut Document, element: NodeId, keep_tail: bool) {
    if document.nodes[element].parent.is_none() {
        return;
    }
    if !keep_tail {
        for tail in tail_nodes(document, element) {
            document.detach(tail);
        }
    }
    document.detach(element);
}

pub fn strip(document: &mut Document, element: NodeId) {
    if document.nodes[element].parent.is_none() {
        return;
    }
    for child in document.nodes[element].children.clone() {
        let cloned = clone_tree(document, child);
        insert_before(document, element, cloned);
    }
    document.detach(element);
}

pub fn strip_tags(document: &mut Document, root: NodeId, tags: &[&str]) {
    for tag in tags {
        for element in document.tagged(root, tag).into_iter().rev() {
            strip(document, element);
        }
    }
}

pub(crate) fn strip_tags_in_place(document: &mut Document, root: NodeId, tags: &[&str]) {
    for element in document.elements(root) {
        if !tags.contains(&document.nodes[element].tag.as_str())
            || document.nodes[element].parent.is_none()
        {
            continue;
        }
        for child in document.nodes[element].children.clone() {
            insert_before(document, element, child);
        }
        document.detach(element);
    }
}

pub fn strip_elements(document: &mut Document, root: NodeId, keep_tail: bool, tags: &[&str]) {
    for tag in tags {
        for element in document.tagged(root, tag).into_iter().rev() {
            remove(document, element, keep_tail);
        }
    }
}

pub fn iter_text(document: &Document, root: NodeId, separator: &str) -> String {
    iter_text_filtered(document, root, separator, |_| false)
}

pub(crate) fn iter_text_filtered(
    document: &Document,
    root: NodeId,
    separator: &str,
    exclude: impl Fn(&Node) -> bool,
) -> String {
    let mut output = String::new();
    let mut last_level = 0;
    let mut pending = vec![(root, 0)];
    while let Some((index, level)) = pending.pop() {
        let node = &document.nodes[index];
        if exclude(node) {
            continue;
        }
        if is_void(document, index) {
            output.push_str(separator);
        } else if node.kind == Kind::Text {
            if level != last_level {
                output.push_str(separator);
            }
            output.push_str(&node.data);
        }
        last_level = level;
        pending.extend(node.children.iter().rev().map(|&child| (child, level + 1)));
    }
    trim_space(&output).into()
}

pub(crate) fn extraction_text(document: &Document, root: NodeId) -> String {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut present = false;
    let mut pending = vec![(root, false)];
    while let Some((element, exit)) = pending.pop() {
        let node = &document.nodes[element];
        if !exit && node.kind == Kind::Text {
            current.push_str(&node.data);
            present = true;
            continue;
        }
        if present {
            parts.push(std::mem::take(&mut current));
            present = false;
        }
        if !exit && matches!(node.kind, Kind::Element | Kind::Document) {
            pending.push((element, true));
            pending.extend(node.children.iter().rev().map(|&child| (child, false)));
        }
    }
    trim_space(&parts.join(" ")).to_owned()
}

pub fn to_string(document: &Document, root: NodeId) -> String {
    let mut copy = document.clone();
    let container = copy.create_element("tmp");
    let cloned = clone_tree(&mut copy, root);
    copy.append(container, cloned);
    for tail in tail_nodes(document, root) {
        let cloned = clone_tree(&mut copy, tail);
        copy.append(container, cloned);
    }
    copy.inner_html(container)
}
