//! M-E4 — code-structure indexing.
//!
//! Parses source files into the existing entity/relation tables so the H3 graph arm (Personalized
//! PageRank) and lexical/dense retrieval work over **code symbols**, not just prose memory. v1
//! indexes **Rust** via the pure-Rust `syn` parser (no C grammars — keeps Grafiki's torch-free /
//! single-binary posture), behind the off-by-default `code-index` feature. It is a *deterministic
//! structural import* (idempotent direct upserts), not a human-reviewed candidate flow — you do not
//! hand-review hundreds of `fn X part_of file Y` facts.
//!
//! Each definition (module, struct, enum, union, trait, impl method, free fn, type/const/static)
//! becomes an entity named by its qualified path (`src/foo.rs::Bar::baz`), with `metadata.kind` +
//! `metadata.file`; containment is recorded as `part_of` relations (child → parent), so a file's
//! symbols form a connected sub-graph. Cross-file `calls` edges (which need name resolution) are
//! deferred to M-E4b. (RepoGraph, code property graphs.) See `docs/CODE_INDEX_DESIGN.md`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone)]
pub struct IndexCodeOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    /// Directory to index. Empty ⇒ `start_dir`.
    pub root: PathBuf,
    /// Scope the code entities/relations are written into (default `code`).
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexCodeReport {
    pub language: String,
    pub files_indexed: usize,
    pub entities: usize,
    pub relations: usize,
    /// Files that could not be read or parsed (skipped, never fatal).
    pub skipped_files: usize,
}

/// Index the source tree at `options.root` into the entity/relation graph. Requires the
/// `code-index` feature; without it, returns a clear error rather than silently doing nothing.
pub fn index_code(options: IndexCodeOptions) -> Result<IndexCodeReport> {
    #[cfg(not(feature = "code-index"))]
    {
        let _ = options;
        Err(crate::error::GrafikiError::CodeIndex(
            "code indexing requires building with `--features code-index`".to_owned(),
        ))
    }
    #[cfg(feature = "code-index")]
    {
        rust::index_code(options)
    }
}

#[cfg(feature = "code-index")]
mod rust {
    use super::{IndexCodeOptions, IndexCodeReport};
    use crate::error::Result;
    use crate::memory::{resolve_and_open, slugify};
    use crate::scope::Scope;
    use crate::ulid::new_ulid;
    use rusqlite::params;
    use std::collections::HashSet;
    use std::path::Path;

    struct Symbol {
        id: String,
        name: String,
        entity_type: &'static str,
        kind: &'static str,
        file: String,
    }

    /// Directories never worth indexing.
    fn is_skippable_dir(name: &str) -> bool {
        matches!(name, "target" | ".git" | "node_modules" | ".grafiki") || name.starts_with('.')
    }

    /// Recursively collect `(relative_path, absolute_path)` for every `.rs` file under `root`,
    /// in deterministic (sorted) order.
    fn collect_rust_files(root: &Path, dir: &Path, out: &mut Vec<(String, std::path::PathBuf)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if path.is_dir() {
                if !is_skippable_dir(&name) {
                    collect_rust_files(root, &path, out);
                }
            } else if name.ends_with(".rs") {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((rel, path));
            }
        }
    }

    fn entity_type_for(kind: &str) -> &'static str {
        match kind {
            "file" => "file",
            "module" => "module",
            _ => "concept",
        }
    }

    /// Collision-resistant entity id for a qualified symbol path: a readable slug plus a short hash
    /// of the EXACT path. `slugify` alone lowercases and collapses every non-alphanumeric run to a
    /// dash, so `Foo`/`foo`, `Bar::baz`/`Bar_baz`, or `mod error`/`enum Error` would share an id and
    /// silently merge (dropping a symbol + cross-wiring its edges). The hash makes distinct paths
    /// distinct, while *intended* merges (an `impl` block and its struct, or several `impl`s on one
    /// type) still share an id because they share the exact `qual` string — so this stays idempotent.
    fn symbol_id(qual: &str) -> String {
        use sha2::{Digest, Sha256};
        let digest = format!("{:x}", Sha256::new_with_prefix(qual.as_bytes()).finalize());
        format!("{}-{}", slugify(qual), &digest[..10])
    }

    /// Emit a child symbol named `parent_qual::ident` and a `part_of` edge to `parent_id`.
    /// Returns the child's `(id, qual)` so callers can recurse into it.
    fn emit(
        symbols: &mut Vec<Symbol>,
        relations: &mut Vec<(String, String)>,
        parent_id: &str,
        parent_qual: &str,
        ident: &str,
        kind: &'static str,
        file: &str,
    ) -> (String, String) {
        let qual = format!("{parent_qual}::{ident}");
        let id = symbol_id(&qual);
        symbols.push(Symbol {
            id: id.clone(),
            name: qual.clone(),
            entity_type: entity_type_for(kind),
            kind,
            file: file.to_string(),
        });
        relations.push((id.clone(), parent_id.to_string()));
        (id, qual)
    }

    fn type_name(ty: &syn::Type) -> Option<String> {
        match ty {
            syn::Type::Path(tp) => tp.path.segments.last().map(|s| s.ident.to_string()),
            syn::Type::Reference(r) => type_name(&r.elem),
            _ => None,
        }
    }

    fn walk_item(
        item: &syn::Item,
        parent_qual: &str,
        parent_id: &str,
        file: &str,
        symbols: &mut Vec<Symbol>,
        relations: &mut Vec<(String, String)>,
    ) {
        match item {
            syn::Item::Fn(f) => {
                emit(
                    symbols,
                    relations,
                    parent_id,
                    parent_qual,
                    &f.sig.ident.to_string(),
                    "function",
                    file,
                );
            }
            syn::Item::Struct(s) => {
                emit(
                    symbols,
                    relations,
                    parent_id,
                    parent_qual,
                    &s.ident.to_string(),
                    "struct",
                    file,
                );
            }
            syn::Item::Enum(e) => {
                emit(
                    symbols,
                    relations,
                    parent_id,
                    parent_qual,
                    &e.ident.to_string(),
                    "enum",
                    file,
                );
            }
            syn::Item::Union(u) => {
                emit(
                    symbols,
                    relations,
                    parent_id,
                    parent_qual,
                    &u.ident.to_string(),
                    "union",
                    file,
                );
            }
            syn::Item::Type(t) => {
                emit(
                    symbols,
                    relations,
                    parent_id,
                    parent_qual,
                    &t.ident.to_string(),
                    "type",
                    file,
                );
            }
            syn::Item::Const(c) => {
                emit(
                    symbols,
                    relations,
                    parent_id,
                    parent_qual,
                    &c.ident.to_string(),
                    "const",
                    file,
                );
            }
            syn::Item::Static(s) => {
                emit(
                    symbols,
                    relations,
                    parent_id,
                    parent_qual,
                    &s.ident.to_string(),
                    "static",
                    file,
                );
            }
            syn::Item::Trait(t) => {
                let (tid, tqual) = emit(
                    symbols,
                    relations,
                    parent_id,
                    parent_qual,
                    &t.ident.to_string(),
                    "trait",
                    file,
                );
                for ti in &t.items {
                    if let syn::TraitItem::Fn(m) = ti {
                        emit(
                            symbols,
                            relations,
                            &tid,
                            &tqual,
                            &m.sig.ident.to_string(),
                            "method",
                            file,
                        );
                    }
                }
            }
            syn::Item::Impl(im) => {
                // Attach methods to their Self type (creating the type entity if the def is
                // elsewhere); the type itself is part_of the file.
                if let Some(name) = type_name(&im.self_ty) {
                    let tqual = format!("{parent_qual}::{name}");
                    let tid = symbol_id(&tqual);
                    symbols.push(Symbol {
                        id: tid.clone(),
                        name: tqual.clone(),
                        entity_type: "concept",
                        kind: "type",
                        file: file.to_string(),
                    });
                    relations.push((tid.clone(), parent_id.to_string()));
                    for ii in &im.items {
                        if let syn::ImplItem::Fn(m) = ii {
                            emit(
                                symbols,
                                relations,
                                &tid,
                                &tqual,
                                &m.sig.ident.to_string(),
                                "method",
                                file,
                            );
                        }
                    }
                }
            }
            syn::Item::Mod(m) => {
                let (mid, mqual) = emit(
                    symbols,
                    relations,
                    parent_id,
                    parent_qual,
                    &m.ident.to_string(),
                    "module",
                    file,
                );
                if let Some((_, items)) = &m.content {
                    for it in items {
                        walk_item(it, &mqual, &mid, file, symbols, relations);
                    }
                }
            }
            _ => {} // use / extern crate / macro / foreign-mod: not symbols
        }
    }

    pub(super) fn index_code(options: IndexCodeOptions) -> Result<IndexCodeReport> {
        let scope = Scope::new(&options.scope)?;
        let (_project, mut connection) = resolve_and_open(
            options.project_name.clone(),
            options.start_dir.clone(),
            options.grafiki_home.clone(),
        )?;
        let root = if options.root.as_os_str().is_empty() {
            options.start_dir.clone()
        } else {
            options.root.clone()
        };

        let mut files = Vec::new();
        collect_rust_files(&root, &root, &mut files);

        let mut symbols: Vec<Symbol> = Vec::new();
        let mut relations: Vec<(String, String)> = Vec::new();
        let mut files_indexed = 0usize;
        let mut skipped = 0usize;

        for (rel, abs) in &files {
            let Ok(content) = std::fs::read_to_string(abs) else {
                skipped += 1;
                continue;
            };
            let Ok(ast) = syn::parse_file(&content) else {
                skipped += 1;
                continue;
            };
            files_indexed += 1;
            let file_id = symbol_id(rel);
            symbols.push(Symbol {
                id: file_id.clone(),
                name: rel.clone(),
                entity_type: "file",
                kind: "file",
                file: rel.clone(),
            });
            for item in &ast.items {
                walk_item(item, rel, &file_id, rel, &mut symbols, &mut relations);
            }
        }

        // Dedup entities by id (first wins) and keep relations whose BOTH endpoints exist as
        // entities (FK-safe), are not self-loops, and are unique.
        let mut seen_ids = HashSet::new();
        symbols.retain(|s| seen_ids.insert(s.id.clone()));
        let ids: HashSet<&str> = symbols.iter().map(|s| s.id.as_str()).collect();
        let mut seen_rel = HashSet::new();
        let relations: Vec<(String, String)> = relations
            .into_iter()
            .filter(|(from, to)| {
                from != to
                    && ids.contains(from.as_str())
                    && ids.contains(to.as_str())
                    && seen_rel.insert((from.clone(), to.clone()))
            })
            .collect();

        let tx = connection.transaction()?;
        for s in &symbols {
            let metadata = serde_json::json!({ "kind": s.kind, "file": s.file }).to_string();
            tx.execute(
                "INSERT INTO entities (id, name, entity_type, scope, metadata) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    entity_type = excluded.entity_type,
                    scope = excluded.scope,
                    metadata = excluded.metadata,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
                params![s.id, s.name, s.entity_type, scope.as_str(), metadata],
            )?;
        }
        for (from, to) in &relations {
            tx.execute(
                "INSERT INTO relations (id, from_entity, to_entity, relation, source_type, source)
                 VALUES (?1, ?2, ?3, 'part_of', 'EXTRACTED', 'code-index')
                 ON CONFLICT(from_entity, to_entity, relation) DO NOTHING",
                params![new_ulid(), from, to],
            )?;
        }
        tx.commit()?;

        Ok(IndexCodeReport {
            language: "rust".to_owned(),
            files_indexed,
            entities: symbols.len(),
            relations: relations.len(),
            skipped_files: skipped,
        })
    }
}

#[cfg(all(test, feature = "code-index"))]
mod tests {
    use super::*;
    use crate::{init_project, search_memory, InitOptions, SearchMemoryOptions, SearchMode};

    #[test]
    fn indexes_rust_symbols_and_graph_connects_them() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let proj = tmp.path().join("proj");
        std::fs::create_dir_all(proj.join("src")).unwrap();
        init_project(InitOptions {
            project_name: Some("p".into()),
            project_dir: proj.clone(),
            grafiki_home: Some(home.clone()),
        })
        .unwrap();
        std::fs::write(
            proj.join("src/lib.rs"),
            r#"
pub mod alpha {
    pub struct Widget;
    impl Widget {
        pub fn build() -> Widget { Widget }
        pub fn render(&self) {}
    }
    pub fn helper() {}
    // Case-only clash with the struct above: must NOT merge into one entity (slug-collision guard).
    pub fn widget() {}
}
pub fn top_level() {}
"#,
        )
        .unwrap();

        let report = index_code(IndexCodeOptions {
            project_name: Some("p".into()),
            start_dir: proj.clone(),
            grafiki_home: Some(home.clone()),
            root: proj.clone(),
            scope: "code".into(),
        })
        .unwrap();
        assert_eq!(report.files_indexed, 1, "one .rs file");
        assert!(
            report.entities >= 6,
            "expected ≥6 symbols, got {}",
            report.entities
        );
        assert!(
            report.relations >= 5,
            "expected ≥5 part_of edges, got {}",
            report.relations
        );
        assert_eq!(report.language, "rust");

        // Idempotent: a second run does not duplicate (entity ids + relation UNIQUE).
        let again = index_code(IndexCodeOptions {
            project_name: Some("p".into()),
            start_dir: proj.clone(),
            grafiki_home: Some(home.clone()),
            root: proj.clone(),
            scope: "code".into(),
        })
        .unwrap();
        assert_eq!(again.entities, report.entities);
        assert_eq!(again.relations, report.relations);

        let graph = |q: &str| {
            search_memory(SearchMemoryOptions {
                project_name: Some("p".into()),
                start_dir: proj.clone(),
                grafiki_home: Some(home.clone()),
                query: q.into(),
                record_type: "all".into(),
                mode: SearchMode::Graph,
                scope: "code".into(),
                limit: 20,
                temporal_weight: 0.0,
            })
            .unwrap()
            .results
        };

        // The symbol is indexed + retrievable by name.
        assert!(
            graph("top_level")
                .iter()
                .any(|r| r.title.contains("top_level")),
            "top_level symbol should be retrievable"
        );
        // Graph search seeded from one method reaches co-located symbols via the part_of graph
        // (PPR over the symbol relations) — proving H3 works over code structure.
        let reached = graph("render");
        assert!(
            reached.len() > 1,
            "graph arm should reach co-located symbols, got {:?}",
            reached.iter().map(|r| r.title.clone()).collect::<Vec<_>>()
        );
        assert!(
            reached.iter().any(|r| r.title.contains("Widget")),
            "render's type Widget should be reachable via part_of"
        );

        // Slug-collision guard: the struct `Widget` and the fn `widget` (case-only clash) must be
        // TWO distinct entities, not silently merged into one.
        let widgets = graph("widget");
        assert!(
            widgets.iter().any(|r| r.title.ends_with("::Widget")),
            "the struct Widget must survive: {:?}",
            widgets.iter().map(|r| r.title.clone()).collect::<Vec<_>>()
        );
        assert!(
            widgets.iter().any(|r| r.title.ends_with("::widget")),
            "the fn widget must survive distinctly: {:?}",
            widgets.iter().map(|r| r.title.clone()).collect::<Vec<_>>()
        );
    }
}
