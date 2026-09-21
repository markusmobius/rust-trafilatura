use crate::{
    baseline, etree, external,
    html_processing::{self, Cache},
    main_extractor, selector, text, Document, ExtractionFocus, NodeId, Options, Tree,
};
use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    UnsupportedCharset(String),
    CharsetNotDetected,
    MissingDocument,
    HtmlLanguage(String),
    RequiredTitle,
    RequiredUrl,
    RequiredDate,
    OutputTreeTooLong(usize),
    OutputTooShort(usize, usize),
    DuplicateBody,
    WrongLanguage { wanted: String, found: String },
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => std::fmt::Display::fmt(error, formatter),
            Self::UnsupportedCharset(label) => write!(formatter, "unsupported charset: {label:?}"),
            Self::CharsetNotDetected => formatter.write_str("Charset not detected."),
            Self::MissingDocument => formatter.write_str("HTML document is nil"),
            Self::HtmlLanguage(language) => {
                write!(formatter, "web page language is not {language}")
            }
            Self::RequiredTitle => formatter.write_str("title is required"),
            Self::RequiredUrl => formatter.write_str("url is required"),
            Self::RequiredDate => formatter.write_str("date is required"),
            Self::OutputTreeTooLong(count) => {
                write!(formatter, "output tree to long, discarding file : {count}")
            }
            Self::OutputTooShort(content, comments) => write!(
                formatter,
                "text and comments are not long enough: {content} {comments}"
            ),
            Self::DuplicateBody => formatter.write_str("extracted body has been duplicated"),
            Self::WrongLanguage { wanted, found } => {
                write!(formatter, "wrong language, want {wanted} got {found}")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Clone, Debug)]
pub struct ExtractResult {
    pub content_node: Tree,
    pub comments_node: Option<Tree>,
    pub content_text: String,
    pub comments_text: String,
    pub metadata: crate::Metadata,
}

pub fn extract_document(document: &Document, options: &Options) -> Result<ExtractResult, Error> {
    extract_node(document, 0, options)
}

pub fn extract_node(
    document: &Document,
    root: NodeId,
    options: &Options,
) -> Result<ExtractResult, Error> {
    if root >= document.nodes.len() {
        return Err(Error::MissingDocument);
    }
    extract_tree(copy_tree(document, root), options)
}

pub(crate) fn extract_tree(mut tree: Tree, options: &Options) -> Result<ExtractResult, Error> {
    let mut options = options.clone();
    let config = options.config.get_or_insert_default().clone();
    let mut cache = Cache::new(config.cache_size);
    if !options.target_language.is_empty()
        && !check_html_language(&tree.document, tree.root, &options, false)
    {
        return Err(Error::HtmlLanguage(options.target_language));
    }
    let mut metadata = crate::metadata::extract_metadata(&tree.document, &options);
    if options.has_essential_metadata {
        if metadata.title.is_empty() {
            return Err(Error::RequiredTitle);
        }
        if metadata.url.is_empty() {
            return Err(Error::RequiredUrl);
        }
        if metadata.date == rust_htmldate::ExtractionResult::default().date_time {
            return Err(Error::RequiredDate);
        }
    }
    if options.original_url.is_none() && !metadata.url.is_empty() {
        options.original_url = crate::Url::request(&metadata.url)
    }
    if !options.prune_selector.is_empty() {
        if let Some(selector) = crate::css::Selector::parse(&options.prune_selector) {
            selector.prune(&mut tree.document, tree.root)
        }
    }
    let mut sequence = extraction_sequence(&mut tree.document, tree.root, &mut cache, &options);
    let body = &mut sequence.content;
    if options.max_tree_size > 0
        && etree::children(&body.document, body.root).len() as i64 > options.max_tree_size
    {
        etree::strip_tags(
            &mut body.document,
            body.root,
            crate::settings::FORMAT_TAG_CATALOG,
        );
        let count = etree::children(&body.document, body.root).len();
        if count as i64 > options.max_tree_size {
            return Err(Error::OutputTreeTooLong(count));
        }
    }
    let len_text = sequence.content_text.chars().count();
    let len_comments = sequence.comments_text.chars().count();
    options.log(format_args!(
        "extracted {len_text} content characters and {len_comments} comment characters"
    ));
    if (len_comments as i64) < config.min_extracted_comment_size {
        options.log(format_args!("not enough comments"));
    }
    if (len_text as i64) < config.min_output_size
        && (len_comments as i64) < config.min_output_comment_size
    {
        return Err(Error::OutputTooShort(len_text, len_comments));
    }
    if options.deduplicate
        && html_processing::duplicate_test(&body.document, body.root, &mut cache, &options)
    {
        return Err(Error::DuplicateBody);
    }
    let language = language_classifier(&sequence.content_text, &sequence.comments_text);
    if !options.target_language.is_empty() && language != options.target_language {
        return Err(Error::WrongLanguage {
            wanted: options.target_language,
            found: language,
        });
    }
    metadata.language = language;
    html_processing::post_cleaning(&mut body.document, body.root);
    if let Some(comments) = sequence.comments.as_mut() {
        html_processing::post_cleaning(&mut comments.document, comments.root)
    }
    let content_text = baseline::plain_text(&body.document, Some(body.root));
    let comments_text = sequence
        .comments
        .as_ref()
        .map(|body| baseline::plain_text(&body.document, Some(body.root)))
        .unwrap_or_default();
    Ok(ExtractResult {
        content_node: sequence.content,
        comments_node: sequence.comments,
        content_text,
        comments_text,
        metadata,
    })
}

static HTML_LANGUAGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)[a-z]{2}").unwrap());
static LANGUAGE_IDENTIFIER: LazyLock<Result<rust_py3langid::Identifier, rust_py3langid::Error>> =
    LazyLock::new(rust_py3langid::Identifier::new);

pub(crate) fn check_html_language(
    document: &Document,
    root: NodeId,
    options: &Options,
    strict: bool,
) -> bool {
    let html = if document.nodes[root].tag == "html" {
        root
    } else {
        document
            .tagged(root, "html")
            .first()
            .copied()
            .unwrap_or(root)
    };
    let matches_language = |value: &str| {
        HTML_LANGUAGE
            .find_iter(value)
            .any(|language| text::to_lower(language.as_str()) == options.target_language)
    };
    for (key, value) in [
        ("http-equiv", "content-language"),
        ("property", "og:locale"),
    ] {
        let nodes: Vec<_> = document
            .tagged(root, "meta")
            .into_iter()
            .filter(|&element| {
                let node = &document.nodes[element];
                node.attr(key) == value && node.has_attr("content")
            })
            .collect();
        if !nodes.is_empty() {
            return nodes
                .iter()
                .any(|&element| matches_language(document.nodes[element].attr("content")));
        }
    }
    if strict && document.nodes[html].has_attr("lang") {
        return matches_language(document.nodes[html].attr("lang"));
    }
    true
}

pub(crate) fn language_classifier(content: &str, comments: &str) -> String {
    let value = if comments.chars().count() >= content.chars().count() {
        comments
    } else {
        content
    };
    LANGUAGE_IDENTIFIER
        .as_ref()
        .map(|identifier| identifier.identify(value).language)
        .unwrap_or_default()
}

pub(crate) fn forum_thread_page(document: &Document, root: NodeId) -> bool {
    static FORUM_TYPE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#""@type"[\s\x1c-\x1f]*:[\s\x1c-\x1f]*(?:"DiscussionForumPosting"|\[[^\]]*"DiscussionForumPosting")"#).unwrap()
    });
    for script in document.tagged(root, "script") {
        if document.nodes[script].attr("type") != "application/ld+json" {
            continue;
        }
        if FORUM_TYPE.is_match(&etree::text(document, script)) {
            return true;
        }
    }
    false
}

pub(crate) struct Sequence {
    pub content: Tree,
    pub content_text: String,
    pub comments: Option<Tree>,
    pub comments_text: String,
}

fn copy_tree(document: &Document, root: NodeId) -> Tree {
    let mut copied = Document { nodes: Vec::new() };
    let root = etree::import_tree(&mut copied, document, root);
    Tree {
        document: copied,
        root,
    }
}

fn prepare_tree(document: &Document, root: NodeId, options: &Options) -> Tree {
    let mut cleaned = copy_tree(document, root);
    html_processing::prepare_core(&mut cleaned.document, cleaned.root, options);
    cleaned
}

fn recall_retry(mut retry: Tree, options: &Options) -> ((Tree, String), Option<(Tree, String)>) {
    let rescue_options = options;
    let options = Options {
        focus: ExtractionFocus::FavorRecall,
        ..options.clone()
    };
    let mut cache = Cache::new(options.config.clone().unwrap_or_default().cache_size);
    let (mut cleaned, original) = if options.enable_fallback {
        (
            prepare_tree(&retry.document, retry.root, &options),
            Some(retry),
        )
    } else {
        html_processing::prepare_core(&mut retry.document, retry.root, &options);
        (retry, None)
    };
    let (body, content) =
        main_extractor::extract_content(&mut cleaned.document, cleaned.root, &mut cache, &options);
    cleaned.root = body;
    if let Some(original) = original {
        let (body, content) = external::compare_external_extraction(
            &original.document,
            original.root,
            &mut cleaned.document,
            body,
            &options,
        );
        cleaned.root = body;
        let distiller =
            external::distiller_rescue(&original.document, original.root, rescue_options);
        ((cleaned, content), distiller)
    } else {
        ((cleaned, content), None)
    }
}

pub(crate) fn extraction_sequence(
    document: &mut Document,
    mut root: NodeId,
    cache: &mut Cache,
    options: &Options,
) -> Sequence {
    let is_forum = forum_thread_page(document, root);
    if options.exclude_comments && (options.focus == ExtractionFocus::FavorPrecision || !is_forum) {
        root = html_processing::prune_unwanted_nodes(
            document,
            root,
            selector::REMOVED_COMMENTS,
            false,
        );
    }
    let mut cleaned = prepare_tree(document, root, options);
    let mut comments = None;
    let mut comments_text = String::new();
    let mut forum_posts = None;
    if !options.exclude_comments {
        let (body, content) =
            main_extractor::extract_comments(&mut cleaned.document, cleaned.root, cache, options);
        comments = body.map(|body| copy_tree(&cleaned.document, body));
        comments_text = content;
        if !comments_text.is_empty() && is_forum {
            forum_posts = comments.take();
            comments_text.clear();
            cleaned = prepare_tree(document, root, options);
        }
    }
    if options.focus == ExtractionFocus::FavorPrecision && !is_forum {
        cleaned.root = html_processing::prune_unwanted_nodes(
            &mut cleaned.document,
            cleaned.root,
            selector::REMOVED_COMMENTS,
            false,
        );
    }
    let (body, mut content) =
        main_extractor::extract_content(&mut cleaned.document, cleaned.root, cache, options);
    cleaned.root = body;
    if options.enable_fallback {
        (cleaned.root, content) = external::compare_external_extraction(
            document,
            root,
            &mut cleaned.document,
            cleaned.root,
            options,
        );
    }
    let mut length = content.chars().count();
    let min_size = options
        .config
        .clone()
        .unwrap_or_default()
        .min_extracted_size;
    if (length as i64) < min_size && options.focus != ExtractionFocus::FavorPrecision {
        options.log(format_args!("using baseline recovery"));
        let mut baseline = copy_tree(document, root);
        (baseline.root, content) = baseline::baseline_owned(&mut baseline.document, baseline.root);
        cleaned = baseline;
        length = content.chars().count();
        forum_posts = None;
    }
    if options.focus == ExtractionFocus::Balanced
        && length > 0
        && length < 3000
        && (length as f64)
            < 0.2
                * baseline::html_to_text_readonly(document, root)
                    .chars()
                    .count() as f64
    {
        options.log(format_args!("retrying with recall focus"));
        let mut retry = copy_tree(document, root);
        if !is_forum {
            retry.root = html_processing::prune_unwanted_nodes(
                &mut retry.document,
                retry.root,
                selector::REMOVED_COMMENTS,
                false,
            )
        }
        let ((retry_body, retry_text), distiller) = recall_retry(retry, options);
        let retry_length = retry_text.chars().count();
        let distiller_length = distiller
            .as_ref()
            .map_or(0, |(_, content)| content.chars().count());
        if distiller_length > retry_length && distiller_length > 2 * length {
            (cleaned, content) = distiller.unwrap();
            forum_posts = None;
        } else if retry_length as i64 >= min_size && retry_length as f64 > 1.5 * length as f64 {
            (cleaned, content) = (retry_body, retry_text);
            forum_posts = None;
        }
    }
    if let Some(posts) = forum_posts {
        let existing: Vec<_> = etree::children(&cleaned.document, cleaned.root)
            .iter()
            .map(|&element| text::trim(&cleaned.document.text(element)))
            .collect();
        let body_text = existing.join("\n");
        let posts = etree::import_tree(&mut cleaned.document, &posts.document, posts.root);
        let mut changed = false;
        for post in etree::children(&cleaned.document, posts) {
            let post_text = text::trim(&cleaned.document.text(post));
            if !post_text.is_empty() && !body_text.contains(&post_text) {
                etree::append(&mut cleaned.document, cleaned.root, post);
                changed = true;
            }
        }
        if changed {
            content = etree::extraction_text(&cleaned.document, cleaned.root);
        }
    }
    Sequence {
        content: cleaned,
        content_text: content,
        comments,
        comments_text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recall_retry_preserves_caller_rescue_focus() {
        let document = crate::parse_html(&format!(
            "<article><p>{}</p><p>Second paragraph.</p></article>",
            "Article sentence with enough content. ".repeat(20)
        ));
        let candidate_document = crate::parse_html(&format!(
            "<form><p>{}</p></form>",
            "Candidate content retained only by recall cleaning. ".repeat(20)
        ));
        for focus in [
            ExtractionFocus::Balanced,
            ExtractionFocus::FavorRecall,
            ExtractionFocus::FavorPrecision,
        ] {
            for enable_fallback in [false, true] {
                let options = Options {
                    focus,
                    enable_fallback,
                    fallback_candidates: Some(crate::FallbackCandidates {
                        distiller: Some(Tree {
                            document: candidate_document.clone(),
                            root: 0,
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                };
                let expected = enable_fallback
                    .then(|| external::distiller_rescue(&document, 0, &options))
                    .flatten();
                let ((_, _), actual) = recall_retry(copy_tree(&document, 0), &options);
                let rendered = |candidate: Option<(Tree, String)>| {
                    candidate.map(|(tree, text)| (tree.document.outer_html(tree.root), text))
                };
                assert_eq!(
                    rendered(actual),
                    rendered(expected),
                    "{focus:?}, fallback {enable_fallback}"
                );
            }
        }
    }
}
