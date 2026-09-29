# Changelog

## 2.2.8 - 2026-09-29

- Documentation-only release of `rust-trafilatura`; runtime source and
  dependency pins are unchanged from 2.2.7.
- Apply the approved nine-section README format, covering supplied HTML,
  upstream fidelity, native performance, runnable usage and actual options.
- Require named creator acknowledgments in AGENTS.md and explicitly credit
  Adrien Barbaresi, Markus Mobius, Arc90, starrhorne, iterationlabs, gfxmonk,
  the Go Authors and Radhi Fadlillah for their respective contributions.
- Keep optional fallback evidence in this package's Options section and use
  full package names. Benchmark versions and reports remain unchanged; no new run.

## 2.2.7 - 2026-09-29

- Documentation-only release; runtime source and dependency pins are unchanged
  from 2.2.6.
- Align the README with Go-Trafilatura's library-first structure and explain the
  measured reason for removing supplied fallback candidates.
- Use the same September 29 six-engine comparison and measurement conventions
  as the other five library READMEs.
- Include AGENTS.md with instructions for keeping README, UPSTREAM, CHANGELOG,
  release notes and crate documentation consistent.
- Package the revised documentation on crates.io. Benchmark rows retain the
  versions actually measured; this release introduces no new measurements.

## 2.2.6 - 2026-09-29

- Remove caller-supplied fallback candidates. Candidates extracted before
  Trafilatura's input cleanup can retain long boilerplate, such as legal footers,
  that passes fallback length checks and replaces the article.
- In a controlled Go 2.2.2 comparison on 6,554 saved pages, supplying candidates
  raised final external fallback from **780 pages (11.90%)** to **2,408 pages
  (36.74%)**. Most of the increase came from DomDistiller: **4 to 1,614 pages**.
  This motivates generating fallback candidates inside Trafilatura instead.
- When fallback is enabled, use only internally generated bundled
  readability-lxml. Mozilla-style Readability is a different algorithm, and
  DomDistiller is not Python's jusText. The complete 2.2.6 policy selects external
  fallback on **202/6,554 pages (3.08%)**; this is not a candidate-removal-only
  comparison or an accuracy score. FAST has no external fallback. Native recall
  and baseline recovery remain. Legacy candidate fields and selector variants
  are retained for source compatibility but ignored.
- Parse noscript contents as HTML children in reader extraction. Add explicit
  scripting-mode parser APIs using Readability 0.6.5; existing parser defaults
  remain unchanged. See [UPSTREAM.md](UPSTREAM.md) for verification details.

## 2.2.5 - 2026-09-28

- Add native `ReadabilityFallback::ReadabilityLxml`, following Python
  Trafilatura 2.2.0's bundled readability-lxml without a Python runtime.
- Preserve the library's default Mozilla mode and explicit caller candidates.
  Candidate ordering, acceptance, DomDistiller preparation and cleanup remain
  unchanged. Applications can select Lxml explicitly and should not reuse
  incompatible standalone candidates.
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
  orders, and the existing executable protocol.
- Independently compare complete decoded sources and DOM values against the
  prior released suite on all 2,659 development pages, with matching benchmark
  outputs for all three engines. Existing Go/Python expectations are unchanged.

Version 2.2.4 is available as both a public GitHub source release and a crates.io
package. Parsing gains do not imply that each extraction stage becomes faster;
the coordinated suite is measured separately.