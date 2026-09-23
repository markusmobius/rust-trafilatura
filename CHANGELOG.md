# Changelog

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

This is a GitHub source release. The repository remains private, and no crate
is published to crates.io. Parsing gains do not imply that each extraction
stage becomes faster; the coordinated suite is measured separately.