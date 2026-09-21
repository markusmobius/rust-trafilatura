import argparse
import hashlib
import importlib.metadata
import io
import json
import logging
import platform
import subprocess
import sys
import tarfile
import time
from copy import deepcopy
from pathlib import Path

import trafilatura
from packaging.requirements import Requirement
from packaging.utils import canonicalize_name
from trafilatura.utils import language_classifier


COMMIT = "c1bc9531a2a978326112ca9987e1382745116136"
PYTHON_VERSION = "3.12.13"
ROOT = Path(__file__).resolve().parents[1]
REQUIRED_VERSIONS = {
    "trafilatura": "2.2.0",
    "py3langid": "0.4.0",
    "numpy": "2.5.2",
    "lxml": "6.1.3",
    "htmldate": "1.10.0",
    "dateparser": "1.4.2",
    "python-dateutil": "2.9.0.post0",
    "courlan": "1.4.0",
    "justext": "3.0.2",
}


def reference_identity(upstream, git):
    if platform.python_version() != PYTHON_VERSION:
        raise ValueError(f"Expected Python {PYTHON_VERSION}, got {platform.python_version()}")
    for name, expected in REQUIRED_VERSIONS.items():
        actual = importlib.metadata.version(name)
        if actual != expected:
            raise ValueError(f"Expected {name} {expected}, got {actual}")
    archive = subprocess.run(
        [git, "-C", str(upstream), "-c", "core.autocrlf=false", "archive", "--format=tar", COMMIT, "trafilatura"],
        check=True, capture_output=True,
    ).stdout
    source_hashes = {}
    installed = Path(trafilatura.__file__).resolve().parent
    with tarfile.open(fileobj=io.BytesIO(archive)) as source:
        for member in source.getmembers():
            if not member.isfile() or not (member.name.endswith(".py") or member.name.endswith("settings.cfg")):
                continue
            relative = Path(member.name).relative_to("trafilatura")
            expected = source.extractfile(member).read().replace(b"\r\n", b"\n")
            actual = (installed / relative).read_bytes().replace(b"\r\n", b"\n")
            if actual != expected:
                raise ValueError(f"Installed Python source differs from {COMMIT}: {relative}")
            source_hashes[relative.as_posix()] = hashlib.sha256(expected).hexdigest()
    if "utils.py" not in source_hashes:
        raise ValueError("Pinned Python source archive is incomplete")
    packages = {}
    pending = ["trafilatura", "py3langid"]
    while pending:
        name = canonicalize_name(pending.pop())
        if name in packages:
            continue
        distribution = importlib.metadata.distribution(name)
        packages[name] = distribution.version
        for dependency in distribution.requires or []:
            requirement = Requirement(dependency)
            if requirement.marker is None or requirement.marker.evaluate({"extra": ""}):
                pending.append(requirement.name)
    return {"commit": COMMIT, "python": PYTHON_VERSION, "packages": packages, "source_sha256_lf": source_hashes}


def language_cases():
    inputs = [
        "", "1234",
        "This is an English article about the development of software and scientific research.",
        "Ceci est un article en fran\u00e7ais sur le d\u00e9veloppement des logiciels et la recherche scientifique.",
        "Dies ist ein deutscher Artikel \u00fcber die Entwicklung von Software und wissenschaftliche Forschung.",
        "Este es un art\u00edculo en espa\u00f1ol sobre el desarrollo de programas y la investigaci\u00f3n cient\u00edfica.",
        "\u65e5\u672c\u8a9e\u306e\u6587\u7ae0\u3092\u8aad\u307f\u307e\u3059\u3002", "caf\u00e9", "abcd",
        "Texte en fran\u00e7ais", "Texto en espa\u00f1ol ",
        "Die Kommentare sind aber etwas l\u00e4nger.", "Hier ist ein Text auf Deutsch", "This is English.",
        "\u00e9" * 8, "\u65e5\u672c\u8a9e", "e\u0301", "\u00e9",
    ]
    return [
        {"content": content, "comments": comments, "language": language_classifier(content, comments)}
        for content in inputs for comments in inputs
    ]


def language_extraction_cases():
    phrases = [
        "This is an English article about the development of software and scientific research.",
        "Ceci est un article en fran\u00e7ais sur le d\u00e9veloppement des logiciels et la recherche scientifique.",
        "Este es un art\u00edculo en espa\u00f1ol sobre el desarrollo de programas y la investigaci\u00f3n cient\u00edfica.",
    ]
    headers = ["", '<meta property="og:locale" content="en_US">', '<meta http-equiv="content-language" content="fr">']
    cases = []
    for phrase in phrases:
        for header in headers:
            html = f'<html lang="fr"><head>{header}</head><body><article><p>{" ".join([phrase] * 8)}</p></article></body></html>'
            for target in (None, "", "en", "fr"):
                result = trafilatura.bare_extraction(html, fast=True, with_metadata=True, target_language=target)
                cases.append({
                    "html": html, "target": target, "fast": True, "accepted": result is not None,
                    "language": result.language if result is not None else None,
                    "content": result.raw_text or "" if result is not None else "",
                    "comments": result.comments or "" if result is not None else "",
                })
    return cases


def metadata_attribute_cases():
    from trafilatura.metadata import extract_metadata

    cases = []
    for tag in ("a", "div", "span"):
        for attribute in ("class", "id"):
            for value in ("author", "username", "byline", " author", "author ", " username ", "\tusername\n", "byline\u00a0", "author name", "author  name"):
                html = f'<html><head><title>Attribute selection</title></head><body><{tag} {attribute}="{value}">Maria Example</{tag}></body></html>'
                result = extract_metadata(html)
                cases.append({"html": html, "author": result.author})
    return cases


def content_selector_cases():
    from lxml import etree
    from trafilatura.xpaths import (
        BODY_XPATH, OVERALL_DISCARD_XPATH, PRECISION_DISCARD_XPATH, COMMENTS_XPATH,
        COMMENTS_DISCARD_XPATH, REMOVE_COMMENTS_XPATH, DISCARD_IMAGE_ELEMENTS, TEASER_DISCARD_XPATH,
    )

    inputs = json.loads((ROOT / "testdata/go-reference.json").read_bytes())["selectors"]
    cases = []
    for sample in inputs:
        value, layout = sample["value"], sample["layout"]
        attributes = [
            [("id", value)], [("class", value)],
            [("id", "unrelated"), ("class", value)], [("class", value), ("id", "unrelated")],
            [(key, value) for key in ("style", "role", "itemprop", "data-component", "aria-hidden")],
            [("id", f"\u00a0{value}\t"), ("class", f"\u00a0{value}\t")],
        ][layout]
        if layout == 4 and value == "data-lp-replacement-content":
            attributes.append((value, ""))
        cases.append({"tag": sample["tag"], "attributes": attributes})
    for value in (" hidden", " hidden ", "hide", " hide", "\thidden", " ad ", "\u00a0hidden", "FOOTER", "Footer", "byLine", "reply-test",
                  "article", "article ", "article rating", "article_rating", " article ", "post", " post ", "POSTCONTENT", "articleBody", "ARTICLEBODY", "Main-Content", "MAIN-CONTENT", "dsq-comments", "dsq_comments", "COMMENT", "Comment"):
        for attributes in ([("class", value)], [("id", "keep"), ("class", value)], [("class", value), ("id", "keep")]):
            cases.append({"tag": "div", "attributes": attributes})
    for attributes in ([('id', 'keep'), ('style', 'visibility:hidden')], [('style', 'visibility:hidden'), ('id', 'keep')]):
        cases.append({"tag": "div", "attributes": attributes})
        tags = {"li": "item", "dd": "item", "dt": "item", "ol": "list", "ul": "list", "dl": "list",
            "blockquote": "quote", "pre": "quote", "q": "quote"}
    groups = (BODY_XPATH, OVERALL_DISCARD_XPATH, PRECISION_DISCARD_XPATH, COMMENTS_XPATH,
              COMMENTS_DISCARD_XPATH, REMOVE_COMMENTS_XPATH, DISCARD_IMAGE_ELEMENTS, TEASER_DISCARD_XPATH)
    for case in cases:
        root = etree.Element("body")
        element = etree.SubElement(root, tags.get(case["tag"], case["tag"]))
        for key, value in case["attributes"]:
            element.set(key, value)
        case["matches"] = [element in query(root) for group in groups for query in group]
    return cases


def pruning_cases():
    from lxml import etree
    from trafilatura.htmlprocessing import prune_unwanted_nodes
    from trafilatura.utils import load_html
    from trafilatura.xpaths import (
        BODY_XPATH, OVERALL_DISCARD_XPATH, PRECISION_DISCARD_XPATH, COMMENTS_XPATH,
        COMMENTS_DISCARD_XPATH, REMOVE_COMMENTS_XPATH, DISCARD_IMAGE_ELEMENTS, TEASER_DISCARD_XPATH,
    )

    groups = (BODY_XPATH, OVERALL_DISCARD_XPATH, PRECISION_DISCARD_XPATH, COMMENTS_XPATH,
              COMMENTS_DISCARD_XPATH, REMOVE_COMMENTS_XPATH, DISCARD_IMAGE_ELEMENTS, TEASER_DISCARD_XPATH)
    tags = {"li": "item", "dd": "item", "dt": "item", "ol": "list", "ul": "list", "dl": "list",
            "blockquote": "quote", "pre": "quote", "q": "quote"}

    def snapshot(element):
        if element.tag is etree.Comment:
            return {"comment": element.text or ""}
        children = [element.text] if element.text else []
        for child in element:
            children.append(snapshot(child))
            if child.tail:
                children.append(child.tail)
        return {"tag": element.tag, "attributes": list(element.attrib.items()), "children": children}

    cases = []
    for sample in json.loads((ROOT / "testdata/go-reference.json").read_bytes())["pruning"]:
        tree = load_html(sample["html"])
        input_tree = snapshot(tree)
        for element in tree.iter():
            element.tag = tags.get(element.tag, element.tag)
        result = prune_unwanted_nodes(tree, groups[sample["group"]], with_backup=sample["backup"])
        cases.append({"html": sample["html"], "input_tree": input_tree, "group": sample["group"], "backup": sample["backup"], "tree": snapshot(result)})
    return cases


def import_native_tree(item):
    from lxml import etree
    from lxml.html import Element

    if "comment" in item:
        comment = etree.Comment()
        comment.text = item["comment"]
        return comment
    element = Element(item["tag"])
    for key, value in item["attributes"]:
        element.set(key, value)
    previous = None
    for child in item["children"]:
        if isinstance(child, str):
            if previous is None:
                element.text = (element.text or "") + child
            else:
                previous.tail = (previous.tail or "") + child
        else:
            previous = import_native_tree(child)
            element.append(previous)
    return element


def native_core_cases():
    from trafilatura import deduplication
    from trafilatura.core import trafilatura_sequence
    from trafilatura.htmlprocessing import handle_textnode, process_node
    from trafilatura.settings import DEFAULT_CONFIG, Extractor
    from trafilatura.utils import textfilter

    source = json.loads((ROOT / "testdata/go-reference.json").read_bytes())

    def document(html):
        return import_native_tree(source["parsed_inputs"][html])

    def options(sample):
        flags = sample["flags"]
        return Extractor(fast=True, formatting=True, recall=sample["focus"] == 1, precision=sample["focus"] == 2,
                         images=bool(flags & 1), links=bool(flags & 2), dedup=bool(flags & 4),
                         comments=not flags & 16, tables=not flags & 32, url="https://example.com/news/page")

    def text(body):
        return "".join(body.itertext()) if body is not None else ""

    text_nodes = []
    original_cache = deduplication.LRU_TEST
    try:
        for sample in source["text_nodes"]:
            deduplication.LRU_TEST = deduplication.LRUCache(maxsize=sample["capacity"] if sample["capacity"] > 0 else 4096)
            probe = document(sample["html"]).xpath('//*[@id="probe"]')[0]
            original_tag = probe.tag
            probe.tag = {"img": "graphic", "br": "lb", "hr": "lb"}.get(probe.tag, probe.tag)
            converted_tag = probe.tag
            config = deepcopy(DEFAULT_CONFIG)
            config.set("DEFAULT", "MIN_DUPLCHECK_SIZE", "0")
            settings = Extractor(dedup=True, config=config)
            filtered, accepted = textfilter(probe), []
            for iteration in range(6):
                result = process_node(probe, settings) if sample["light"] else handle_textnode(probe, settings, sample["fix"], sample["spaces"])
                accepted.append(result is not None)
                if iteration == 3:
                    deduplication.LRU_TEST.put("different cache entry", 1)
            text_nodes.append({**{key: sample[key] for key in ("html", "fix", "spaces", "light", "capacity")},
                               "filtered": filtered, "accepted": accepted, "text": probe.text or "", "tail": probe.tail or "",
                               "tag": original_tag if probe.tag == converted_tag else probe.tag})
        deduplication.LRU_TEST = deduplication.LRUCache(maxsize=4096)
        sequences = []
        for sample in source["sequences"]:
            if sample["flags"] & 8:
                sequences.append(None)
                continue
            deduplication.LRU_TEST.clear()
            result = trafilatura_sequence(document(sample["html"]), options(sample), "https://example.com/news/page")
            sequences.append({**{key: sample[key] for key in ("html", "focus", "flags")}, "snapshot": result[1], "content": text(result[0]), "comments": text(result[3]), "comments_snapshot": result[4]})
        extraction = []
        for sample in source["extraction"]:
            if sample["flags"] & 8 or sample["variant"] in (1, 2):
                extraction.append(None)
                continue
            deduplication.LRU_TEST.clear()
            settings = options(sample)
            settings.lang = "en" if sample["variant"] == 3 else None
            settings.max_tree_size = 1 if sample["variant"] == 4 else None
            if sample["variant"] == 5:
                settings.min_output_size = settings.min_output_comm_size = 50000
            pruning = './/p[contains(concat(" ", normalize-space(@class), " "), " bar ")]|.//aside' if sample["variant"] == 6 else None
            result = trafilatura.bare_extraction(document(sample["html"]), options=settings, prune_xpath=pruning)
            extraction.append({**{key: sample[key] for key in ("html", "focus", "flags", "variant")}, "accepted": result is not None, "content": text(result.body) if result is not None else "",
                               "comments": text(result.commentsbody) if result is not None else ""})
        paragraph = "This article explains a useful scientific result with enough context for a reader. " * 8
        elements = [
            f'<h2 class="footer">Section label</h2><p>{paragraph}</p>',
            f'<p><strong class="footer">Important introduction</strong> {paragraph}</p>',
            f'<p><em style="display:none">Emphasized introduction</em> {paragraph}</p>',
            f'<noindex><p>Excluded section. {paragraph}</p></noindex><p>{paragraph}</p>',
            f'<noindex><p>Only section. {paragraph}</p></noindex>',
            f'<details><summary>Expandable label</summary><p>{paragraph}</p></details>',
            f'<ul><li><a href="/chapter">{paragraph}</a></li><li>Final item.</li></ul>',
            f'<dl><dt><a href="/chapter">{paragraph}</a></dt><dd>Final description.</dd></dl>',
            f'<blockquote><span class="hljs-code">Quoted statement.</span>{paragraph}</blockquote>Following quotation.',
            f'<pre><span class="other hljs-code">Code statement.</span>{paragraph}</pre>Following code.',
            f'<h2>Section <b>heading</b> tail</h2><p>{paragraph}</p>',
            f'<p>Lead text <code>status</code> followed by an explanation. {paragraph}</p>',
            f'<p><strong><em>Nested introduction</em></strong></p><p>{paragraph}</p>',
            f'<p>Lead <del>deleted <b>nested</b> words</del> conclusion. {paragraph}</p>',
            f'<p>Lead <code>status <b>nested</b> words</code> conclusion. {paragraph}</p>',
            f'<article><div>Loose navigation label.</div><div class="sidebar"><p>{paragraph}{paragraph}</p></div></article>',
            f'<p>{paragraph}</p><div id="comments"><p>First <b>comment</b> with a tail.</p><p>Next\n line <i>with emphasis</i>.</p></div>',
            f'<p>{paragraph}</p><ul><li><p>Outer list item</p><ul><li>from <b>Monday</b> onward</li><li>second item</li></ul></li><li>last item</li></ul>',
            f'<p>Start <b>format<br>first break<br>second break</b> tail <i>remaining words</i></p><p>{paragraph}</p>',
            f'<p>Start <b><custom>nested text</custom><other>second nested text</other></b> tail <i>remaining words</i></p><p>{paragraph}</p>',
            f'<figure><figure><p>Inner image caption.</p></figure></figure><figure><figcaption><p>Later image caption.</p></figcaption></figure><p>{paragraph}</p>',
            f'<div><div><span></span></div>carried tail</div><p>{paragraph}</p>',
            f'<div class="post-content"><h2><a href="/linked">{"Linked navigation sentence. " * 5}</a></h2></div><article><div>Loose navigation label.</div><p>{paragraph}{paragraph}</p></article>',
            f'<p>{paragraph}</p><blockquote><p><wbr>some<wbr> words<wbr> remain</p></blockquote>',
            f'<p>{paragraph}</p><blockquote><p>Lead <b>word<br>first<br>second<custom>last</custom></b> tail <i>end</i></p><p>Later quotation.</p></blockquote>',
            f'<p>{paragraph}</p><table><caption>Caption  with\n  spacing <b>bold caption</b> ending</caption><tr><td>Cell one</td><td>Cell two</td></tr></table>',
            f'<p>{paragraph}</p><table><tr><td><p>Lead <b>word<br>first<br>second</b> tail <i>end</i></p>\n  trailing cell text\n </td><td><ul><li>Outer<ul><li>Inner <b>words</b></li></ul></li></ul></td></tr></table>',
        ]
        edge_sequences = []
        for element in elements:
            for focus in range(3):
                for flags in (0, 2):
                    sample = {"html": '<html><head></head><body><main>' + element + '</main></body></html>', "focus": focus, "flags": flags}
                    deduplication.LRU_TEST.clear()
                    from trafilatura.utils import load_html
                    result = trafilatura_sequence(load_html(sample["html"]), options(sample), "https://example.com/news/page")
                    edge_sequences.append({**sample, "snapshot": result[1], "content": text(result[0]), "comments": text(result[3]), "comments_snapshot": result[4]})
        return {"text_nodes": text_nodes, "sequences": sequences, "extraction": extraction, "edge_sequences": edge_sequences}
    finally:
        deduplication.LRU_TEST = original_cache


def text_filter_cases():
    from types import SimpleNamespace
    from trafilatura.utils import textfilter

    values = ["", "\u001f", "\u0130nstagram", "\u0131nstagram", "\u017fhare", "\u212a"]
    for word in ("Drucken", "Print", "Google", "Facebook", "PDF", "More on this", "Mehr zum Thema:"):
        for prefix in ("", " ", "[", "_", "some ", "\u62a5\u544a", "\u0301"):
            for suffix in ("", ":", "...", " information", "\nPDF", "\rPDF", "\u2028PDF"):
                values.append(prefix + word + suffix)
    cases = []
    for value in values:
        element = SimpleNamespace(text=value, tail=None)
        cases.append({"text": value, "filtered": textfilter(element)})
    return cases


def forum_cases():
    from trafilatura.core import _forum_thread_page
    from trafilatura.utils import load_html
    inputs = [case["html"] for case in json.loads((ROOT / "testdata/go-reference.json").read_bytes())["forums"]]
    return [{"html": html, "forum": bool(_forum_thread_page(load_html('<html><head>' + html + '</head><body></body></html>')))} for html in inputs]


def content_snapshot_cases():
    from trafilatura.core import trafilatura_sequence
    from trafilatura.htmlprocessing import tree_cleaning, convert_tags
    from trafilatura.main_extractor import extract_content
    from trafilatura.settings import Extractor
    from trafilatura.utils import load_html

    cases = []
    for size in (2, 4, 6):
        paragraph = "A substantial repeated article paragraph. " * size
        for copies in (1, 2, 3):
            html = '<html><head></head><body><main>' + f'<p>{paragraph}</p>' * copies + '<table><tr><td>data</td></tr></table></main></body></html>'
            for focus in range(3):
                options = Extractor(fast=True, formatting=True, comments=False, recall=focus == 1, precision=focus == 2)
                tree = convert_tags(tree_cleaning(load_html(html), options), options)
                body, snapshot, length = extract_content(tree, options)
                sequence = trafilatura_sequence(load_html(html), options)
                cases.append({
                    "html": html, "focus": focus, "snapshot": snapshot, "length": length,
                    "cleaned": " ".join(body.itertext()).strip(),
                    "sequence_snapshot": sequence[1], "sequence_cleaned": " ".join(sequence[0].itertext()).strip(),
                })
    return cases


def corpus_worker(corpus, identity):
    from lxml import etree
    from trafilatura.utils import load_html

    pages = json.loads(corpus.read_bytes())
    documents = [load_html(page["html"]) for page in pages]
    print(json.dumps({"ready": len(pages), "reference": identity}), flush=True)
    for line in sys.stdin:
        request = json.loads(line)
        if request.get("fallback"):
            raise ValueError("Python is only the non-fallback reference; fallback comparisons must use Go")
        inputs = documents
        input_errors = [""] * len(pages)
        if "input_documents" in request:
            if len(request["input_documents"]) != len(pages):
                raise ValueError("Supplied DOM count differs from corpus")
            inputs = []
            for index, item in enumerate(request["input_documents"]):
                try:
                    document = import_native_tree(item)
                    if request.get("preprocess_comments"):
                        etree.strip_elements(document, etree.Comment, with_tail=False)
                    inputs.append(document)
                except ValueError as error:
                    inputs.append(None)
                    input_errors[index] = f"DOM import unavailable: {error}"
        counts = {"true_positives": 0, "false_negatives": 0, "false_positives": 0, "true_negatives": 0}
        outputs, errors = [], []
        started = time.perf_counter_ns()
        for page, document, input_error in zip(pages, inputs, input_errors):
            try:
                result = None if input_error else trafilatura.bare_extraction(
                    deepcopy(document), url=page["url"], fast=True, with_metadata=True,
                    include_formatting=True,
                    favor_precision=request.get("focus") == "precision", favor_recall=request.get("focus") == "recall",
                    include_comments=request.get("comments", False), include_images=request.get("images", False),
                    include_links=request.get("links", False), include_tables=not request.get("exclude_tables", False),
                    deduplicate=request.get("deduplicate", False), target_language=request.get("target_language") or None,
                )
            except Exception as error:
                result = None
                input_error = f"Python reference exception: {type(error).__name__}: {error}"
            content = (result.raw_text or "") if result is not None else ""
            for snippet in page["with"]:
                counts["true_positives" if content and snippet in content else "false_negatives"] += 1
            for snippet in page["without"]:
                counts["false_positives" if content and snippet in content else "true_negatives"] += 1
            if result is None:
                errors.append({"file": page["file"], "error": input_error or "extraction rejected"})
            if request.get("outputs"):
                metadata = None
                if result is not None:
                    names = {"Title": "title", "Author": "author", "URL": "url", "Hostname": "hostname",
                             "Description": "description", "Sitename": "sitename", "Date": "date",
                             "Categories": "categories", "Tags": "tags", "ID": "id", "Fingerprint": "fingerprint",
                             "License": "license", "Language": "language", "Image": "image", "PageType": "pagetype"}
                    metadata = {key: getattr(result, field) for key, field in names.items()}
                outputs.append({
                    "file": page["file"], "text": content, "comments": (result.comments or "") if result is not None else "",
                    "selected_text": "".join(result.body.itertext()) if result is not None else "",
                    "selected_comments": "".join(result.commentsbody.itertext()) if result is not None and result.commentsbody is not None else "",
                    "html": etree.tostring(result.body, encoding="unicode") if result is not None else "",
                    "comments_html": etree.tostring(result.commentsbody, encoding="unicode") if result is not None and result.commentsbody is not None else "",
                      "metadata": metadata, "error": "" if result is not None else input_error or "extraction rejected",
                })
        print(json.dumps({"elapsed_ns": time.perf_counter_ns() - started, "counts": counts,
                          "outputs": outputs, "errors": errors,
                          "dom_import_errors": [{"file": page["file"], "error": error} for page, error in zip(pages, input_errors) if error]}, ensure_ascii=True), flush=True)


def main():
    parser = argparse.ArgumentParser(description="Generate one pinned Python oracle for both native Trafilatura ports")
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--go-source", type=Path, required=True)
    parser.add_argument("--git", default="git")
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--corpus", type=Path)
    arguments = parser.parse_args()
    logging.getLogger("trafilatura").setLevel(logging.CRITICAL)
    identity = reference_identity(arguments.upstream, arguments.git)
    if arguments.corpus:
        previous = json.loads((ROOT / "testdata/python-reference.json").read_bytes())
        if any(previous.get(key) != value for key, value in identity.items()):
            raise ValueError("Python corpus reference identity differs from the verified fixture")
        corpus_worker(arguments.corpus, identity)
        return
    result = {**identity, "languages": language_cases(), "language_extraction": language_extraction_cases(),
              "metadata_attributes": metadata_attribute_cases(), "selectors": content_selector_cases(), "pruning": pruning_cases(),
              "content_snapshots": content_snapshot_cases(), "text_filters": text_filter_cases(), "native_core": native_core_cases(), "forums": forum_cases()}
    if any(case["language"] is None for case in result["languages"]):
        raise ValueError("Python language detector is unavailable")
    encoded = (json.dumps(result, ensure_ascii=True, indent=2, sort_keys=True) + "\n").encode("utf-8")
    outputs = [ROOT / "testdata/python-reference.json", arguments.go_source / "test-files/python-2.2.0-reference.json"]
    for output in outputs:
        if output.exists():
            previous = json.loads(output.read_bytes())
            if any(previous.get(key) != value for key, value in identity.items()):
                raise ValueError(f"Python reference identity changed: {output}")
        if arguments.write:
            output.write_bytes(encoded)
        elif not output.exists() or output.read_bytes() != encoded:
            raise ValueError(f"Python fixture differs: {output}; use --write only for a reviewed fixture update")
        print(f"{output}: sha256={hashlib.sha256(encoded).hexdigest()}")
    print(f"Pinned Python {COMMIT}: {len(result['languages'])} classifier and {len(result['language_extraction'])} public extraction cases; {len(identity['packages'])} selected packages")


if __name__ == "__main__":
    main()