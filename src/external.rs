use crate::{etree, html_processing, settings, text, Document, ExtractionFocus, NodeId, Options};

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

pub fn compare_external_extraction(
    original: &Document,
    original_root: NodeId,
    extracted: &mut Document,
    mut extracted_root: NodeId,
    options: &Options,
) -> (NodeId, String) {
    #[cfg(feature = "lab-profile")]
    crate::profile::fallback_event("comparison_started", "");
    let extracted_text = text::trim(&etree::iter_text(extracted, extracted_root, " "));
    let len_extracted = extracted_text.chars().count();
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
    let mut prepared = Document { nodes: Vec::new() };
    let mut root = etree::import_tree(&mut prepared, original, original_root);
    etree::strip_elements(&mut prepared, root, true, &["fencedframe"]);
    if options.focus == ExtractionFocus::FavorPrecision {
        root = html_processing::prune_unwanted_nodes(
            &mut prepared,
            root,
            crate::selector::FALLBACK_DISCARDED,
            false,
        );
    }
    #[cfg(feature = "lab-profile")]
    crate::profile::fallback_event("extractor_run", "readability");
    let candidate = crate::readability_lxml::extract(&prepared, root);
    #[cfg(feature = "lab-profile")]
    crate::profile::fallback_candidate("candidate_considered", "readability", false);
    let candidate_text = text::trim(&etree::iter_text(&candidate.document, candidate.root, " "));
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
        options.log(format_args!(
            "candidate readability-lxml is usable ({len_candidate} characters)"
        ));
        extracted_root = etree::import_tree(extracted, &candidate.document, candidate.root);
        #[cfg(feature = "lab-profile")]
        crate::profile::fallback_candidate("candidate_selected", "readability", false);
        sanitize_tree(extracted, extracted_root, options)
    }
    (
        extracted_root,
        text::trim(&etree::iter_text(extracted, extracted_root, " ")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supplied_candidates_cannot_override_internal_lxml() {
        let input = crate::parse_html("<html><body></body></html>");
        let candidate = crate::Tree {
            document: crate::parse_html(&format!(
                "<article><p>{}</p></article>",
                "Explicit candidate content with supporting details. ".repeat(20)
            )),
            root: 0,
        };
        for kind in ["readability", "distiller", "custom"] {
            let mut candidates = crate::FallbackCandidates::default();
            match kind {
                "readability" => candidates.readability = Some(candidate.clone()),
                "distiller" => candidates.distiller = Some(candidate.clone()),
                _ => candidates.others.push(candidate.clone()),
            }
            let options = Options {
                fallback_candidates: Some(candidates),
                ..Default::default()
            };
            let mut output = crate::parse_html("<body></body>");
            let (_, content) = compare_external_extraction(&input, 0, &mut output, 0, &options);
            assert!(
                content.is_empty(),
                "legacy {kind} candidate was used: {content}"
            );
        }
    }
}
