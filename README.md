# Rust-Trafilatura

Rust-Trafilatura extracts main text, comments and metadata from supplied HTML
while preserving useful formatting and document structure. It is a native Rust
port of [Go-Trafilatura](https://github.com/markusmobius/go-trafilatura), which
follows Adrien Barbaresi's Python [Trafilatura](https://github.com/adbar/trafilatura).

The extraction target is Go-Trafilatura 2.2.6: Python Trafilatura 2.2.0's
non-fallback core, the Go port's deliberate differences and bundled readability-lxml.
Identical output on every possible input is not guaranteed. [CHANGELOG.md](CHANGELOG.md)
records releases; [UPSTREAM.md](UPSTREAM.md) explains compatibility and evidence.

The current release is **2.2.7**, available on
[crates.io](https://crates.io/crates/rust-trafilatura/2.2.7) and
[GitHub](https://github.com/markusmobius/rust-trafilatura/releases/tag/v2.2.7).
This documentation-only patch keeps 2.2.6's runtime source and dependency pins.

## Table of Contents

- [Philosophy and Scope](#philosophy-and-scope)
- [Extraction Choices](#extraction-choices)
- [Language Detection](#language-detection)
- [Usage as a Rust Package](#usage-as-a-rust-package)
- [Native Readability-Lxml](#native-readability-lxml)
- [Usage as a Protocol Worker](#usage-as-a-protocol-worker)
- [Dependencies](#dependencies)
- [Current Quality and Speed](#current-quality-and-speed)
- [Non-FAST Trafilatura](#non-fast-trafilatura)
- [Development](#development)

## Philosophy and Scope

**Bring your own HTML.** Use `extract` with a reader or extract from an existing
DOM. The original URL supplies context for metadata and relative links; it does
not cause a page download. Caller-owned input is preserved.

The library extracts content, comments and metadata, including JSON-LD and dates.
It supports tables, images, links, formatting and recovery from embedded content.
Results contain owned HTML trees, plain text and metadata. Returned HTML is not
a security sanitizer; sanitize separately before displaying untrusted content.

Extraction is native Rust. There is no runtime Go/Python bridge, browser, page
fetcher or model download. Callers control acquisition and concurrency. Python's
crawler, output-format suite and process-global cache APIs are outside this port.

## Extraction Choices

- **Python-aligned core through Go.** Follow Go-Trafilatura's pinned Python 2.2.0
  non-fallback behavior and documented differences, rather than website exceptions.
- **One internal fallback implementation.** `enable_fallback: true` permits only
  bundled readability-lxml. Supplied candidates, Mozilla Readability and DomDistiller
  are not used by Trafilatura. Fallback is off by default; native recall and baseline
  recovery remain available in FAST mode.
- **Complete cleanup and accurate recovery decisions.** Remove all matching
  unwanted elements and measure the final cleaned body for recovery decisions,
  following Go's deliberate corrections to Python behavior.
- **Whitespace-tolerant author selection.** Preserve Go's normalized selector
  IDs/classes and single-word metadata authors without broadening content selectors.
- **Native parsing and dates.** Parser repairs, rendering and date interpretation
  can differ from Python even when extraction rules agree. Detailed boundaries
  and known differences are recorded in [UPSTREAM.md](UPSTREAM.md).

## Language Detection

Rust-py3langid provides automatic language metadata using the py3langid model.
The embedded model is initialized lazily, without a download. Classification
uses the longer of body and comments by Unicode character count; comments win ties.

A target language rejects a mismatching or empty label. Without a target,
misclassification affects metadata but does not discard the article. Short,
noisy or multilingual text can be mislabeled. This follows Go's automatic
annotation rather than Python's target-only classification.

## Usage as a Rust Package

Use Rust 1.98.1 or newer and a native C toolchain. No sibling checkouts are needed.

```toml
[dependencies]
rust-trafilatura = "=2.2.7"
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

`extract` detects or accepts a charset, decompresses gzip, normalizes Unicode,
removes soft hyphens and parses HTML. `Options` controls focus, comments, tables,
links, images, per-call deduplication, language, author exclusions, CSS pruning,
output limits and dates. Comments and tables are on by default; links, images
and external fallback are off. `Config` retains Go's thresholds.

### Parsed Input

Use `extract_document`, `extract_node` or `extract_shared_document` to avoid
repeating parsing. These APIs preserve the supplied tree and do not decode or
normalize it again. `SharedDocument` can also supply input to standalone
Readability and DomDistiller; each extractor keeps its own working copies.

For Trafilatura input, use `parse_html_with_scripting(source, false)` or
`parse_shared_html_with_scripting(source, false)`. Like reader extraction,
these parse noscript markup as child nodes. The older `parse_html` and
`parse_shared_html` APIs keep scripting enabled; `parse_shared_bytes` adds
decoding for in-memory bytes. Retain a separate default-parsed input for
standalone Mozilla Readability's noscript image recovery.

## Native Readability-Lxml

We removed caller-supplied fallback candidates because they can bypass
Trafilatura's input cleanup. A standalone result may retain long boilerplate,
such as a legal footer, that passes the fallback length checks and replaces
the article.

In a controlled comparison using Go 2.2.2 on 6,554 pages, supplied candidates
raised final external fallback from **780 pages (11.90%)** to **2,408 (36.74%)**.
DomDistiller accounted for most of the increase: **4 to 1,614 selections**.
Trafilatura now prepares its own candidates and uses only bundled readability-lxml.
Standalone Mozilla Readability and DomDistiller remain separate extractors.
See [UPSTREAM.md](UPSTREAM.md#why-supplied-candidates-were-removed) for the controls
and the distinction between candidate reuse and fallback algorithm choice.

Fallback is off by default. Enable it with `Options { enable_fallback: true,
..Default::default() }`. Legacy `readability_fallback` and `fallback_candidates`
fields remain source-compatible but are ignored. The Apache-2.0 fallback follows
Python Trafilatura 2.2.0's bundled readability-lxml; the separate Readability
dependency supplies parsing/shared-input APIs, not this fallback algorithm.

## Usage as a Protocol Worker

Install from crates.io with `cargo install rust-trafilatura --version 2.2.7 --locked --bin rustHTML`,
or build a source checkout with `cargo build --locked --release --bin rustHTML`.

This executable implements a Trafilatura-only file/TCP protocol, not a page
downloader or a multi-extractor application. It always uses FAST. Standalone
Readability, DomDistiller, raw-metadata and date requests are unsupported here.
See [the protocol reference](UPSTREAM.md#protocol-worker) for framing, payloads
and error behavior.

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

The [2026-09-29 shared benchmark](https://github.com/markusmobius/content-extractor-benchmark/blob/ec719092d12f4d2a438dd29d9f4405aab6e0a321/README.md#results-2026-09-29)
compares all six implementations on **2,659 saved pages**: 983 LegoNews,
181 ScrapingHub and 1,495 WCXB. All six READMEs use this same comparison.

### Extraction Speed

| Extractor | Go Version | Rust Version | Go ms/page | Rust ms/page | Go/Rust |
| --- | --- | --- | ---: | ---: | ---: |
| Readability | 0.6.0 | 0.6.5 | 4.755 | 3.945 | 1.21x |
| DomDistiller | 1.0.0 | 1.0.1 | 6.159 | 3.400 | 1.81x |
| Trafilatura FAST | 2.2.6 | 2.2.6 | 11.329 | 6.570 | 1.72x |

Times are means of **all four measured passes after one warmup**. Go/Rust is
Go time divided by Rust time, not an old/new release speedup. Measured versions
are shown explicitly; later documentation-only releases are not new measurements.

The run used Windows 11, Ryzen AI 7 PRO 350, Go 1.27.1 and Rust 1.98.1 GNU
with ThinLTO/mimalloc. Extraction includes required working copies, metadata
and text rendering. File I/O, startup, IPC, response serialization and scoring
are excluded. Comments, pagination and Trafilatura external fallback are off;
tables are on. Power and sleep checks passed.

Parsing is separate: **Go 11.283 / Rust 6.386 ms/page**, charged once per
language/page for the shared suite. It includes decoding, DOM construction and
the separate Trafilatura noscript tree when needed. These are extraction-stage
comparisons, not complete request latencies.

### Text Quality

Go and Rust have the same text scores for each engine. Errors are listed in
LegoNews / ScrapingHub / WCXB order and remain in the scoring denominators.

| Extractor | LegoNews F1 | ScrapingHub F1 | WCXB F1 | Errors |
| --- | ---: | ---: | ---: | --- |
| Readability | 87.82711% | 95.20557% | 78.47603% | 7 / 0 / 28 |
| DomDistiller | 86.74080% | 92.74280% | 74.39696% | 0 / 0 / 0 |
| Trafilatura FAST | 90.91534% | 96.15663% | 78.51703% | 4 / 0 / 10 |

The corpora use different scoring rules; their F1 scores must not be averaged.
Equal text scores do not imply identical metadata: Trafilatura differs on one
title and one author field. The [full report](https://github.com/markusmobius/content-extractor-benchmark/blob/49c426d6135df81b7d492bea7e6aec8e6d77d80c/go_rust_shared_performance_2026_09_29.json)
contains metadata scores, differences, every pass and source/build identities.

## Non-FAST Trafilatura

A [separate run](https://github.com/markusmobius/content-extractor-benchmark/blob/49c426d6135df81b7d492bea7e6aec8e6d77d80c/go_rust_lxml_performance_2026_09_29.json)
measured both 2.2.6 ports with bundled readability-lxml enabled, using the same
corpora and four-pass protocol. Its measurements are not pooled with FAST.

| Mode | Go ms/page | Rust ms/page | Go/Rust |
| --- | ---: | ---: | ---: |
| Non-FAST lxml | 24.745 | 10.910 | 2.27x |

Parsing was Go 11.518 / Rust 6.545 ms/page. Both ports scored **91.13924% /
95.98168% / 79.56922% F1**, with **4 / 0 / 9 errors** in corpus order.
WCXB F1 is below the older 81.17181% configuration that also allowed DomDistiller
rescue; FAST has one extra LegoNews rejection. The simpler fallback policy is
not a claim of universally better quality. Python jusText is not implemented.

### Fallback Selection Rates

On a separate **6,554-page unannotated corpus**, the final returned sources
were as follows in both ports. These are selection rates, not accuracy scores;
failures stay in the denominator. Standalone extractors are outside this count.

| Final Content Source | FAST | Non-FAST |
| --- | ---: | ---: |
| Native core | 5,726 | 5,604 |
| Native recall | 81 | 58 |
| Bundled readability-lxml | 0 | 202 |
| Mozilla / DomDistiller / custom | 0 | 0 |
| Internal baseline | 726 | 669 |
| No result | 21 | 21 |
| **External fallback total** | **0 / 6,554 (0%)** | **202 / 6,554 (3.082%)** |

In non-FAST, lxml ran on all inputs but supplied final content on only 202;
calls and temporary selections are not the final fallback rate. Native recall
and baseline are internal recovery, not external fallback. See
[UPSTREAM.md](UPSTREAM.md) for the evidence and limits.

## Development

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