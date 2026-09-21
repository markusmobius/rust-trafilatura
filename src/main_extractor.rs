use crate::{
    etree,
    html_processing::{handle_text_node, process_node, Cache},
    settings::*,
    text, Document, Kind, NodeId, Options, Url,
};
use std::collections::{BTreeSet, HashMap, HashSet};

pub type Tags = BTreeSet<&'static str>;

fn mark_done(document: &mut Document, element: NodeId) {
    if document.nodes[element].kind == Kind::Element {
        document.nodes[element].tag = "done".into();
    } else {
        document.nodes[element].data = "done".into();
    }
}

fn sub_element(document: &mut Document, parent: NodeId, tag: &str) -> NodeId {
    let element = document.create_element(tag);
    document.append(parent, element);
    element
}

fn is_text_element(document: &Document, element: NodeId) -> bool {
    !text::trim(&etree::iter_text(document, element, "")).is_empty()
}

fn clone_for_extraction(document: &mut Document, element: NodeId) -> NodeId {
    let processed = etree::clone_tree(document, element);
    let container = document.create_element("div");
    document.append(container, processed);
    etree::set_tail_slot(
        document,
        processed,
        etree::tail_slot(document, element).as_deref(),
    );
    processed
}

pub fn handle_titles(
    document: &mut Document,
    element: NodeId,
    cache: &mut Cache,
    options: &Options,
) -> Option<NodeId> {
    if document.nodes[element].tag == "summary" {
        document.nodes[element].tag = "b".into()
    }
    let title = if etree::children(document, element).is_empty() {
        process_node(document, element, Some(cache), options)?
    } else {
        let title = clone_for_extraction(document, element);
        for child in etree::children(document, element) {
            if let Some(processed) =
                handle_text_node(document, child, Some(&mut *cache), false, false, options)
            {
                etree::append(document, title, processed);
            }
            mark_done(document, child);
        }
        title
    };
    is_text_element(document, title).then_some(title)
}

pub fn handle_formatting(
    document: &mut Document,
    element: NodeId,
    cache: &mut Cache,
    options: &Options,
) -> Option<NodeId> {
    let formatting = process_node(document, element, Some(cache), options)?;
    let allowed_parent = document.nodes[element].parent.is_some_and(|parent| {
        let tag = document.nodes[parent].tag.as_str();
        tag == "p"
            || [
                CELL_TAGS,
                HEADING_TAGS,
                EMPHASIS_TAGS,
                ITEM_TAGS,
                QUOTE_TAGS,
            ]
            .iter()
            .any(|tags| tags.contains(&tag))
    });
    if allowed_parent {
        return Some(formatting);
    }
    let paragraph = document.create_element("p");
    etree::append(document, paragraph, formatting);
    Some(paragraph)
}

fn define_new_element(
    document: &mut Document,
    processed: Option<NodeId>,
    parent: NodeId,
    keep_children: bool,
) {
    let Some(processed) = processed else { return };
    let child = sub_element(document, parent, &document.nodes[processed].tag.clone());
    etree::set_text_slot(
        document,
        child,
        etree::text_slot(document, processed).as_deref(),
    );
    etree::set_tail_slot(
        document,
        child,
        etree::tail_slot(document, processed).as_deref(),
    );
    document.nodes[child].attrs = document.nodes[processed]
        .attrs
        .iter()
        .filter(|attribute| {
            matches!(
                attribute.key.as_str(),
                "href" | "target" | "src" | "alt" | "title" | "lang"
            )
        })
        .cloned()
        .collect();
    if keep_children {
        for nested in etree::children(document, processed) {
            let tag = document.nodes[nested].tag.as_str();
            if INLINE_TAGS.contains(&tag) || BREAK_TAGS.contains(&tag) {
                define_new_element(document, Some(nested), child, true);
                for carried in etree::iter(document, nested, &[]) {
                    mark_done(document, carried)
                }
            }
        }
    }
}

fn process_nested_element(
    document: &mut Document,
    child: NodeId,
    new_child: NodeId,
    cache: &mut Cache,
    options: &Options,
) {
    etree::set_text_slot(
        document,
        new_child,
        etree::text_slot(document, child).as_deref(),
    );
    let mut elements = etree::ElementIterator::descendants(document, child, &[]);
    while let Some(element) = elements.next(document) {
        let tag = document.nodes[element].tag.as_str();
        if LIST_TAGS.contains(&tag) {
            if let Some(list) = handle_lists(document, element, cache, options) {
                document.append(new_child, list)
            }
        } else if INLINE_TAGS.contains(&tag) {
            define_new_element(document, Some(element), new_child, true);
        } else {
            let processed =
                handle_text_node(document, element, Some(&mut *cache), false, false, options);
            define_new_element(document, processed, new_child, false);
        }
        mark_done(document, element);
    }
}

pub fn handle_lists(
    document: &mut Document,
    element: NodeId,
    cache: &mut Cache,
    options: &Options,
) -> Option<NodeId> {
    let result = document.create_element(&document.nodes[element].tag.clone());
    let initial = etree::text(document, element);
    if !text::trim_space(&initial).is_empty() {
        let item = sub_element(document, result, "li");
        etree::set_text(document, item, &initial);
    }
    let mut children = etree::ElementIterator::descendants(document, element, ITEM_TAGS);
    while let Some(child) = children.next(document) {
        let tag = document.nodes[child].tag.clone();
        let new_child = document.create_element(if tag == "done" { "li" } else { &tag });
        if etree::children(document, child).is_empty() {
            if let Some(processed) = process_node(document, child, Some(&mut *cache), options) {
                let mut value = etree::text(document, processed);
                let tail = etree::tail(document, processed);
                let tail = text::trim_space(&tail);
                if !tail.is_empty() {
                    value.push(' ');
                    value.push_str(tail)
                }
                etree::set_text(document, new_child, &value);
                etree::append(document, result, new_child);
            }
        } else {
            process_nested_element(document, child, new_child, cache, options);
            let tail = etree::tail(document, child);
            if !text::trim_space(&tail).is_empty() {
                if let Some(last) = etree::children(document, new_child)
                    .into_iter()
                    .rev()
                    .find(|&node| document.nodes[node].tag != "done")
                {
                    let previous = etree::tail(document, last);
                    let combined = if text::trim_space(&previous).is_empty() {
                        tail
                    } else {
                        format!("{previous} {tail}")
                    };
                    etree::set_tail(document, last, &combined);
                }
            }
        }
        if !etree::text(document, new_child).is_empty()
            || !etree::children(document, new_child).is_empty()
        {
            etree::append(document, result, new_child);
        }
        mark_done(document, child);
    }
    mark_done(document, element);
    is_text_element(document, result).then_some(result)
}

fn is_code_block(document: &Document, element: NodeId) -> bool {
    if !document.nodes[element].attr("lang").is_empty() || document.nodes[element].tag == "code" {
        return true;
    }
    if document.nodes[element]
        .parent
        .is_some_and(|parent| document.nodes[parent].attr("class").contains("highlight"))
    {
        return true;
    }
    document
        .tagged(element, "code")
        .first()
        .is_some_and(|&code| {
            etree::children(document, element).len() == 1
                && text::trim_space(&etree::text(document, element)).is_empty()
                && text::trim_space(&etree::tail(document, code)).is_empty()
        })
}

pub fn handle_code_blocks(document: &mut Document, element: NodeId) -> NodeId {
    let processed = clone_for_extraction(document, element);
    for child in etree::iter(document, element, &[]) {
        mark_done(document, child)
    }
    document.nodes[processed].tag = "code".into();
    for child in etree::iter(document, processed, &[]) {
        document.nodes[child].attrs.clear()
    }
    processed
}

pub fn handle_quotes(
    document: &mut Document,
    element: NodeId,
    cache: &mut Cache,
    options: &Options,
) -> Option<NodeId> {
    if is_code_block(document, element) {
        return Some(handle_code_blocks(document, element));
    }
    let processed = document.create_element(&document.nodes[element].tag.clone());
    etree::set_text_slot(
        document,
        processed,
        etree::text_slot(document, element).as_deref(),
    );
    let mut descendants = etree::ElementIterator::descendants(document, element, &[]);
    while let Some(child) = descendants.next(document) {
        let tag = document.nodes[child].tag.as_str();
        if tag == "img" {
            let image = handle_image(document, child, options);
            define_new_element(document, image, processed, false);
        } else if tag == "p" && !etree::children(document, child).is_empty() {
            let mut potential: Tags = TAG_CATALOG.iter().copied().collect();
            potential.extend(["a", "img"]);
            if let Some(paragraph) = handle_paragraphs(document, child, &potential, cache, options)
            {
                etree::append(document, processed, paragraph);
            }
        } else if INLINE_TAGS.contains(&tag) {
            define_new_element(document, Some(child), processed, true);
        } else {
            let node = process_node(document, child, Some(&mut *cache), options);
            define_new_element(document, node, processed, false);
        }
        mark_done(document, child);
    }
    if !is_text_element(document, processed) {
        return None;
    }
    etree::strip_tags(document, processed, QUOTE_TAGS);
    Some(processed)
}

pub fn handle_other_elements(
    document: &mut Document,
    element: NodeId,
    potential: &Tags,
    cache: &mut Cache,
    options: &Options,
) -> Option<NodeId> {
    let tag = document.nodes[element].tag.clone();
    if tag == "div" && document.nodes[element].attr("class").contains("w3-code") {
        return Some(handle_code_blocks(document, element));
    }
    if !potential.contains(tag.as_str()) {
        return None;
    }
    if matches!(tag.as_str(), "div" | "details") {
        let processed = handle_text_node(document, element, Some(cache), false, true, options)?;
        if !text::trim(&etree::text(document, processed)).is_empty() {
            document.nodes[processed].attrs.clear();
            if document.nodes[processed].tag == "div" {
                document.nodes[processed].tag = "p".into()
            }
            return Some(processed);
        }
    }
    None
}

pub fn handle_paragraphs(
    document: &mut Document,
    element: NodeId,
    potential: &Tags,
    cache: &mut Cache,
    options: &Options,
) -> Option<NodeId> {
    document.nodes[element].attrs.clear();
    if etree::children(document, element).is_empty() {
        return process_node(document, element, Some(cache), options);
    }
    let processed = document.create_element(&document.nodes[element].tag.clone());
    let mut elements = etree::ElementIterator::including(element);
    while let Some(child) = elements.next(document) {
        let tag = document.nodes[child].tag.clone();
        if tag == "done" || !potential.contains(tag.as_str()) {
            continue;
        }
        let next = handle_text_node(document, child, Some(&mut *cache), false, true, options);
        let Some(next) = next else {
            mark_done(document, child);
            continue;
        };
        if tag == "p" {
            let previous = etree::text(document, processed);
            let addition = etree::text_slot(document, next);
            let value = if previous.is_empty() {
                addition
            } else {
                Some(format!("{previous} {}", addition.unwrap_or_default()))
            };
            etree::set_text_slot(document, processed, value.as_deref());
        } else if tag == "img" {
            if let Some(image) = handle_image(document, next, options) {
                etree::append(document, processed, image)
            }
        } else {
            let formatting = EMPHASIS_TAGS.contains(&tag.as_str()) || tag == "a";
            let mut keep_children = false;
            let children = etree::children(document, next);
            if formatting && !children.is_empty() {
                let wraps_inline = tag == "a"
                    || children
                        .iter()
                        .any(|&nested| INLINE_TAGS.contains(&document.nodes[nested].tag.as_str()));
                keep_children = wraps_inline;
                if !wraps_inline {
                    let mut following = children.first().copied();
                    while let Some(nested) = following {
                        following = etree::next_element(document, nested);
                        let tail = etree::tail(document, nested);
                        if BREAK_TAGS.contains(&document.nodes[nested].tag.as_str())
                            && !tail.is_empty()
                        {
                            etree::set_tail(
                                document,
                                nested,
                                &format!(" {}", tail.trim_start_matches(text::is_space)),
                            );
                        } else {
                            let value = etree::text(document, nested);
                            if !text::trim(&value).is_empty() {
                                etree::set_text(document, nested, &format!(" {value}"))
                            }
                        }
                        etree::strip_tags_in_place(
                            document,
                            next,
                            &[&document.nodes[nested].tag.clone()],
                        );
                    }
                    keep_children = false;
                }
            }
            define_new_element(document, Some(next), processed, keep_children);
        }
        mark_done(document, child);
    }
    let children = etree::children(document, processed);
    if let Some(&last) = children.last() {
        if BREAK_TAGS.contains(&document.nodes[last].tag.as_str())
            && etree::tail_slot(document, last).is_none()
        {
            etree::remove(document, last, false)
        }
        return Some(processed);
    }
    (!etree::text(document, processed).is_empty()).then_some(processed)
}

pub fn handle_image(document: &mut Document, element: NodeId, options: &Options) -> Option<NodeId> {
    let container = document.create_element("div");
    let processed = sub_element(document, container, &document.nodes[element].tag.clone());
    let source = [
        document.nodes[element].attr("data-src"),
        document.nodes[element].attr("src"),
    ]
    .into_iter()
    .find(|source| text::is_image_file(source))
    .or_else(|| {
        document.nodes[element]
            .attrs
            .iter()
            .find(|attribute| {
                attribute.key.starts_with("data-src") && text::is_image_file(&attribute.value)
            })
            .map(|attribute| attribute.value.as_str())
    })
    .unwrap_or("")
    .to_owned();
    if !source.is_empty() {
        document.nodes[processed].set_attr("src", &source)
    }
    for name in ["alt", "title"] {
        let value = document.nodes[element].attr(name).to_owned();
        if !value.is_empty() {
            document.nodes[processed].set_attr(name, &value)
        }
    }
    if source.is_empty() {
        return None;
    }
    let source = if let Some(base) = &options.original_url {
        Url::parse(&source).map_or(source.clone(), |relative| {
            base.resolve(relative).to_string()
        })
    } else if let Some(relative) = source.strip_prefix("//") {
        format!("http://{relative}")
    } else {
        source
    };
    document.nodes[processed].set_attr("src", &source);
    etree::set_tail_slot(
        document,
        processed,
        etree::tail_slot(document, element).as_deref(),
    );
    Some(processed)
}

pub(crate) fn table_span(document: &Document, cell: NodeId, attribute: &str) -> usize {
    let value = document.nodes[cell].attr(attribute);
    if value.is_empty() {
        return 1;
    }
    let mut span = 0;
    for character in value.chars() {
        let scalar = character as u32;
        let digit = crate::go_unicode::DIGIT_RANGES
            .iter()
            .find(|&&(start, end, stride)| {
                scalar >= start && scalar <= end && (scalar - start).is_multiple_of(stride)
            })
            .map(|&(start, _, stride)| (scalar - start) / stride % 10);
        let Some(digit) = digit else { return 1 };
        span = (span * 10 + digit as usize).min(100);
    }
    span
}

fn flush_rowspan_cells(document: &mut Document, row: NodeId, rowspans: &mut HashMap<usize, usize>) {
    let mut column = etree::children(document, row).len();
    while let Some(&remaining) = rowspans.get(&column) {
        if remaining == 0 {
            break;
        }
        sub_element(document, row, "td");
        if remaining == 1 {
            rowspans.remove(&column);
        } else {
            rowspans.insert(column, remaining - 1);
        }
        column += 1;
    }
}

fn finalize_table_row(
    document: &mut Document,
    table: NodeId,
    row: NodeId,
    rowspans: &mut HashMap<usize, usize>,
    max_columns: usize,
) {
    flush_rowspan_cells(document, row, rowspans);
    while etree::children(document, row).len() < max_columns {
        sub_element(document, row, "td");
    }
    if etree::children(document, row).into_iter().any(|cell| {
        !etree::text(document, cell).is_empty() || !etree::children(document, cell).is_empty()
    }) {
        etree::append(document, table, row);
    }
}

fn fill_table_cell(
    document: &mut Document,
    target: NodeId,
    cell: NodeId,
    nested: &HashSet<NodeId>,
    potential: &Tags,
    cache: &mut Cache,
    options: &Options,
) {
    if etree::children(document, cell).is_empty() {
        if let Some(processed) = process_node(document, cell, Some(cache), options) {
            etree::set_text_slot(
                document,
                target,
                etree::text_slot(document, processed).as_deref(),
            );
            etree::set_tail_slot(
                document,
                target,
                etree::tail_slot(document, processed).as_deref(),
            );
        }
        return;
    }
    etree::set_text_slot(
        document,
        target,
        etree::text_slot(document, cell).as_deref(),
    );
    etree::set_tail_slot(
        document,
        target,
        etree::tail_slot(document, cell).as_deref(),
    );
    mark_done(document, cell);
    let mut descendants = etree::ElementIterator::descendants(document, cell, &[]);
    while let Some(child) = descendants.next(document) {
        let tag = document.nodes[child].tag.clone();
        if tag == "done" {
            continue;
        }
        if nested.contains(&child) {
            let tail = etree::tail(document, child);
            if tag == "table" && !tail.is_empty() {
                if let Some(&last) = etree::children(document, target).last() {
                    etree::set_tail(
                        document,
                        last,
                        &format!("{}{tail}", etree::tail(document, last)),
                    );
                } else {
                    etree::set_text(
                        document,
                        target,
                        &format!("{}{tail}", etree::text(document, target)),
                    );
                }
            }
            continue;
        }
        let processed = if CELL_TAGS.contains(&tag.as_str())
            || EMPHASIS_TAGS.contains(&tag.as_str())
            || matches!(tag.as_str(), "a" | "del" | "s" | "strike")
        {
            let processed =
                handle_text_node(document, child, Some(&mut *cache), false, true, options);
            if processed.is_none() && !etree::children(document, child).is_empty() {
                define_new_element(document, Some(child), target, true);
                for carried in etree::iter(document, child, &[]) {
                    mark_done(document, carried)
                }
                continue;
            }
            processed
        } else if LIST_TAGS.contains(&tag.as_str())
            && options.focus == crate::ExtractionFocus::FavorRecall
        {
            if let Some(list) = handle_lists(document, child, cache, options) {
                etree::append(document, target, list)
            }
            mark_done(document, child);
            continue;
        } else {
            handle_text_element(document, child, potential, cache, options)
        };
        define_new_element(document, processed, target, true);
        mark_done(document, child);
    }
}

pub fn handle_table(
    document: &mut Document,
    table: NodeId,
    potential: &Tags,
    cache: &mut Cache,
    options: &Options,
) -> Option<NodeId> {
    let processed = document.create_element("table");
    let mut potential = potential.clone();
    potential.insert("div");
    etree::strip_tags(document, table, &["thead", "tbody", "tfoot"]);
    let mut nested = HashSet::new();
    for child in etree::iter_descendants(document, table, &["table"]) {
        nested.extend(etree::iter(document, child, &[]))
    }
    let mut max_columns = 0;
    for row in etree::children(document, table) {
        if document.nodes[row].tag != "tr" {
            continue;
        }
        let mut columns = 0;
        for cell in etree::children(document, row) {
            if CELL_TAGS.contains(&document.nodes[cell].tag.as_str()) {
                columns = (columns + table_span(document, cell, "colspan")).min(100)
            }
        }
        max_columns = max_columns.max(columns);
    }
    for caption in etree::children(document, table) {
        if document.nodes[caption].tag != "caption" {
            continue;
        }
        let value = etree::extraction_text(document, caption);
        if !value.is_empty() {
            let row = sub_element(document, processed, "tr");
            let cell = sub_element(document, row, "th");
            etree::set_text(document, cell, &value);
            while etree::children(document, row).len() < max_columns {
                sub_element(document, row, "td");
            }
        }
        mark_done(document, caption);
    }
    let mut header_emitted = false;
    let mut row_has_header = false;
    let mut new_row = document.create_element("tr");
    let mut rowspans = HashMap::new();
    for element in etree::children(document, table) {
        let tag = document.nodes[element].tag.as_str();
        let cells = if tag == "tr" {
            if !etree::children(document, new_row).is_empty() {
                finalize_table_row(document, processed, new_row, &mut rowspans, max_columns);
                header_emitted |= row_has_header;
            }
            new_row = document.create_element("tr");
            row_has_header = false;
            flush_rowspan_cells(document, new_row, &mut rowspans);
            etree::children(document, element)
        } else if CELL_TAGS.contains(&tag) {
            vec![element]
        } else {
            if tag != "table" {
                mark_done(document, element)
            }
            continue;
        };
        for cell in cells {
            let tag = document.nodes[cell].tag.as_str();
            if !CELL_TAGS.contains(&tag) {
                continue;
            }
            let is_header = tag == "th" && !header_emitted;
            row_has_header |= is_header;
            flush_rowspan_cells(document, new_row, &mut rowspans);
            let colspan = table_span(document, cell, "colspan");
            let rowspan = table_span(document, cell, "rowspan");
            if rowspan > 1 {
                let column = etree::children(document, new_row).len();
                for offset in 0..colspan {
                    rowspans.insert(column + offset, rowspan - 1);
                }
            }
            let tag = if is_header { "th" } else { "td" };
            let target = sub_element(document, new_row, tag);
            fill_table_cell(document, target, cell, &nested, &potential, cache, options);
            for _ in 1..colspan {
                sub_element(document, new_row, tag);
            }
            mark_done(document, cell);
        }
        mark_done(document, element);
    }
    finalize_table_row(document, processed, new_row, &mut rowspans, max_columns);
    (!etree::children(document, processed).is_empty()).then_some(processed)
}

pub fn handle_text_element(
    document: &mut Document,
    element: NodeId,
    potential: &Tags,
    cache: &mut Cache,
    options: &Options,
) -> Option<NodeId> {
    let tag = document.nodes[element].tag.as_str();
    if LIST_TAGS.contains(&tag) {
        return handle_lists(document, element, cache, options);
    }
    if QUOTE_TAGS.contains(&tag) || tag == "code" {
        return handle_quotes(document, element, cache, options);
    }
    if HEADING_TAGS.contains(&tag) {
        return handle_titles(document, element, cache, options);
    }
    if tag == "p" {
        return handle_paragraphs(document, element, potential, cache, options);
    }
    if BREAK_TAGS.contains(&tag) {
        if !text::trim(&etree::tail(document, element)).is_empty() {
            if let Some(processed) = process_node(document, element, Some(&mut *cache), options) {
                let paragraph = document.create_element("p");
                etree::set_text(document, paragraph, &etree::tail(document, processed));
                return Some(paragraph);
            }
        }
    } else if EMPHASIS_TAGS.contains(&tag) || matches!(tag, "a" | "span" | "del" | "s" | "strike") {
        return handle_formatting(document, element, cache, options);
    } else if tag == "table" && potential.contains("table") {
        return handle_table(document, element, potential, cache, options);
    } else if tag == "img" && potential.contains("img") {
        return handle_image(document, element, options);
    }
    handle_other_elements(document, element, potential, cache, options)
}

pub fn potential_tags(options: &Options) -> Tags {
    let mut potential: Tags = TAG_CATALOG.iter().copied().collect();
    if !options.exclude_tables {
        potential.extend(["table", "tr", "th", "td"])
    }
    if options.include_images {
        potential.insert("img");
    }
    if options.include_links {
        potential.insert("a");
    }
    potential
}

pub fn prune_unwanted_sections(
    document: &mut Document,
    root: NodeId,
    potential: &Tags,
    options: &Options,
    keep_teasers: bool,
) -> NodeId {
    use crate::{
        html_processing::{delete_by_link_density, link_density_test_tables, prune_unwanted_nodes},
        selector, ExtractionFocus,
    };
    let mut root = prune_unwanted_nodes(document, root, selector::OVERALL_DISCARDED, true);
    if !options.include_images {
        root = prune_unwanted_nodes(document, root, selector::DISCARDED_IMAGES, false)
    }
    if options.focus != ExtractionFocus::FavorRecall {
        if !keep_teasers {
            root = prune_unwanted_nodes(document, root, selector::DISCARDED_TEASERS, false)
        }
        if options.focus == ExtractionFocus::FavorPrecision {
            root = prune_unwanted_nodes(document, root, selector::PRECISION_DISCARDED, false)
        }
    }
    for _ in 0..2 {
        delete_by_link_density(document, root, options, true, &["div"]);
        delete_by_link_density(document, root, options, false, LIST_TAGS);
        delete_by_link_density(document, root, options, false, &["p"]);
    }
    if potential.contains("table") || options.focus == ExtractionFocus::FavorPrecision {
        for table in etree::iter(document, root, &["table"]).into_iter().rev() {
            if link_density_test_tables(document, table) {
                etree::remove(document, table, false)
            }
        }
    }
    if options.focus == ExtractionFocus::FavorPrecision {
        for child in etree::children(document, root).into_iter().rev() {
            if !HEADING_TAGS.contains(&document.nodes[child].tag.as_str()) {
                break;
            }
            etree::remove(document, child, false);
        }
        delete_by_link_density(document, root, options, false, HEADING_TAGS);
        delete_by_link_density(document, root, options, false, QUOTE_TAGS);
    }
    root
}

fn recover_wild_text(
    document: &mut Document,
    root: NodeId,
    result: NodeId,
    potential: &Tags,
    cache: &mut Cache,
    options: &Options,
) {
    let mut potential = potential.clone();
    let recall = options.focus == crate::ExtractionFocus::FavorRecall;
    if recall {
        potential.insert("div");
        potential.extend(BREAK_TAGS.iter().copied());
    }
    let root = prune_unwanted_sections(
        document,
        root,
        &potential,
        options,
        !options.enable_fallback,
    );
    if !potential.contains("a") {
        etree::strip_tags(document, root, &["a", "ref", "span"])
    } else {
        etree::strip_tags(document, root, &["span"])
    }
    let mut existing = String::new();
    let mut existing_length = 0;
    let mut elements = HashSet::new();
    for element in etree::children(document, result) {
        let value = text::trim(&document.text(element));
        if !value.is_empty() {
            existing.push_str(&value);
            existing.push('\n');
            existing_length += value.chars().count() + 1;
        }
        elements.insert(value);
    }
    let candidates: Vec<_> = document
        .elements(root)
        .into_iter()
        .filter(|&element| {
            let node = &document.nodes[element];
            QUOTE_TAGS.contains(&node.tag.as_str())
                || matches!(node.tag.as_str(), "code" | "p" | "table")
                || (node.tag == "div" && node.attr("class").contains("w3-code"))
                || (recall
                    && (node.tag == "div"
                        || BREAK_TAGS.contains(&node.tag.as_str())
                        || LIST_TAGS.contains(&node.tag.as_str())))
        })
        .collect();
    for element in candidates {
        let Some(processed) = handle_text_element(document, element, &potential, cache, options)
        else {
            continue;
        };
        let value = text::trim(&document.text(processed));
        let under_cap = existing_length <= DEDUPE_SCAN_CAP;
        if !value.is_empty()
            && (elements.contains(&value)
                || (value.chars().count() > MIN_DUPLICATE_LENGTH
                    && under_cap
                    && existing.contains(&value)))
        {
            continue;
        }
        etree::append(document, result, processed);
        if under_cap {
            existing.push_str(&value);
            existing.push('\n');
            existing_length += value.chars().count() + 1;
        }
        elements.insert(value);
    }
}

pub fn extract_content(
    document: &mut Document,
    root: NodeId,
    cache: &mut Cache,
    options: &Options,
) -> (NodeId, String) {
    let backup = etree::clone_tree(document, root);
    let result = document.create_element("body");
    let mut potential = potential_tags(options);
    let config = options.config.clone().unwrap_or_default();
    for &rule in crate::selector::CONTENT {
        let Some(selected) = crate::selector::query(document, root, rule) else {
            continue;
        };
        let subtree = prune_unwanted_sections(document, selected, &potential, options, false);
        if etree::children(document, subtree).is_empty() {
            continue;
        }
        let mut paragraph_root = if subtree == selected { root } else { subtree };
        while let Some(parent) = document.nodes[paragraph_root].parent {
            paragraph_root = parent;
        }
        let paragraphs: String = document
            .tagged(paragraph_root, "p")
            .into_iter()
            .map(|paragraph| document.text(paragraph))
            .collect();
        let factor = if options.focus == crate::ExtractionFocus::FavorPrecision {
            1
        } else {
            3
        };
        if paragraphs.is_empty()
            || (paragraphs.chars().count() as i64) < config.min_extracted_size.wrapping_mul(factor)
        {
            potential.insert("div");
        }
        if !potential.contains("a") {
            etree::strip_tags(document, subtree, &["a"])
        }
        if !potential.contains("span") {
            etree::strip_tags(document, subtree, &["span"])
        }
        let mut elements = document.elements(subtree);
        if !elements.is_empty()
            && elements
                .iter()
                .all(|&element| document.nodes[element].tag == "br")
        {
            elements = vec![subtree]
        }
        let mut processed = Vec::new();
        for element in elements {
            if let Some(element) =
                handle_text_element(document, element, &potential, cache, options)
            {
                processed.push(element)
            }
        }
        etree::extend(document, result, &processed);
        for child in etree::children(document, result).into_iter().rev() {
            let tag = document.nodes[child].tag.as_str();
            if !HEADING_TAGS.contains(&tag) && tag != "a" {
                break;
            }
            etree::remove(document, child, false);
        }
        if etree::children(document, result)
            .into_iter()
            .filter(|&element| document.nodes[element].tag != "img")
            .count()
            > 1
        {
            break;
        }
    }
    let value = etree::extraction_text(document, result);
    if etree::children(document, result).is_empty()
        || (value.chars().count() as i64) < config.min_extracted_size
    {
        recover_wild_text(document, backup, result, &potential, cache, options);
    }
    let mut previous = String::new();
    for element in etree::children(document, result) {
        let current = text::trim(&document.text(element));
        if !current.is_empty()
            && current == previous
            && current.chars().count() > MIN_DUPLICATE_LENGTH
        {
            etree::remove(document, element, false);
        } else {
            previous = current
        }
    }
    etree::strip_elements(document, result, false, &["done"]);
    etree::strip_tags(document, result, &["div"]);
    (result, etree::extraction_text(document, result))
}

pub fn extract_comments(
    document: &mut Document,
    root: NodeId,
    cache: &mut Cache,
    options: &Options,
) -> (Option<NodeId>, String) {
    let result = document.create_element("body");
    for &rule in crate::selector::COMMENTS {
        let Some(subtree) = crate::selector::query(document, root, rule) else {
            continue;
        };
        let subtree = crate::html_processing::prune_unwanted_nodes(
            document,
            subtree,
            crate::selector::DISCARDED_COMMENTS,
            false,
        );
        etree::strip_tags(document, subtree, &["a", "span"]);
        let mut processed = Vec::new();
        for element in document.elements(subtree) {
            if !TAG_CATALOG.contains(&document.nodes[element].tag.as_str()) {
                continue;
            }
            if let Some(element) =
                handle_text_node(document, element, Some(&mut *cache), true, false, options)
            {
                document.nodes[element].attrs.clear();
                processed.push(element);
            }
        }
        etree::extend(document, result, &processed);
        if !etree::children(document, result).is_empty() {
            etree::remove(document, subtree, false);
            break;
        }
    }
    let value = etree::extraction_text(document, result);
    if value.is_empty() {
        (None, String::new())
    } else {
        (Some(result), value)
    }
}
