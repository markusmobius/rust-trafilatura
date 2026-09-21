use crate::{
    baseline, etree, html_processing, settings, text, Document, ExtractionFocus, Kind, NodeId,
    Options, Tree,
};
use std::borrow::Cow;

const TAGS_TO_SANITIZE: &[&str] = &[
    "aside",
    "audio",
    "button",
    "fencedframe",
    "fieldset",
    "figure",
    "footer",
    "iframe",
    "input",
    "label",
    "link",
    "nav",
    "noindex",
    "noscript",
    "object",
    "option",
    "select",
    "source",
    "svg",
    "time",
];

pub fn candidate_is_usable(
    candidate: &Document,
    candidate_root: NodeId,
    extracted: &Document,
    extracted_root: NodeId,
    len_candidate: usize,
    len_extracted: usize,
    options: &Options,
) -> bool {
    if len_candidate == 0 || len_candidate == len_extracted {
        return false;
    }
    if len_extracted > len_candidate.saturating_mul(2) {
        return false;
    }
    if len_extracted == 0 {
        return true;
    }
    let raw_json = text::trim(&etree::iter_text(candidate, candidate_root, " ")).starts_with('{');
    if !raw_json
        && (len_candidate > len_extracted.saturating_mul(2)
            || (options.focus == ExtractionFocus::FavorRecall
                && len_candidate as f64 > 1.5 * len_extracted as f64))
    {
        return true;
    }
    let paragraphs = extracted.tagged(extracted_root, "p");
    let paragraph_text = paragraphs
        .iter()
        .any(|&paragraph| !extracted.text(paragraph).is_empty());
    let min_size = options
        .config
        .clone()
        .unwrap_or_default()
        .min_extracted_size;
    if len_candidate as i64 > min_size.wrapping_mul(2)
        && (!paragraph_text || extracted.tagged(extracted_root, "table").len() > paragraphs.len())
    {
        return true;
    }
    options.focus == ExtractionFocus::FavorRecall
        && len_candidate > len_extracted
        && !extracted.elements(extracted_root).iter().any(|&element| {
            matches!(
                extracted.nodes[element].tag.as_str(),
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
            )
        })
        && candidate
            .elements(candidate_root)
            .iter()
            .any(|&element| matches!(candidate.nodes[element].tag.as_str(), "h2" | "h3" | "h4"))
}

pub fn sanitize_tree(document: &mut Document, root: NodeId, options: &Options) {
    html_processing::doc_cleaning(document, root, options);
    for element in document.elements(root).into_iter().rev() {
        if TAGS_TO_SANITIZE.contains(&document.nodes[element].tag.as_str()) {
            document.detach(element)
        }
    }
    if !options.include_links {
        etree::strip_tags(document, root, &["a"])
    }
    etree::strip_tags(document, root, &["span"]);
    html_processing::convert_tags(document, root, options);
    for table in etree::iter(document, root, &["table"]) {
        let mut seen_header = false;
        for row in document.tagged(table, "tr") {
            let headers = document.tagged(row, "th");
            if headers.is_empty() {
                continue;
            }
            if seen_header {
                for header in headers {
                    document.nodes[header].tag = "td".into()
                }
            }
            seen_header = true;
        }
    }
    let mut invalid = Vec::new();
    for element in document.elements(root) {
        let tag = &document.nodes[element].tag;
        if !settings::VALID_TAG_CATALOG.contains(&tag.as_str()) && !invalid.contains(tag) {
            invalid.push(tag.clone())
        }
    }
    let invalid: Vec<_> = invalid.iter().map(String::as_str).collect();
    etree::strip_tags(document, root, &invalid);
}

fn to_readability(document: &Document, root: NodeId) -> rust_readability::Document {
    let mut output = rust_readability::Document { nodes: Vec::new() };
    let mut pending = vec![(root, None)];
    while let Some((original, parent)) = pending.pop() {
        let source = &document.nodes[original];
        let index = output.nodes.len();
        let mut node = rust_readability::Node::new(match source.kind {
            Kind::Document => rust_readability::Kind::Document,
            Kind::Element => rust_readability::Kind::Element,
            Kind::Text => rust_readability::Kind::Text,
            Kind::Comment => rust_readability::Kind::Comment,
            Kind::Doctype => rust_readability::Kind::Doctype,
        });
        node.tag = source.tag.as_str().into();
        node.data = source.data.as_str().into();
        node.attrs = source
            .attrs
            .iter()
            .map(|attribute| rust_readability::Attribute {
                namespace: attribute.namespace.as_str().into(),
                key: attribute.key.as_str().into(),
                value: attribute.value.as_str().into(),
            })
            .collect();
        node.parent = parent;
        output.nodes.push(node);
        if let Some(parent) = parent {
            output.nodes[parent].children.push(index)
        }
        pending.extend(
            source
                .children
                .iter()
                .rev()
                .map(|&child| (child, Some(index))),
        );
    }
    output
}

fn readability_candidate(document: &Document, root: NodeId, options: &Options) -> Option<Tree> {
    let document = to_readability(document, root);
    let result = rust_readability::from_document(&document, options.original_url.as_ref()).ok()?;
    Some(Tree {
        root: result.node?,
        document: crate::from_readability(result.document.document),
    })
}

pub fn distiller_rescue(
    document: &Document,
    root: NodeId,
    options: &Options,
) -> Option<(Tree, String)> {
    let mut body = if let Some(candidate) = options
        .fallback_candidates
        .as_ref()
        .and_then(|candidates| candidates.distiller.as_ref())
    {
        let mut document = Document { nodes: Vec::new() };
        let root = etree::import_tree(&mut document, &candidate.document, candidate.root);
        Tree { document, root }
    } else {
        let mut cleaned = Document { nodes: Vec::new() };
        let root = etree::import_tree(&mut cleaned, document, root);
        baseline::basic_cleaning(&mut cleaned, root);
        let options = rust_domdistiller::Options {
            original_url: options.original_url.as_ref().map(ToString::to_string),
            skip_pagination: true,
            ..Default::default()
        };
        let result = rust_domdistiller::apply_to_node(&cleaned, root, &options).ok()?;
        Tree {
            document: result.node,
            root: 0,
        }
    };
    sanitize_tree(&mut body.document, body.root, options);
    let content = text::trim(&etree::iter_text(&body.document, body.root, " "));
    Some((body, content))
}

pub fn compare_external_extraction(
    original: &Document,
    original_root: NodeId,
    extracted: &mut Document,
    mut extracted_root: NodeId,
    options: &Options,
) -> (NodeId, String) {
    let extracted_text = text::trim(&etree::iter_text(extracted, extracted_root, " "));
    let mut len_extracted = extracted_text.chars().count();
    let min_size = options
        .config
        .clone()
        .unwrap_or_default()
        .min_extracted_size;
    if options.focus == ExtractionFocus::FavorRecall
        && len_extracted as i64 > min_size.wrapping_mul(10)
    {
        return (extracted_root, extracted_text);
    }
    let mut cleaned = Document { nodes: Vec::new() };
    let mut cleaned_root = etree::import_tree(&mut cleaned, original, original_root);
    etree::strip_elements(&mut cleaned, cleaned_root, true, &["fencedframe"]);
    if options.focus == ExtractionFocus::FavorPrecision {
        cleaned_root = html_processing::prune_unwanted_nodes(
            &mut cleaned,
            cleaned_root,
            crate::selector::FALLBACK_DISCARDED,
            false,
        );
    }
    let candidates = options.fallback_candidates.as_ref();
    let count_custom = candidates.map_or(0, |candidates| candidates.others.len());
    let mut used_fallback = false;
    for index in 0..count_custom + 2 {
        let candidate = if index < count_custom {
            Some(Cow::Borrowed(&candidates.unwrap().others[index]))
        } else if index == count_custom {
            if let Some(candidate) =
                candidates.and_then(|candidates| candidates.readability.as_ref())
            {
                Some(Cow::Borrowed(candidate))
            } else {
                readability_candidate(&cleaned, cleaned_root, options).map(Cow::Owned)
            }
        } else if let Some(candidate) =
            candidates.and_then(|candidates| candidates.distiller.as_ref())
        {
            Some(Cow::Borrowed(candidate))
        } else {
            distiller_rescue(&cleaned, cleaned_root, options).map(|(body, _)| Cow::Owned(body))
        };
        let Some(candidate) = candidate else { continue };
        let candidate_text =
            text::trim(&etree::iter_text(&candidate.document, candidate.root, " "));
        let len_candidate = candidate_text.chars().count();
        if candidate_is_usable(
            &candidate.document,
            candidate.root,
            extracted,
            extracted_root,
            len_candidate,
            len_extracted,
            options,
        ) {
            let title = if index < count_custom {
                "custom"
            } else if index == count_custom {
                "readability"
            } else {
                "distiller"
            };
            options.log(format_args!(
                "candidate {title} is usable ({len_candidate} characters)"
            ));
            extracted_root = etree::import_tree(extracted, &candidate.document, candidate.root);
            len_extracted = len_candidate;
            used_fallback = true;
        }
        if len_extracted as i64 >= min_size {
            break;
        }
    }
    if used_fallback {
        sanitize_tree(extracted, extracted_root, options)
    }
    (
        extracted_root,
        text::trim(&etree::iter_text(extracted, extracted_root, " ")),
    )
}
