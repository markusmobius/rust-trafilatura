use crate::{
    etree, metadata, text, Config, DateOptions, Document, HtmlDateMode, GO_REFERENCE_COMMIT,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct Fixture {
    commit: String,
    go_version: String,
    unicode: String,
    modules: std::collections::BTreeMap<String, String>,
    config: Value,
    text: Vec<TextCase>,
    lists: Vec<ListCase>,
    dom: Vec<DomCase>,
    metadata: Vec<MetadataCase>,
    dates: Vec<DateCase>,
    urls: Vec<UrlCase>,
    url_resolution: Vec<UrlResolutionCase>,
    normalization: Vec<NormalizationCase>,
    casing: Vec<StringCase>,
    emojis: Vec<StringCase>,
    preparation: Vec<PreparationCase>,
    conversion: Vec<ConversionCase>,
    link_density: Vec<LinkDensityCase>,
    text_nodes: Vec<TextNodeCase>,
    handlers: Vec<HandlerCase>,
    spans: Vec<SpanCase>,
    selectors: Vec<SelectorCase>,
    pruning: Vec<PruningCase>,
    baseline: Vec<BaselineCase>,
    baseline_text: Vec<StringCase>,
    post_cleaning: Vec<StringCase>,
    fallback_selection: Vec<FallbackSelectionCase>,
    sanitization: Vec<SanitizationCase>,
    native_fallbacks: Vec<NativeFallbackCase>,
    language_gates: Vec<LanguageGateCase>,
    languages: Vec<LanguageCase>,
    forums: Vec<ForumCase>,
    css: Vec<CssCase>,
    css_regex: Vec<RegexCase>,
}

#[derive(Deserialize)]
struct ExtractionCase {
    html: String,
    focus: usize,
    flags: usize,
    variant: usize,
    error: String,
    content: String,
    comments: String,
    content_text: String,
    comments_text: String,
    metadata: Value,
}

#[derive(Deserialize)]
struct RegexCase {
    pattern: String,
    input: String,
    valid: bool,
    matches: bool,
}

#[derive(Deserialize)]
struct CssCase {
    html: String,
    selector: String,
    valid: bool,
    matches: Option<Vec<String>>,
    pruned: String,
}

#[derive(Deserialize)]
struct LanguageGateCase {
    html: String,
    target: String,
    strict: bool,
    accepted: bool,
}

#[derive(Deserialize)]
struct LanguageCase {
    content: String,
    comments: String,
    language: String,
}

#[derive(Deserialize)]
struct PythonFixture {
    commit: String,
    python: String,
    packages: std::collections::BTreeMap<String, String>,
    languages: Vec<LanguageCase>,
    language_extraction: Vec<LanguageExtractionCase>,
    metadata_attributes: Vec<MetadataAttributeCase>,
    selectors: Vec<PythonSelectorCase>,
    pruning: Vec<PythonPruningCase>,
    content_snapshots: Vec<PythonSnapshotCase>,
    text_filters: Vec<PythonTextFilterCase>,
    native_core: PythonCoreCases,
    forums: Vec<ForumCase>,
}

#[derive(Deserialize)]
struct PythonCoreCases {
    text_nodes: Vec<PythonTextNodeCase>,
    edge_sequences: Vec<PythonSequenceCase>,
}

#[derive(Deserialize)]
struct WorktreeFixture {
    handlers: Vec<HandlerCase>,
    content: Vec<ContentCase>,
    sequences: Vec<ContentCase>,
    extraction: Vec<ExtractionCase>,
}

fn worktree_fixture() -> WorktreeFixture {
    serde_json::from_slice(include_bytes!("../testdata/go-worktree.json")).unwrap()
}

#[derive(Deserialize)]
struct PythonTextNodeCase {
    filtered: bool,
    accepted: Vec<bool>,
    text: String,
    tail: String,
    tag: String,
}

#[derive(Deserialize)]
struct PythonSequenceCase {
    html: String,
    focus: usize,
    flags: usize,
    snapshot: String,
    content: String,
    comments: String,
    comments_snapshot: String,
}

fn assert_python_selected_text(actual: &str, expected: &str, context: &str) {
    let compact = |value: &str| {
        value
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>()
    };
    assert_eq!(compact(actual), compact(expected), "{context}");
}

#[derive(Deserialize)]
struct PythonTextFilterCase {
    text: String,
    filtered: bool,
}

#[derive(Deserialize)]
struct PythonSnapshotCase {
    html: String,
    focus: usize,
    snapshot: String,
    length: usize,
    cleaned: String,
    sequence_cleaned: String,
}

#[derive(Deserialize)]
struct PythonPruningCase {
    html: String,
    input_tree: serde_json::Value,
    group: usize,
    backup: bool,
    tree: serde_json::Value,
}

#[derive(Deserialize)]
struct PythonSelectorCase {
    tag: String,
    attributes: Vec<(String, String)>,
    matches: Vec<bool>,
}

#[derive(Deserialize)]
struct MetadataAttributeCase {
    html: String,
    author: Option<String>,
}

#[derive(Deserialize)]
struct LanguageExtractionCase {
    html: String,
    target: Option<String>,
    fast: bool,
    accepted: bool,
    language: Option<String>,
    content: String,
    comments: String,
}

#[derive(Deserialize)]
struct ForumCase {
    html: String,
    forum: bool,
}

#[derive(Deserialize)]
struct NativeFallbackCase {
    html: String,
    extracted: String,
    focus: usize,
    flags: usize,
    variant: usize,
    output: String,
    text: String,
    rescued: String,
    rescued_text: String,
}

#[derive(Deserialize)]
struct FallbackSelectionCase {
    candidate: String,
    extracted: String,
    focus: usize,
    minimum: i64,
    candidate_length: usize,
    extracted_length: usize,
    usable: bool,
}

#[derive(Deserialize)]
struct SanitizationCase {
    html: String,
    focus: usize,
    flags: usize,
    output: String,
}

#[derive(Deserialize)]
struct BaselineCase {
    html: String,
    bodies: Option<Vec<String>>,
    teasers: Option<Vec<String>>,
    output: String,
    text: String,
    plain: String,
    flat: String,
    cleaned: String,
}

#[derive(Deserialize)]
struct ContentCase {
    html: String,
    focus: usize,
    flags: usize,
    content: String,
    content_text: String,
    comments: String,
    comments_text: String,
    mutated: String,
}

#[derive(Deserialize)]
struct SelectorCase {
    tag: String,
    value: String,
    layout: usize,
    matches: Vec<bool>,
}

#[derive(Deserialize)]
struct PruningCase {
    html: String,
    group: usize,
    backup: bool,
    output: String,
    mutated: String,
}

#[derive(Deserialize)]
struct SpanCase {
    input: String,
    output: usize,
}

#[derive(Deserialize)]
struct HandlerCase {
    html: String,
    handler: String,
    focus: usize,
    images: bool,
    links: bool,
    deduplicate: bool,
    output: String,
    mutated: String,
}

#[derive(Deserialize)]
struct TextNodeCase {
    html: String,
    fix: bool,
    spaces: bool,
    light: bool,
    capacity: i64,
    accepted: Vec<bool>,
    output: String,
}

#[derive(Deserialize)]
struct LinkDensityCase {
    html: String,
    focus: usize,
    include_images: bool,
    nodes: Vec<LinkDensityNode>,
    deleted: Vec<String>,
}

#[derive(Deserialize)]
struct LinkDensityNode {
    length: usize,
    short: usize,
    non_empty: usize,
    selected: usize,
    high: bool,
    table_high: bool,
}

#[derive(Deserialize)]
struct ConversionCase {
    html: String,
    exclude_tables: bool,
    include_images: bool,
    include_links: bool,
    original_url: String,
    converted: String,
}

#[derive(Deserialize)]
struct PreparationCase {
    html: String,
    focus: usize,
    exclude_tables: bool,
    include_images: bool,
    cleaned: String,
}

#[derive(Deserialize)]
struct StringCase {
    input: String,
    output: String,
}

#[derive(Deserialize)]
struct NormalizationCase {
    input: String,
    unescaped: String,
    cleaned: String,
    json: String,
    name: String,
    tags: String,
    title: String,
    no_emoji: String,
    authors: String,
    authors_existing: String,
}

#[derive(Deserialize)]
struct UrlResolutionCase {
    input: String,
    base: String,
    created: String,
    validated: String,
    absolute: bool,
}

#[derive(Deserialize)]
struct UrlCase {
    input: String,
    absolute: bool,
    base: String,
    domain: String,
    validated: String,
    validated_absolute: bool,
}

#[derive(Deserialize)]
struct MetadataCase {
    html: String,
    title: String,
    title_parts: (String, String, String),
    sitename: String,
    license: String,
    dom_url: String,
    open_graph: Value,
    meta: Value,
    json_ld: Value,
    extracted: Value,
    dom_author: String,
    categories: Option<Vec<String>>,
    tags: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct DateCase {
    html: String,
    mode: usize,
    fallback: bool,
    custom: usize,
    #[serde(rename = "override")]
    overridden: usize,
    url: String,
    date: String,
}

#[derive(Deserialize)]
struct TextCase {
    input: String,
    trimmed: String,
    words: usize,
    filtered: String,
    image: bool,
}

#[derive(Deserialize)]
struct ListCase {
    input: Option<Vec<String>>,
    output: Option<Vec<String>>,
    cleaned: Vec<String>,
}

#[derive(Deserialize)]
struct DomCase {
    html: String,
    operation: String,
    value: String,
    output: String,
    elements: Value,
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("../testdata/go-reference.json")).unwrap()
}

#[test]
fn pinned_reference_and_defaults() {
    let reference = fixture();
    assert_eq!(reference.commit, GO_REFERENCE_COMMIT);
    assert_eq!(reference.go_version, "go1.27.1");
    assert_eq!(reference.unicode, "17.0.0");
    assert_eq!(reference.modules.len(), 72);
    assert_eq!(reference.text.len(), 3517);
    assert_eq!(reference.lists.len(), 7);
    assert_eq!(reference.dom.len(), 144);
    assert_eq!(reference.metadata.len(), 90);
    assert_eq!(reference.dates.len(), 576);
    assert_eq!(reference.urls.len(), 36);
    assert_eq!(reference.url_resolution.len(), 252);
    assert_eq!(reference.normalization.len(), 7820);
    for (name, version) in [
        ("go-htmldate", "v1.10.1"),
        ("go-dateparser", "v1.4.7"),
        ("go-dateutil/v2", "v2.9.1"),
        ("go-py3langid", "v0.4.0"),
        ("go-readabilityV2", "v0.6.0"),
    ] {
        assert_eq!(
            reference.modules[&format!("github.com/markusmobius/{name}")],
            version
        );
    }
    let config = Config::default();
    assert_eq!(
        reference.config,
        json!({
            "CacheSize": config.cache_size,
            "MaxDuplicateCount": config.max_duplicate_count,
            "MinDuplicateCheckSize": config.min_duplicate_check_size,
            "MinExtractedSize": config.min_extracted_size,
            "MinExtractedCommentSize": config.min_extracted_comment_size,
            "MinOutputSize": config.min_output_size,
            "MinOutputCommentSize": config.min_output_comment_size,
        })
    );
}

#[test]
fn published_language_dependency() {
    let identifier = rust_py3langid::Identifier::new().unwrap();
    assert_eq!(identifier.classes().len(), 140);
    let result = identifier.identify("This text is in English.");
    assert_eq!(result.language, "en");
    assert_eq!(result.score, -68.56228637695312);
}

#[test]
fn pinned_go_url_helpers() {
    let reference = fixture();
    for case in reference.urls {
        assert_eq!(crate::url::is_absolute_url(&case.input), case.absolute);
        assert_eq!(crate::url::get_base_url(&case.input), case.base);
        assert_eq!(crate::url::get_domain_url(&case.input), case.domain);
        assert_eq!(
            crate::url::validate_url(&case.input, None),
            (case.validated, case.validated_absolute),
            "URL {:?}",
            case.input
        );
    }
    for case in reference.url_resolution {
        let base = crate::Url::parse(&case.base).unwrap();
        assert_eq!(
            crate::url::create_absolute_url(&case.input, Some(&base)),
            case.created,
            "resolve {:?} against {:?}",
            case.input,
            case.base
        );
        assert_eq!(
            crate::url::validate_url(&case.input, Some(&base)),
            (case.validated, case.absolute),
            "validate {:?} against {:?}",
            case.input,
            case.base
        );
    }
}

#[test]
fn pinned_go_author_normalization() {
    let reference = fixture();
    for case in reference.normalization {
        assert_eq!(
            text::remove_emojis(&case.input),
            case.no_emoji,
            "emoji {:?}",
            case.input
        );
        assert_eq!(
            metadata::normalize_authors("", &case.input),
            case.authors,
            "authors {:?}",
            case.input
        );
        assert_eq!(
            metadata::normalize_authors("John Doe; Jane Smith", &case.input),
            case.authors_existing,
            "existing authors {:?}",
            case.input
        );
    }
    assert!(!reference.emojis.is_empty());
    for case in reference.emojis {
        assert_eq!(
            text::remove_emojis(&case.input),
            case.output,
            "emoji cluster {:?}",
            case.input
        );
    }
}

#[test]
fn pinned_go_title_casing() {
    let reference = fixture();
    for case in reference.normalization {
        assert_eq!(
            text::title_case(&case.input),
            case.title,
            "title {:?}",
            case.input
        );
    }
    assert!(!reference.casing.is_empty());
    for case in reference.casing {
        assert_eq!(
            text::title_case(&case.input),
            case.output,
            "contextual title {:?}",
            case.input
        );
    }
}

#[test]
fn pinned_go_metadata_normalization() {
    for case in fixture().normalization {
        assert_eq!(
            text::unescape_html(&case.input),
            case.unescaped,
            "decode {:?}",
            case.input
        );
        assert_eq!(
            metadata::clean_metadata_text(&case.input),
            case.cleaned,
            "clean {:?}",
            case.input
        );
        assert_eq!(
            metadata::normalize_json_text(&case.input),
            case.json,
            "JSON {:?}",
            case.input
        );
        assert_eq!(
            metadata::validate_metadata_name(&case.input),
            case.name,
            "name {:?}",
            case.input
        );
        assert_eq!(
            metadata::normalize_tags(&case.input),
            case.tags,
            "tags {:?}",
            case.input
        );
    }
}

#[test]
fn pinned_go_text_helpers() {
    let scalars: String = (0..=0x10ffff).filter_map(char::from_u32).collect();
    let filtered: String = scalars
        .chars()
        .filter(|&character| {
            let scalar = character as u32;
            let ranges = crate::go_unicode::KEEP_RANGES;
            let index = ranges.partition_point(|&(_, end)| end < scalar);
            ranges.get(index).is_some_and(|&(start, _)| start <= scalar)
        })
        .collect();
    assert_eq!(text::remove_control_characters(&scalars), filtered);
    let reference = fixture();
    for case in reference.text {
        assert_eq!(
            text::trim(&case.input),
            case.trimmed,
            "trim {:?}",
            case.input
        );
        assert_eq!(
            text::word_count(&case.input),
            case.words,
            "words {:?}",
            case.input
        );
        assert_eq!(
            text::remove_control_characters(&case.input),
            case.filtered,
            "control {:?}",
            case.input
        );
        assert_eq!(
            text::is_image_file(&case.input),
            case.image,
            "image {:?}",
            case.input
        );
    }
    for case in reference.lists {
        assert_eq!(
            text::uniquify_lists(&case.input.clone().unwrap_or_default()),
            case.output.unwrap_or_default(),
            "lists {:?}",
            case.input
        );
        assert_eq!(
            metadata::clean_cat_tags(&case.input.unwrap_or_default()),
            case.cleaned
        );
    }
}

#[test]
fn pinned_go_element_tree() {
    for (index, case) in fixture().dom.into_iter().enumerate() {
        let mut document = Document::parse(&case.html);
        let original = document.clone();
        let target = document
            .elements(0)
            .into_iter()
            .find(|&node| document.nodes[node].attr("id") == "target")
            .unwrap();
        match case.operation.as_str() {
            "inspect" => {}
            "set_text" => etree::set_text(&mut document, target, &case.value),
            "clear_text" => etree::set_text(&mut document, target, ""),
            "set_tail" => etree::set_tail(&mut document, target, &case.value),
            "clear_tail" => etree::set_tail(&mut document, target, ""),
            "remove" => etree::remove(&mut document, target, false),
            "remove_keep_tail" => etree::remove(&mut document, target, true),
            "strip" => etree::strip(&mut document, target),
            "append" => {
                let destination = document
                    .elements(0)
                    .into_iter()
                    .find(|&node| document.nodes[node].attr("id") == "destination")
                    .unwrap();
                etree::append(&mut document, destination, target);
            }
            "strip_tags" => etree::strip_tags(&mut document, 0, &["span", "em"]),
            "strip_elements" => etree::strip_elements(&mut document, 0, false, &["span", "em"]),
            "strip_elements_keep_tail" => {
                etree::strip_elements(&mut document, 0, true, &["span", "em"])
            }
            unexpected => panic!("unknown DOM operation {unexpected}"),
        }
        assert_eq!(
            document.to_html(),
            case.output,
            "DOM {index} {}",
            case.operation
        );
        let snapshot = document.clone();
        let elements = etree::iter(&document, 0, &[]).into_iter().map(|node| {
            json!({
                "tag": document.nodes[node].tag,
                "id": document.nodes[node].attr("id"),
                "text": etree::text(&document, node),
                "tail": etree::tail(&document, node),
                "iter_text": etree::iter_text(&document, node, "|"),
                "html": etree::to_string(&document, node),
                "image": text::is_image_element(&document.nodes[node]),
                "descendants": etree::iter_descendants(&document, node, &[]).iter().map(|&descendant| document.nodes[descendant].tag.as_str()).collect::<Vec<_>>(),
                "selected": etree::iter(&document, node, &["div", "span", "br"]).iter().map(|&selected| document.nodes[selected].tag.as_str()).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>();
        assert_eq!(
            json!(elements),
            case.elements,
            "elements {index} {}",
            case.operation
        );
        assert_eq!(
            document, snapshot,
            "read-only operations changed the document"
        );
        assert_eq!(
            original,
            Document::parse(&case.html),
            "cloned input changed"
        );
        for (parent, node) in document.nodes.iter().enumerate() {
            for &child in &node.children {
                assert_eq!(document.nodes[child].parent, Some(parent));
            }
        }
    }
}

#[test]
fn pinned_go_document_preparation() {
    let reference = fixture();
    assert_eq!(reference.preparation.len(), 192);
    for (index, case) in reference.preparation.into_iter().enumerate() {
        let mut document = Document::parse(&case.html);
        let options = crate::Options {
            focus: [
                crate::ExtractionFocus::Balanced,
                crate::ExtractionFocus::FavorRecall,
                crate::ExtractionFocus::FavorPrecision,
            ][case.focus],
            exclude_tables: case.exclude_tables,
            include_images: case.include_images,
            ..Default::default()
        };
        crate::html_processing::doc_cleaning(&mut document, 0, &options);
        assert_eq!(document.to_html(), case.cleaned, "cleaning {index}");
    }
}

#[test]
fn core_stripping_preserves_reachable_tree() {
    let mut inputs: Vec<_> = fixture()
        .preparation
        .into_iter()
        .map(|case| case.html)
        .collect();
    inputs.push("<main><small>before<mark>middle<small>inner</small>tail</mark>after</small><p>kept</p></main>".into());
    inputs.push("<main><font>first<!--comment--><bdi><img src='x'>tail<font>end</font></bdi>last</font><ruby>base<rt>annotation</rt></ruby></main>".into());
    inputs.push("<main><template><hgroup><h2>Heading</h2><small>subtitle</small></hgroup></template><table><tbody><tr><td>Cell</td></tr></tbody></table></main>".into());
    for (index, input) in inputs.into_iter().enumerate() {
        let original = crate::parse_html(&input);
        for include_images in [false, true] {
            let mut tags = crate::settings::TAGS_TO_STRIP.to_vec();
            tags.sort_unstable();
            tags.retain(|tag| !include_images || *tag != "img");
            let mut expected = original.clone();
            etree::strip_tags(&mut expected, 0, &tags);
            let mut actual = original.clone();
            etree::strip_tags_in_place(&mut actual, 0, &tags);
            assert_eq!(actual.nodes.len(), original.nodes.len());
            let mut expected_tree = Document { nodes: Vec::new() };
            let mut actual_tree = Document { nodes: Vec::new() };
            etree::import_tree(&mut expected_tree, &expected, 0);
            etree::import_tree(&mut actual_tree, &actual, 0);
            assert_eq!(
                actual_tree, expected_tree,
                "stripping {index}, images {include_images}"
            );
        }
    }
}

#[test]
fn current_go_core_cleaning_removes_all_matches() {
    for focus in [
        crate::ExtractionFocus::Balanced,
        crate::ExtractionFocus::FavorRecall,
        crate::ExtractionFocus::FavorPrecision,
    ] {
        for tag in ["noindex", "script", "style"] {
            let html = format!(
                "<html><body><p>Article paragraph remains.</p>\
                 <{tag}>unwanted first</{tag}>First tail.\
                 <{tag}>unwanted second</{tag}>Second tail.\
                 <div><{tag}>unwanted nested</{tag}>Nested tail.</div>\
                 </body></html>"
            );
            let mut document = crate::parse_html(&html);
            let options = crate::Options {
                focus,
                ..Default::default()
            };
            crate::html_processing::prepare_core(&mut document, 0, &options);
            assert!(document.tagged(0, tag).is_empty(), "{focus:?} {tag}");
            let content = document.text(0);
            assert!(!content.contains("unwanted"), "{focus:?} {tag}: {content}");
            for retained in [
                "Article paragraph remains.",
                "First tail.",
                "Second tail.",
                "Nested tail.",
            ] {
                assert!(content.contains(retained), "{focus:?} {tag}: {content}");
            }
        }
    }
}

#[test]
fn pinned_go_document_conversion() {
    let reference = fixture();
    assert_eq!(reference.conversion.len(), 480);
    for (index, case) in reference.conversion.into_iter().enumerate() {
        let mut document = crate::parse_html(&case.html);
        let options = crate::Options {
            exclude_tables: case.exclude_tables,
            include_images: case.include_images,
            include_links: case.include_links,
            original_url: (!case.original_url.is_empty())
                .then(|| crate::Url::parse(&case.original_url).unwrap()),
            ..Default::default()
        };
        crate::html_processing::convert_tags(&mut document, 0, &options);
        assert_eq!(document.to_html(), case.converted, "conversion {index}");
    }
}

#[test]
fn pinned_go_complete_metadata() {
    let options = crate::Options {
        html_date_mode: HtmlDateMode::Disabled,
        ..Default::default()
    };
    for (index, case) in fixture().metadata.into_iter().enumerate() {
        let document = Document::parse(&case.html);
        let original = document.clone();
        assert_metadata(
            &metadata::extract_metadata(&document, &options),
            case.extracted,
            &format!("complete metadata {index}"),
        );
        assert_eq!(document, original);
    }
}

#[test]
fn pinned_go_link_density() {
    let reference = fixture();
    assert_eq!(reference.link_density.len(), 564);
    for (index, case) in reference.link_density.into_iter().enumerate() {
        let document = crate::parse_html(&case.html);
        let options = crate::Options {
            focus: [
                crate::ExtractionFocus::Balanced,
                crate::ExtractionFocus::FavorRecall,
                crate::ExtractionFocus::FavorPrecision,
            ][case.focus],
            include_images: case.include_images,
            ..Default::default()
        };
        let elements = document.elements(0);
        assert_eq!(elements.len(), case.nodes.len());
        for (element, expected) in elements.into_iter().zip(case.nodes) {
            let (length, short, non_empty) = crate::html_processing::collect_link_info(
                &document,
                &document.tagged(element, "a"),
            );
            let (selected, high) =
                crate::html_processing::link_density_test(&document, element, &options);
            assert_eq!(
                (
                    length,
                    short,
                    non_empty.len(),
                    selected.len(),
                    high,
                    crate::html_processing::link_density_test_tables(&document, element)
                ),
                (
                    expected.length,
                    expected.short,
                    expected.non_empty,
                    expected.selected,
                    expected.high,
                    expected.table_high
                ),
                "density {index} element {element}"
            );
        }
        for (backtracking, expected) in [false, true].into_iter().zip(case.deleted) {
            let mut copy = document.clone();
            crate::html_processing::delete_by_link_density(
                &mut copy,
                0,
                &options,
                backtracking,
                &["div", "p", "ul", "table"],
            );
            assert_eq!(
                copy.to_html(),
                expected,
                "delete density {index}, backtracking {backtracking}"
            );
        }
    }
}

#[test]
fn pinned_go_text_nodes() {
    use crate::html_processing::{handle_text_node, process_node, text_filter, Cache};
    let reference = fixture();
    let python = python_fixture().native_core;
    assert_eq!(reference.text_nodes.len(), 736);
    for (index, case) in reference.text_nodes.into_iter().enumerate() {
        let mut document = crate::parse_html(&case.html);
        let probe = document
            .elements(0)
            .into_iter()
            .find(|&node| document.nodes[node].attr("id") == "probe")
            .unwrap();
        let mut cache = Cache::new(case.capacity);
        let options = crate::Options {
            deduplicate: true,
            config: Some(Config {
                min_duplicate_check_size: 0,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            text_filter(&document, probe),
            python.text_nodes[index].filtered,
            "Python text filter {index}"
        );
        let html_void_contract = etree::is_void(&document, probe)
            && !crate::settings::BREAK_TAGS.contains(&document.nodes[probe].tag.as_str());
        let accepted = if html_void_contract {
            &case.accepted
        } else {
            &python.text_nodes[index].accepted
        };
        for (iteration, &expected) in accepted.iter().enumerate() {
            let accepted = if case.light {
                process_node(&mut document, probe, Some(&mut cache), &options)
            } else {
                handle_text_node(
                    &mut document,
                    probe,
                    Some(&mut cache),
                    case.fix,
                    case.spaces,
                    &options,
                )
            };
            assert_eq!(
                accepted.is_some(),
                expected,
                "text node {index}, iteration {iteration}"
            );
            if iteration == 3 {
                cache.put("different cache entry".into(), 1)
            }
        }
        if html_void_contract {
            assert_eq!(
                document.to_html(),
                case.output,
                "Go HTML void-element contract {index}"
            );
        } else {
            assert_eq!(
                etree::text(&document, probe),
                python.text_nodes[index].text,
                "Python text node text {index}"
            );
            assert_eq!(
                etree::tail(&document, probe),
                python.text_nodes[index].tail,
                "Python text node tail {index}"
            );
            assert_eq!(
                document.nodes[probe].tag, python.text_nodes[index].tag,
                "Python text node tag {index}"
            );
        }
    }
}

#[test]
fn pinned_go_content_handlers() {
    use crate::{html_processing::Cache, main_extractor::*};
    let reference = fixture();
    assert_eq!(reference.handlers.len(), 672);
    assert!(reference.spans.len() > 2000);
    let mut document = crate::parse_html("<table><tr><td>cell</td></tr></table>");
    let cell = document.tagged(0, "td")[0];
    for case in reference.spans {
        document.nodes[cell].set_attr("colspan", &case.input);
        assert_eq!(
            table_span(&document, cell, "colspan"),
            case.output,
            "span {:?}",
            case.input
        );
    }
    for (index, case) in worktree_fixture().handlers.into_iter().enumerate() {
        let mut document = crate::parse_html(&case.html);
        let probe = document
            .elements(0)
            .into_iter()
            .find(|&node| document.nodes[node].attr("id") == "probe")
            .unwrap();
        let options = crate::Options {
            focus: [
                crate::ExtractionFocus::Balanced,
                crate::ExtractionFocus::FavorRecall,
                crate::ExtractionFocus::FavorPrecision,
            ][case.focus],
            include_images: case.images,
            include_links: case.links,
            deduplicate: case.deduplicate,
            original_url: crate::Url::parse("https://example.org/path/article"),
            ..Default::default()
        };
        let mut cache = Cache::new(4096);
        let mut potential: Tags = crate::settings::TAG_CATALOG.iter().copied().collect();
        potential.extend(["div", "table", "tr", "th", "td"]);
        if case.images {
            potential.insert("img");
        }
        if case.links {
            potential.insert("a");
        }
        let result = match case.handler.as_str() {
            "title" => handle_titles(&mut document, probe, &mut cache, &options),
            "list" => handle_lists(&mut document, probe, &mut cache, &options),
            "quote" => handle_quotes(&mut document, probe, &mut cache, &options),
            "paragraph" => {
                handle_paragraphs(&mut document, probe, &potential, &mut cache, &options)
            }
            "formatting" => handle_formatting(&mut document, probe, &mut cache, &options),
            "image" => handle_image(&mut document, probe, &options),
            "other" => {
                handle_other_elements(&mut document, probe, &potential, &mut cache, &options)
            }
            "table" => handle_table(&mut document, probe, &potential, &mut cache, &options),
            "text" => handle_text_element(&mut document, probe, &potential, &mut cache, &options),
            other => panic!("Unknown handler {other}"),
        };
        assert_eq!(
            result
                .map(|root| etree::to_string(&document, root))
                .unwrap_or_default(),
            case.output,
            "handler output {index}: {}",
            case.handler
        );
        assert_eq!(
            document.to_html(),
            case.mutated,
            "handler mutation {index}: {}",
            case.handler
        );
    }
}

#[test]
fn pinned_content_selector_references() {
    use crate::selector::*;
    let reference = fixture();
    let python = python_fixture();
    let groups = [
        CONTENT,
        OVERALL_DISCARDED,
        PRECISION_DISCARDED,
        COMMENTS,
        DISCARDED_COMMENTS,
        REMOVED_COMMENTS,
        DISCARDED_IMAGES,
        DISCARDED_TEASERS,
    ];
    assert!(reference.selectors.len() > 10000);
    for (index, case) in reference.selectors.into_iter().enumerate() {
        let mut document = crate::parse_html("");
        let element = document.create_element(&case.tag);
        let node = &mut document.nodes[element];
        match case.layout {
            0 => node.set_attr("id", &case.value),
            1 => node.set_attr("class", &case.value),
            2 => {
                node.set_attr("id", "unrelated");
                node.set_attr("class", &case.value);
            }
            3 => {
                node.set_attr("class", &case.value);
                node.set_attr("id", "unrelated");
            }
            4 => {
                for key in ["style", "role", "itemprop", "data-component", "aria-hidden"] {
                    node.set_attr(key, &case.value)
                }
                node.set_attr(&case.value, "");
            }
            5 => {
                let padded = format!("\u{a0}{}\t", case.value);
                node.set_attr("id", &padded);
                node.set_attr("class", &padded);
            }
            _ => unreachable!(),
        }
        let matched: Vec<_> = groups
            .into_iter()
            .flatten()
            .map(|rule| rule(node))
            .collect();
        assert_eq!(
            matched, python.selectors[index].matches,
            "Python selector {index}, tag {}, value {:?}, layout {}",
            case.tag, case.value, case.layout
        );
        let fallback: Vec<_> = FALLBACK_DISCARDED.iter().map(|rule| rule(node)).collect();
        assert_eq!(fallback, case.matches[5..7], "Go fallback selector {index}");
    }
    assert_eq!(reference.pruning.len(), 288);
    for (index, case) in reference
        .pruning
        .into_iter()
        .enumerate()
        .filter(|(_, case)| case.group == 1)
    {
        let mut document = crate::parse_html(&case.html);
        let root = crate::html_processing::prune_unwanted_nodes(
            &mut document,
            0,
            FALLBACK_DISCARDED,
            case.backup,
        );
        assert_eq!(
            document.outer_html(root),
            case.output,
            "pruned output {index}"
        );
        assert_eq!(document.to_html(), case.mutated, "pruned mutation {index}");
    }
}

#[test]
fn pinned_python_content_selectors() {
    use crate::selector::*;
    let reference = python_fixture();
    let groups = [
        CONTENT,
        OVERALL_DISCARDED,
        PRECISION_DISCARDED,
        COMMENTS,
        DISCARDED_COMMENTS,
        REMOVED_COMMENTS,
        DISCARDED_IMAGES,
        DISCARDED_TEASERS,
    ];
    assert!(reference.selectors.len() > 10000);
    for (index, case) in reference.selectors.into_iter().enumerate() {
        let mut document = crate::parse_html("");
        let element = document.create_element(&case.tag);
        let node = &mut document.nodes[element];
        for (key, value) in &case.attributes {
            node.set_attr(key, value)
        }
        let matched: Vec<_> = groups
            .into_iter()
            .flatten()
            .map(|rule| rule(node))
            .collect();
        assert_eq!(
            matched, case.matches,
            "Python selector {index}: {} {:?}",
            case.tag, case.attributes
        );
    }
}

fn python_tree_snapshot(document: &crate::Document, root: crate::NodeId) -> serde_json::Value {
    use crate::Kind;
    use serde_json::{json, Value};
    let node = &document.nodes[root];
    if node.kind == Kind::Document {
        return python_tree_snapshot(document, document.tagged(root, "html")[0]);
    }
    if node.kind == Kind::Comment {
        return json!({"comment": node.data});
    }
    let tag = match node.tag.as_str() {
        "li" | "dd" | "dt" => "item",
        "ol" | "ul" | "dl" => "list",
        "blockquote" | "pre" | "q" => "quote",
        tag => tag,
    };
    let attributes: Vec<_> = node
        .attrs
        .iter()
        .map(|attribute| (&attribute.key, &attribute.value))
        .collect();
    let mut children = Vec::new();
    for &child in &node.children {
        let child_node = &document.nodes[child];
        if child_node.kind == Kind::Text {
            if child_node.data.is_empty() {
                continue;
            }
            if let Some(Value::String(previous)) = children.last_mut() {
                previous.push_str(&child_node.data);
            } else {
                children.push(Value::String(child_node.data.to_string()));
            }
        } else if matches!(child_node.kind, Kind::Element | Kind::Comment) {
            children.push(python_tree_snapshot(document, child));
        }
    }
    json!({"tag": tag, "attributes": attributes, "children": children})
}

fn python_tree_import(document: &mut crate::Document, value: &serde_json::Value) -> crate::NodeId {
    if let Some(text) = value.as_str() {
        return etree::create_text(document, text);
    }
    if let Some(comment) = value.get("comment").and_then(serde_json::Value::as_str) {
        let node = etree::create_text(document, comment);
        document.nodes[node].kind = crate::Kind::Comment;
        return node;
    }
    let node = document.create_element(value["tag"].as_str().unwrap());
    for attribute in value["attributes"].as_array().unwrap() {
        document.nodes[node].set_attr(
            attribute[0].as_str().unwrap(),
            attribute[1].as_str().unwrap(),
        );
    }
    for child in value["children"].as_array().unwrap() {
        let child = python_tree_import(document, child);
        document.append(node, child);
    }
    node
}

#[test]
fn pinned_python_pruning() {
    use crate::selector::*;
    let groups = [
        CONTENT,
        OVERALL_DISCARDED,
        PRECISION_DISCARDED,
        COMMENTS,
        DISCARDED_COMMENTS,
        REMOVED_COMMENTS,
        DISCARDED_IMAGES,
        DISCARDED_TEASERS,
    ];
    let reference = python_fixture();
    assert_eq!(reference.pruning.len(), 288);
    for (index, case) in reference.pruning.into_iter().enumerate() {
        let mut document = crate::parse_html("");
        let root = python_tree_import(&mut document, &case.input_tree);
        let root = crate::html_processing::prune_unwanted_nodes(
            &mut document,
            root,
            groups[case.group],
            case.backup,
        );
        assert_eq!(
            python_tree_snapshot(&document, root),
            case.tree,
            "Python pruning {index}: {}",
            case.html
        );
    }
}

#[test]
fn pruning_backup_is_lazy() {
    let rules: &[crate::selector::Rule] = &[|node| node.tag == "aside"];
    let mut document = crate::parse_html("<article><p>Retained article text.</p></article>");
    let before = document.clone();
    let root = crate::html_processing::prune_unwanted_nodes(&mut document, 0, rules, true);
    assert_eq!(root, 0);
    assert_eq!(document, before);

    let mut document = crate::parse_html("<article><aside>All removed text.</aside></article>");
    let original = document.to_html();
    let root = crate::html_processing::prune_unwanted_nodes(&mut document, 0, rules, true);
    assert_ne!(root, 0);
    assert_eq!(document.outer_html(root), original);
    assert!(!document.to_html().contains("aside"));

    let mut document = crate::parse_html("<article></article>");
    let original = document.to_html();
    let root = crate::html_processing::prune_unwanted_nodes(&mut document, 0, rules, true);
    assert_ne!(root, 0);
    assert_eq!(document.outer_html(root), original);
}

#[test]
fn current_go_content_and_comments() {
    let reference = worktree_fixture();
    assert_eq!(reference.content.len(), 2112);
    for (index, case) in reference.content.into_iter().enumerate() {
        let mut document = crate::parse_html(&case.html);
        let options = crate::Options {
            focus: [
                crate::ExtractionFocus::Balanced,
                crate::ExtractionFocus::FavorRecall,
                crate::ExtractionFocus::FavorPrecision,
            ][case.focus],
            include_images: case.flags & 1 != 0,
            include_links: case.flags & 2 != 0,
            deduplicate: case.flags & 4 != 0,
            enable_fallback: case.flags & 8 != 0,
            exclude_comments: case.flags & 16 != 0,
            original_url: crate::Url::parse("https://example.org/path/article"),
            ..Default::default()
        };
        let mut cache = crate::html_processing::Cache::new(4096);
        crate::html_processing::doc_cleaning(&mut document, 0, &options);
        crate::html_processing::convert_tags(&mut document, 0, &options);
        let (comments, comments_text) = if options.exclude_comments {
            (None, String::new())
        } else {
            crate::main_extractor::extract_comments(&mut document, 0, &mut cache, &options)
        };
        let (content, content_text) =
            crate::main_extractor::extract_content(&mut document, 0, &mut cache, &options);
        assert_eq!(
            document.outer_html(content),
            case.content,
            "content HTML {index}"
        );
        assert_eq!(content_text, case.content_text, "content snapshot {index}");
        assert_eq!(
            comments
                .map(|root| document.outer_html(root))
                .unwrap_or_default(),
            case.comments,
            "comments HTML {index}"
        );
        assert_eq!(comments_text, case.comments_text, "comments text {index}");
        assert_eq!(document.to_html(), case.mutated, "content mutation {index}");
    }
}

#[test]
fn pinned_python_core_edges() {
    let reference = python_fixture().native_core;
    assert_eq!(reference.edge_sequences.len(), 162);
    for (index, expected) in reference.edge_sequences.into_iter().enumerate() {
        let mut document = crate::parse_html(&expected.html);
        let options = crate::Options {
            focus: [
                crate::ExtractionFocus::Balanced,
                crate::ExtractionFocus::FavorRecall,
                crate::ExtractionFocus::FavorPrecision,
            ][expected.focus],
            include_links: expected.flags & 2 != 0,
            original_url: crate::Url::parse("https://example.com/news/page"),
            ..Default::default()
        };
        let mut cache = crate::html_processing::Cache::new(4096);
        let result = crate::core::extraction_sequence(&mut document, 0, &mut cache, &options);
        let expected_content =
            if expected.flags & 1 == 0 && expected.html.contains("<figure><figure>") {
                expected
                    .content
                    .strip_prefix("Later image caption.")
                    .unwrap_or(&expected.content)
            } else {
                &expected.content
            };
        assert_python_selected_text(
            &result.content.document.text(result.content.root),
            expected_content,
            &format!("Python core edge {index}"),
        );
        let compact = |value: &str| value.split_whitespace().collect::<String>();
        if compact(&expected.snapshot) != compact(&expected.content)
            || expected_content != expected.content
        {
            assert_python_selected_text(
                &result.content_text,
                expected_content,
                &format!("Go final-body edge snapshot {index}"),
            );
        } else {
            assert_eq!(
                text::trim(&result.content_text),
                text::trim(&expected.snapshot),
                "Python core edge snapshot {index}"
            );
        }
        assert_eq!(
            result.comments_text, expected.comments_snapshot,
            "Python core edge comments snapshot {index}"
        );
        let comments = result
            .comments
            .map(|tree| tree.document.text(tree.root))
            .unwrap_or_default();
        assert_python_selected_text(
            &comments,
            &expected.comments,
            &format!("Python core edge comments {index}"),
        );
    }
}

#[test]
fn pinned_python_text_filter() {
    let reference = python_fixture();
    assert_eq!(reference.text_filters.len(), 349);
    for case in reference.text_filters {
        let mut document = crate::parse_html("");
        let element = document.create_element("p");
        etree::set_text(&mut document, element, &case.text);
        assert_eq!(
            crate::html_processing::text_filter(&document, element),
            case.filtered,
            "Python text filter {:?}",
            case.text
        );
    }
}

#[test]
fn current_go_content_measures_final_body() {
    for (index, case) in python_fixture().content_snapshots.into_iter().enumerate() {
        let options = crate::Options {
            focus: [
                crate::ExtractionFocus::Balanced,
                crate::ExtractionFocus::FavorRecall,
                crate::ExtractionFocus::FavorPrecision,
            ][case.focus],
            exclude_comments: true,
            ..Default::default()
        };
        let mut document = crate::parse_html(&case.html);
        let mut cache = crate::html_processing::Cache::new(4096);
        crate::html_processing::doc_cleaning(&mut document, 0, &options);
        crate::html_processing::convert_tags(&mut document, 0, &options);
        let (body, snapshot) =
            crate::main_extractor::extract_content(&mut document, 0, &mut cache, &options);
        assert_eq!(
            snapshot,
            etree::extraction_text(&document, body),
            "final body {index}"
        );
    }
}

#[test]
fn pinned_python_content_snapshots() {
    let reference = python_fixture();
    assert_eq!(reference.content_snapshots.len(), 27);
    let short_paragraph = "A substantial repeated article paragraph. "
        .repeat(2)
        .trim()
        .to_owned();
    let medium_paragraph = "A substantial repeated article paragraph. "
        .repeat(4)
        .trim()
        .to_owned();
    let short_baseline = format!("{short_paragraph}\n{short_paragraph}\n{short_paragraph}\ndata");
    for (index, case) in reference.content_snapshots.into_iter().enumerate() {
        let options = crate::Options {
            focus: [
                crate::ExtractionFocus::Balanced,
                crate::ExtractionFocus::FavorRecall,
                crate::ExtractionFocus::FavorPrecision,
            ][case.focus],
            exclude_comments: true,
            ..Default::default()
        };
        let mut document = crate::parse_html(&case.html);
        let mut cache = crate::html_processing::Cache::new(4096);
        crate::html_processing::prepare_core(&mut document, 0, &options);
        let (body, snapshot) =
            crate::main_extractor::extract_content(&mut document, 0, &mut cache, &options);
        assert_eq!(
            case.snapshot.chars().count(),
            case.length,
            "frozen Python length {index}"
        );
        assert_eq!(
            snapshot,
            etree::extraction_text(&document, body),
            "Go final-body snapshot {index}"
        );
        assert_eq!(
            snapshot.split_whitespace().collect::<Vec<_>>(),
            case.cleaned.split_whitespace().collect::<Vec<_>>(),
            "Go cleaned snapshot {index}"
        );
        assert!(
            snapshot.chars().count() <= case.length,
            "Go final-body length {index}"
        );
        assert_eq!(
            text::trim(&etree::iter_text(&document, body, " ")),
            case.cleaned,
            "Python cleaned text {index}"
        );
        let mut document = crate::parse_html(&case.html);
        let mut cache = crate::html_processing::Cache::new(4096);
        let sequence = crate::core::extraction_sequence(&mut document, 0, &mut cache, &options);
        let expected_sequence = match index {
            6 | 7 => short_baseline.as_str(),
            12 | 13 | 15 | 16 => medium_paragraph.as_str(),
            _ => case.sequence_cleaned.as_str(),
        };
        assert_eq!(
            sequence.content_text.split_whitespace().collect::<Vec<_>>(),
            expected_sequence.split_whitespace().collect::<Vec<_>>(),
            "Go final-body sequence snapshot {index}"
        );
        assert_eq!(
            etree::iter_text(&sequence.content.document, sequence.content.root, " ")
                .split_whitespace()
                .collect::<Vec<_>>(),
            expected_sequence.split_whitespace().collect::<Vec<_>>(),
            "Go recovery sequence text tokens {index}"
        );
    }
}

#[test]
fn pinned_go_baseline_recovery() {
    let reference = fixture();
    assert_eq!(reference.baseline.len(), 27);
    for (index, case) in reference.baseline.into_iter().enumerate() {
        let mut document = crate::parse_html(&case.html);
        let original = document.to_html();
        let (bodies, teasers) = crate::baseline::collect_json_content(&document, 0);
        assert_eq!(
            bodies,
            case.bodies.unwrap_or_default(),
            "JSON bodies {index}"
        );
        assert_eq!(
            teasers,
            case.teasers.unwrap_or_default(),
            "JSON teasers {index}"
        );
        let (body, value) = crate::baseline::baseline(&mut document, 0);
        assert_eq!(
            document.outer_html(body),
            case.output,
            "baseline body {index}"
        );
        assert_eq!(value, case.text, "baseline text {index}");
        assert_eq!(
            crate::baseline::plain_text(&document, Some(body)),
            case.plain,
            "plain text {index}"
        );
        let unchanged = document.clone();
        assert_eq!(
            crate::baseline::html_to_text_readonly(&document, 0),
            case.flat,
            "read-only HTML text {index}"
        );
        assert_eq!(document, unchanged);
        assert_eq!(
            crate::baseline::html_to_text(&mut document, 0),
            case.flat,
            "HTML text {index}"
        );
        assert_eq!(document.to_html(), original, "baseline caller {index}");
        crate::baseline::basic_cleaning(&mut document, 0);
        assert_eq!(
            document.to_html(),
            case.cleaned,
            "baseline cleaning {index}"
        );
    }
    for case in reference.baseline_text {
        assert_eq!(
            crate::baseline::render_baseline_text(&case.input),
            case.output,
            "render baseline {:?}",
            case.input
        );
    }
}

#[test]
fn pinned_go_post_cleaning() {
    let reference = fixture();
    assert_eq!(reference.post_cleaning.len(), 34);
    for (index, case) in reference.post_cleaning.into_iter().enumerate() {
        let mut document = crate::parse_html(&case.input);
        crate::html_processing::post_cleaning(&mut document, 0);
        assert_eq!(document.to_html(), case.output, "post cleaning {index}");
    }
}

#[test]
fn pinned_go_fallback_selection_and_sanitization() {
    let reference = fixture();
    assert_eq!(reference.fallback_selection.len(), 900);
    assert_eq!(reference.sanitization.len(), 576);
    let focuses = [
        crate::ExtractionFocus::Balanced,
        crate::ExtractionFocus::FavorRecall,
        crate::ExtractionFocus::FavorPrecision,
    ];
    for (index, case) in reference.fallback_selection.into_iter().enumerate() {
        let candidate = crate::parse_html(&case.candidate);
        let extracted = crate::parse_html(&case.extracted);
        let options = crate::Options {
            config: Some(Config {
                min_extracted_size: case.minimum,
                ..Default::default()
            }),
            focus: focuses[case.focus],
            ..Default::default()
        };
        assert_eq!(
            text::trim(&etree::iter_text(&candidate, 0, " "))
                .chars()
                .count(),
            case.candidate_length
        );
        assert_eq!(
            text::trim(&etree::iter_text(&extracted, 0, " "))
                .chars()
                .count(),
            case.extracted_length
        );
        assert_eq!(
            crate::external::candidate_is_usable(
                &candidate,
                0,
                &extracted,
                0,
                case.candidate_length,
                case.extracted_length,
                &options
            ),
            case.usable,
            "candidate {index}"
        );
    }
    for (index, case) in reference.sanitization.into_iter().enumerate() {
        let mut document = crate::parse_html(&case.html);
        let options = crate::Options {
            focus: focuses[case.focus],
            include_images: case.flags & 1 != 0,
            include_links: case.flags & 2 != 0,
            exclude_tables: case.flags & 4 != 0,
            ..Default::default()
        };
        crate::external::sanitize_tree(&mut document, 0, &options);
        assert_eq!(document.to_html(), case.output, "sanitize {index}");
    }
}

#[test]
fn pinned_go_native_fallbacks() {
    let reference = fixture();
    assert_eq!(reference.native_fallbacks.len(), 1152);
    let focuses = [
        crate::ExtractionFocus::Balanced,
        crate::ExtractionFocus::FavorRecall,
        crate::ExtractionFocus::FavorPrecision,
    ];
    for (index, case) in reference.native_fallbacks.into_iter().enumerate() {
        let original = crate::parse_html(&case.html);
        let before = original.clone();
        let mut extracted = crate::parse_html(&case.extracted);
        let root = extracted.tagged(0, "body")[0];
        let mut options = crate::Options {
            focus: focuses[case.focus],
            include_images: case.flags & 1 != 0,
            include_links: case.flags & 2 != 0,
            original_url: Some(crate::Url::parse("https://example.com/news/page").unwrap()),
            ..Default::default()
        };
        if case.variant != 0 {
            let document = crate::parse_html(&format!("<div><h2>Custom candidate</h2><p>{}</p><a href='../more'>more</a><img src='image.jpg'></div>", "Custom candidate text. ".repeat(20)));
            let candidate = crate::Tree {
                root: document.tagged(0, "div")[0],
                document,
            };
            let mut candidates = crate::FallbackCandidates::default();
            if case.variant == 1 {
                let mut document = Document { nodes: Vec::new() };
                let root = document.create_element("div");
                candidates.others = vec![crate::Tree { document, root }, candidate];
            } else if case.variant == 2 {
                candidates.readability = Some(candidate.clone());
                candidates.distiller = Some(candidate)
            }
            options.fallback_candidates = Some(candidates);
        }
        let candidates_before = options.fallback_candidates.clone();
        let (body, content) = crate::external::compare_external_extraction(
            &original,
            0,
            &mut extracted,
            root,
            &options,
        );
        assert_eq!(
            extracted.outer_html(body),
            case.output,
            "fallback output {index}"
        );
        assert_eq!(content, case.text, "fallback text {index}");
        let rescued = crate::external::distiller_rescue(&original, 0, &options);
        assert_eq!(
            rescued
                .as_ref()
                .map(|(body, _)| body.document.outer_html(body.root))
                .unwrap_or_default(),
            case.rescued,
            "rescue output {index}"
        );
        assert_eq!(
            rescued.map(|(_, content)| content).unwrap_or_default(),
            case.rescued_text,
            "rescue text {index}"
        );
        assert_eq!(original, before, "fallback original {index}");
        if let Some(before) = candidates_before {
            let after = options.fallback_candidates.unwrap();
            assert_eq!(after.others, before.others);
            assert_eq!(after.readability, before.readability);
            assert_eq!(after.distiller, before.distiller);
        }
    }
}

#[test]
fn pinned_go_language_and_forum_gates() {
    let reference = fixture();
    let python = python_fixture();
    assert_eq!(reference.language_gates.len(), 192);
    assert_eq!(reference.languages.len(), 81);
    assert_eq!(reference.forums.len(), 30);
    for (index, case) in reference.language_gates.into_iter().enumerate() {
        let document = crate::parse_html(&case.html);
        let options = crate::Options {
            target_language: case.target,
            ..Default::default()
        };
        assert_eq!(
            crate::core::check_html_language(&document, 0, &options, case.strict),
            case.accepted,
            "language gate {index}"
        );
    }
    for (index, case) in reference.languages.into_iter().enumerate() {
        let upstream = python
            .languages
            .iter()
            .find(|upstream| upstream.content == case.content && upstream.comments == case.comments)
            .expect("saved Go language input must also be checked against Python");
        if case.language != upstream.language {
            assert_eq!(
                case.content.chars().count(),
                case.comments.chars().count(),
                "unexpected Go/Python difference {index}"
            );
        }
        assert_eq!(
            crate::core::language_classifier(&case.content, &case.comments),
            upstream.language,
            "Python authority for saved Go language case {index}"
        );
    }
    for (index, case) in python.forums.into_iter().enumerate() {
        let document = crate::parse_html(&case.html);
        assert_eq!(
            crate::core::forum_thread_page(&document, 0),
            case.forum,
            "Python forum {index}"
        );
    }
}

fn python_fixture() -> PythonFixture {
    let reference: PythonFixture =
        serde_json::from_str(include_str!("../testdata/python-reference.json")).unwrap();
    assert_eq!(reference.commit, "c1bc9531a2a978326112ca9987e1382745116136");
    assert_eq!(reference.python, "3.12.13");
    assert_eq!(reference.packages["py3langid"], "0.4.0");
    reference
}

#[test]
fn pinned_python_language_classifier() {
    let reference = python_fixture();
    assert_eq!(reference.languages.len(), 324);
    for (index, case) in reference.languages.into_iter().enumerate() {
        assert_eq!(
            crate::core::language_classifier(&case.content, &case.comments),
            case.language,
            "Python language {index}: content={:?} comments={:?}",
            case.content,
            case.comments
        );
    }
}

#[test]
fn pinned_python_language_extraction() {
    let reference = python_fixture();
    assert_eq!(reference.language_extraction.len(), 36);
    for (index, case) in reference.language_extraction.into_iter().enumerate() {
        let document = crate::parse_html(&case.html);
        let before = document.clone();
        let options = crate::Options {
            target_language: case.target.unwrap_or_default(),
            enable_fallback: !case.fast,
            html_date_mode: HtmlDateMode::Disabled,
            ..Default::default()
        };
        let result = crate::extract_document(&document, &options);
        assert_eq!(
            document, before,
            "Python language extraction caller {index}"
        );
        assert_eq!(
            result.is_ok(),
            case.accepted,
            "Python language extraction {index}: {result:?}"
        );
        if let Ok(result) = result {
            let expected_language = if options.target_language.is_empty() {
                assert!(case.language.as_deref().unwrap_or_default().is_empty());
                match index {
                    0 | 1 | 4 | 5 | 8 | 9 => "en",
                    12 | 13 | 16 | 17 | 20 | 21 => "fr",
                    24 | 25 | 28 | 29 | 32 | 33 => "es",
                    _ => panic!("unexpected no-target language case {index}"),
                }
            } else {
                case.language.as_deref().unwrap_or_default()
            };
            assert_eq!(
                result.metadata.language, expected_language,
                "Go automatic language metadata {index}"
            );
            assert_eq!(
                result.content_text, case.content,
                "Python language body {index}"
            );
            assert_eq!(
                result.comments_text, case.comments,
                "Python language comments {index}"
            );
        }
    }
}

#[test]
fn current_go_leading_bom_is_document_text() {
    let html = "\u{feff}<!doctype html><html><head><title>Head title</title>\
                <meta name='author' content='Jane Example'>\
                <meta name='keywords' content='subject'></head>\
                <body><article><p>This article explains a scientific experiment and its results for interested readers.</p></article></body></html>";
    let document = crate::parse_html(html);
    let head = document.tagged(0, "head")[0];
    let body = document.tagged(0, "body")[0];
    assert!(document.nodes[head].children.is_empty());
    assert!(document.text(body).starts_with('\u{feff}'));
    let options = crate::Options {
        html_date_mode: HtmlDateMode::Disabled,
        exclude_comments: true,
        ..Default::default()
    };
    let from_document = crate::extract_document(&document, &options).unwrap();
    let from_reader = crate::extract(html.as_bytes(), &options).unwrap();
    assert!(from_document.metadata.title.is_empty());
    assert!(from_document.metadata.author.is_empty());
    assert!(from_document.metadata.tags.is_empty());
    assert_eq!(from_document.metadata, from_reader.metadata);
    assert_eq!(from_document.content_text, from_reader.content_text);
    assert!(from_reader
        .content_text
        .contains("This article explains a scientific experiment"));
}

#[test]
fn encoding_maximum_confidence_matches_full_detection() {
    let samples: Vec<Vec<u8>> = vec![
        Vec::new(),
        b"Plain ASCII text.".to_vec(),
        "\u{feff}Plain ASCII text.".as_bytes().to_vec(),
        "\u{e9}\u{e8}\u{ea}\u{eb}".as_bytes().to_vec(),
        "\u{feff}\u{e9}\u{e8}\u{ea}\u{eb}\x1b$B\x1b$B\x1b$B\x1b$B\x1b$B"
            .as_bytes()
            .to_vec(),
        vec![0xff, 0xfe, b'A', 0, b'B', 0],
        vec![0xfe, 0xff, 0, b'A', 0, b'B'],
    ];
    for sample in samples {
        for byte in 0..=u8::MAX {
            let mut input = sample.clone();
            input.push(byte);
            let mut expected = ("", 0);
            for score in crate::encoding::scores(&input) {
                assert!((0..=100).contains(&score.1));
                if score.1 > expected.1 {
                    expected = score;
                }
            }
            let actual = crate::encoding::decode(&input);
            if expected.1 == 0 {
                assert!(matches!(actual, Err(crate::Error::CharsetNotDetected)));
            } else {
                assert_eq!(
                    actual.unwrap(),
                    crate::encoding::decode_as(&input, expected.0),
                    "input {input:?}"
                );
            }
        }
    }
}

#[test]
fn encoding_normalization_matches_full_pipeline() {
    use unicode_normalization::UnicodeNormalization;

    let mut samples = vec![
        String::new(),
        "Plain ASCII text.".into(),
        "\u{feff}A\u{e9}\u{f1}\u{b0}".into(),
        "\u{1100}\u{1161}\u{11a8}".into(),
        "\u{ac01}\u{ad}\u{0301}".into(),
        "a\u{ad}\u{301}e\u{ad}\u{308}".into(),
    ];
    for length in [0, 1, 29, 30, 31, 32, 60, 61] {
        for mark in ['\u{300}', '\u{344}', '\u{5b0}', '\u{1d165}'] {
            for separator in ["", "\u{ad}", "\u{34f}", "\u{c0}", "\u{1100}\u{1161}"] {
                let marks = mark.to_string().repeat(length);
                samples.push(format!("a{marks}{separator}{marks}z"));
            }
        }
    }
    for scalar in 0..=0x10ffff {
        if let Some(character) = char::from_u32(scalar) {
            let sample = character.to_string();
            let expected: String = sample
                .nfd()
                .stream_safe()
                .filter(|&value| value != '\u{ad}')
                .stream_safe()
                .nfc()
                .collect();
            assert_eq!(
                crate::encoding::decode_as(sample.as_bytes(), "UTF-8"),
                expected,
                "scalar {scalar:x}"
            );
        }
    }
    for sample in samples {
        let expected: String = sample
            .nfd()
            .stream_safe()
            .filter(|&value| value != '\u{ad}')
            .stream_safe()
            .nfc()
            .collect();
        assert_eq!(
            crate::encoding::decode_as(sample.as_bytes(), "UTF-8"),
            expected,
            "input {sample:?}"
        );
    }
}

#[test]
fn reader_input_contract() {
    use std::io::{Read, Write};

    struct OneByteReader(std::io::Cursor<Vec<u8>>);
    impl Read for OneByteReader {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            let count = output.len().min(1);
            self.0.read(&mut output[..count])
        }
    }
    struct FailedReader;
    impl Read for FailedReader {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("reader failure"))
        }
    }
    let options = crate::Options {
        html_date_mode: HtmlDateMode::Disabled,
        ..Default::default()
    };
    let source = "<html><body><p>A\u{308}ffin and Caf\u{e9}. Soft\u{ad}hyphen.</p></body></html>";
    let logged = crate::Options {
        enable_log: true,
        ..options.clone()
    };
    assert_eq!(
        crate::extract(source.as_bytes(), &logged)
            .unwrap()
            .content_text,
        crate::extract(source.as_bytes(), &options)
            .unwrap()
            .content_text
    );
    for label in ["", "utf-8", " UTF-8 "] {
        let options = crate::Options {
            input_encoding: label.into(),
            ..options.clone()
        };
        let result = crate::extract(source.as_bytes(), &options).unwrap();
        assert_eq!(result.content_text, "\u{c4}ffin and Caf\u{e9}. Softhyphen.");
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(source.as_bytes()).unwrap();
        let compressed = encoder.finish().unwrap();
        let result = crate::extract(
            OneByteReader(std::io::Cursor::new(compressed.clone())),
            &options,
        )
        .unwrap();
        assert_eq!(result.content_text, "\u{c4}ffin and Caf\u{e9}. Softhyphen.");
        assert!(matches!(
            crate::extract(&compressed[..compressed.len() - 4], &options),
            Err(crate::Error::Io(_))
        ));
    }
    let latin = crate::Options {
        input_encoding: "windows-1252".into(),
        ..options.clone()
    };
    assert_eq!(
        crate::extract(b"<p>Caf\xe9.</p>".as_slice(), &latin)
            .unwrap()
            .content_text,
        "Caf\u{e9}."
    );
    for label in ["latin1", "iso-8859-1", "us-ascii", "cp1252"] {
        let options = crate::Options {
            input_encoding: label.into(),
            ..options.clone()
        };
        assert_eq!(
            crate::extract(b"<p>\x80 Caf\xe9.</p>".as_slice(), &options)
                .unwrap()
                .content_text,
            "\u{20ac} Caf\u{e9}."
        );
    }
    let mut concatenated = Vec::new();
    for part in ["<p>Concatenated ", "gzip members.</p>"] {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(part.as_bytes()).unwrap();
        concatenated.extend(encoder.finish().unwrap());
    }
    assert_eq!(
        crate::extract(
            OneByteReader(std::io::Cursor::new(concatenated.clone())),
            &options
        )
        .unwrap()
        .content_text,
        "Concatenated gzip members."
    );
    let checksum = concatenated.len() - 8;
    concatenated[checksum] ^= 1;
    assert!(matches!(
        crate::extract(concatenated.as_slice(), &options),
        Err(crate::Error::Io(_))
    ));
    assert!(matches!(
        crate::extract([0x1f, 0x8b].as_slice(), &options),
        Err(crate::Error::Io(_))
    ));
    assert!(
        matches!(crate::extract(FailedReader, &options), Err(crate::Error::Io(error)) if error.to_string() == "reader failure")
    );
    let invalid = crate::Options {
        input_encoding: "not-a-charset".into(),
        ..options
    };
    assert!(matches!(
        crate::extract(FailedReader, &invalid),
        Err(crate::Error::UnsupportedCharset(_))
    ));
}

#[test]
fn pinned_python_metadata_attributes() {
    let reference = python_fixture();
    assert_eq!(reference.metadata_attributes.len(), 60);
    for (index, case) in reference.metadata_attributes.into_iter().enumerate() {
        let document = crate::parse_html(&case.html);
        let options = crate::Options {
            html_date_mode: HtmlDateMode::Disabled,
            ..Default::default()
        };
        let result = metadata::extract_metadata(&document, &options);
        let expected_author = if matches!(index, 5 | 6 | 25 | 26 | 45 | 46) {
            assert!(case.author.as_deref().unwrap_or_default().is_empty());
            "Maria Example"
        } else {
            case.author.as_deref().unwrap_or_default()
        };
        assert_eq!(result.author, expected_author, "{}", case.html);
    }
    let document = crate::parse_html(
        r#"<html><body><div id=" author "><span class=" title ">Article title</span><span class=" date ">Date label</span><section id=" comments-thread ">Other Writer</section><a class=" username ">Jane Example</a></div></body></html>"#,
    );
    let before = document.clone();
    let result = metadata::extract_metadata(
        &document,
        &crate::Options {
            html_date_mode: HtmlDateMode::Disabled,
            ..Default::default()
        },
    );
    assert_eq!(result.author, "Jane Example");
    assert_eq!(document, before);
}

#[test]
fn current_go_metadata_selector_whitespace() {
    let document = crate::parse_html(
        "<html><head><title>Fallback title</title></head><body>\
         <div class=' \tentry-title\n '>Selected title</div>\
         <div class=' \ttags\n '><a href='/category/selected'>Selected category</a>\
         <a href='/tag/selected'>Selected tag</a></div></body></html>",
    );
    let before = document.clone();
    assert_eq!(metadata::extract_dom_title(&document), "Selected title");
    assert_eq!(
        metadata::extract_dom_categories(&document),
        ["Selected category"]
    );
    assert_eq!(metadata::extract_dom_tags(&document), ["Selected tag"]);
    assert_eq!(document, before);
    for attributes in [
        "id=' \tpostpath-details\n '",
        "class=' \tpost-info\n details '",
    ] {
        let document = crate::parse_html(&format!(
            "<div {attributes}><a href='/category/selected'>Selected category</a></div>"
        ));
        assert_eq!(
            metadata::extract_dom_categories(&document),
            ["Selected category"]
        );
    }
}

#[test]
fn current_go_extraction_sequence() {
    let reference = worktree_fixture();
    assert_eq!(reference.sequences.len(), 4992);
    let focuses = [
        crate::ExtractionFocus::Balanced,
        crate::ExtractionFocus::FavorRecall,
        crate::ExtractionFocus::FavorPrecision,
    ];
    for (index, case) in reference.sequences.into_iter().enumerate() {
        let mut document = crate::parse_html(&case.html);
        let options = crate::Options {
            focus: focuses[case.focus],
            include_images: case.flags & 1 != 0,
            include_links: case.flags & 2 != 0,
            deduplicate: case.flags & 4 != 0,
            enable_fallback: case.flags & 8 != 0,
            exclude_comments: case.flags & 16 != 0,
            exclude_tables: case.flags & 32 != 0,
            original_url: Some(crate::Url::parse("https://example.com/news/page").unwrap()),
            ..Default::default()
        };
        let mut cache = crate::html_processing::Cache::new(4096);
        let result = crate::core::extraction_sequence(&mut document, 0, &mut cache, &options);
        assert_eq!(
            result.content.document.outer_html(result.content.root),
            case.content,
            "current Go sequence content {index}"
        );
        assert_eq!(
            result.content_text, case.content_text,
            "current Go sequence text {index}"
        );
        assert_eq!(
            result
                .comments
                .as_ref()
                .map(|body| body.document.outer_html(body.root))
                .unwrap_or_default(),
            case.comments,
            "current Go sequence comments {index}"
        );
        assert_eq!(
            result.comments_text, case.comments_text,
            "current Go sequence comment text {index}"
        );
        assert_eq!(
            document.to_html(),
            case.mutated,
            "sequence mutations {index}"
        );
    }
}

#[test]
fn pinned_go_css_pruning() {
    let reference = fixture();
    assert_eq!(reference.css.len(), 568);
    for (index, case) in reference.css.into_iter().enumerate() {
        let mut document = crate::parse_html(&case.html);
        let selector = crate::css::Selector::parse(&case.selector);
        assert_eq!(
            selector.is_some(),
            case.valid,
            "CSS parse {index}: {}",
            case.selector
        );
        if let Some(selector) = selector {
            let matches: Vec<_> = document
                .elements(0)
                .into_iter()
                .filter(|&element| selector.matches(&document, element))
                .map(|element| document.outer_html(element))
                .collect();
            assert_eq!(
                matches,
                case.matches.unwrap_or_default(),
                "CSS matches {index}: {}",
                case.selector
            );
            selector.prune(&mut document, 0);
        }
        assert_eq!(
            document.to_html(),
            case.pruned,
            "CSS prune {index}: {}",
            case.selector
        );
    }
}

#[test]
fn pinned_go_css_regular_expressions() {
    let reference = fixture();
    assert!(reference.css_regex.len() > 3000);
    let mut mismatches = Vec::new();
    for (index, case) in reference.css_regex.into_iter().enumerate() {
        let compiled = crate::css::go_regex(&case.pattern);
        let valid = compiled.is_some();
        let matches = compiled.is_some_and(|compiled| compiled.is_match(&case.input));
        if valid != case.valid || matches != case.matches {
            mismatches.push(format!(
                "{index}: {:?} on {:?}: valid {valid}/{} match {matches}/{}",
                case.pattern, case.input, case.valid, case.matches
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} regex mismatches:\n{}",
        mismatches.len(),
        mismatches
            .iter()
            .take(60)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn current_go_document_extraction() {
    let reference = worktree_fixture();
    assert_eq!(reference.extraction.len(), 6177);
    let focuses = [
        crate::ExtractionFocus::Balanced,
        crate::ExtractionFocus::FavorRecall,
        crate::ExtractionFocus::FavorPrecision,
    ];
    for (index, case) in reference.extraction.into_iter().enumerate() {
        let document = crate::parse_html(&case.html);
        let before = document.clone();
        let mut options = crate::Options {
            focus: focuses[case.focus],
            include_images: case.flags & 1 != 0,
            include_links: case.flags & 2 != 0,
            deduplicate: case.flags & 4 != 0,
            enable_fallback: case.flags & 8 != 0,
            exclude_comments: case.flags & 16 != 0,
            exclude_tables: case.flags & 32 != 0,
            html_date_mode: HtmlDateMode::Disabled,
            ..Default::default()
        };
        if case.variant != 1 {
            options.original_url = crate::Url::parse("https://example.com/news/page")
        }
        options.has_essential_metadata = matches!(case.variant, 1 | 2);
        if case.variant == 3 {
            options.target_language = "en".into()
        }
        if case.variant == 4 {
            options.max_tree_size = 1
        }
        if case.variant == 5 {
            options.config = Some(Config {
                min_output_size: 50000,
                min_output_comment_size: 50000,
                ..Default::default()
            })
        }
        if case.variant == 6 {
            options.prune_selector = "p.bar, aside".into()
        }
        if case.variant == 7 {
            options.prune_selector = "p:not(".into()
        }
        match crate::extract_document(&document, &options) {
            Ok(result) => {
                assert!(
                    case.error.is_empty(),
                    "expected error {index}: {}",
                    case.error
                );
                assert_eq!(
                    result
                        .content_node
                        .document
                        .outer_html(result.content_node.root),
                    case.content,
                    "current Go extraction content {index}"
                );
                assert_eq!(
                    result
                        .comments_node
                        .as_ref()
                        .map(|body| body.document.outer_html(body.root))
                        .unwrap_or_default(),
                    case.comments,
                    "current Go extraction comments {index}"
                );
                assert_eq!(
                    result.content_text, case.content_text,
                    "current Go extraction text {index}"
                );
                assert_eq!(
                    result.comments_text, case.comments_text,
                    "current Go extraction comment text {index}"
                );
                assert_metadata(
                    &result.metadata,
                    case.metadata,
                    &format!("current Go extraction metadata {index}"),
                );
            }
            Err(error) => {
                assert_eq!(
                    error.to_string(),
                    case.error,
                    "current Go extraction error {index}"
                );
            }
        }
        assert_eq!(document, before, "extraction caller {index}");
    }
    assert!(matches!(
        crate::extract_document(&Document { nodes: Vec::new() }, &crate::Options::default()),
        Err(crate::Error::MissingDocument)
    ));
}

#[test]
fn pinned_go_json_ld() {
    for (index, case) in fixture().metadata.into_iter().enumerate() {
        let document = Document::parse(&case.html);
        let original = document.clone();
        let result = metadata::extract_json_ld(&document, metadata::examine_meta(&document));
        assert_metadata(&result, case.json_ld, &format!("JSON-LD {index}"));
        assert_eq!(document, original);
    }
}

#[test]
fn pinned_go_meta_tags() {
    for (index, case) in fixture().metadata.into_iter().enumerate() {
        let document = Document::parse(&case.html);
        let original = document.clone();
        assert_metadata(
            &metadata::extract_open_graph_meta(&document),
            case.open_graph,
            &format!("OpenGraph {index}"),
        );
        assert_metadata(
            &metadata::examine_meta(&document),
            case.meta,
            &format!("meta tags {index}"),
        );
        assert_eq!(document, original);
    }
}

fn assert_metadata(actual: &metadata::Metadata, mut expected: Value, context: &str) {
    for field in ["Categories", "Tags"] {
        if expected[field].is_null() {
            expected[field] = json!([]);
        }
    }
    assert_eq!(
        json!({
            "Title": actual.title,
            "Author": actual.author,
            "URL": actual.url,
            "Hostname": actual.hostname,
            "Description": actual.description,
            "Sitename": actual.sitename,
            "Date": actual.date.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true),
            "Categories": actual.categories,
            "Tags": actual.tags,
            "ID": actual.id,
            "Fingerprint": actual.fingerprint,
            "License": actual.license,
            "Language": actual.language,
            "Image": actual.image,
            "PageType": actual.page_type,
        }),
        expected,
        "{context}"
    );
}

#[test]
fn pinned_go_metadata_helpers() {
    for (index, case) in fixture().metadata.into_iter().enumerate() {
        let document = Document::parse(&case.html);
        let original = document.clone();
        assert_eq!(
            metadata::examine_title_element(&document),
            case.title_parts,
            "title parts {index}"
        );
        assert_eq!(
            metadata::extract_dom_title(&document),
            case.title,
            "title {index}"
        );
        assert_eq!(
            metadata::extract_dom_sitename(&document),
            case.sitename,
            "sitename {index}"
        );
        assert_eq!(
            metadata::extract_license(&document),
            case.license,
            "license {index}"
        );
        assert_eq!(
            metadata::extract_dom_url(&document),
            case.dom_url,
            "DOM URL {index}"
        );
        assert_eq!(
            metadata::extract_dom_author(&document),
            case.dom_author,
            "DOM author {index}"
        );
        assert_eq!(
            metadata::extract_dom_categories(&document),
            case.categories.unwrap_or_default(),
            "DOM categories {index}"
        );
        assert_eq!(
            metadata::extract_dom_tags(&document),
            case.tags.unwrap_or_default(),
            "DOM tags {index}"
        );
        assert_eq!(document, original);
    }
}

#[test]
fn pinned_go_date_integration() {
    use chrono::{DateTime, SecondsFormat, TimeZone};
    use rust_htmldate::Timezone;

    for (index, case) in fixture().dates.into_iter().enumerate() {
        let document = rust_htmldate::Document::parse(&case.html);
        let original = document.clone();
        let extraction_document = Document::parse(&case.html);
        let original_extraction_document = extraction_document.clone();
        let mut options = DateOptions {
            enable_fallback: case.fallback,
            html_date_mode: [
                HtmlDateMode::Default,
                HtmlDateMode::Fast,
                HtmlDateMode::Extensive,
                HtmlDateMode::Disabled,
            ][case.mode],
            ..Default::default()
        };
        if case.custom != 0 {
            options.html_date_options = Some(rust_htmldate::Options {
                extract_time: true,
                use_original_date: case.custom == 1,
                skip_extensive_search: case.custom == 1,
                url: "https://ignored.example/2010/01/02".into(),
                min_date: Some(Timezone::Utc.with_ymd_and_hms(1995, 1, 1, 0, 0, 0).unwrap()),
                max_date: Some(
                    Timezone::Utc
                        .with_ymd_and_hms(2025, 12, 31, 23, 59, 59)
                        .unwrap(),
                ),
                ..Default::default()
            });
        }
        if case.overridden != 0 {
            let date = DateTime::parse_from_rfc3339("2012-06-07T08:09:10+02:00").unwrap();
            options.html_date_override = Some(rust_htmldate::ExtractionResult {
                date_time: date
                    .with_timezone(&Timezone::fixed("", date.offset().local_minus_utc()).unwrap()),
                has_time: case.overridden == 2,
                has_timezone: true,
                src_string: String::new(),
            });
        }
        let date = metadata::extract_date(&document, &case.url, &options);
        assert_eq!(
            date.to_rfc3339_opts(SecondsFormat::AutoSi, true),
            case.date,
            "date case {index}"
        );
        let imported_date =
            metadata::extract_date_from_dom(&extraction_document, &case.url, &options);
        assert_eq!(
            imported_date.to_rfc3339_opts(SecondsFormat::AutoSi, true),
            case.date,
            "imported date case {index}"
        );
        assert_eq!(extraction_document, original_extraction_document);
        assert_eq!(document, original, "date extraction changed input {index}");
        if let Some(custom) = options.html_date_options {
            assert_eq!(custom.url, "https://ignored.example/2010/01/02");
            assert_eq!(custom.use_original_date, case.custom == 1);
            assert_eq!(custom.skip_extensive_search, case.custom == 1);
        }
    }
}
