use crate::{etree, settings, Document, ExtractionFocus, Kind, NodeId, Options};
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::sync::LazyLock;

pub struct Cache {
    capacity: i64,
    keys: VecDeque<String>,
    values: HashMap<String, i64>,
}

impl Cache {
    pub fn new(capacity: i64) -> Self {
        Self {
            capacity,
            keys: VecDeque::new(),
            values: HashMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<i64> {
        self.values.get(key).copied()
    }

    pub fn put(&mut self, key: String, value: i64) {
        if let Some(existing) = self.values.get_mut(&key) {
            *existing = value;
            return;
        }
        if self.capacity > 0 && self.keys.len() as i64 >= self.capacity {
            if let Some(oldest) = self.keys.pop_front() {
                self.values.remove(&oldest);
            }
        }
        self.keys.push_back(key.clone());
        self.values.insert(key, value);
    }
}

static TEXT_FILTER: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
    r"(?i)^[^\p{L}\p{N}_]*(Drucken|E-?Mail|Facebook|Flipboard|Google|Instagram|Linkedin|Mail|PDF|Pinterest|Pocket|Print|QQ|Reddit|Twitter|WeChat|WeiBo|Whatsapp|Xing|Mehr zum Thema:?|More on this.{0,8})$"
).unwrap()
});

pub fn text_filter(document: &Document, element: NodeId) -> bool {
    let text = etree::text(document, element);
    let text = if text.is_empty() {
        etree::tail(document, element)
    } else {
        text
    };
    text.is_empty()
        || text
            .chars()
            .all(|character| character.is_whitespace() || matches!(character, '\u{1c}'..='\u{1f}'))
        || text
            .split([
                '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}',
                '\u{2029}',
            ])
            .any(|line| TEXT_FILTER.is_match(&line.replace(['\u{130}', '\u{131}'], "i")))
}

pub fn duplicate_test(
    document: &Document,
    element: NodeId,
    cache: &mut Cache,
    options: &Options,
) -> bool {
    let text = crate::text::trim(&etree::iter_text(document, element, " "));
    let config = options.config.clone().unwrap_or_default();
    if text.chars().count() as i64 <= config.min_duplicate_check_size {
        return false;
    }
    let count = cache.get(&text).unwrap_or(0);
    cache.put(text, count.wrapping_add(1));
    count > config.max_duplicate_count
}

pub fn handle_text_node(
    document: &mut Document,
    element: NodeId,
    cache: Option<&mut Cache>,
    fix_comments: bool,
    preserve_spaces: bool,
    options: &Options,
) -> Option<NodeId> {
    let tag = document.nodes[element].tag.clone();
    if tag == "img" && crate::text::is_image_element(&document.nodes[element]) {
        return Some(element);
    }
    let mut text = etree::text(document, element);
    let mut tail = etree::tail(document, element);
    let no_children = etree::children(document, element).is_empty();
    if tag == "done" || (no_children && text.is_empty() && tail.is_empty()) {
        return None;
    }
    let line_break = matches!(tag.as_str(), "br" | "hr" | "lb");
    if !fix_comments && line_break {
        if !preserve_spaces {
            etree::set_tail(document, element, &crate::text::trim(&tail))
        }
        return Some(element);
    }
    if text.is_empty() && no_children {
        let original_tail = etree::tail_slot(document, element);
        text = tail;
        tail = String::new();
        if fix_comments && line_break {
            document.nodes[element].tag = "p".into()
        }
        etree::set_text_slot(document, element, original_tail.as_deref());
        etree::set_tail_slot(document, element, Some(""));
    }
    if !preserve_spaces {
        text = crate::text::trim(&text);
        etree::set_text_slot(
            document,
            element,
            (!text.is_empty()).then_some(text.as_str()),
        );
        if !tail.is_empty() {
            tail = crate::text::trim(&tail);
            etree::set_tail_slot(
                document,
                element,
                (!tail.is_empty()).then_some(tail.as_str()),
            );
        }
    }
    if text.is_empty() && text_filter(document, element) {
        return None;
    }
    if options.deduplicate
        && cache.is_some_and(|cache| duplicate_test(document, element, cache, options))
    {
        return None;
    }
    Some(element)
}

pub fn process_node(
    document: &mut Document,
    element: NodeId,
    cache: Option<&mut Cache>,
    options: &Options,
) -> Option<NodeId> {
    let mut text = etree::text(document, element);
    let mut tail = etree::tail(document, element);
    let tag = document.nodes[element].tag.clone();
    if tag == "done"
        || (etree::children(document, element).is_empty() && text.is_empty() && tail.is_empty())
    {
        return None;
    }
    text = crate::text::trim(&text);
    tail = crate::text::trim(&tail);
    if tag == "wbr" && (!text.is_empty() || !tail.is_empty()) {
        document.nodes[element].tag = "span".into();
    }
    etree::set_text_slot(
        document,
        element,
        (!text.is_empty()).then_some(text.as_str()),
    );
    etree::set_tail_slot(
        document,
        element,
        (!tail.is_empty()).then_some(tail.as_str()),
    );
    if !matches!(tag.as_str(), "br" | "hr" | "lb") && text.is_empty() && !tail.is_empty() {
        text = tail;
        tail = String::new();
        etree::set_text(document, element, &text);
        etree::set_tail(document, element, &tail);
    }
    if !text.is_empty() || !tail.is_empty() {
        if text_filter(document, element) {
            return None;
        }
        if options.deduplicate
            && cache.is_some_and(|cache| duplicate_test(document, element, cache, options))
        {
            return None;
        }
    }
    Some(element)
}

pub fn doc_cleaning(document: &mut Document, root: NodeId, options: &Options) {
    doc_cleaning_mode(document, root, options, false);
}

pub(crate) fn prepare_core(document: &mut Document, root: NodeId, options: &Options) {
    doc_cleaning_mode(document, root, options, true);
    convert_tags_mode(document, root, options, true);
}

fn doc_cleaning_mode(document: &mut Document, root: NodeId, options: &Options, core: bool) {
    let mut cleaning: BTreeSet<_> = settings::TAGS_TO_CLEAN.iter().copied().collect();
    let mut stripping: BTreeSet<_> = settings::TAGS_TO_STRIP.iter().copied().collect();
    if core {
        cleaning.insert("noindex");
    }
    if options.exclude_tables {
        cleaning.extend(["table", "td", "th", "tr"]);
    } else {
        for figure in document.tagged(root, "figure") {
            if !document.tagged(figure, "table").is_empty() {
                document.nodes[figure].tag = "div".into();
            }
        }
        for table in document.tagged(root, "table") {
            if matches!(document.nodes[table].attr("role"), "presentation" | "none") {
                document.nodes[table].tag = "div".into();
            }
        }
    }
    if options.include_images {
        for tag in ["figure", "picture", "source"] {
            cleaning.remove(tag);
        }
        stripping.remove("img");
    }
    if core {
        let tags: Vec<_> = stripping.into_iter().collect();
        etree::strip_tags_in_place(document, root, &tags);
    } else {
        for tag in stripping {
            etree::strip_tags(document, root, &[tag]);
        }
    }
    let backup = if options.focus == ExtractionFocus::FavorRecall
        && !document.tagged(root, "p").is_empty()
    {
        Some(document.clone())
    } else {
        None
    };
    if core {
        let mut order = settings::TAGS_TO_CLEAN.to_vec();
        order.insert(
            order.iter().position(|tag| *tag == "noscript").unwrap(),
            "noindex",
        );
        order.extend(["table", "td", "th", "tr"]);
        let mut matches: HashMap<_, Vec<NodeId>> = order
            .iter()
            .copied()
            .filter(|tag| cleaning.contains(tag))
            .map(|tag| (tag, Vec::new()))
            .collect();
        for element in document.elements(root) {
            if let Some(elements) = matches.get_mut(document.nodes[element].tag.as_str()) {
                elements.push(element);
            }
        }
        for tag in order.into_iter().filter(|tag| cleaning.contains(tag)) {
            let elements: Vec<_> = matches
                .remove(tag)
                .unwrap_or_default()
                .into_iter()
                .filter(|&element| {
                    let mut ancestor = document.nodes[element].parent;
                    while let Some(parent) = ancestor {
                        if parent == root {
                            return true;
                        }
                        ancestor = document.nodes[parent].parent;
                    }
                    false
                })
                .collect();
            for element in elements {
                etree::remove(document, element, true);
            }
        }
    } else {
        for tag in cleaning {
            etree::strip_elements(document, root, true, &[tag]);
        }
    }
    if let Some(backup) = backup {
        if document.tagged(root, "p").is_empty() {
            *document = backup;
        }
    }
    remove_html_comments(document, root);
    if core {
        let empty: Vec<_> = document
            .elements(root)
            .into_iter()
            .filter(|&element| {
                settings::EMPTY_TAGS_TO_REMOVE.contains(&document.nodes[element].tag.as_str())
                    && document.nodes[element].children.is_empty()
            })
            .collect();
        for element in empty {
            etree::remove(
                document,
                element,
                options.focus != ExtractionFocus::FavorPrecision,
            );
        }
    } else {
        prune_html(document, root, options);
    }
}

pub fn remove_html_comments(document: &mut Document, root: NodeId) {
    let mut pending = document.nodes[root].children.clone();
    while let Some(index) = pending.pop() {
        pending.extend(document.nodes[index].children.iter().copied());
        if document.nodes[index].kind == Kind::Comment {
            document.detach(index);
        }
    }
}

pub fn prune_html(document: &mut Document, root: NodeId, options: &Options) {
    for index in document.elements(root).into_iter().rev() {
        let node = &document.nodes[index];
        if settings::EMPTY_TAGS_TO_REMOVE.contains(&node.tag.as_str()) && node.children.is_empty() {
            etree::remove(
                document,
                index,
                options.focus != ExtractionFocus::FavorPrecision,
            );
        }
    }
}

pub fn convert_tags(document: &mut Document, root: NodeId, options: &Options) {
    convert_tags_mode(document, root, options, false);
}

fn convert_tags_mode(document: &mut Document, root: NodeId, options: &Options, core: bool) {
    for index in document.tagged(root, "strong") {
        if document.nodes[index]
            .attr("class")
            .contains("schema-faq-question")
        {
            document.nodes[index].tag = "h3".into();
            document.nodes[index].attrs.clear();
        }
    }
    for index in document.elements(root) {
        if matches!(document.nodes[index].tag.as_str(), "sub" | "sup")
            && etree::text(document, index).is_empty()
            && etree::children(document, index).is_empty()
        {
            etree::remove(document, index, true);
        }
    }
    if !options.include_links {
        let mut protected = Vec::new();
        for index in document.tagged(root, "a") {
            let containers: &[&str] = if core {
                &["div", "li", "p"]
            } else {
                &["div", "ul", "ol", "dl", "p"]
            };
            if document.has_ancestor(index, containers)
                || (!options.exclude_tables && document.has_ancestor(index, &["table"]))
            {
                document.nodes[index].tag = "protected-a".into();
                protected.push(index);
            }
        }
        etree::strip_tags(document, root, &["a"]);
        for index in protected {
            document.nodes[index].tag = "a".into();
        }
    } else {
        for index in document.tagged(root, "a") {
            let href = crate::text::trim(document.nodes[index].attr("href"));
            let target = crate::text::trim(document.nodes[index].attr("target"));
            document.nodes[index].attrs.clear();
            for (name, value) in [("href", href), ("target", target)] {
                if !value.is_empty() {
                    let value =
                        crate::url::create_absolute_url(&value, options.original_url.as_ref());
                    document.nodes[index].set_attr(name, &value);
                }
            }
        }
    }
    if core {
        for index in document.elements(root) {
            let tag = document.nodes[index].tag.as_str();
            if settings::EMPHASIS_TAGS.contains(&tag)
                || matches!(tag, "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
            {
                document.nodes[index].attrs.clear();
            }
        }
        for details in document.tagged(root, "details") {
            document.nodes[details].tag = "div".into();
            for summary in document.tagged(details, "summary") {
                document.nodes[summary].tag = "h3".into();
            }
        }
    }
    for index in etree::iter(document, root, &["blockquote", "pre", "q"]) {
        if core && document.nodes[index].tag != "pre" {
            continue;
        }
        let mut code = false;
        if document.nodes[index].tag == "pre" {
            let children = etree::children(document, index);
            code = children.len() == 1 && document.nodes[children[0]].tag == "span";
            let text = etree::text(document, index);
            code |= ["{", "(\"", "('", "\n    "]
                .iter()
                .any(|indicator| text.contains(indicator));
        }
        for span in document.tagged(index, "span") {
            let class = document.nodes[span].attr("class");
            if (!core && class.contains(" hljs")) || class.starts_with("hljs") {
                code = true;
                document.nodes[span].attrs.clear();
            }
        }
        if code {
            document.nodes[index].tag = "code".into();
        }
    }
    if options.include_images && options.include_links {
        for link in document.tagged(root, "a") {
            let Some(parent) = document.nodes[link].parent else {
                continue;
            };
            let images = document.tagged(link, "img");
            let next = etree::next_element(document, link);
            for &image in &images {
                let nodes = std::iter::once(image)
                    .chain(etree::tail_nodes(document, image))
                    .collect::<Vec<_>>();
                for node in nodes {
                    if let Some(next) = next {
                        etree::insert_before(document, next, node);
                    } else {
                        document.append(parent, node);
                    }
                }
            }
            if !images.is_empty() && crate::text::trim_space(&document.text(link)).is_empty() {
                etree::remove(document, link, true);
            }
        }
    }
}

pub fn collect_link_info(document: &Document, links: &[NodeId]) -> (usize, usize, Vec<NodeId>) {
    let mut length = 0;
    let mut short = 0;
    let mut non_empty = Vec::new();
    for &link in links {
        let size = crate::text::trim(&document.text(link)).chars().count();
        if size == 0 {
            continue;
        }
        length += size;
        short += usize::from(size < 10);
        non_empty.push(link);
    }
    (length, short, non_empty)
}

pub fn link_density_test(
    document: &Document,
    element: NodeId,
    options: &Options,
) -> (Vec<NodeId>, bool) {
    let links = document.tagged(element, "a");
    if links.is_empty() || (options.include_images && !document.tagged(element, "img").is_empty()) {
        return (Vec::new(), false);
    }
    let length = crate::text::trim(&document.text(element)).chars().count();
    if links.len() == 1 {
        let threshold = if options.focus == ExtractionFocus::FavorPrecision {
            10
        } else {
            100
        };
        let link_length = crate::text::trim(&document.text(links[0])).chars().count();
        if link_length > threshold && link_length as f64 > length as f64 * 0.9 {
            return (Vec::new(), true);
        }
    }
    let last = etree::next_element(document, element).is_none();
    let limit = match (document.nodes[element].tag.as_str(), last) {
        ("p", true) => 60,
        ("p", false) => 30,
        (_, true) => 300,
        (_, false) => 100,
    };
    if length < limit {
        let (link_length, short, non_empty) = collect_link_info(document, &links);
        let count = non_empty.len();
        let high = count == 0
            || link_length as f64 > length as f64 * 0.8
            || (count > 1 && short as f64 / count as f64 > 0.8);
        return (non_empty, high);
    } else if links.len() > 4 {
        let (link_length, _, non_empty) = collect_link_info(document, &links);
        if link_length as f64 > length as f64 * 0.9 && link_length < 100 * non_empty.len() {
            return (non_empty, true);
        }
    }
    (Vec::new(), false)
}

pub fn link_density_test_tables(document: &Document, table: NodeId) -> bool {
    let links = document.tagged(table, "a");
    if links.is_empty() {
        return false;
    }
    let length = crate::text::trim(&document.text(table)).chars().count();
    if length < 200 {
        return false;
    }
    let (link_length, _, _) = collect_link_info(document, &links);
    link_length as f64 > length as f64 * if length < 1000 { 0.8 } else { 0.5 }
}

pub fn delete_by_link_density(
    document: &mut Document,
    root: NodeId,
    options: &Options,
    backtracking: bool,
    tags: &[&str],
) {
    let (threshold, child_limit) = if options.focus == ExtractionFocus::FavorPrecision {
        (200, 1)
    } else {
        (100, 3)
    };
    let mut removing = Vec::new();
    for element in etree::iter(document, root, tags) {
        let (links, high) = link_density_test(document, element, options);
        if document.nodes[element].tag == "p"
            && document.nodes[element].parent.is_some_and(|parent| {
                matches!(
                    document.nodes[parent].tag.as_str(),
                    "dd" | "dt" | "li" | "th" | "td"
                )
            })
        {
            continue;
        }
        if high {
            removing.push(element);
        } else if backtracking && !links.is_empty() {
            let length = crate::text::trim(&document.text(element)).chars().count();
            if length > 0
                && length < threshold
                && etree::children(document, element).len() >= child_limit
            {
                removing.push(element);
            }
        }
    }
    for element in removing.into_iter().rev() {
        etree::remove(document, element, true);
    }
}

pub fn prune_unwanted_nodes(
    document: &mut Document,
    root: NodeId,
    rules: &[crate::selector::Rule],
    with_backup: bool,
) -> NodeId {
    let old_length = with_backup.then(|| document.text(root).chars().count());
    let mut backup = None;
    for &rule in rules {
        let elements = crate::selector::query_all(document, root, rule);
        if with_backup && backup.is_none() && !elements.is_empty() {
            backup = Some(etree::clone_tree(document, root));
        }
        for element in elements.into_iter().rev() {
            etree::remove(document, element, true);
        }
    }
    if let Some(old_length) = old_length {
        if document.text(root).chars().count() <= old_length / 7 {
            return backup.unwrap_or_else(|| etree::clone_tree(document, root));
        }
    }
    root
}

pub fn post_cleaning(document: &mut Document, root: NodeId) {
    for child in document.elements(root).into_iter().rev() {
        if etree::children(document, child).is_empty()
            && crate::text::trim(&etree::text(document, child)).is_empty()
            && !etree::is_void(document, child)
            && !settings::CELL_TAGS.contains(&document.nodes[child].tag.as_str())
        {
            etree::strip(document, child);
        }
    }
    for element in etree::iter(document, root, &[]) {
        let sized = settings::ELEMENT_WITH_SIZE.contains(&document.nodes[element].tag.as_str());
        document.nodes[element].attrs.retain(|attribute| {
            let key = attribute.key.as_str();
            if matches!(
                key,
                "id" | "class"
                    | "align"
                    | "background"
                    | "bgcolor"
                    | "border"
                    | "cellpadding"
                    | "cellspacing"
                    | "frame"
                    | "hspace"
                    | "rules"
                    | "style"
                    | "valign"
                    | "vspace"
            ) {
                return false;
            }
            if matches!(key, "width" | "height") && !sized {
                return false;
            }
            settings::ALLOWED_ATTRIBUTES.contains(&key)
        });
    }
}
