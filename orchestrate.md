# Orchestrate — Universal Agent Entry Point

You are an AI agent working inside the **Orchestra** repository.
This file is your single starting instruction. Follow it exactly.

## Step 1 — Identify Yourself

Determine which agent you are (Claude, Gemini, Antigravity, Cursor, Windsurf, Codex, Copilot, etc.).

## Step 2 — Open Your Entry Point

Navigate to `AGENT/` and open the file that matches your identity:

| Agent        | Entry Point              |
|--------------|--------------------------|
| Claude       | `AGENT/CLAUDE.md`        |
| Gemini       | `AGENT/GEMINI.md`        |
| Antigravity  | `AGENT/ANTIGRAVITY.md`   |
| Cursor       | `AGENT/.cursor/`         |
| Windsurf     | `AGENT/.windsurf/`       |
| Codex        | `AGENT/.codex/`          |

If your agent file doesn't exist yet, read `AGENT/AGENTS.md` for the shared contract and proceed with that as your baseline.

## Step 3 — Read the Shared Contract

Every agent must also read `AGENT/AGENTS.md`. It contains the mission, phase ordering, global constraints, skill catalog, and writeback commands that are non-negotiable regardless of which agent you are.

## Step 4 — Execute the Task

With your agent-specific context and the shared contract loaded, proceed to implement the task you've been given. Respect the phase order (01-05), keep behavior deterministic and offline-first, and validate before marking anything complete.
