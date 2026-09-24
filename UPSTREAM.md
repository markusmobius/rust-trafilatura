# Upstream and Port Ledger

## Released Suite Benchmark

The [2026-09-23 JSON](https://github.com/markusmobius/content-extractor-benchmark/blob/d433ab637f0a56c0926aa3698f470794a553472f/go_rust_shared_performance_2026_09_23.json)
is authoritative for the current [README tables](README.md#current-quality-and-speed).
Its published-file SHA-256 (LF line endings) is `7d7be9839f1652606cb91850af5134b188f2508be25623df889372dab4a06cc6`.
Read text scores at `quality[worker][engine].evaluations[corpus].overall.f1`,
selected timings at `overall`, and all-four timings at `all_passes`.

Rust pins are Readability 0.6.3 (`52ec5ae744fb132e011ad9153ad3071e1227bdeb`),
DomDistiller 1.0.1 (`e95bff0cea7f7b9639abe04a8531b220b3ee4a6e`) and Trafilatura
2.2.4 (`fd57552f181c59fbb0b232250529ef68e967181b`). Go stays at Readability
0.6.0, DomDistiller 1.0.0 and Trafilatura 2.2.2; full commits and unchanged
dependency graphs are in the embedded build receipts. Rust-Trafilatura is now
public; the recorded source commits remain available.

| Implementation | Author Sets Exact / 1,290 | Author-Unit F1 | Titles Exact / 2,364 | Dates Exact / 1,530 |
| --- | ---: | ---: | ---: | ---: |
| go-readabilityV2-0.6.0 | 640 | 56.38767% | 1,247 | 763 |
| rust-readability-0.6.3 | 640 | 56.38767% | 1,247 | 763 |
| go-domdistiller-1.0.0 | 0 | 0.00000% | 1,106 | 0 |
| rust-domdistiller-1.0.1 | 0 | 0.00000% | 1,106 | 0 |
| go-trafilatura-2.2.2 | 695 | 58.80923% | 1,228 | 1,227 |
| rust-trafilatura-2.2.4 | 696 | 58.86640% | 1,227 | 1,227 |

Metadata uses only nonempty supplied annotations; unannotated is not negative,
and missing output is not filled by another engine. All six scored-output
digests match the preceding September 22 report. Trafilatura differs between
Go/Rust only on the title of `legonews/klaenge-des-verschweigens.de.geschichte.html`
and the author of `legonews/golf.de-augusta.html`; extracted text matches.

The full 2,659-page development run used seed 20260922, one warmup and four
measured passes. Passes 1 and 3 were selected by combined extraction time for
every row (5,318 observations each); all-four means retain 10,636 observations.
Worker order is balanced per page; three-engine order is a partial six-pass
block. Go uses `GOMAXPROCS=1`, `GOGC=100`, without forced collection. Native
timers exclude file reads and IPC; parsing and extraction stay separate.
The 26,590-response audit passed with no recorded sleep and AC power throughout.
This compares released suites, not isolated parser changes or unseen holdout
quality. It does not establish extraction-time neutrality versus older Rust.
Historical standalone results and independent oracle fixtures below are unchanged.

## Authority

The Rust behavior target is released Go-Trafilatura **2.2.2**, commit
`f4684e100869274311107325e3b72e47cc78db20`. The independent port oracle retains
its pre-release `ed2b4c86a5727110178172cb18080efe98fdcdb2` base plus exported
source hashes in [testdata/go-worktree.json](testdata/go-worktree.json).
Those fingerprints and the saved Go commit below remain historical evidence;
fixtures were not regenerated or relabeled as release-generated expectations.

Go tracks Python Trafilatura **2.2.0**, immutable commit
`c1bc9531a2a978326112ca9987e1382745116136`, for **non-fallback** extraction
while explicitly retaining selected Go behavior. Rust follows those decisions,
not a newly regenerated Python expectation. Python comparisons use `fast=True`,
native fallbacks disabled and formatting enabled. Repairs are general algorithm
changes, not filename, phrase or benchmark exceptions.

The **Go fallback pipeline is retained**: ReadabilityV2 0.6.0, DomDistiller,
custom candidates, ordering, lazy stopping, acceptance, sanitization and recall
rescue. No jusText or Python bundled-Readability port is substituted. Changes to
the shared main extractor can change fallback-enabled results without changing
the fallback algorithms.

## Pins

| Component | Pin |
| --- | --- |
| Saved Go-Trafilatura | `72dce36bfe95502563533cf68a9050370a3d7081` |
| Current Go extraction oracle SHA-256 | `28e45fd8fa76c497f3ab969576dcb134306e03c3535cb6b978f77ffe9493be6e` |
| Go toolchain / Unicode | 1.27.1 / 17.0.0 |
| Go-HtmlDate | 1.10.1, `e4137245789a42de79c4c6ed8029b2cf1255f27c` |
| Go-DateParser | 1.4.7, `1554533a164fdcab763e59bd42fe46c86bb98a74` |
| Go-Dateutil | 2.9.1, `1a29d3cc92f3491373cbfddb3059a00b0fa0a1d6` |
| Go-Py3langid | 0.4.0, `d3e0c0861455d7d84daedb994392d2e71a0f6270` |
| Go-ReadabilityV2 | 0.6.0, `db6ab179f951f80ad176850001aaf486e2fbd367` |
| Go-DomDistiller | `25b8d046ffb4053bf68345d6fa59bc9ae1961ad8` |
| Go HTML parser | `golang.org/x/net v0.59.0` |
| Rust toolchain | 1.98.1, edition 2021 |
| Rust-Readability | 0.6.3, Git `52ec5ae744fb132e011ad9153ad3071e1227bdeb` |
| Rust-DomDistiller | 1.0.1, Git `e95bff0cea7f7b9639abe04a8531b220b3ee4a6e` |
| Rust-Py3langid | Registry `rust-py3langid =0.4.0` |
| Rust-HtmlDate | 1.10.2, Git `912ede5196710f23214bb5a831437c35ed5ff5b6` |
| Rust-DateParser | 1.4.7, Git `1e3e2feccd8662113e4092ca87a5567e4e23bc3c` |
| Rust-Dateutil | 2.9.1, Git `3a7537dd3a4e223756fa31fb1941a10da4c78b30` |

The port is native, accepts supplied input, and leaves scheduling to callers.
There is no production interpreter bridge, acquisition subsystem, internal
worker pool or model download. Development references may invoke Go and Python.
Version 2.2.4 is available as a public GitHub source release and a crates.io
package. Publication does not upgrade the parent application automatically.

The source checkout retains the Git pins above; [Cargo.lock](Cargo.lock) records
its exact sources and checksums. Cargo registry packaging resolves the same
version requirements from crates.io, so the published crate has a registry-only
dependency graph. Both distributions contain the same runtime implementation,
including HtmlDate's tree import and Readability's shared-input APIs. Neither
requires private Git access or sibling worktrees. Existing tags are unchanged.

The Python reference uses Python 3.12.13, lxml 6.1.3, py3langid 0.4.0,
NumPy 2.5.2, HtmlDate 1.10.0, dateparser 1.4.2 and dateutil 2.9.0.post0.
All 19 selected reference packages are recorded in the fixture. Native date
dependencies are newer; date differences require separate attribution.

## Independent References

1. [testdata/go-reference.json](testdata/go-reference.json): immutable saved-Go
	helpers, parser inputs and fallback contracts. The generator archives the
	pinned commit with LF endings, verifies 72 selected modules, rejects
	replacements and runs a temporary overlay. It never uses Rust expectations.
2. [testdata/python-reference.json](testdata/python-reference.json): pinned
	Python core behavior. The generator verifies installed source against the
	immutable archive and its dependency pins. Go consumes the identical file.
3. [testdata/go-worktree.json](testdata/go-worktree.json): current Go handlers,
	content/comments, sequences and extraction after shared core repairs. Its
	`current-go-worktree-cross-port` identity includes recursive source hashes.
	It is not evidence of Python correctness on its own.

[tools/go_reference.py](tools/go_reference.py) checks bytes by default;
`--worktree` selects the third oracle and `--write` explicitly regenerates it.
[tools/python_reference.py](tools/python_reference.py) produces independent
Python expectations. Neither tool edits reference implementations or caches.
Environment: `GOWORK=off`, `CGO_ENABLED=0`, `GOTOOLCHAIN=go1.27.1`, `TZ=UTC`.
Date fixtures use absolute inputs and fixed custom bounds.

| Artifact | SHA-256 |
| --- | --- |
| Go go.mod | `b96ae594ce46f503380141a76ce3733cf0342f78ff19ac9e1d4582384e5e851e` |
| Go go.sum | `975f1ec71578a0a9af40cc68a34d2fee0af42b29eefd45bd2a8b6cffcd70f1a5` |
| Frozen Go fixture | `8f2e159ed2c783c128dbe26cf86f529d6381e1bace237f0daa79ff8305caaa95` |
| Generated Go Unicode | `beb04c5dd19a047f16d980a87e89c561435b10bad284b99e17ed467f3f462549` |
| Python core fixture | `5c66c000f1e58dea7a870c3f4d02d41212bca71fff5fea088240ec5c12fab559` |
| Go-Trafilatura license | `c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4` |
| Go standard-library license | `911f8f5782931320f5b8d1160a76365b83aea6447ee6c04fa6d5591467db9dad` |

## Implementation

The library implements reader decoding/gzip, owned DOM extraction, metadata,
OpenGraph/JSON-LD, dates, language filtering, CSS pruning, per-call deduplication,
main content/comments, lists, quotes, code, images, tables, baseline recovery,
recall escalation and native fallbacks. See [README.md](README.md) for APIs.
Caller trees and custom candidates are preserved. Date/fallback adapters import
nodes without serializing a caller tree into another parser. The embedded
language model initializes lazily.

Python-directed repairs include raw body/comment selector attributes,
whole-line filters, forum patterns, comment/code tails,
heading order, live list/paragraph/quote/cell iteration, initial-empty-node
pruning, ordered cleaning, original-document ownership after detachment,
caption whitespace and absent versus empty text slots. Extracted `wbr` text
uses a valid HTML `span`. Core preparation and fallback sanitation are separated
so these changes do not silently replace the retained Go fallback behavior.

Current-Go restorations collect all unwanted cleaning matches before removal,
measure decision text after duplicate/`done` cleanup and `div` unwrapping,
classify every accepted extraction (comments win equal-length ties), and retain
Go's metadata ID/class whitespace accessors without new case folding. Leading
BOM text is preserved through the parser, including Go's resulting head/body
placement. The frozen Python fixture documents the intentional differences;
its expected values were not rewritten to fit Rust.

The reader decoder implementation and tables are copied byte-for-byte from
published Readability 0.6.0,
archive SHA-256
`711ea12177c6cecbdffff6c679ab984e0f957ad2374a3d44e83d397f41b6ce7f`.
[tools/import_readability_encoding.py](tools/import_readability_encoding.py)
verifies those two sources and their license, without overwriting the locally
optimized host module [src/encoding.rs](src/encoding.rs). That module preserves
the complete detector as a reference: it stops early only when first-priority
UTF-8 has the maximum confidence, and bypasses normalization only after a
positive stream-safe NFC quick check with no soft hyphen. Differential tests
cover 1,792 detector inputs, every valid Unicode scalar and combining-mark
boundary sequences. Imported/generated modules are excluded from formatting,
not compilation or testing.

Performance changes retain the same extraction decisions: readers consume their
private parsed tree while borrowed-DOM APIs still copy it; parser conversion
uses the local Readability output sink to avoid an intermediate arena; fallback
conversion uses owned attributes and direct string copies. Selectors traverse in document
order and stop early only for first-match queries. Metadata can borrow already
normalized class strings and read author candidates without cloning the page.
Ordered cleaning collects matching tags once and rechecks reachability before
each tag's removals. Native fallbacks and language/date work are not bypassed.

The 2.2.4 shared parser writes directly into the final DomDistiller document
arena and compacts reachable nodes into preorder while moving the matching
namespace sidecar. It preserves every node, ordered attribute and namespace.
Readability 0.6.3 integrates a private html5ever 0.39.0 tokenizer with bulk ASCII
name copying and common-delimiter scanning; its released tree builder and
exceptional-input handling remain unchanged. Decoder and normalization work,
finalization, decoded-source destruction and temporary parser cleanup all stay
inside parsing. Extraction algorithms and timer boundaries are unchanged.

The worker and benchmark executables select `mimalloc` 0.1.48 by default
(`libmimalloc-sys` 0.1.49 in the lockfile) and release builds use ThinLTO.
`--no-default-features` selects the platform allocator. The library does not
declare a global allocator. No CPU-specific target flags, internal extraction
pool, cross-request result cache or runtime bridge is used. Temporary profiling
hooks are absent from production source.

## Verification Scope

[src/upstream_tests.rs](src/upstream_tests.rs) includes these matrices. Counts
are input/option combinations, not unique pages or independent defects.

| Area | Coverage |
| --- | --- |
| Text and Unicode | 3,517 helper inputs; 7,820 normalization inputs; 25,396 title contexts; full emoji dictionary in eight contexts |
| DOM, URLs and metadata | 144 DOM cases; 288 URLs; 90 metadata surfaces; 576 date combinations; 60 Python metadata-attribute cases |
| Selectors and pruning | More than 10,000 Python signals; 288 same-DOM pruning cases; 568 CSS and 3,470 Go-regex cases |
| Cleaning and handlers | 192 preparation, 480 conversion, 564 link-density, 736 text-node, 672 handler and more than 2,000 span inputs |
| Extraction | 2,112 content/comment combinations; 4,992 sequence slots; 6,177 public-option slots; 27 Python snapshot and 162 Python edge cases |
| Language and forums | 324 Python classifier, 36 public language, 30 forum cases |
| Retained fallbacks | 900 decision, 576 sanitization, 1,152 native-pipeline combinations |

Current-Go content, sequence and public-extraction matrices assert exact native
HTML, metadata, text, errors and caller preservation for all 13,281 cases.
Python slots for excluded fallback/native-only APIs are absent, not counted as
passes. Python selected-text checks and the explicit retained-Go expectations
remain separate. Local Windows/GNU release qualification on 2026-09-19 passed
43 library, four worker and two adapter tests, plus strict all-target Clippy.

The real-worker differential tool builds unmodified parent application sources
in an isolated Go module using current Go-Trafilatura. It checks 25 persistent
file requests, path precedence/XOR, wrong/duplicate/null JSON fields, deep/large
ignored values, Unicode escapes, output-path failure, six partial/back-to-back
TCP requests, case-insensitive persistent envelopes and clean disconnect.
The parent application's graph stays unchanged.

The Go reviewed checker reports 7,363 passing leaves, eight retained
Python-fallback differences, 42 skips and 56 separately reported mappings.
Ordinary `go test` still reports the eight failures; the checker requires exactly
those names and assertion counts.

The 983-page corpus is pinned to commit
`466fdbee8a504441eb78ed11d71c1da220681cab`, SHA-256
`0e8b21bc8c28a88a90d891d91020a21aab95a2cfa83f761dd2b1642d98c757ea`.
It has 2,935 positive and 2,948 negative case-sensitive snippet labels, retaining
duplicates and scoring rejected pages as empty.
[tools/benchmark.py](tools/benchmark.py) retains outputs, errors, sample order,
all timings and compiler/profile/dependency/model/source/binary fingerprints.
It refuses timing when native results differ. Timing excludes initial parsing
and IPC but includes extraction, metadata, configured fallbacks, result lifetime
and scoring. Raw Python and same-DOM checks are separately labelled.

The 2026-09-19 original-file differential also covers the sibling benchmark's
983 LegoNews, 181 ScrapingHub and 1,495 WCXB development pages. Complete native
outputs match current Go on all 2,659 pages: body/comment text, selected text,
native HTML, full metadata and errors. Registry SHA-256 is
`1bba1a1aa61107bec368021baf3d6c3aafa8f76c7c9e135657d61a10478f0a20`;
selected-input SHA-256 is
`6d77f85403e2fa9e84de63d909d896c2d1bfad0b73274f925df177a04293a078`.
Options: balanced, native fallbacks off, comments off, tables on, automatic
language, default dates. The ignored local evidence is under
`target/go-v222-20260919-aligned/`, including build identities and raw outputs.
It must accompany any public claim. Fallback behavior is separately checked by
the matrices above; this corpus comparison does not qualify fallback speed.

## Historical Measured Speed

The 2026-09-20 traversal build was compared with current Go and the preceding
input-optimized Rust build in persistent processes. Each original page ran
serially through all three engines in seeded randomized order before advancing:
one complete warmup followed by **N=1**, 2,659 timed observations per engine.
N=1 does not balance every page's order across repeated passes. All observations
are retained; means are arithmetic, with the overall mean weighted by pages.

Options match the original-file differential above. Request timing includes
IPC, file reads, decoding/parsing, extraction, dates/language and serialization;
startup, warmup, scoring and validation are excluded. This is neither the older
DOM-only benchmark nor fallback-enabled worker timing. No memory run was made.

| Corpus | Pages | Go ms/page | Previous Rust ms/page | Traversal Rust ms/page | Go / Rust |
| --- | ---: | ---: | ---: | ---: | ---: |
| LegoNews | 983 | 10.602568 | 7.107710 | 6.837497 | 1.551x |
| ScrapingHub | 181 | 10.138694 | 6.018255 | 5.797730 | 1.749x |
| WCXB development | 1,495 | 17.290812 | 13.433290 | 12.351938 | 1.400x |
| All selected pages | 2,659 | 14.331400 | 10.590053 | 9.867166 | 1.452x |

The traversal changes improved Rust throughput by 1.073x in this run. The
requested **2-3x Go throughput is not yet achieved**. Corpus scores remain
separate and identical across engines: F1 90.88412%, 96.15663%, 78.49352%; errors
3, 0, 10 respectively. Full native output parity was checked independently,
not inferred from equal scores or within-engine stability.

The raw audit verified selected input IDs, seeded page order, one observation
per engine/page/phase, persistent PIDs, stable matching prediction digests,
artifact hashes and arithmetic means. Timed process CPU totals were 37.109375s
for Go, 28.281250s for previous Rust and 25.687500s for traversal Rust. Host:
Windows 11, Ryzen AI 7 PRO 350, AC power, Balanced plan, default affinity;
Go 1.27.1 with `GOMAXPROCS=1`, Rust 1.98.1 Windows/GNU, ThinLTO and executable
mimalloc. Temporary system/display idle prevention was restored after the run.

Local evidence: sibling benchmark
`results/go-v222-vs-rust-traversal-page-speed-2026-09-20/` and this crate's
`target/go-v222-20260920-traversal/` contain raw responses, timings, CPU audit,
source snapshots and exact compiler/dependency/model/binary fingerprints.
These ignored artifacts must accompany any public performance claim. Do not
pool this run with earlier profiles or comparisons. The first aligned timing
run was host-disturbed and is explicitly disqualified in its `TIMING-CAVEAT.md`;
its apparent ratio is not evidence for a speed claim.

## Boundaries

Zero corpus differences do not prove arbitrary-input equivalence. Parsing,
selected content, serialization, metadata/date dependencies and acceptance are
separate comparisons. Native plain rendering is retained; Python
XML/TEI/Markdown/CSV/YAML and Python wrapper objects are outside the port.

The same-DOM diagnostic explicitly removes comments to match Python's ordinary
parser preprocessing. Four corpus trees cannot be imported by lxml: two have
XML-invalid controls and two invalid tag names. These are reference exclusions,
not successful parity checks. Go/Rust still process all 983 inputs. Malformed,
namespaced and custom DOMs and Unicode-version differences remain general
compatibility risks, not permission for corpus-specific repairs.

`parse_html` uses ReadabilityV2's parser; the re-exported `Document::parse` uses
DomDistiller's older parser. The extraction arena omits element namespaces but
retains ordered attribute namespace/key/value data. Direct edits require valid
acyclic indices and consistent topology. Extraction is not security sanitization.

Per-call Go deduplication remains; Python process-global caches, mutable module
settings, acquisition and configuration loaders are not exported Rust APIs.
Logging is opt-in stderr diagnostics, not byte-identical Go console timestamps.
Worker JSON key ordering is not guaranteed; values, arrays, nulls, escaping and
marker indices are checked. Defensive I/O errors replace unsafe short-read
behavior. Standalone worker processors other than Trafilatura are unsupported.

Windows/GNU and WSL Linux are the local validation targets. macOS/ARM64 runtime
execution and hosted CI are not established by local checks. Go v2.2.2 and
Rust v2.2.4 are GitHub source releases; Rust-Trafilatura 2.2.4 is also available
on crates.io from the public repository.

## Attribution

Go-Trafilatura and its inputs originate with Markus Mobius; upstream Trafilatura
work is by Adrien Barbaresi and contributors. The Apache-2.0 [LICENSE](LICENSE)
is retained. Generated Go data uses [LICENSE-GO.txt](LICENSE-GO.txt). Imported
decoder code retains [LICENSE-READABILITY-ENCODING](LICENSE-READABILITY-ENCODING).
Dependency source, models and notices remain in their pinned packages. Saved
pages and build products remain development artifacts, not library data.