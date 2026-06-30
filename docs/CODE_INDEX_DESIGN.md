# M-E4 — Code-structure indexing

**Status:** done (v1, Rust). **Feature:** `code-index` (off by default). **Depends on:** the
entity/relation tables, H3 graph retrieval (PPR), and the lexical/dense search. **Refs:** RepoGraph,
code property graphs.

## Goal
Make an agent's memory graph include **code symbols**, so retrieval and the H3 graph arm work over a
project's structure — "what's near `weibull_freshness`", "what's in `decay.rs`" — not just prose.

## Decisions (and why)
- **Parser = `syn` (pure Rust), not tree-sitter.** v1 is Rust-only (dogfoods Grafiki's own repo) and
  `syn` gives a robust Rust AST with **no C grammars** — preserving Grafiki's torch-free / single-binary
  posture. The extractor (`walk_item`) is isolated, so a tree-sitter/other-language backend slots in
  later without touching the write path. (The checklist named tree-sitter as the technique; `syn`
  achieves the same goal more cleanly for Rust — same spirit as choosing deterministic Louvain over a
  Leiden dependency in H5.)
- **Feature-gated, off by default.** Mirrors `fastembed`: the lean default binary doesn't pull `syn`.
  `grafiki index-code` always exists; without the feature it returns a clear "build with
  `--features code-index`" error (`GrafikiError::CodeIndex`).
- **Deterministic structural import, NOT the candidate gate.** Code structure is high-volume and
  unambiguous; you don't human-review 500 `fn X part_of file Y` facts. So `index_code` writes entities
  + relations **directly** (idempotent upserts), like the `grafiki init` memory import — distinct from
  the ingest→candidate→review path for extracted prose facts.
- **No schema migration.** Symbols map onto the existing `entity_type` CHECK: `file` → `file`, module →
  `module`, everything else (fn/struct/enum/union/trait/method/type/const/static) → `concept`, with the
  precise kind in `metadata.kind` (+ `metadata.file`). The qualified path is the entity **name**
  (`src/foo.rs::Bar::baz`); its slug is the id.

## What v1 extracts
- One **`file`** entity per `.rs` file (relative path).
- A **symbol** entity per definition: module, struct, enum, union, trait, free fn, impl method,
  trait method, type alias, const, static — named `<file>::<path>::<ident>`.
- **`part_of`** containment edges (child → parent): item → file/module, method → its `Self` type,
  trait method → trait. Edges are **FK-safe by construction** (only emitted between entities that were
  actually created), self-loop-free, and de-duplicated; the type endpoint of an `impl` is materialized
  so external-type impls don't dangle.
- Walk is deterministic (sorted file order); skips `target/`, `.git/`, `node_modules/`, dotdirs;
  unreadable/unparseable files are counted as `skipped`, never fatal.

## Surfaces
- Library: `grafiki_core::index_code(IndexCodeOptions{ root, scope, … }) -> IndexCodeReport`.
- CLI: `grafiki index-code --root <dir> --scope code` (needs a `--features code-index` build).
- Scope defaults to `code` so symbols don't pollute prose-memory search; search that scope to get them.

## Verification
- CI gate `indexes_rust_symbols_and_graph_connects_them` (`#[cfg(feature="code-index")]`): indexes a
  fixture file, asserts entity/relation counts, **idempotency** (a second run adds nothing), and that
  `SearchMode::Graph` seeded from one method **reaches its co-located symbols** (its `Self` type) via
  the `part_of` PPR — i.e. H3 works over code.
- Dogfood: indexing `crates/grafiki-core/src` yields ~787 symbol entities + ~769 `part_of` relations.

## Deferred (M-E4b)
- **Cross-file `calls`/`uses` edges** — need name resolution across files (hard; RepoGraph-style
  heuristics); v1 is definitions + containment only.
- Other languages (tree-sitter backends), signatures/doc-comments as searchable observations, and
  incremental re-index (currently a full re-walk; idempotent so it's safe to re-run).
