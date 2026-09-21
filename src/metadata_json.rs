use crate::{
    json,
    metadata::{normalize_authors, normalize_json_text, Metadata},
    text, Document,
};
use regex::Regex;
use serde_json::{Map, Value};
use std::sync::LazyLock;

const ARTICLE_TYPES: &[&str] = &[
    "article",
    "backgroundnewsarticle",
    "blogposting",
    "medicalscholarlyarticle",
    "newsarticle",
    "opinionnewsarticle",
    "reportagenewsarticle",
    "scholarlyarticle",
    "socialmediaposting",
    "liveblogposting",
];
const PAGE_TYPES: &[&str] = &[
    "aboutpage",
    "checkoutpage",
    "collectionpage",
    "contactpage",
    "faqpage",
    "itempage",
    "medicalwebpage",
    "profilepage",
    "qapage",
    "realestatelisting",
    "searchresultspage",
    "webpage",
    "website",
    "article",
    "advertisercontentarticle",
    "newsarticle",
    "analysisnewsarticle",
    "askpublicnewsarticle",
    "backgroundnewsarticle",
    "opinionnewsarticle",
    "reportagenewsarticle",
    "reviewnewsarticle",
    "report",
    "satiricalarticle",
    "scholarlyarticle",
    "medicalscholarlyarticle",
    "socialmediaposting",
    "blogposting",
    "liveblogposting",
    "discussionforumposting",
    "techarticle",
    "blog",
    "jobposting",
];

fn go_pattern(pattern: &str) -> Regex {
    Regex::new(
        &pattern
            .replace(r"\s", r"[ \t\n\f\r]")
            .replace(r"\w", "[A-Za-z0-9_]"),
    )
    .unwrap()
}

static CONTEXT: LazyLock<Regex> = LazyLock::new(|| go_pattern(r"(?i)^https?://schema\.org"));
static NAME: LazyLock<Regex> = LazyLock::new(|| go_pattern(r#"(?i)"name\\?":\s*\\?"([^"\\]+)"#));
static AUTHOR: LazyLock<Regex> = LazyLock::new(|| {
    go_pattern(
        r#"(?s)"author"\s*:[^}\[]+?"name?\\?"\s*:\s*\\?"([^"\\]+)|"author"[^}\[]+?"names?".+?"([^"]+)"#,
    )
});
static PERSON: LazyLock<Regex> =
    LazyLock::new(|| go_pattern(r#"(?s)"[Pp]erson"[^}]+?"names?".+?"([^"]+)"#));
static AUTHOR_REMOVE: LazyLock<Regex> = LazyLock::new(|| {
    go_pattern(
        r#",?(?:"\w+"\s*:?[:|,\[])?\{?"@type"\s*:\s*"(?:[Ii]mageObject|[Oo]rganization|[Ww]eb[Pp]age)",[^}\[]+}[\]|}]?"#,
    )
});
static PUBLISHER: LazyLock<Regex> =
    LazyLock::new(|| go_pattern(r#"(?s)"publisher"\s*:[^}]+?"name?\\?"\s*:\s*\\?"([^"\\]+)"#));
static TYPE: LazyLock<Regex> = LazyLock::new(|| go_pattern(r#"(?s)"@type"\s*:\s*"([^"]*)""#));
static CATEGORY: LazyLock<Regex> =
    LazyLock::new(|| go_pattern(r#"(?s)"articleSection"\s*:\s*"([^"\\]+)"#));
static ARTICLE_NAME: LazyLock<Regex> =
    LazyLock::new(|| go_pattern(r#"(?s)"@type"\s*:\s*"[Aa]rticle",\s*"name"\s*:\s*"([^"\\]+)"#));
static HEADLINE: LazyLock<Regex> =
    LazyLock::new(|| go_pattern(r#"(?s)"headline"\s*:\s*"([^"\\]+)"#));

fn string_values(object: &Map<String, Value>, key: &str) -> Vec<String> {
    json::items(object.get(key).unwrap_or(&Value::Null))
        .iter()
        .filter_map(Value::as_str)
        .map(text::trim)
        .filter(|value| !value.is_empty())
        .collect()
}

fn single_string(object: &Map<String, Value>, key: &str) -> String {
    string_values(object, key)
        .into_iter()
        .next()
        .unwrap_or_default()
}

pub(crate) fn schema_types(object: &Map<String, Value>) -> Vec<String> {
    string_values(object, "@type")
        .iter()
        .map(|value| text::to_lower(value))
        .collect()
}

fn schema_names(value: &Value, expected_types: &[&str]) -> Vec<String> {
    let mut names = Vec::new();
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        match value {
            Value::String(value) => {
                let mut name = value.as_str();
                if value.contains(['{', '\\', '}']) {
                    if let Some(parts) = NAME.captures(value) {
                        name = parts.get(1).unwrap().as_str();
                    }
                }
                let name = text::trim(name);
                if !name.is_empty() {
                    names.push(name);
                }
            }
            Value::Array(values) => pending.extend(values.iter().rev()),
            Value::Object(object) => {
                let types = schema_types(object);
                if !expected_types.is_empty()
                    && !types.is_empty()
                    && !types
                        .iter()
                        .any(|kind| expected_types.contains(&kind.as_str()))
                {
                    continue;
                }
                let mut current = string_values(object, "name");
                if current.is_empty() && types.iter().any(|kind| kind == "person") {
                    let name = text::trim(&format!(
                        "{} {} {}",
                        single_string(object, "givenName"),
                        single_string(object, "additionalName"),
                        single_string(object, "familyName")
                    ));
                    if !name.is_empty() {
                        current.push(name);
                    }
                }
                if current.is_empty() {
                    current = string_values(object, "legalName");
                }
                if current.is_empty() {
                    current = string_values(object, "alternateName");
                }
                if !current.is_empty() {
                    names.extend(current);
                } else if let Some(child @ (Value::Object(_) | Value::Array(_))) =
                    object.get("name")
                {
                    pending.push(child);
                }
            }
            _ => {}
        }
    }
    names
}

fn plausible_sitename(current: &str, candidate: &str, kind: &str) -> bool {
    !candidate.is_empty()
        && (current.is_empty()
            || (candidate.chars().count() > current.chars().count() && kind != "webpage")
            || (current.starts_with("http") && !candidate.starts_with("http")))
}

fn process_metadata(parents: &[&Value], mut metadata: Metadata) -> Metadata {
    for parent in parents {
        let Some(content) = parent.as_object() else {
            continue;
        };
        if let Some(publisher) = content.get("publisher").and_then(Value::as_object) {
            let candidate = publisher
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if plausible_sitename(&metadata.sitename, candidate, "") {
                metadata.sitename = candidate.into();
            }
        }
        let types = schema_types(content);
        let Some(kind) = types.first().map(String::as_str) else {
            continue;
        };
        if metadata.page_type.is_empty() && PAGE_TYPES.contains(&kind) {
            metadata.page_type = normalize_json_text(kind);
        }
        match kind {
            "newsmediaorganization" | "organization" | "webpage" | "website" => {
                let candidate = ["name", "legalName", "alternateName"]
                    .iter()
                    .map(|key| single_string(content, key))
                    .find(|value| !value.is_empty())
                    .unwrap_or_default();
                if plausible_sitename(&metadata.sitename, &candidate, kind) {
                    metadata.sitename = candidate;
                }
            }
            "person" => {
                if let Some(name) = content.get("name").and_then(Value::as_str) {
                    if !name.starts_with("http") {
                        metadata.author = normalize_authors(&metadata.author, name);
                    }
                }
            }
            kind if ARTICLE_TYPES.contains(&kind) => {
                let authors = content.get("author").unwrap_or(&Value::Null);
                let decoded = authors.as_str().and_then(json::decode);
                let authors = decoded.as_ref().map_or(authors, |decoded| &decoded.0);
                for author in json::items(authors) {
                    if let Some(name) = author.as_str() {
                        metadata.author = normalize_authors(&metadata.author, name);
                        continue;
                    }
                    let Some(object) = author.as_object() else {
                        continue;
                    };
                    if object
                        .get("@type")
                        .is_some_and(|kind| kind.as_str() != Some("Person"))
                    {
                        continue;
                    }
                    let mut name = schema_names(author, &[]).join("; ");
                    if name.is_empty()
                        && object
                            .get("givenName")
                            .is_some_and(|value| !value.is_null())
                        && object
                            .get("familyName")
                            .is_some_and(|value| !value.is_null())
                    {
                        name = text::trim(&format!(
                            "{} {} {}",
                            single_string(object, "givenName"),
                            single_string(object, "additionalName"),
                            single_string(object, "familyName")
                        ));
                    }
                    metadata.author = normalize_authors(&metadata.author, &name);
                }
                if metadata.categories.is_empty() {
                    metadata.categories = string_values(content, "articleSection");
                }
                if metadata.title.is_empty() {
                    if kind == "article" {
                        metadata.title = single_string(content, "name");
                    }
                    if metadata.title.is_empty() {
                        metadata.title = single_string(content, "headline");
                    }
                }
            }
            _ => {}
        }
    }
    metadata
}

pub(crate) fn extract_json_metadata(value: &Value, metadata: Metadata) -> Metadata {
    let mut parents = Vec::new();
    for item in json::items(value) {
        let Some(parent) = item.as_object() else {
            continue;
        };
        if !CONTEXT.is_match(
            parent
                .get("@context")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        ) {
            continue;
        }
        if let Some(graph) = parent.get("@graph") {
            parents.extend(json::items(graph));
        } else if parent
            .get("@type")
            .and_then(Value::as_str)
            .is_some_and(|kind| text::to_lower(kind).contains("liveblogposting"))
            && parent
                .get("liveBlogUpdate")
                .is_some_and(|value| !value.is_null())
        {
            parents.extend(json::items(&parent["liveBlogUpdate"]));
        } else {
            parents.push(item);
        }
    }
    process_metadata(&parents, metadata)
}

fn extract_authors(input: &str, pattern: &Regex) -> String {
    let mut input = input.to_owned();
    let mut authors = String::new();
    while let Some(parts) = pattern.captures(&input) {
        let name = parts
            .iter()
            .skip(1)
            .flatten()
            .next()
            .map_or("", |part| part.as_str());
        if !name.contains(' ') {
            break;
        }
        authors = normalize_authors(&authors, name);
        let matched = parts.get(0).unwrap();
        input.replace_range(matched.range(), "");
    }
    authors
}

pub(crate) fn recover_metadata(input: &str, mut metadata: Metadata) -> Metadata {
    let author_input = AUTHOR_REMOVE.replace_all(input, "");
    let mut authors = extract_authors(&author_input, &AUTHOR);
    if authors.is_empty() {
        authors = extract_authors(&author_input, &PERSON);
    }
    if !authors.is_empty() {
        metadata.author = authors;
    }
    if let Some(parts) = TYPE.captures(input) {
        let candidate = normalize_json_text(&text::to_lower(&parts[1]));
        if PAGE_TYPES.contains(&candidate.as_str()) {
            metadata.page_type = candidate;
        }
    }
    if let Some(parts) = PUBLISHER.captures(input) {
        if !parts[1].contains(',') {
            let candidate = normalize_json_text(&parts[1]);
            if plausible_sitename(&metadata.sitename, &candidate, "") {
                metadata.sitename = candidate;
            }
        }
    }
    if let Some(parts) = CATEGORY.captures(input) {
        metadata.categories = vec![normalize_json_text(&parts[1])];
    }
    for pattern in [&*ARTICLE_NAME, &*HEADLINE] {
        if metadata.title.is_empty() {
            if let Some(parts) = pattern.captures(input) {
                metadata.title = normalize_json_text(&parts[1]);
            }
        }
    }
    metadata
}

pub fn extract_json_ld(document: &Document, mut metadata: Metadata) -> Metadata {
    for index in document.tagged(0, "script") {
        if !matches!(
            document.nodes[index].attr("type"),
            "application/ld+json" | "application/settings+json"
        ) {
            continue;
        }
        let input = normalize_json_text(&document.text(index));
        if input.is_empty() {
            continue;
        }
        metadata = match json::decode(&input) {
            Some(decoded) => extract_json_metadata(&decoded.0, metadata),
            None => recover_metadata(&input, metadata),
        };
    }
    metadata
}
