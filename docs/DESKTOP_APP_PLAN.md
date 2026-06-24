# Grafiki Desktop App Plan

## Product Direction

Grafiki Desktop should feel like a sharper, more focused Macro-style workspace for AI project memory.

The app is not a generic notes app, document editor, task manager, chat client, or CRM. It is a memory console for developers and AI agents working on a codebase. Every screen should answer one of these questions:

- What does this project already know?
- What should the next AI session know before it starts?
- What decisions are active, stale, superseded, or risky?
- What entities, files, modules, and concepts are connected?
- What memory was retrieved, why was it retrieved, and how fresh is it?
- What should be saved back into memory after this session?

## Macro-Inspired Behaviors To Keep

Grafiki should adopt the parts of Macro that make it feel fast and serious:

- Keyboard-first navigation with visible shortcuts.
- Command palette as the fastest path to almost every action.
- Multi-pane workspace where users can keep search, graph, session, and detail views open together.
- URL-synced pane state so a workspace layout can be restored, shared, bookmarked, and debugged.
- Fast global search across all first-class records.
- A launcher for creating memory records without hunting through menus.
- Compact, professional density with small controls, clear icons, and restrained surfaces.
- Entity-first thinking: sessions, decisions, observations, context documents, files, scopes, and relations are all connected objects.
- Strong provenance: every important record shows source, confidence, scope, freshness, and related records.

## Design Personality

The visual direction should be sharper than Macro while staying practical:

- Light, document-grade operational shell first, with a dark mode later if it improves long-session work.
- High contrast text, precise borders, compact rows, and calm spacing.
- One confident accent color for active states and retrieval highlights.
- Monospace only for IDs, paths, snippets, commands, and diagnostics.
- No marketing hero screen. The app opens directly into the working memory console.
- No decorative panels that do not help the user understand project memory.
- Use familiar icons for navigation and actions.

## First Desktop Shell

The first usable desktop screen should include:

- Left rail: Overview, Search, Graph, Relations, Sessions, Decisions, Context, Settings.
- Top status strip: project, database path, daemon state, embedding backend, indexed count, stale count.
- Main pane: current route content.
- Right inspector: selected item details, relations, source/provenance, retrieval score, freshness, and quick actions.
- Command palette: search memory, open route, start session, end session, save decision, add observation, add context, rebuild embeddings, open graph around selection.
- Launcher: create decision, observation, state item, context document, session handoff, relation.

## Multi-Pane Layout

Multi-pane layout is a first-class requirement, not a later polish task.

The desktop app should support:

- Opening any entity or view in the current pane.
- Opening a result in a new pane.
- Splitting horizontally at first, with vertical split support later if needed.
- Closing, replacing, and reordering panes.
- Tracking the active pane for keyboard commands.
- Restoring focus when panes are inserted or removed.
- Persisting the last workspace layout locally.
- Encoding pane layout in the route so state survives refresh and can be shared.

Initial route shape:

```text
/app/panes/:encodedLayout
```

Possible encoded pane examples:

```text
overview
search?q=auth+middleware
decision/01JABC...
session/01JDEF...
graph?entity=router
context/01JGHI...
```

The exact encoding can be adjusted during implementation, but the model should be explicit:

```text
PaneContent =
  overview
  | search(query, filters)
  | graph(focus_entity, depth, filters)
  | session(id)
  | decision(id)
  | observation(id)
  | context(id)
  | state(id)
  | settings(tab)
```

## Core Views

### Overview

Shows the current project memory health:

- Active session or latest session.
- Recent decisions.
- Open state items.
- Recently changed memory.
- Retrieval/indexing status.
- Suggested cleanup: stale observations, orphan entities, missing context, failed embedding jobs.

### Search

The primary working view.

- Supports keyword, semantic, and hybrid search modes.
- Shows score, source type, scope, freshness, and why it matched.
- Lets a user open any result in the inspector or a new pane.
- Supports filters for record type, scope, confidence, tags, and time.

### Graph

Visualizes project memory as connected entities and records.

- Starts with focused graph around a selected entity or search result.
- Shows entity, decision, observation, context, and relation nodes.
- Provides filters to hide low-confidence or stale records.
- Can open selected graph nodes into a pane or inspector.

### Relations

Shows graph links as maintainable AI memory records.

- Browse and filter relation records by type.
- Inspect from/to entities, relation type, weight, confidence, source type, and source.
- Correct relation metadata from the detail pane.
- Remove incorrect or stale links without deleting the underlying entities.

### Sessions

Shows AI session history and handoff quality.

- Active and recent sessions.
- Session goals, scope, status, outputs, changed files, and child handoffs.
- Start/end/handoff actions.

### Decisions

Shows durable project decisions.

- Active, superseded, revisit, and revoked decisions.
- Reasoning, alternatives, tags, scope, supersession chain, and related records.

### Context

Shows trusted context documents and snippets.

- Architecture notes, project rules, coding conventions, integration details, and setup notes.
- Context records are memory inputs, not a general document system.

### Settings

Controls local behavior:

- Project database.
- Daemon and HTTP settings.
- Embedding provider and sqlite-vec status.
- MCP setup hints.
- Import/export.
- Privacy and connector policy when connectors arrive.

## Tauri Architecture

Grafiki Desktop should use Tauri because the core is already Rust.

Preferred first architecture:

- `grafiki-core` remains the source of truth.
- A new desktop crate wraps `grafiki-core` with Tauri commands.
- The frontend calls Tauri commands for local reads/writes.
- The HTTP daemon remains available for external clients and agents.
- The desktop app can later start, stop, or monitor the daemon, but the first UI should not require HTTP to work.

This keeps the desktop app local-first and avoids making the UI depend on a server process for basic workflows.

## Implementation Order

1. Create Tauri shell and frontend workspace.
2. Add app frame: left rail, top status strip, main pane, inspector.
3. Add route model and URL-synced pane manager.
4. Add command palette and launcher.
5. Wire overview/search to existing Grafiki commands.
6. Add decision/session/context/detail panes.
7. Add graph view using existing graph/export data.
8. Add daemon controls, embedding status, and packaging polish.

Current desktop progress: steps 1-8 are active in the app. The launcher can write decisions, observations, state, context, relations, and handoffs through Tauri commands. Search supports keyword, semantic, and hybrid modes with scope and record-type filters persisted into the URL-synced pane state; it also shows retrieval index freshness and can process or rebuild embeddings from the search view. Selected memory can load full detail/provenance/related records into the inspector and detail pane. Settings can choose a project folder with a native dialog and initialize it. Sessions can start/end work, browse real session history, complete a specific active session, create a direct handoff from a specific active session, review the generated handoff context, copy it, open parent/child session detail records, edit session type/status/goal/scope/summary/accomplishments/remaining/files, and inspect parent/child/handoff metadata. Decisions/Relations/State/Context panes list real records, Relations can be filtered and safely removed, State and Context support inline edit plus safe delete, and the detail pane supports maintenance edits/deletes for decisions, entities, observations, relations, context, state, and session records. Settings can import/export JSON, process/rebuild embeddings, and start/status/stop the local HTTP daemon through the bundled `grafiki` CLI sidecar. A debug macOS `.app` plus `.dmg` can be built with `npm run tauri:build:debug` or `scripts/build_desktop_debug.sh`. The bundle includes a custom Grafiki icon, bundles the CLI sidecar, and has been smoke-tested from both the build output and `/Applications`.

## Non-Goals

- Do not clone Macro's source code, assets, icons, or exact visual identity.
- Do not add email, chat, docs, calls, or CRM.
- Do not turn context documents into a full Notion-like editor.
- Do not hide provenance behind vague AI summaries.
- Do not let automatic extraction silently become trusted memory.
