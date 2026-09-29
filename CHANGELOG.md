# Changelog

## 2.2.6 - 2026-09-29

- Permit only internally generated bundled readability-lxml when fallback is
  enabled. Remove Mozilla, DomDistiller and supplied/custom fallback execution,
  including `external::distiller_rescue`. Native recall and baseline remain.
- Keep legacy candidate fields and enum variants source-compatible but ignored;
  the default selector is now `ReadabilityLxml`. Fallback remains off by default.
- Add `parse_html_with_scripting` and `parse_shared_html_with_scripting`, backed
  by published Readability 0.6.5. Reader extraction opts out of scripting so
  noscript markup is parsed as children. Existing shared-parser defaults and
  standalone Mozilla extraction/image recovery are unchanged.
- Make the packaged rustHTML worker permanently FAST, with extensive date
  extraction retained. No standalone candidates are accepted by Trafilatura.
- Refresh the independent current-Go extraction/fallback oracle while preserving
  historical Go and Python references and all 1,152 fallback matrix inputs.
- Validate 55 active library tests, five worker tests, documentation and strict
  all-target/all-feature Clippy on Windows/GNU Rust 1.98.1, with both allocators.
  Fresh quality, timing and fallback-selection rates will be linked from the
  [shared benchmark](https://github.com/markusmobius/content-extractor-benchmark).

## 2.2.5 - 2026-09-28

- Add native `ReadabilityFallback::ReadabilityLxml`, following Python
  Trafilatura 2.2.0's bundled readability-lxml without a Python runtime.
- Preserve the library's default Mozilla mode and explicit caller candidates.
  Candidate ordering, acceptance, DomDistiller preparation and cleanup remain
  unchanged. The packaged Trafilatura-only `rustHTML` worker explicitly selects
  Lxml; applications should not reuse incompatible standalone candidates.
- Retain lazy fallback-input preparation and pin Readability 0.6.4 for prepared
  retry reuse and exact cached score updates. Optional `lab-profile` diagnostics
  compile out of normal builds. No result cache or concurrency is introduced.
- Match all 6,541 exact-input Python candidate trees. The corrected Go/Rust
  application lab matches complete outputs on all 6,554 pages, with 4.29%
  external fallback versus Python's 3.36%; these are selection, not error rates.
- Keep existing Go reference assertions and add the short-article/legal-footer
  worker regression. DomDistiller is still not jusText; full Python parity and
  a 2x speedup are not claimed. Fresh annotated quality and performance results
  are recorded in the [shared benchmark](https://github.com/markusmobius/content-extractor-benchmark).

## crates.io Publication - 2026-09-23

- Publish `rust-trafilatura` 2.2.4 from the now-public repository. The registry
  archive uses crates.io dependencies throughout and excludes Python caches.
- Keep runtime sources, dependency versions, release tags and benchmark results
  unchanged. Git source builds retain their existing pinned dependencies.

## Documentation - 2026-09-23

- Refresh README quality and six-engine speed comparisons from the published
  [benchmark JSON](https://github.com/markusmobius/content-extractor-benchmark/blob/d433ab637f0a56c0926aa3698f470794a553472f/go_rust_shared_performance_2026_09_23.json),
  with separate metadata scores and exact provenance in [UPSTREAM.md](UPSTREAM.md).
- Trafilatura text F1 remains 90.88412% / 96.15663% / 78.49352% on LegoNews /
  ScrapingHub / WCXB. Selected Go/Rust extraction is 6.815 / 4.000 ms/page
  (1.70x); all-four means are 6.839 / 4.026 ms/page. Shared parsing is separate.
- Retain the two observed Go/Rust metadata differences; do not infer full
  metadata equality from matching text scores. No release tag is moved.

## 2.2.4 - 2026-09-23

- Build the shared immutable input directly in the final document arena,
  preserving element namespaces, ordered attributes and reachable preorder.
- Pin released Rust-Readability 0.6.3 for eager tokenizer fast paths. Other
  dependency versions, reader decoding and extraction algorithms are unchanged.
- Keep DOM construction, compaction and parser cleanup inside parsing; no
  lazy work, omitted content, output caching or internal parallelism is added.
- Preserve public APIs, shared-input independence across all six extraction
  orders, and the existing `rustHTML` worker protocol.
- Independently compare complete decoded sources and DOM values against the
  prior released suite on all 2,659 development pages, with matching benchmark
  outputs for all three engines. Existing Go/Python expectations are unchanged.

Version 2.2.4 is available as both a public GitHub source release and a crates.io
package. Parsing gains do not imply that each extraction stage becomes faster;
the coordinated suite is measured separately.