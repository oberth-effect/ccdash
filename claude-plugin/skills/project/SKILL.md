---
name: project
description: Place this session under a project of the workspace (a repo directory name), so its notes default to it and it hears about that project's sessions and issues. Use when the owner runs /ccdash:project <name>.
argument-hint: <project>
disable-model-invocation: true
allowed-tools:
  - mcp__plugin_ccdash_ccdash__claim
---

# Place the session under a project

The owner names the project: **$ARGUMENTS**

With no name, ask which project they mean and stop.

This only tells ccdash where the session works and changes no files, so it
belongs in plan mode too. Call `claim` with `project` set to that name and
`what` saying in a few words what this session is working on, keeping the paths
of a claim it already holds. If nothing is under way yet, `what` says it is
starting on that project.

Reply with one line: the session is now under that project. If `claim` refuses
the name, pass its answer on and change nothing else.
