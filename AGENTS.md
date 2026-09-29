# Rust-Trafilatura Maintenance

These are instructions for LLM agents and maintainers updating README.md,
UPSTREAM.md, CHANGELOG.md, benchmark claims and releases. Read current files
and git status first; preserve unrelated work and explicit user constraints.

## Product and Scope

This is the native Rust extraction library, with Go-Trafilatura as its behavioral
reference. Use Go-Trafilatura's README structure and plain language when aligning
these ports. Explain purpose, scope, extraction choices and usage before technical
validation details. Keep application-worker history out of library documentation.
Finite tests are evidence of compatibility, not proof of complete Python parity.

FAST disables external fallback, not native recall or baseline recovery.
Non-FAST uses only internally generated bundled readability-lxml. Standalone
Mozilla Readability and DomDistiller remain independent extractors. Removing
their use as candidates does not remove standalone APIs or consumer flags.

## Document Roles

- README.md: purpose, scope, usage, important choices, concise current quality
  and speed. Explain why a behavior changed; avoid oracle hashes and incident logs.
- UPSTREAM.md: exact source/dependency pins, compatibility boundaries, controlled
  experiments, validation commands, hashes, coverage and unresolved differences.
- CHANGELOG.md: dated, versioned user-visible changes and their reasons. It must
  not mention rustHTML. Do not paste benchmark reports or worker deployment history.
- GitHub release notes: the same release-facing changes and benchmark definitions,
  not a separate set of results. Link detailed evidence instead of copying logs.
- AGENTS.md: durable instructions, not a status journal or current results.

## Candidate Removal Explanation

Explain that candidates extracted before Trafilatura's input cleanup can retain
long boilerplate that passes length checks and replaces article content. Do not
say DomDistiller performs no cleaning: the relevant difference is input
preparation. Use the historical same-version control, clearly labeled: Go 2.2.2
selected external fallback on 780/6,554 pages (11.90%) with generated candidates
and 2,408/6,554 (36.74%) with supplied candidates; DomDistiller selections were
4 versus 1,614. The complete 2.2.6 lxml-only policy measured 202/6,554 (3.08%).
That last reduction also changes the algorithm set and is not solely the effect
of candidate removal. Selection rates are not accuracy scores. Count final
returned sources, not calls or temporary selections; keep failures in the
denominator and internal recovery separate. Mozilla Readability is not bundled
readability-lxml, and DomDistiller is not Python's jusText.

## Comparable Benchmarks

The shared contract lives in
[the benchmark maintenance guide](https://github.com/markusmobius/content-extractor-benchmark/blob/master/AGENTS.md).
Apply it consistently to go-domdistiller, rust-domdistiller, go-readabilityV2,
rust-readability, go-trafilatura and rust-trafilatura.

1. Keep `## Current Quality and Speed` in every README. Use the same completed
	shared-suite report, measured versions and table in all six repositories.
2. Table columns are `Extractor`, `Go Version`, `Rust Version`, `Go ms/page`,
	`Rust ms/page`, `Go/Rust`. Rows are Readability, DomDistiller and Trafilatura
	FAST. Show milliseconds/page to three decimals and ratios to two decimals.
3. Derive numbers from structured reports; compute ratios before rounding.
	The current shared protocol uses all four measured passes after one warmup.
	Never mix dates, machines, modes, means/medians or selected/all-pass results.
4. Charge shared parsing once per language/page and report it separately.
	State corpus/counts, environment, options, warmups/passes and timing boundaries.
	Include any separate noscript parse; do not hide it in untimed setup.
5. Report non-FAST Trafilatura as a separately labeled comparison from its own
	report. A within-run Go/Rust ratio is not an old/new release speedup.
6. Keep corpus F1 scores separate, with errors retained and five-decimal
	percentages. Different scoring definitions cannot be averaged. Matching text
	scores do not establish identical HTML or metadata; retain known differences.
7. Link immutable benchmark commits/reports. Preserve old reports unchanged and
	label historical tables. Do not mix annotated quality and fallback corpora.
8. Documentation-only releases retain actual measured version labels. Do not
	rerun measurements for wording changes or relabel an old run as a new one.

## Workflow and Crate Checks

1. Identify the change and its evidence before editing. Keep algorithm, worker,
	benchmark and documentation tasks separate. Do not change runtime behavior
	to justify wording or restart a completed benchmark campaign.
2. Update README, UPSTREAM and CHANGELOG in their roles. Coordinate the common
	benchmark section with all six repositories and the benchmark repository.
3. Validate numbers, Markdown links, examples, option names and versions. Compare
	the six common sections and run `git diff --check`. Use the pinned toolchain
	and the repository's format, doc-test, test and Clippy checks as appropriate.
	Report optional fixtures or platforms not actually exercised.
4. Obtain explicit authorization before commits, pushes, versions, tags, GitHub
	releases or `cargo publish`. Never move published tags or replace archives.
5. Crates.io freezes its README in the published archive. Finalize README,
	UPSTREAM, CHANGELOG and AGENTS before packaging. Updating an existing
	release body does not update its crate README; request a new patch version.
6. For documentation-only patches, keep runtime source and dependency pins
	unchanged. Bump package identity and its lockfile entry, and update install
	examples/current-version links. Do not relabel measured benchmark versions.
7. Inspect Cargo's file list and archive from the clean release commit. Include
	intended docs, avoid temporary tools/secrets, verify examples and compare
	runtime sources with the previous released archive. Registry normalization
	of Git dependencies is expected; verify its resolved versions/checksums.
8. Check package-name ownership and version availability before publishing.
	Verify the uploaded crate checksum, exact source/doc bytes, normal registry
	installation and GitHub release page. A tag alone is not registry publication.
9. Hash exact Git/published bytes, not assumed Windows checkout bytes. Keep
	credentials out of logs. Preserve immutable old evidence and release identities.
10. Report actual validation and publication state concisely. Detailed source
	 pins and receipts belong in UPSTREAM/reports, not the README or changelog.