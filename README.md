# Rust-Trafilatura

Native Rust extraction of article content, comments and metadata from supplied
HTML. The library includes reader decoding, DOM extraction, cleaning, tables,
formatting, links/images, JSON-LD, dates, language filtering, deduplication,
recovery and native fallback extractors. The `rustHTML` executable implements the
Trafilatura-only contract of the existing Go worker.

**Version 2.2.3** is a GitHub source release requiring Rust 1.98.1 and a native
C toolchain. The repository remains private and requires authorized Git access.
Dependencies use released registry packages or pinned Git tags/commits; sibling
checkouts are not required. This crate is not published to crates.io.

## Compatibility

The compatibility target is the current Go-Trafilatura **2.2.2 development
worktree**, based on `ed2b4c86a5727110178172cb18080efe98fdcdb2` plus the
source-hashed changes in [testdata/go-worktree.json](testdata/go-worktree.json).
Go continues to track Python Trafilatura 2.2.0, commit
`c1bc9531a2a978326112ca9987e1382745116136`, for non-fallback extraction, with
explicit retained Go behavior. Rust matches those Go choices: complete cleaning,
final-body recovery measurement, automatic language metadata and normalized
metadata selector IDs/classes. Python fixtures remain independent and unchanged.

Fallbacks retain Go-Trafilatura's ReadabilityV2 0.6.0, DomDistiller, custom
candidates, ordering, acceptance, lazy stopping, sanitization and recall rescue.
They do not use jusText or Python's bundled Readability. The saved Go reference
is `72dce36bfe95502563533cf68a9050370a3d7081`; it identifies the historical
helper/fallback oracle, not the current extraction target.

Compatibility is an algorithmic goal, not a set of page exceptions. The tests
and corpus checks provide evidence, not proof over arbitrary HTML. Python's
lxml parser and XML serializers differ from the native HTML parser and plain
text renderer. Same-DOM selected-content comparisons are reported separately
from complete native output comparisons. See [UPSTREAM.md](UPSTREAM.md).

## Library

```toml
[dependencies]
rust-trafilatura = { git = "https://github.com/markusmobius/rust-trafilatura", tag = "v2.2.3" }
```

```rust
use rust_trafilatura::{extract, HtmlDateMode, Options, Url};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let html = "<html><body><article><h1>Example article</h1>\
    <p>The experiment provides evidence for a reproducible result.</p>\
    </article></body></html>";
let options = Options {
    original_url: Url::request("https://example.org/article"),
    exclude_comments: true,
    html_date_mode: HtmlDateMode::Disabled,
    ..Default::default()
};
let result = extract(html.as_bytes(), &options)?;
assert!(result.content_text.contains("reproducible result"));
let html = result.content_node.document.outer_html(result.content_node.root);
assert!(html.contains("<p>"));
# Ok(())
# }
```

- `extract(impl Read, &Options)` detects or accepts a charset, decompresses gzip
  including concatenated members, normalizes Unicode, removes soft hyphens and
  parses HTML. Reader/decompression failures are returned as `Error::Io`.
- `parse_html(&str)` uses the ReadabilityV2 parser's output sink to build the
  extraction DOM without an intermediate Readability arena.
- `parse_shared_html(&str)` and `parse_shared_bytes(&[u8])` construct a
  `SharedDocument` for all three native extractors. The byte API decodes bytes
  already in memory; it does not open files.
- `extract_shared_document(&SharedDocument, &Options)` borrows that input.
  The same object implements Readability's `DomSource` and DomDistiller's
  `AsRef<Document>` input contract, retaining namespaces for Readability.
- `extract_document(&Document, &Options)` and `extract_node(&Document, NodeId,
  &Options)` clone caller-owned input. They do not decode or normalize it again.
- `ExtractResult` contains owned content/comment trees, plain text and metadata.
  `Error` distinguishes missing input, charset problems, metadata requirements,
  output limits, duplicate bodies and language rejection.

`Options` controls balanced/precision/recall focus, comments, tables, links,
images, per-call deduplication, target language, author exclusions, CSS pruning,
output limits, date extraction and optional fallback candidates. Fallbacks,
images and links are off by default; comments and tables are on. `Config`
retains Go's thresholds. An explicit date configuration takes priority over the
date mode, and a date override bypasses extraction.

The owned DOM is re-exported from Rust-DomDistiller. Prefer `parse_html` for the
current parser; `Document::parse` is the dependency's older parser. Direct arena
edits require valid acyclic indices and consistent parent/child relationships.
The extraction helpers are not an HTML security sanitizer.

All extraction is native and caller-scheduled. There is no production Go/Python
bridge, page fetcher, model download or internal worker pool. The language model
is embedded and initialized lazily. Date and fallback adapters import nodes
without serializing the caller's tree into another parser.

Shared input does not replace the engines' internal working trees. Each engine
makes its own required copies; Readability imports once per call and then uses
copy-on-write retries. The shared object is unchanged by extraction. This is an
additive library API; it does not change the production worker protocol below.

## Worker

Build with `cargo build --locked --release --bin rustHTML`.

With no arguments, the worker writes `ready` to stdout and reads newline-ended
requests in the form `JSON<TAB>output-path`. It writes the JSON result XORed with
255 to that file and reports `ok` or `error`. The process handles successive
requests. An invalid tab-delimited line ends the loop.

With `port parent-id readiness-guid`, it connects to localhost, sends readiness
and exchanges framed JSON messages. Each frame starts with two big-endian
32-bit integers: payload length and chunk size. Response chunks are 1 MiB.
Input envelopes use `GUID` and `Command`; replies use `MType`, `GUID` and
`Content`. Fields are case-insensitive, and omitted/null envelope fields retain
their previous values, matching the Go event loop. Partial reads/writes and
clean disconnects are handled; truncated frames return an I/O error.

Example command payload:

```json
{"HTML":"<article><p>Article text.</p></article>","URL":"https://example.org/","RunTrafilatura":true,"Verbose":true}
```

`HTMLPath` takes precedence over `HTML`. A file beginning with `||XOR||` has its
remaining bytes XOR-decoded with 255. Worker input is parsed directly, just as
in Go; it does not use the library reader's normalization. Extraction enables
fallbacks, images, links and tables, excludes comments, and returns Go's five
outer response sections. Only `Trafilatura` is populated. `Verbose` controls raw
HTML; processed text retains formatting/link/image markers and ordered URLs.

Standalone `RunReadability`, `RunDistiller`, `RunMeta` and `RunDate` are explicitly
unsupported. They return a file-protocol error or terminate the TCP request with
an error, rather than pretending to implement those outputs. Malformed JSON,
unreadable input files, invalid URLs and extraction rejections otherwise retain
the Go worker's zero-valued output contract.

## Dependencies

| Library | Version | Role |
| --- | --- | --- |
| rust-domdistiller | 1.0.1, pinned Git tag | Owned DOM and fallback extraction |
| rust-htmldate | 1.10.2, pinned Git tag | Date extraction |
| rust-dateparser | 1.4.7, pinned Git source | Indirect through HtmlDate |
| rust-dateutil | 2.9.1, pinned Git source | Indirect through date libraries |
| rust-py3langid | 0.4.0 | Embedded native language identification |
| rust-readability-v2 | 0.6.2, pinned Git tag | HTML parser, shared-input view and Readability fallback |
| mimalloc | 0.1.48 | Default allocator for the worker and benchmark executables only |

The selected graph is **not registry-only**. Exact source commits, versions and
checksums are retained in [Cargo.lock](Cargo.lock). No local path dependencies
or runtime Go/Python bridges are required.

Release executables use ThinLTO and the `mimalloc` feature by default. Build with
`--no-default-features` to use the platform allocator. The library itself does
not install a global allocator, so an embedding application retains control.
Compiler profile and allocator choices are part of the benchmark identity;
executable timings do not describe every embedding application's configuration.

## Patch Qualification

The [paired extraction benchmark](https://github.com/markusmobius/content-extractor-benchmark/blob/5edcfd090f1590c9bbf26d7543fbdc2ab615e117/rust_shared_performance_2026_09_21.json)
compares 2.2.2 with 2.2.3 in coordinated three-engine Rust suites on 2,659 pages.
One full warmup precedes four paired passes. File reads are untimed and parsing
is measured separately; **Trafilatura fallback and comments are disabled**.
Extraction takes 5.824 versus 5.838 ms/page on the same best two passes (+0.24%);
the all-four-pass difference is +0.25%. Both pass the 5% regression gate, and
scored text, metadata and errors match on every page. These are Windows GNU,
Rust 1.98.1, ThinLTO/mimalloc results, not a standalone or Go/Rust speed comparison.

## Verification

Use Rust 1.98.1 and `TZ=UTC`. Default tests need no Go or Python after Cargo
dependencies are fetched. Windows/GNU and Linux use separate target directories.

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --release
cargo build --locked --release
```

The suite covers text/Unicode/URL helpers, selectors, metadata/date options,
cleaning, handlers, content/comments, fallback decisions, full extraction option
matrices, reader boundaries and the worker. Expected values come from independent
Python or Go executions, never from the Rust implementation under test.

Development-only reference tools:

```sh
python tools/go_reference.py --go-source ../../go-trafilatura
python tools/go_reference.py --go-source ../../go-trafilatura --worktree
python tools/python_reference.py --upstream /path/to/pinned/trafilatura --go-source ../../go-trafilatura
python tools/worker_reference.py --go-source ../../go-trafilatura --rust-binary target/release/rustHTML
```

The first command checks the immutable Go fixture; `--worktree` checks the
separately labelled current-Go cross-port fixture. `--write` explicitly
regenerates references. The Python tool verifies its source and dependency pins.
The worker tool builds unmodified application source in a temporary module with
current Go-Trafilatura substituted there, leaving the parent application intact.

[tools/benchmark.py](tools/benchmark.py) compares all 983 saved pages, exact
native outputs, snippet precision/recall/F1/accuracy and controlled extraction
timing. Python core checks can use raw input or a separately labelled same-DOM
diagnostic. Timing in that older tool uses preparsed DOMs and includes snippet
scoring; it is not interchangeable with end-to-end page latency.

The same example also supports `benchmark --jsonl --focus balanced` for the
sibling content-extractor benchmark's paired page runner. It reads original
files, decodes/parses, extracts and flushes one response per request. Add
`--native-output` for complete native-output differential checks, not speed
measurement. On the 2026-09-19 development selection, all 2,659 pages matched
current Go's native HTML, body/comment text, full metadata and error strings.
This is corpus evidence, not a claim of equality for every possible input.