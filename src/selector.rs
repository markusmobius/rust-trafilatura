use crate::{
    text::{is_space, to_lower, trim_space},
    Document, Node, NodeId,
};

pub type Rule = fn(&Node) -> bool;

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn starts_any(value: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| value.starts_with(prefix))
}

fn id(node: &Node) -> &str {
    node.attr("id")
}
fn class(node: &Node) -> &str {
    node.attr("class")
}
fn first_id_class(node: &Node) -> &str {
    node.attrs
        .iter()
        .find(|attribute| matches!(attribute.key.as_str(), "id" | "class"))
        .map_or("", |attribute| attribute.value.as_str())
}
fn content_tag(node: &Node) -> bool {
    matches!(node.tag.as_str(), "article" | "div" | "main" | "section")
}
fn section_tag(node: &Node) -> bool {
    matches!(
        node.tag.as_str(),
        "div" | "dd" | "dt" | "li" | "ul" | "ol" | "dl" | "p" | "section" | "span"
    )
}
fn comment_tag(node: &Node) -> bool {
    matches!(node.tag.as_str(), "div" | "ol" | "ul" | "dl" | "section")
}

pub const CONTENT: &[Rule] = &[
    |node| {
        let (id, class) = (id(node), class(node));
        content_tag(node)
            && (matches!(class, "post" | "entry")
                || contains_any(
                    class,
                    &[
                        "post-text",
                        "post_text",
                        "post-body",
                        "post-entry",
                        "postentry",
                        "post-content",
                        "post_content",
                        "post_inner_wrapper",
                        "article-text",
                        "ArticleContent",
                        "page-content",
                        "text-content",
                        "article__container",
                        "postcontent",
                        "postContent",
                        "articletext",
                        "articleText",
                        "articlebody",
                        "articleBody",
                    ],
                )
                || [id, class].iter().any(|value| {
                    contains_any(
                        value,
                        &[
                            "entry-content",
                            "article-content",
                            "article__content",
                            "article-body",
                            "article__body",
                            "body-text",
                            "art-content",
                        ],
                    )
                })
                || node.attr("itemprop") == "articleBody"
                || contains_any(id, &["articlebody", "articleBody"])
                || id == "articleContent")
    },
    |node| node.tag == "article",
    |node| {
        let (id, class) = (id(node), class(node));
        content_tag(node)
            && (contains_any(
                class,
                &[
                    "post-bodycopy",
                    "storycontent",
                    "story-content",
                    "theme-content",
                    "blog-content",
                    "section-content",
                    "single-content",
                    "single-post",
                    "main-column",
                    "wpb_text_column",
                    "story-body",
                    "field-body",
                ],
            ) || matches!(
                class,
                "postarea" | "art-postcontent" | "text" | "cell" | "story"
            ) || id.starts_with("primary")
                || class.starts_with("article ")
                || matches!(id, "article" | "story")
                || id.contains("story-body")
                || to_lower(class).contains("fulltext")
                || node.attr("role") == "article")
    },
    |node| {
        let (id, class) = (id(node), class(node));
        content_tag(node)
            && ([id, class]
                .iter()
                .any(|value| contains_any(value, &["content-main", "content-body"]))
                || contains_any(class, &["content_main", "content__body"])
                || id.contains("contentBody")
                || id
                    .replace('C', "c")
                    .replace('M', "m")
                    .contains("main-content")
                || class
                    .replace('C', "c")
                    .replace('M', "m")
                    .contains("main-content")
                || class
                    .replace('C', "c")
                    .replace('P', "p")
                    .contains("page-content")
                || id == "content"
                || class == "content")
    },
    |node| {
        node.tag == "main"
            || (matches!(node.tag.as_str(), "article" | "div" | "section")
                && [id(node), class(node), node.attr("role")]
                    .iter()
                    .any(|value| value.starts_with("main")))
    },
];

pub const OVERALL_DISCARDED: &[Rule] = &[
    |node| {
        if !section_tag(node) {
            return false;
        }
        let (id, class) = (node.attr("id"), node.attr("class"));
        [id, class].iter().any(|value| {
            value.starts_with("shar")
                || contains_any(
                    value,
                    &[
                        "social",
                        "viral",
                        "newsletter",
                        "syndication",
                        "tags",
                        "sidebar",
                        "banner",
                        "breadcrumb",
                        "bread-crumb",
                        "button",
                        "author",
                    ],
                )
        }) || contains_any(
            id,
            &[
                "bmdh",
                "footer",
                "Footer",
                "share",
                "Share",
                "nav",
                "Nav",
                "menu",
                "related",
                "message-container",
                "premium",
            ],
        ) || contains_any(
            class,
            &[
                "subnav",
                "avigation",
                "navbar",
                "navbox",
                "menu",
                "bar",
                " ad ",
                "-ad-",
                "outbrain",
                "taboola",
                "criteo",
                "paid-content",
                "paidcontent",
                "widget",
                "footer",
                "Footer",
                "byline",
                "Byline",
                "share-",
                "sociable",
                "embedded",
                "embed",
                "tag-list",
                "consent",
                "modal-content",
                "permission",
                "elated",
                "next-",
                "-stories",
                "most-popular",
                "meta",
                "rating",
                "attachment",
                "timestamp",
                "user-info",
                "user-profile",
                "-icon",
                "article-infos",
                "message-container",
                "slide",
                "viewport",
                "overlay",
                "options",
                "expand",
                "obfuscated",
                "blurred",
                "mol-factbox",
                "yin",
                "zlylin",
                "nfoline",
            ],
        ) || starts_any(id, &["jp-", "dpsp-content"])
            || starts_any(class, &["nav", "post-nav", "ZendeskForm"])
            || first_id_class(node).contains("cookie")
            || node.attr("role").replace('N', "n").contains("nav")
            || node.attr("data-component").contains("MostPopularStories")
            || node.has_attr("data-lp-replacement-content")
    },
    |node| {
        let (id, class, style) = (node.attr("id"), node.attr("class"), node.attr("style"));
        let id_style = node
            .attrs
            .iter()
            .find(|attribute| matches!(attribute.key.as_str(), "id" | "style"))
            .map_or("", |attribute| attribute.value.as_str());
        contains_any(
            class,
            &[
                "comments-title",
                "nocomments",
                "-reply-",
                "message",
                "akismet",
                "suggest-links",
                "-hide-",
                "hide-print",
                " hidden",
                " hide",
                "noprint",
                "notloaded",
            ],
        ) || first_id_class(node).starts_with("reply-")
            || id_style.contains("hidden")
            || class.starts_with("hide-")
            || contains_any(id, &["reader-comments", "akismet"])
            || contains_any(style, &["display:none", "display: none"])
            || node.attr("aria-hidden") == "true"
    },
];

pub const FALLBACK_DISCARDED: &[Rule] = &[
    |node| {
        if !section_tag(node) {
            return false;
        }
        let (id, class) = (trim_space(node.attr("id")), trim_space(node.attr("class")));
        let combined = format!("{id} {class}");
        contains_any(&to_lower(id), &["footer", "share", "nav"])
            || contains_any(&to_lower(class), &["footer", "byline"])
            || contains_any(id, &["related", "menu", "bmdh", "premium"])
            || starts_any(id, &["shar", "jp-", "dpsp-content"])
            || starts_any(class, &["shar", "nav", "post-nav", "ZendeskForm"])
            || contains_any(
                &combined,
                &[
                    "viral",
                    "social",
                    "syndication",
                    "newsletter",
                    "tags",
                    "sidebar",
                    "banner",
                    "breadcrumb",
                    "bread-crumb",
                    "author",
                    "button",
                    "message-container",
                ],
            )
            || contains_any(
                class,
                &[
                    "elated",
                    "share-",
                    "sociable",
                    "embedded",
                    "embed",
                    "subnav",
                    "tag-list",
                    "bar",
                    "meta",
                    "menu",
                    "avigation",
                    "navbar",
                    "navbox",
                    "rating",
                    "widget",
                    "attachment",
                    "timestamp",
                    "user-info",
                    "user-profile",
                    "-ad-",
                    "-icon",
                    "article-infos",
                    "nfoline",
                    "outbrain",
                    "taboola",
                    "criteo",
                    "options",
                    "expand",
                    "consent",
                    "modal-content",
                    " ad ",
                    "permission",
                    "next-",
                    "-stories",
                    "most-popular",
                    "mol-factbox",
                    "yin",
                    "zlylin",
                    "slide",
                    "viewport",
                    "overlay",
                    "paid-content",
                    "paidcontent",
                    "obfuscated",
                    "blurred",
                ],
            )
            || first_id_class(node).contains("cookie")
            || to_lower(node.attr("role")).contains("nav")
            || node.attr("data-component").contains("MostPopularStories")
            || node.has_attr("data-lp-replacement-content")
    },
    |node| {
        let (id, class, style) = (
            trim_space(node.attr("id")),
            trim_space(node.attr("class")),
            node.attr("style"),
        );
        contains_any(
            class,
            &[
                "comments-title",
                "nocomments",
                "-reply-",
                "message",
                "akismet",
                "suggest-links",
                "-hide-",
                "hide-print",
                " hidden",
                " hide",
                "noprint",
                "notloaded",
            ],
        ) || format!("{id}{class}").starts_with("reply-")
            || contains_any(id, &["reader-comments", "akismet"])
            || class.starts_with("hide-")
            || format!("{id}{style}").contains("hidden")
            || contains_any(style, &["display:none", "display: none"])
            || node.attr("aria-hidden") == "true"
    },
];

pub const PRECISION_DISCARDED: &[Rule] = &[
    |node| node.tag == "header",
    |node| {
        section_tag(node)
            && (first_id_class(node).contains("bottom")
                || first_id_class(node)
                    .split(is_space)
                    .any(|word| word == "link")
                || node.attr("style").contains("border"))
    },
];

pub const COMMENTS: &[Rule] = &[
    |node| {
        comment_tag(node)
            && (contains_any(first_id_class(node), &["commentlist", "comment-list"])
                || contains_any(
                    class(node),
                    &["comment-page", "comments-content", "post-comments"],
                ))
    },
    |node| {
        comment_tag(node)
            && (starts_any(first_id_class(node), &["comments", "comment-"])
                || class(node).starts_with("Comments")
                || class(node).contains("article-comments"))
    },
    |node| comment_tag(node) && starts_any(id(node), &["comol", "disqus_thread", "dsq-comments"]),
    |node| {
        matches!(node.tag.as_str(), "div" | "section")
            && (id(node).starts_with("social") || class(node).contains("comment"))
    },
];

pub const DISCARDED_COMMENTS: &[Rule] = &[
    |node| matches!(node.tag.as_str(), "div" | "section") && id(node).starts_with("respond"),
    |node| {
        matches!(
            node.tag.as_str(),
            "cite" | "quote" | "blockquote" | "pre" | "q"
        )
    },
    |node| {
        contains_any(
            class(node),
            &[
                "comments-title",
                "nocomments",
                "-reply-",
                "message",
                "signin",
            ],
        ) || first_id_class(node).starts_with("reply-")
            || first_id_class(node).contains("akismet")
            || node.attr("style").contains("display:none")
    },
];

pub const REMOVED_COMMENTS: &[Rule] = &[|node| {
    (comment_tag(node) || node.tag == "details")
        && (starts_any(id(node), &["comment", "Comment"])
            || starts_any(class(node), &["comment", "Comment"])
            || contains_any(class(node), &["article-comments", "post-comments"])
            || starts_any(id(node), &["comol", "disqus_thread", "dsq-comments"]))
}];
pub const DISCARDED_IMAGES: &[Rule] = &[|node| {
    section_tag(node) && (id(node).contains("caption") || class(node).contains("caption"))
}];
pub const DISCARDED_TEASERS: &[Rule] = &[|node| {
    section_tag(node)
        && (id(node).replace('T', "t").contains("teaser")
            || class(node).replace('T', "t").contains("teaser"))
}];

pub fn query(document: &Document, root: NodeId, rule: Rule) -> Option<NodeId> {
    let mut pending: Vec<_> = document.nodes[root]
        .children
        .iter()
        .rev()
        .copied()
        .collect();
    while let Some(element) = pending.pop() {
        let node = &document.nodes[element];
        if node.kind == crate::Kind::Element && rule(node) {
            return Some(element);
        }
        pending.extend(node.children.iter().rev().copied());
    }
    None
}

pub fn query_all(document: &Document, root: NodeId, rule: Rule) -> Vec<NodeId> {
    let mut matches = Vec::new();
    let mut pending: Vec<_> = document.nodes[root]
        .children
        .iter()
        .rev()
        .copied()
        .collect();
    while let Some(element) = pending.pop() {
        let node = &document.nodes[element];
        if node.kind == crate::Kind::Element && rule(node) {
            matches.push(element);
        }
        pending.extend(node.children.iter().rev().copied());
    }
    matches
}
