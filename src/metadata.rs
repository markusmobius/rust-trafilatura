pub use crate::metadata_json::extract_json_ld;
use crate::{etree, text::trim, DateOptions, Document, Node, NodeId};
use chrono::DateTime;
use regex::Regex;
use std::{collections::HashSet, sync::LazyLock};

#[derive(Clone, Debug, PartialEq)]
pub struct Metadata {
    pub title: String,
    pub author: String,
    pub url: String,
    pub hostname: String,
    pub description: String,
    pub sitename: String,
    pub date: DateTime<rust_htmldate::Timezone>,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub id: String,
    pub fingerprint: String,
    pub license: String,
    pub language: String,
    pub image: String,
    pub page_type: String,
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            title: String::new(),
            author: String::new(),
            url: String::new(),
            hostname: String::new(),
            description: String::new(),
            sitename: String::new(),
            date: rust_htmldate::ExtractionResult::default().date_time,
            categories: Vec::new(),
            tags: Vec::new(),
            id: String::new(),
            fingerprint: String::new(),
            license: String::new(),
            language: String::new(),
            image: String::new(),
            page_type: String::new(),
        }
    }
}

const META_AUTHOR: &[&str] = &[
    "article:author",
    "atc-metaauthor",
    "author",
    "authors",
    "byl",
    "citation_author",
    "creator",
    "dc.creator",
    "dc.creator.aut",
    "dc:creator",
    "dcterms.creator",
    "dcterms.creator.aut",
    "dcsext.author",
    "parsely-author",
    "rbauthors",
    "sailthru.author",
    "shareaholic:article_author_name",
];
const META_TITLE: &[&str] = &[
    "citation_title",
    "dc.title",
    "dcterms.title",
    "fb_title",
    "headline",
    "parsely-title",
    "sailthru.title",
    "shareaholic:title",
    "rbtitle",
    "title",
    "twitter:title",
];
const META_DESCRIPTION: &[&str] = &[
    "dc.description",
    "dc:description",
    "dcterms.abstract",
    "dcterms.description",
    "description",
    "sailthru.description",
    "twitter:description",
];
const META_PUBLISHER: &[&str] = &[
    "article:publisher",
    "citation_journal_title",
    "copyright",
    "dc.publisher",
    "dc:publisher",
    "dcterms.publisher",
    "publisher",
    "sailthru.publisher",
    "rbpubname",
    "twitter:site",
];
const META_TAG: &[&str] = &[
    "citation_keywords",
    "dcterms.subject",
    "keywords",
    "parsely-tags",
    "shareaholic:keywords",
    "tags",
];
const META_IMAGE: &[&str] = &[
    "image",
    "og:image",
    "og:image:url",
    "og:image:secure_url",
    "twitter:image",
    "twitter:image:src",
];

static TITLE_CLEANER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(.+)?[ \t\n\f\r]+[\u{2013}\u{2022}\u{00b7}\u{2014}|\u{2044}*\u{22c6}~\u{2039}\u{00ab}<\u{203a}\u{00bb}>:-][ \t\n\f\r]+(.+)$").unwrap()
});
static CC_LICENSE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)/(by-nc-nd|by-nc-sa|by-nc|by-nd|by-sa|by|zero)/([1-9]\.[0-9])").unwrap()
});
static CC_LICENSE_TEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(cc|creative commons) (by-nc-nd|by-nc-sa|by-nc|by-nd|by-sa|by|zero) ?([1-9]\.[0-9])?",
    )
    .unwrap()
});
static HTML_STRIP_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(<!--.*?-->|<[^>]*>)").unwrap());
static JSON_WHITESPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\[nrt]").unwrap());
static JSON_UNICODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\u[0-9a-fA-F]{4}").unwrap());
static URL_CHECK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)https?://").unwrap());
static AUTHOR_EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}(?-u:\b)").unwrap()
});
static AUTHOR_HTML: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<[^>]+>").unwrap());
static AUTHOR_SEPARATOR: LazyLock<Regex> = LazyLock::new(|| {
    let word = crate::go_unicode::AUTHOR_WORD_CLASS;
    Regex::new(&format!(
        r"(?i)/|;|,|\||&|(?:^|[^{word}])[ua]nd(?:$|[^{word}])"
    ))
    .unwrap()
});
static AUTHOR_SOCIAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)@[^ \t\n\f\r]+").unwrap());
static AUTHOR_SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[._+]").unwrap());
static AUTHOR_NICKNAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)["\u{2018}({\[\u{2019}'][^"]+?[\u{2018}\u{2019}"')\]}]"#).unwrap()
});
static AUTHOR_SPECIAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"[^{}]+$|[:()?*$#!%/<>{{}}~\u{{bf}}]",
        crate::go_unicode::AUTHOR_WORD_CLASS
    ))
    .unwrap()
});
static AUTHOR_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^([a-z\u{e4}\u{f6}\u{fc}\u{df}]+(ed|t))? ?(written by|words by|words|by|von|from) ",
    )
    .unwrap()
});
static AUTHOR_DIGITS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"[{}].+?$", crate::go_unicode::NUMBER_CLASS)).unwrap());
static AUTHOR_PREPOSITION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)[ \t\n\f\r]+(am|on|for|at|in|to|from|of|via|with|\u{2014}|-|\u{2013})[ \t\n\f\r]+(.*)").unwrap()
});
static CATEGORY_HREF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)/categor(?:y|ies)/").unwrap());
static TAG_HREF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)/tags?/").unwrap());
static SITENAME_FINDER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)https?://(?:www\.|w[0-9]+\.)?([^/]+)").unwrap());

pub fn extract_metadata(document: &Document, options: &crate::Options) -> Metadata {
    let mut metadata = examine_meta(document);
    metadata.author = remove_blacklisted_authors(&metadata.author, &options.blacklisted_authors);
    metadata = extract_json_ld(document, metadata);
    metadata.author = remove_blacklisted_authors(&metadata.author, &options.blacklisted_authors);
    if metadata.title.is_empty() {
        metadata.title = extract_dom_title(document);
    }
    if metadata.author.is_empty() {
        metadata.author =
            remove_blacklisted_authors(&extract_dom_author(document), &options.blacklisted_authors);
    }
    if metadata.url.is_empty() {
        metadata.url = extract_dom_url(document);
    }
    if !metadata.url.is_empty() {
        let (url, absolute) = crate::url::validate_url(&metadata.url, None);
        metadata.url = if absolute { url } else { String::new() };
    }
    if metadata.url.is_empty() {
        if let Some(original) = &options.original_url {
            metadata.url = original.to_string();
        }
    }
    if !metadata.url.is_empty() {
        metadata.hostname = crate::url::get_domain_url(&metadata.url);
    }
    metadata.date = extract_date_from_dom(document, &metadata.url, &DateOptions::from(options));
    if metadata.sitename.is_empty() {
        metadata.sitename = extract_dom_sitename(document);
    }
    if !metadata.sitename.is_empty() {
        metadata.sitename = metadata
            .sitename
            .strip_prefix('@')
            .unwrap_or(&metadata.sitename)
            .into();
        if !metadata.sitename.contains('.')
            && !metadata
                .sitename
                .chars()
                .next()
                .is_some_and(crate::text::is_upper)
        {
            metadata.sitename = crate::text::title_case(&metadata.sitename);
        }
    } else if !metadata.url.is_empty() {
        if let Some(parts) = SITENAME_FINDER.captures(&metadata.url) {
            metadata.sitename = parts[1].into();
        }
    }
    if metadata.categories.is_empty() {
        metadata.categories = extract_dom_categories(document);
    }
    if !metadata.categories.is_empty() {
        metadata.categories = clean_cat_tags(&metadata.categories);
    }
    if metadata.tags.is_empty() {
        metadata.tags = extract_dom_tags(document);
    }
    if !metadata.tags.is_empty() {
        metadata.tags = clean_cat_tags(&metadata.tags);
    }
    metadata.license = extract_license(document);
    for field in [
        &mut metadata.title,
        &mut metadata.author,
        &mut metadata.url,
        &mut metadata.hostname,
        &mut metadata.description,
        &mut metadata.sitename,
        &mut metadata.id,
        &mut metadata.fingerprint,
        &mut metadata.license,
        &mut metadata.language,
        &mut metadata.image,
        &mut metadata.page_type,
    ] {
        if field.chars().count() > 10_000 {
            *field = field
                .chars()
                .take(9_999)
                .chain(std::iter::once('\u{2026}'))
                .collect();
        }
        *field = clean_metadata_text(field);
    }
    metadata
}

pub fn clean_metadata_text(value: &str) -> String {
    let decoded = crate::text::unescape_html(value).replace('\u{b}', "");
    trim(&crate::text::remove_control_characters(&decoded))
}

pub fn normalize_json_text(value: &str) -> String {
    let normalized = if value.contains('\\') {
        let unspaced = JSON_WHITESPACE.replace_all(value, "");
        let decoded = JSON_UNICODE.replace_all(&unspaced, |captures: &regex::Captures<'_>| {
            let scalar = u32::from_str_radix(&captures[0][2..], 16).unwrap();
            char::from_u32(scalar).map_or_else(String::new, |character| character.to_string())
        });
        crate::text::unescape_html(&decoded)
    } else {
        value.into()
    };
    trim(&HTML_STRIP_TAG.replace_all(&normalized, ""))
}

pub fn validate_metadata_name(name: &str) -> String {
    if !name.contains(' ') || name.starts_with("http") || name.contains(['{', '\\', '}']) {
        String::new()
    } else {
        name.into()
    }
}

pub fn normalize_tags(value: &str) -> String {
    trim(&crate::text::unescape_html(value))
        .replace(['\'', '"'], "")
        .split(", ")
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn normalize_authors(authors: &str, input: &str) -> String {
    use crate::text;

    if URL_CHECK.is_match(input) || AUTHOR_EMAIL.is_match(input) {
        return authors.into();
    }
    let mut input = if input.contains(r"\u") {
        normalize_json_text(input)
    } else {
        input.into()
    };
    if input.contains("&#") || input.contains("&amp;") {
        input = text::unescape_html(&input);
    }
    let input = AUTHOR_HTML.replace_all(&input, "");
    let mut names: Vec<String> = if authors.is_empty() {
        Vec::new()
    } else {
        authors.split("; ").map(String::from).collect()
    };
    let mut seen: HashSet<String> = names.iter().cloned().collect();
    for candidate in AUTHOR_SEPARATOR.split(&input) {
        let mut author = text::remove_emojis(&trim(candidate));
        author = AUTHOR_SOCIAL.replace_all(&author, "").into_owned();
        author = trim(&AUTHOR_SPACES.replace_all(&author, " "));
        for expression in [
            &*AUTHOR_NICKNAME,
            &*AUTHOR_SPECIAL,
            &*AUTHOR_PREFIX,
            &*AUTHOR_DIGITS,
            &*AUTHOR_PREPOSITION,
        ] {
            author = expression.replace_all(&author, "").into_owned();
        }
        if author.is_empty() || (!author.contains(['-', ' ']) && author.chars().count() >= 50) {
            continue;
        }
        if !author.chars().next().is_some_and(text::is_upper) {
            author = text::title_case(&author);
        }
        if seen.insert(author.clone()) {
            names.push(author);
        }
    }
    names
        .iter()
        .filter(|author| {
            !names
                .iter()
                .any(|other| *author != other && other.contains(author.as_str()))
        })
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("; ")
        .trim_matches([';', ' '])
        .into()
}

pub fn remove_blacklisted_authors(current: &str, blacklisted: &[impl AsRef<str>]) -> String {
    let blocked: HashSet<_> = blacklisted
        .iter()
        .map(|name| crate::text::to_lower(name.as_ref()))
        .collect();
    current
        .split(';')
        .map(crate::text::trim_space)
        .filter(|author| !blocked.contains(&crate::text::to_lower(author)))
        .collect::<Vec<_>>()
        .join("; ")
}

fn set_if_empty(current: &mut String, candidate: String) {
    if current.is_empty() {
        *current = candidate;
    }
}

pub fn extract_open_graph_meta(document: &Document) -> Metadata {
    let mut metadata = Metadata::default();
    for index in document.tagged(0, "meta") {
        let node = &document.nodes[index];
        if !node.attr("property").starts_with("og:") {
            continue;
        }
        let property = trim(node.attr("property"));
        let content = if property == "og:article:tag" {
            trim(&crate::text::unescape_html(node.attr("content")))
        } else {
            clean_metadata_text(node.attr("content"))
        };
        if content.is_empty() {
            continue;
        }
        match property.as_str() {
            "og:site_name" => metadata.sitename = content,
            "og:title" => metadata.title = content,
            "og:description" => metadata.description = content,
            "og:author" | "og:article:author" => metadata.author = normalize_authors("", &content),
            "og:image" | "og:image:url" | "og:image:secure_url" => metadata.image = content,
            "og:url" if crate::url::is_absolute_url(&content) => metadata.url = content,
            "og:article:tag" => metadata.tags = clean_cat_tags(&[content]),
            "og:type" => metadata.page_type = content,
            _ => {}
        }
    }
    metadata
}

pub fn examine_meta(document: &Document) -> Metadata {
    let mut metadata = extract_open_graph_meta(document);
    if [
        &metadata.title,
        &metadata.author,
        &metadata.url,
        &metadata.description,
        &metadata.sitename,
        &metadata.image,
        &metadata.page_type,
    ]
    .iter()
    .all(|value| !value.is_empty())
    {
        return metadata;
    }
    let mut temporary_sitename = String::new();
    for index in document.tagged(0, "meta") {
        let node = &document.nodes[index];
        if !node.has_attr("content") || !in_head(document, index) {
            continue;
        }
        let content = HTML_STRIP_TAG.replace_all(node.attr("content"), "");
        let property = trim(node.attr("property"));
        let name = trim(&crate::text::to_lower(node.attr("name")));
        let content = if property == "article:tag"
            || (property.is_empty() && META_TAG.contains(&name.as_str()))
        {
            normalize_tags(&content)
        } else {
            clean_metadata_text(&content)
        };
        if content.is_empty() {
            continue;
        }
        if !property.is_empty() {
            match property.as_str() {
                property if property.starts_with("og:") => {}
                "article:tag" => metadata.tags.push(content),
                "author" | "article:author" => {
                    metadata.author = normalize_authors(&metadata.author, &content)
                }
                "article:publisher" => set_if_empty(&mut metadata.sitename, content),
                property if META_IMAGE.contains(&property) => {
                    set_if_empty(&mut metadata.image, content)
                }
                _ => {}
            }
            continue;
        }
        if !name.is_empty() {
            match name.as_str() {
                name if META_AUTHOR.contains(&name) => {
                    metadata.author = normalize_authors(
                        &metadata.author,
                        &HTML_STRIP_TAG.replace_all(&content, ""),
                    );
                }
                name if META_TITLE.contains(&name) => set_if_empty(&mut metadata.title, content),
                name if META_DESCRIPTION.contains(&name) => {
                    set_if_empty(&mut metadata.description, content)
                }
                name if META_PUBLISHER.contains(&name) => {
                    set_if_empty(&mut metadata.sitename, content)
                }
                name if matches!(name, "twitter:site" | "application-name")
                    || name.contains("twitter:app:name") =>
                {
                    temporary_sitename = content;
                }
                "twitter:url" => {
                    if metadata.url.is_empty() && crate::url::is_absolute_url(&content) {
                        metadata.url = content;
                    }
                }
                name if META_IMAGE.contains(&name) => set_if_empty(&mut metadata.image, content),
                name if META_TAG.contains(&name) => metadata.tags.push(content),
                _ => {}
            }
            continue;
        }
        match trim(node.attr("itemprop")).as_str() {
            "author" => metadata.author = normalize_authors(&metadata.author, &content),
            "description" => set_if_empty(&mut metadata.description, content),
            "headline" => set_if_empty(&mut metadata.title, content),
            _ => {}
        }
    }
    set_if_empty(&mut metadata.sitename, temporary_sitename);
    metadata.author = validate_metadata_name(&metadata.author);
    metadata.categories = clean_cat_tags(&metadata.categories);
    metadata.tags = clean_cat_tags(&metadata.tags);
    metadata
}

pub fn extract_date(
    document: &rust_htmldate::Document,
    metadata_url: &str,
    options: &DateOptions,
) -> DateTime<rust_htmldate::Timezone> {
    extract_date_with(metadata_url, options, |resolved| {
        rust_htmldate::from_document(document, resolved)
    })
}

pub fn extract_date_from_dom(
    document: &Document,
    metadata_url: &str,
    options: &DateOptions,
) -> DateTime<rust_htmldate::Timezone> {
    extract_date_with(metadata_url, options, |resolved| {
        extract_date_from_source(document, resolved)
    })
}

fn extract_date_from_source(
    document: &Document,
    options: &rust_htmldate::Options,
) -> rust_htmldate::ExtractionResult {
    use rust_htmldate::{TreeAttributeRef, TreeNodeRef, TreeSource};

    struct Source<'tree>(&'tree Document);

    impl TreeSource for Source<'_> {
        type Handle = crate::NodeId;

        fn root(&self) -> Self::Handle {
            0
        }

        fn node(&self, index: Self::Handle) -> TreeNodeRef<'_> {
            let node = &self.0.nodes[index];
            match node.kind {
                crate::Kind::Document => TreeNodeRef::Document,
                crate::Kind::Element => TreeNodeRef::Element(&node.tag),
                crate::Kind::Text => TreeNodeRef::Text(&node.data),
                crate::Kind::Comment => TreeNodeRef::Comment(&node.data),
                crate::Kind::Doctype => TreeNodeRef::Doctype(&node.data),
            }
        }

        fn attributes(&self, index: Self::Handle) -> impl Iterator<Item = TreeAttributeRef<'_>> {
            self.0.nodes[index]
                .attrs
                .iter()
                .map(|attribute| TreeAttributeRef {
                    namespace: &attribute.namespace,
                    name: &attribute.key,
                    value: &attribute.value,
                })
        }

        fn children(&self, index: Self::Handle) -> impl DoubleEndedIterator<Item = Self::Handle> {
            self.0.nodes[index].children.iter().copied()
        }
    }

    rust_htmldate::from_tree_source(&Source(document), options)
}

fn extract_date_with(
    metadata_url: &str,
    options: &DateOptions,
    extract: impl FnOnce(&rust_htmldate::Options) -> rust_htmldate::ExtractionResult,
) -> DateTime<rust_htmldate::Timezone> {
    let zero = rust_htmldate::ExtractionResult::default().date_time;
    if let Some(overridden) = &options.html_date_override {
        return if overridden.has_time {
            overridden.date_time.clone()
        } else {
            zero
        };
    }
    let Some(resolved) = options.resolved_options(metadata_url) else {
        return zero;
    };
    let result = extract(&resolved);
    if result.is_zero() {
        zero
    } else {
        result.date_time
    }
}

pub fn examine_title_element(document: &Document) -> (String, String, String) {
    let title = document
        .tagged(0, "title")
        .into_iter()
        .find(|&node| {
            document.nodes[node]
                .parent
                .is_some_and(|parent| document.nodes[parent].tag == "head")
        })
        .map_or_else(String::new, |node| trim(&document.text(node)));
    let (first, second) = TITLE_CLEANER.captures(&title).map_or_else(
        || (String::new(), String::new()),
        |parts| {
            (
                parts.get(1).map_or("", |value| value.as_str()).into(),
                parts[2].into(),
            )
        },
    );
    (title, first, second)
}

fn title_rule(node: &Node, rule: usize) -> bool {
    let normalized_class = crate::text::trim_cow(node.attr("class"));
    let class = normalized_class.as_ref();
    let id = crate::text::trim_space(node.attr("id"));
    match rule {
        0 => {
            matches!(node.tag.as_str(), "h1" | "h2")
                && ([
                    "post-title",
                    "entry-title",
                    "headline",
                    "post__title",
                    "article-title",
                ]
                .iter()
                .any(|pattern| class.contains(pattern))
                    || id.contains("headline")
                    || node.attr("itemprop").contains("headline"))
        }
        1 => matches!(class, "entry-title" | "post-title"),
        2 => {
            matches!(node.tag.as_str(), "h1" | "h2" | "h3")
                && (class.contains("title") || id.contains("title"))
        }
        _ => unreachable!(),
    }
}

pub fn extract_dom_title(document: &Document) -> String {
    let headings = document.tagged(0, "h1");
    if headings.len() == 1 {
        let title = trim(&document.text(headings[0]));
        if !title.is_empty() {
            return title;
        }
    }
    let elements = document.elements(0);
    for rule in 0..3 {
        for &element in &elements {
            if title_rule(&document.nodes[element], rule) {
                let title = trim(&etree::iter_text(document, element, " "));
                if (3..200).contains(&title.chars().count()) {
                    return title;
                }
            }
        }
    }
    let (title, first, second) = examine_title_element(document);
    for candidate in [first, second, title] {
        if !candidate.is_empty() && !candidate.contains('.') {
            return candidate;
        }
    }
    for heading in headings {
        let title = trim(&document.text(heading));
        if !title.is_empty() {
            return title;
        }
    }
    document
        .tagged(0, "h2")
        .first()
        .map_or_else(String::new, |&heading| trim(&document.text(heading)))
}

pub fn extract_dom_sitename(document: &Document) -> String {
    let (_, first, second) = examine_title_element(document);
    if first.contains('.') {
        first
    } else if second.contains('.') {
        second
    } else {
        String::new()
    }
}

fn author_rule(node: &Node, rule: usize) -> bool {
    let normalized_class = crate::text::trim_cow(node.attr("class"));
    let class = normalized_class.as_ref();
    let id = crate::text::trim_space(node.attr("id"));
    let itemprop = node.attr("itemprop");
    match rule {
        0 => {
            node.tag == "author"
                || (matches!(
                    node.tag.as_str(),
                    "a" | "address" | "div" | "link" | "p" | "span" | "strong"
                ) && (matches!(node.attr("rel"), "author" | "me")
                    || id == "author"
                    || class == "author"
                    || itemprop == "author name"
                    || ["author-name", "authorname", "AuthorName", "authorName"]
                        .iter()
                        .any(|pattern| class.contains(pattern))
                    || matches!(node.attr("data-testid"), "AuthorCard" | "AuthorURL")))
        }
        1 => {
            matches!(node.tag.as_str(), "a" | "div" | "h3" | "h4" | "p" | "span")
                && (class.contains("author")
                    || id.contains("author")
                    || itemprop.contains("author")
                    || matches!(class, "byline" | "username" | "byl" | "BBL")
                    || [
                        "channel-name",
                        "zuozhe",
                        "bianji",
                        "xiaobian",
                        "submitted-by",
                        "posted-by",
                        "journalist-name",
                    ]
                    .iter()
                    .any(|pattern| class.contains(pattern))
                    || ["zuozhe", "bianji", "xiaobian"]
                        .iter()
                        .any(|pattern| id.contains(pattern)))
        }
        2 => {
            crate::text::to_lower(id).contains("author")
                || crate::text::to_lower(class).contains("author")
                || class.contains("screenname")
                || crate::text::to_lower(node.attr("data-component")).contains("byline")
                || itemprop.contains("author")
                || class.contains("writer")
                || crate::text::to_lower(class).contains("byline")
        }
        _ => unreachable!(),
    }
}

fn discard_author_rule(node: &Node, rule: usize) -> bool {
    if rule == 1 {
        return matches!(node.tag.as_str(), "time" | "figure");
    }
    let id = crate::text::trim_space(node.attr("id"));
    let normalized_class = crate::text::trim_cow(node.attr("class"));
    let class = normalized_class.as_ref();
    matches!(node.tag.as_str(), "a" | "div" | "section" | "span")
        && (id == "comments"
            || matches!(class, "comments" | "title" | "date")
            || ["commentlist", "comment-list", "ProductReviews"]
                .iter()
                .any(|pattern| id.contains(pattern))
            || [
                "commentlist",
                "sidebar",
                "is-hidden",
                "quote",
                "comment-list",
                "embedly-instagram",
                "article-share",
                "article-support",
                "print",
                "category",
                "meta-date",
                "meta-reviewer",
            ]
            .iter()
            .any(|pattern| class.contains(pattern))
            || id.starts_with("comments")
            || class.starts_with("comments")
            || class.starts_with("Comments")
            || node.attr("data-component").contains("Figure"))
}

pub fn extract_dom_author(document: &Document) -> String {
    let exclude = |node: &Node| discard_author_rule(node, 0) || discard_author_rule(node, 1);
    let mut elements = Vec::new();
    let mut pending: Vec<_> = document.nodes[0].children.iter().rev().copied().collect();
    while let Some(index) = pending.pop() {
        let node = &document.nodes[index];
        if node.kind == crate::Kind::Element {
            if exclude(node) {
                continue;
            }
            elements.push(index);
        }
        pending.extend(node.children.iter().rev().copied());
    }
    for rule in 0..3 {
        for &index in &elements {
            if author_rule(&document.nodes[index], rule) {
                let author = trim(&etree::iter_text_filtered(document, index, " ", exclude));
                if (3..120).contains(&author.chars().count()) {
                    return normalize_authors("", &author);
                }
            }
        }
    }
    String::new()
}

fn category_ancestor(node: &Node, rule: usize) -> bool {
    let normalized_class = crate::text::trim_cow(node.attr("class"));
    let class = normalized_class.as_ref();
    let id = crate::text::trim_space(node.attr("id"));
    match rule {
        0 => {
            node.tag == "div"
                && ([
                    "post-info",
                    "postinfo",
                    "post-meta",
                    "postmeta",
                    "meta",
                    "entry-meta",
                    "entry-info",
                    "entry-utility",
                ]
                .iter()
                .any(|prefix| class.starts_with(prefix))
                    || id.starts_with("postpath"))
        }
        1 => {
            node.tag == "p"
                && (class.starts_with("postmeta")
                    || class.starts_with("entry-categories")
                    || class == "postinfo"
                    || id == "filedunder")
        }
        2 => {
            node.tag == "footer"
                && (class.starts_with("entry-meta") || class.starts_with("entry-footer"))
        }
        3 => {
            matches!(node.tag.as_str(), "li" | "span")
                && (matches!(class, "post-category" | "postcategory" | "entry-category")
                    || class.contains("cat-links"))
        }
        4 => node.tag == "header" && class == "entry-header",
        5 => node.tag == "div" && matches!(class, "row" | "tags"),
        _ => unreachable!(),
    }
}

fn tag_ancestor(node: &Node, rule: usize) -> bool {
    let normalized_class = crate::text::trim_cow(node.attr("class"));
    let class = normalized_class.as_ref();
    match rule {
        0 => node.tag == "div" && class == "tags",
        1 => node.tag == "p" && class.starts_with("entry-tags"),
        2 => {
            node.tag == "div"
                && (matches!(class, "row" | "jp-relatedposts" | "entry-utility")
                    || ["tag", "postmeta", "meta"]
                        .iter()
                        .any(|prefix| class.starts_with(prefix)))
        }
        3 => class == "entry-meta" || class.contains("topics") || class.contains("tags-links"),
        _ => unreachable!(),
    }
}

fn extract_link_metadata(
    document: &Document,
    count: usize,
    ancestor_rule: fn(&Node, usize) -> bool,
    href_pattern: &Regex,
) -> Vec<String> {
    let anchors: Vec<_> = document
        .tagged(0, "a")
        .into_iter()
        .filter(|&index| {
            let href = crate::text::trim_cow(document.nodes[index].attr("href"));
            !href.is_empty() && href_pattern.is_match(&href)
        })
        .collect();
    for rule in 0..count {
        let mut entries = Vec::new();
        for &index in &anchors {
            let mut parent = document.nodes[index].parent;
            while let Some(ancestor) = parent {
                if ancestor_rule(&document.nodes[ancestor], rule) {
                    let content = trim(&document.text(index));
                    if !content.is_empty() {
                        entries.push(content);
                    }
                    break;
                }
                parent = document.nodes[ancestor].parent;
            }
        }
        if !entries.is_empty() {
            return entries;
        }
    }
    Vec::new()
}

pub fn extract_dom_categories(document: &Document) -> Vec<String> {
    let mut categories = extract_link_metadata(document, 6, category_ancestor, &CATEGORY_HREF);
    if categories.is_empty() {
        for index in document.tagged(0, "meta") {
            let node = &document.nodes[index];
            if in_head(document, index)
                && (node.attr("property") == "article:section"
                    || node.attr("name").contains("subject"))
            {
                let content = trim(node.attr("content"));
                if !content.is_empty() {
                    categories.push(content);
                }
            }
        }
    }
    clean_cat_tags(&categories)
}

pub fn extract_dom_tags(document: &Document) -> Vec<String> {
    clean_cat_tags(&extract_link_metadata(document, 4, tag_ancestor, &TAG_HREF))
}

fn in_head(document: &Document, node: NodeId) -> bool {
    let mut parent = document.nodes[node].parent;
    while let Some(index) = parent {
        if document.nodes[index].tag == "head" {
            return true;
        }
        parent = document.nodes[index].parent;
    }
    false
}

pub fn extract_dom_url(document: &Document) -> String {
    let elements = document.elements(0);
    let mut url = String::new();
    for rule in 0..3 {
        let selected = elements.iter().copied().find(|&index| {
            let node = &document.nodes[index];
            let matches = match rule {
                0 => node.tag == "link" && node.attr("rel") == "canonical",
                1 => node.tag == "base",
                _ => {
                    node.tag == "link"
                        && node.attr("rel") == "alternate"
                        && node.attr("hreflang") == "x-default"
                }
            };
            matches && in_head(document, index)
        });
        if let Some(index) = selected {
            url = trim(document.nodes[index].attr("href"));
            if !url.is_empty() {
                break;
            }
        }
    }
    if url.starts_with('/') {
        for index in elements {
            let node = &document.nodes[index];
            if node.tag != "meta" || !node.has_attr("content") || !in_head(document, index) {
                continue;
            }
            let name = trim(node.attr("name"));
            let property = trim(node.attr("property"));
            let attribute = if name.is_empty() { &property } else { &name };
            if attribute.starts_with("og:") || attribute.starts_with("twitter:") {
                let base = crate::url::get_base_url(&trim(node.attr("content")));
                if !base.is_empty() {
                    url = base + &url;
                    break;
                }
            }
        }
    }
    url
}

pub fn parse_license_element(document: &Document, node: NodeId, strict: bool) -> String {
    let href = trim(document.nodes[node].attr("href"));
    if let Some(parts) = CC_LICENSE.captures(&href) {
        return format!("CC {} {}", parts[1].to_uppercase(), &parts[2]);
    }
    let text = trim(&document.text(node));
    if !text.is_empty() && !strict {
        return text;
    }
    CC_LICENSE_TEXT
        .find(&text)
        .map_or_else(String::new, |found| found.as_str().into())
}

pub fn extract_license(document: &Document) -> String {
    let anchors = document.tagged(0, "a");
    for &anchor in &anchors {
        let node = &document.nodes[anchor];
        if node.has_attr("href") && node.attr("rel") == "license" {
            let license = parse_license_element(document, anchor, false);
            if !license.is_empty() {
                return license;
            }
        }
    }
    for anchor in anchors {
        if !document.nodes[anchor].has_attr("href") {
            continue;
        }
        let mut parent = document.nodes[anchor].parent;
        while let Some(index) = parent {
            let node = &document.nodes[index];
            if node.tag == "footer"
                || (node.tag == "div"
                    && (node.attr("class").contains("footer")
                        || node.attr("id").contains("footer")))
            {
                let license = parse_license_element(document, anchor, true);
                if !license.is_empty() {
                    return license;
                }
                break;
            }
            parent = node.parent;
        }
    }
    String::new()
}

pub fn clean_cat_tags(entries: &[impl AsRef<str>]) -> Vec<String> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for entry in entries {
        let entry = trim(entry.as_ref());
        if !entry.is_empty() && seen.insert(entry.clone()) {
            result.push(entry);
        }
    }
    result
}
