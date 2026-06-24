# Demo Script

## Demo 1: The Agent Joins An Old Project

1. Start with a repo that has existing project context.
2. Run:

```bash
grafiki init grafiki --path .
grafiki candidates list --scope grafiki
```

3. Approve one imported candidate.
4. Ask:

```bash
grafiki ask "What should I know before changing the desktop app?" --scope grafiki/desktop --agent codex
```

5. Show the evidence in the answer.
6. Open Agent Activity:

```bash
grafiki agent-activity --scope grafiki/desktop
```

## Demo 2: Capture A Coding Session

1. Start capture in the desktop Review pane.
2. Import a Codex, Claude Code, or Cursor transcript:

```bash
grafiki capture import-transcripts --agent codex --scope grafiki/core --summarize
```

3. Ingest or create a few terminal/git/file events.
4. Summarize capture.
5. Approve the generated memory.
6. Ask Grafiki about the same topic and show the cited answer.

## Demo 3: Stop Repeating A Rejected Approach

1. Create or import a decision explaining a rejected approach.
2. Ask Grafiki whether the agent should use that approach.
3. Show Grafiki returning the active decision with evidence.
