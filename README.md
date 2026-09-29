# Rust-Trafilatura

Native Rust extraction of article content, comments and metadata from supplied
HTML. The library includes reader decoding, DOM extraction, cleaning, tables,
formatting, links/images, JSON-LD, dates, language filtering, deduplication,
recovery and native fallback extractors. The `rustHTML` executable implements the
Trafilatura-only contract of the existing Go worker.

**Version 2.2.6** requires Rust 1.98.1 and a native C toolchain. It is available
on [crates.io](https://crates.io/crates/rust-trafilatura/2.2.6) and as a public
[GitHub source release](https://github.com/markusmobius/rust-trafilatura/releases/tag/v2.2.6).
The registry package uses crates.io dependencies throughout; Git source builds
retain pinned Git dependencies. Sibling checkouts are not required. Release
changes are recorded in [CHANGELOG.md](CHANGELOG.md).

## Compatibility

The compatibility target is Go-Trafilatura **2.2.6**. The current independent
Go oracle in [testdata/go-worktree.json](testdata/go-worktree.json) records source
hashes, modules, extraction results and the lxml-only fallback matrix. Historical
Go helper and Python fixtures remain unchanged.
Go continues to track Python Trafilatura 2.2.0, commit
`c1bc9531a2a978326112ca9987e1382745116136`, for non-fallback extraction, with
explicit retained Go behavior. Rust matches those Go choices: complete cleaning,
final-body recovery measurement, automatic language metadata and normalized
metadata selector IDs/classes. Python fixtures remain independent and unchanged.

Non-FAST extraction permits only internally generated bundled readability-lxml.
Mozilla, DomDistiller and supplied/custom candidates are never used by
Trafilatura. Native recall and baseline recovery remain in FAST mode. Python's
jusText recovery is not implemented. Standalone Mozilla/Go-ReadabilityV2 remains
a separate, unchanged extractor. The saved Go reference
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
rust-trafilatura = "=2.2.6"
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
  parses HTML with scripting disabled. Reader/decompression failures are returned as `Error::Io`.
- `parse_html(&str)` uses the ReadabilityV2 parser's output sink to build the
  extraction DOM without an intermediate Readability arena.
- `parse_shared_html(&str)` and `parse_shared_bytes(&[u8])` construct a
  `SharedDocument` for all three native extractors. The byte API decodes bytes
  already in memory; it does not open files.
- `parse_html_with_scripting(source, false)` and
  `parse_shared_html_with_scripting(source, false)` build Trafilatura input with
  parsed noscript children. Existing parser entry points keep scripting enabled;
  preserve their original input for standalone Mozilla noscript image recovery.
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
output limits, date extraction and the optional internal lxml fallback. Fallbacks,
images and links are off by default; comments and tables are on. `Config`
retains Go's thresholds. An explicit date configuration takes priority over the
date mode, and a date override bypasses extraction.

Set `enable_fallback: true` for lxml, or `false` for FAST.
Legacy `readability_fallback` and `fallback_candidates` fields are ignored;
Trafilatura always prepares its own fallback input. The Apache-2.0
port follows Python Trafilatura 2.2.0's bundled readability-lxml, including Arc90,
starrhorne/iterationlabs and gfxmonk/python-readability ancestry. Readability 0.6.5
supplies the additive parser-mode API, not Trafilatura's fallback. See [CHANGELOG.md](CHANGELOG.md);
optional `lab-profile` diagnostics are compiled out normally.

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

Version 2.2.4 builds shared input directly in the final document arena, retaining
ordered attributes and a namespace sidecar through reachable-preorder
compaction. Readability 0.6.3 supplies eager tokenizer fast paths. Parsing still
finishes all decoding, normalization, DOM construction and temporary cleanup
before extraction starts; no content is omitted or materialized lazily.

## Worker

Install from crates.io with `cargo install rust-trafilatura --version 2.2.6 --locked --bin rustHTML`,
or build a source checkout with `cargo build --locked --release --bin rustHTML`.

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
images, links and tables, disables external fallback, excludes comments, and returns Go's five
outer response sections. Only `Trafilatura` is populated. `Verbose` controls raw
HTML; processed text retains formatting/link/image markers and ordered URLs.
The worker always uses FAST with scripting-disabled Trafilatura input. Extensive
date extraction is explicitly retained. Native recall and baseline remain.

Standalone `RunReadability`, `RunDistiller`, `RunMeta` and `RunDate` are explicitly
unsupported. They return a file-protocol error or terminate the TCP request with
an error, rather than pretending to implement those outputs. Malformed JSON,
unreadable input files, invalid URLs and extraction rejections otherwise retain
the Go worker's zero-valued output contract.

## Dependencies

| Library | Version | Role |
| --- | --- | --- |
| rust-domdistiller | 1.0.1 | Owned DOM types, not fallback extraction |
| rust-htmldate | 1.10.2 | Date extraction |
| rust-dateparser | 1.4.7 | Indirect through HtmlDate |
| rust-dateutil | 2.9.1 | Indirect through date libraries |
| rust-py3langid | 0.4.0 | Embedded native language identification |
| rust-readability-v2 | 0.6.5 | HTML parser and shared-input view, not fallback extraction |
| mimalloc | 0.1.48 | Default allocator for the worker and benchmark executables only |

The crates.io distribution resolves every dependency from the registry. The Git
source checkout retains pinned Git dependencies, with exact source commits,
versions and checksums in [Cargo.lock](Cargo.lock). Both distributions use the
same runtime implementation. No local path dependencies or runtime Go/Python
bridges are required.

Release executables use ThinLTO and the `mimalloc` feature by default. Build with
`--no-default-features` to use the platform allocator. The library itself does
not install a global allocator, so an embedding application retains control.
Compiler profile and allocator choices are part of the benchmark identity;
executable timings do not describe every embedding application's configuration.

## Current Quality and Speed

The following measurements are historical 2.2.5 results, not 2.2.6 measurements.
Fresh 2.2.6 quality, timing and fallback-selection results are being collected.

The [2026-09-28 benchmark](https://github.com/markusmobius/content-extractor-benchmark/blob/97c0f3f67261c275ceb2ab532ee05992dbce8cc7/README.md#results-2026-09-28) uses 2,659 saved
development pages: 983 LegoNews, 181 ScrapingHub and 1,495 WCXB. Their F1
scores use different rules and must not be averaged. Errors are listed in
that order and remain in the denominators.

| Implementation | Fallback | LegoNews F1 | ScrapingHub F1 | WCXB F1 | Errors | Extraction ms/page |
| --- | --- | ---: | ---: | ---: | --- | ---: |
| go-trafilatura-2.2.5 | Off | 90.88412% | 96.15663% | 78.49352% | 3 / 0 / 10 | 8.716 |
| rust-trafilatura-2.2.5 | Off | 90.88412% | 96.15663% | 78.49352% | 3 / 0 / 10 | 5.324 |
| go-trafilatura-2.2.5 | Native Lxml | 91.14833% | 95.94602% | 81.17181% | 3 / 0 / 9 | 23.737 |
| rust-trafilatura-2.2.5 | Native Lxml | 91.14833% | 95.94602% | 81.17181% | 3 / 0 / 9 | 10.517 |

Timings are means of **all four measured passes** after one warmup, not best-of
selection. Windows 11 / Ryzen AI 7 PRO 350; Go 1.27.1 and Rust 1.98.1 GNU with
ThinLTO/mimalloc. Native extraction includes working copies, metadata and
text rendering; file I/O, startup, IPC and scoring are excluded.
Shared parsing is charged once per language/page: Go 7.308 and Rust 3.369 ms
with fallback off; Go 8.710 and Rust 4.137 ms in the separate Lxml run.
That historical run explicitly selected Lxml and generated its own candidates;
2.2.6 no longer offers Mozilla fallback. Comments and pagination are off; tables are on.
DomDistiller is not Python jusText, and full Python parity is not claimed.
These are not standalone request latencies or isolated old/new-version speedups.

[Disabled-mode JSON](https://github.com/markusmobius/content-extractor-benchmark/blob/97c0f3f67261c275ceb2ab532ee05992dbce8cc7/go_rust_shared_performance_2026_09_28.json),
[native-Lxml JSON](https://github.com/markusmobius/content-extractor-benchmark/blob/97c0f3f67261c275ceb2ab532ee05992dbce8cc7/go_rust_lxml_performance_2026_09_28.json), and
[release validation](https://github.com/markusmobius/content-extractor-benchmark/blob/97c0f3f67261c275ceb2ab532ee05992dbce8cc7/release_validation_2026_09_28.json)
retain separate metadata scores, output differences, exact source/build pins,
all pass totals and verification limits. Historical results use other protocols.

### Fallback Selection Rates

The separate [6,554-page application-worker validation](https://github.com/markusmobius/content-extractor-benchmark/blob/97c0f3f67261c275ceb2ab532ee05992dbce8cc7/release_validation_2026_09_28.json)
was captured before release packaging, with native readability-lxml explicitly
selected, internally generated candidates and dates disabled. This unannotated
corpus is not the 2,659-page benchmark above or a measurement of Mozilla mode.

Cells show pages and percentages of all 6,554 inputs, including failures.

| Final Content Source | Go | Rust | Python 2.2.0 |
| --- | ---: | ---: | ---: |
| Trafilatura core (including recall) | 5,655 (86.283%) | 5,655 (86.283%) | 5,695 (86.894%) |
| readability-lxml | 206 (3.143%) | 206 (3.143%) | 158 (2.411%) |
| DomDistiller | 75 (1.144%) | 75 (1.144%) | Not used |
| jusText | Not used | Not used | 62 (0.946%) |
| Internal baseline recovery | 597 (9.109%) | 597 (9.109%) | 605 (9.231%) |
| No result | 21 (0.320%) | 21 (0.320%) | 34 (0.519%) |
| **External fallback total** | **281 (4.287%)** | **281 (4.287%)** | **220 (3.357%)** |

Before correction, the Rust worker and Go v2.2.2 supplied-candidate reference
selected external fallback on **2,408 pages (36.741%)** of this same corpus,
**10.95x** Python's rate. Internally prepared native-Lxml candidates reduce that
to **4.287% versus Python's 3.357% (1.28x)**. This compares worker configurations,
not the unchanged Mozilla library default.

Go and Rust's aggregate external-fallback rate is within **0.931 percentage
points** of Python (61 pages). This is comparable frequency, not identical
behavior or complete extraction parity: Go and Rust use DomDistiller, not
jusText. Counts identify the final returned source, not engine calls or accuracy;
recall and internal baseline recovery are excluded from the external total.

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
native outputs and snippet precision/recall/F1/accuracy. Python core checks can
use raw input or a separately labelled same-DOM diagnostic.

The same example also supports `benchmark --jsonl --focus balanced` for the
sibling content-extractor benchmark's paired page runner. It reads original
files, decodes/parses, extracts and flushes one response per request. Add
`--native-output` for complete native-output differential checks, not speed
measurement.