# rust-trafilatura

`rust-trafilatura` extracts main text, comments and metadata from supplied HTML,
preserving useful formatting and document structure. It is a native Rust port of
[go-trafilatura](https://github.com/markusmobius/go-trafilatura), based on
Adrien Barbaresi's [adbar/trafilatura](https://github.com/adbar/trafilatura).

## Philosophy

Our extractor packages share three principles:

1. **Bring your own HTML.** Keep page acquisition separate from extraction.
   The primary workflow uses HTML supplied by the caller, who controls fetching,
   caching, rendering, retries and scheduling.
2. **Stay close to upstream.** Preserve the algorithms and behavior of each
   package's declared upstream reference as closely as possible. Document
   deliberate differences and compatibility limits in [UPSTREAM.md](UPSTREAM.md)
   rather than claiming exact equivalence on every page.
3. **Provide very fast Go and Rust packages.** Run extraction natively, without
   a Python or Java runtime. Improve throughput and allocation efficiency while
   preserving intended behavior, and substantiate performance with reproducible
   benchmarks that report quality alongside speed.

## Overview

The current `rust-trafilatura` release is **2.2.8**. It accepts HTML readers or
parsed trees and returns owned content/comment trees, plain text and metadata,
including JSON-LD, dates and language. Tables, images, links and per-extraction
deduplication are configurable. Input trees are preserved; no page fetching or
runtime Go/Python bridge is used. Callers own acquisition and concurrency.

The behavioral reference is `go-trafilatura` 2.2.6: `adbar/trafilatura` 2.2.0's
non-fallback core, documented Go differences and optional bundled readability-lxml.
This documentation-only release preserves the preceding release's runtime
source and dependency pins.

## Installation

```sh
cargo add rust-trafilatura@=2.2.8
```

Use Rust 1.98.1 or newer and a native C build toolchain. Import the crate as
`rust_trafilatura`. No sibling checkouts are needed for registry installation.
See [Cargo.toml](Cargo.toml) for dependencies and [CHANGELOG.md](CHANGELOG.md)
for release changes.

## Usage

Extract text from HTML already held in memory:

```rust
use rust_trafilatura::{extract, HtmlDateMode, Options, Url};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"<html><head><title>Research results</title></head><body><article>
<h1>Research results</h1>
<p>The research team compared several methods for extracting articles from saved
web pages. Every method received the same original HTML, and the evaluation
kept the reference text separate from the input supplied to each extractor.</p>
<p>The report records the complete experiment, including errors and repeated
measurements. Its results describe this collection of pages and do not promise
the same quality or execution time for every website.</p>
</article></body></html>"#;
    let options = Options {
        original_url: Url::request("https://example.org/research"),
        exclude_comments: true,
        html_date_mode: HtmlDateMode::Disabled,
        ..Options::default()
    };
    let result = extract(source.as_bytes(), &options)?;

    println!("{}", result.content_text);
    Ok(())
}
```

| Entry Point | Input |
| --- | --- |
| `extract` | Reader bytes, with decoding, normalization and HTML parsing |
| `extract_document` | An existing parsed document |
| `extract_node` | A selected subtree |
| `extract_shared_document` | A shared parsed document |

Use `content_text` for text and `content_node` for the owned output tree;
`metadata` contains title, author, date and language. Comment output is separate.
See the [rust-trafilatura API reference](https://docs.rs/rust-trafilatura/2.2.8/rust_trafilatura/)
for complete signatures and result types.

### Protocol Worker

The optional executable accepts saved-HTML file/TCP requests, not page downloads:

```sh
cargo install rust-trafilatura --version 2.2.8 --locked --bin rustHTML
```

It implements only `rust-trafilatura` extraction and always uses FAST. It is
not a multi-extractor application. Framing, supported fields and errors are
documented in the [protocol reference](UPSTREAM.md#protocol-worker).

## Options

Start with `Options::default()`:

| Option | Default | Effect |
| --- | --- | --- |
| `original_url` | Unset | URL context for metadata and relative links; no download. |
| `input_encoding` | Automatic | A known encoding label skips statistical charset detection. |
| `enable_fallback` | `false` | FAST mode; `true` permits the bundled readability-lxml candidate. |
| `focus` | `Balanced` | Balance precision and recall, or favor either explicitly. |
| `exclude_comments` / `exclude_tables` | `false` | Keep comments and tables unless explicitly excluded. |
| `include_images` / `include_links` | `false` | Opt into images and links in extracted HTML. |
| `target_language` | Unset | Reject a mismatching or empty language label when a target is supplied. |
| `html_date_mode` | `Default` | Choose `Fast`, `Extensive` or `Disabled`, or follow the fallback setting. |

### Input, Language and Allocation

`input_encoding` describes the bytes actually supplied. Parsed-document APIs
do not repeat decoding or normalization. To match reader parsing, use
`parse_html_with_scripting(source, false)` or
`parse_shared_html_with_scripting(source, false)` so noscript markup becomes
child nodes. Older parser APIs keep scripting enabled; that flag controls
tree construction, not JavaScript execution. Caller-owned trees are preserved.

`rust-py3langid` supplies automatic language metadata using an embedded model,
initialized lazily without a download. It classifies the longer of body/comments;
comments win ties. Without a target, a wrong label affects metadata rather than
discarding the article. Short, noisy or multilingual input can be mislabeled.

The `mimalloc` feature controls the worker and benchmark executables' allocator.
Use `--no-default-features` for their platform allocator. The library installs
no global allocator; embedding applications retain control. Registry packages
use registry dependencies, while the source checkout retains its pinned Git
references in [Cargo.lock](Cargo.lock).

### Optional Fallback

FAST disables external fallback, not native recall or baseline recovery.
Enabling fallback uses only the internally prepared bundled readability-lxml
implementation. Legacy `readability_fallback` and `fallback_candidates` fields
are ignored; they cannot select another extractor.

Caller-supplied candidates were removed because they could bypass input cleanup
and retain long boilerplate that passed length checks. In the controlled
`go-trafilatura` 2.2.2 comparison, supplied candidates raised final external
fallback from **780/6,554 (11.90%)** to **2,408/6,554 (36.74%)**;
`go-domdistiller` selections rose from **4 to 1,614**. This is an input-preparation
issue, not a claim that another extractor performs no cleanup. See the
[controlled evidence](UPSTREAM.md#why-supplied-candidates-were-removed).

A [separate non-FAST run](https://github.com/markusmobius/content-extractor-benchmark/blob/49c426d6135df81b7d492bea7e6aec8e6d77d80c/go_rust_lxml_performance_2026_09_29.json)
measured `go-trafilatura` 2.2.6 at **24.745 ms/page** and `rust-trafilatura` 2.2.6
at **10.910 ms/page** (**2.27x** Go/Rust), with parsing at **11.518 / 6.545 ms/page**.
Both scored **91.13924% / 95.98168% / 79.56922% F1**, with **4 / 0 / 9 errors**
in LegoNews / ScrapingHub / WCXB order. Do not pool these timings with FAST.
The simpler policy is not universally more accurate; its WCXB score is below
the older **81.17181%** configuration that also allowed `go-domdistiller` rescue.

On a separate 6,554-page unannotated corpus, final external fallback supplied
**0/6,554 (0%)** results in FAST and **202/6,554 (3.082%)** in non-FAST. These are
selection rates, not accuracy scores; failures remain in the denominator.
Calls and temporary selections are not final-source counts. The current policy
changes both candidate preparation and the algorithm set, not just reuse.

## Current Quality and Speed

The [2026-09-29 shared benchmark](https://github.com/markusmobius/content-extractor-benchmark/blob/ec719092d12f4d2a438dd29d9f4405aab6e0a321/README.md#results-2026-09-29)
compares the six packages below on **2,659 saved pages**: 983 LegoNews,
181 ScrapingHub and 1,495 WCXB.

### Extraction Speed

| Go Package (Measured Version) | Rust Package (Measured Version) | Go ms/page | Rust ms/page | Go/Rust |
| --- | --- | ---: | ---: | ---: |
| `go-readabilityV2` 0.6.0 | `rust-readability-v2` 0.6.5 | 4.755 | 3.945 | 1.21x |
| `go-domdistiller` 1.0.0 | `rust-domdistiller` 1.0.1 | 6.159 | 3.400 | 1.81x |
| `go-trafilatura` 2.2.6 (FAST) | `rust-trafilatura` 2.2.6 (FAST) | 11.329 | 6.570 | 1.72x |

Times are means of **all four measured passes after one warmup**. Go/Rust is
the named Go package's time divided by the named Rust package's time, not an
old/new release speedup. Later documentation-only releases do not change the
versions actually measured.

The run used Windows 11, Ryzen AI 7 PRO 350, Go 1.27.1 and Rust 1.98.1 GNU
with ThinLTO/mimalloc. Extraction includes required working copies, metadata
and text rendering. File I/O, startup, IPC, response serialization and scoring
are excluded. Comments and pagination are off; tables are on.
`go-trafilatura` and `rust-trafilatura` use FAST with external fallback disabled.
Power and sleep checks passed.

Parsing is separate: **Go 11.283 / Rust 6.386 ms/page**, charged once per
language/page for the shared suite. It includes decoding, DOM construction and
the separate `go-trafilatura` / `rust-trafilatura` noscript tree when needed.
These are extraction-stage comparisons, not complete request latencies.

### Text Quality

Each named pair has equal text scores. Errors are listed in LegoNews /
ScrapingHub / WCXB order and remain in the scoring denominators.

| Go Package | Rust Package | LegoNews F1 | ScrapingHub F1 | WCXB F1 | Errors |
| --- | --- | ---: | ---: | ---: | --- |
| `go-readabilityV2` | `rust-readability-v2` | 87.82711% | 95.20557% | 78.47603% | 7 / 0 / 28 |
| `go-domdistiller` | `rust-domdistiller` | 86.74080% | 92.74280% | 74.39696% | 0 / 0 / 0 |
| `go-trafilatura` (FAST) | `rust-trafilatura` (FAST) | 90.91534% | 96.15663% | 78.51703% | 4 / 0 / 10 |

The corpora use different scoring rules; their F1 scores must not be averaged.
Equal text scores do not imply identical metadata: `go-trafilatura` and
`rust-trafilatura` differ on one title and one author field. The
[full report](https://github.com/markusmobius/content-extractor-benchmark/blob/49c426d6135df81b7d492bea7e6aec8e6d77d80c/go_rust_shared_performance_2026_09_29.json)
contains metadata scores, differences, every pass and source/build identities.

## Compatibility and Limitations

- **Declared reference.** Follow `go-trafilatura` 2.2.6 and its pinned
  `adbar/trafilatura` 2.2.0 non-fallback core. Finite test agreement is not
  universal Python output parity.
- **Deliberate differences.** Complete cleanup, cleaned-body recovery decisions,
  author-selector normalization and automatic language annotation follow the
  Go reference; [UPSTREAM.md](UPSTREAM.md) records their boundaries.
- **Native dependencies.** HTML repairs, rendering and date interpretation can
  differ from Python even when extraction rules agree.
- **Limited scope.** No page downloader, JavaScript engine or computed layout.
  The Python crawler, format suite, global caches and `miso-belica/jusText`
  fallback are not implemented.
- **Not a sanitizer.** Sanitize extracted HTML before displaying untrusted input.

## Development

Use Rust 1.98.1 and `TZ=UTC`. Default tests need no Go or Python after Cargo
dependencies are fetched. Windows/GNU and Linux use separate target directories.

```sh
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo package --locked
```

Expected results come from independent Go/Python executions. Optional reference
and corpus checks require their pinned sources and toolchains; see
[UPSTREAM.md](UPSTREAM.md). Package verification requires a clean release checkout.
Documentation and release requirements are in [AGENTS.md](AGENTS.md).

## License and Credits

`rust-trafilatura` retains [Apache-2.0](LICENSE) and the adapted Go compatibility
code's [BSD-3-Clause terms](LICENSE-GO.txt). Reader adaptations retain
[Radhi Fadlillah's MIT notice](LICENSE-READABILITY-ENCODING). Dependencies and
adapted source files retain their original notices.

Adrien Barbaresi created [adbar/trafilatura](https://github.com/adbar/trafilatura),
the original Python package. Radhi Fadlillah wrote the initial Go port from
Python. Markus Mobius maintains the `go-trafilatura` and `rust-trafilatura` ports.
The Go Authors and Radhi Fadlillah are also credited for the adapted compatibility
and reader code identified above.

The bundled readability-lxml ancestry credits Arc90 for the original algorithm,
starrhorne and iterationlabs for the Ruby port, and gfxmonk for the Python port.
The [pinned upstream notice](https://github.com/adbar/trafilatura/blob/c1bc9531a2a978326112ca9987e1382745116136/trafilatura/readability_lxml.py)
also links the `timbertson/python-readability` and `buriy/python-readability`
contributors. For research citation, see Adrien Barbaresi's
[2021 ACL/IJCNLP paper](https://aclanthology.org/2021.acl-demo.15/),
[2019 KONVENS paper](https://hal.archives-ouvertes.fr/hal-02447264/document) and
[2016 WAC-X paper](https://hal.archives-ouvertes.fr/hal-01371704v2/document).