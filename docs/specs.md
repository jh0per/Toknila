# Importing and exporting spec folders

Toknila reads the folders that spec-driven toolkits such as GitHub Spec Kit produce
(`specs/232-some-feature/` with `spec.md`, `plan.md`, `research.md`, `data-model.md`,
`contracts/`, `quickstart.md`, `tasks.md`) and writes progress back into their `tasks.md`.

```sh
tok project new APP
tok chain import specs/            # every spec folder inside, one chain each
tok chain import specs/232-some-feature   # or a single folder
tok chain export 232-some-feature --spec specs/232-some-feature
```

## How `tasks.md` maps to a chain

| In the file | In Toknila |
|---|---|
| The folder | One chain named after the folder; its goal is the `# Tasks: <title>` line plus the folder's documents by relative path |
| `## Phase N: Name` (or any other `##` section that holds tasks) | One card; the text between the heading and the first task is its prompt |
| `## Phase N: User Story K - Title` | A card titled `USK: Title`, so the story label stays visible |
| `- [ ] T005 [P] [US1] text` | One subtask whose text is kept verbatim, T-id included; ticked when `[x]` |
| Tasks before the first `##` heading | One card, first in the chain, with no phase number, titled after the `# Tasks:` title, else the `# ` title of `spec.md`, else the folder name |
| Sections without tasks (Dependencies, Parallel Opportunities, ...) | Not cards |

Order: cards follow the file. Story phases run in parallel, each after the last non-story phase
before them. A non-story phase waits for the stories since the previous non-story phase, or for
that phase when there are none (so Polish waits for all stories).

An imported chain never starts by itself (autostart is off). A card whose tasks are all checked
is imported as Merged through the ordinary state machine (Ready, Running, Review, Merged), so its
dependents become Ready as usual.

A card whose tasks are all checked but which sits behind a card that still has open tasks (work
done in another order than the file lists it) is imported as Merged too, marked "done out of
order". The state machine takes it there with its own event (`done_out_of_order`), the journal
records it, and the ticket keeps the keys of the earlier open cards (`done_out_of_order.behind`
in the ticket detail), so a view can show it. Stories that run in parallel are not behind each
other. The import report lists each such card, for example
`phase-3 done out of order: phase-2 has 1 open subtask (T011)`.

Import reports each folder (`imported`, `skipped` when there is no `tasks.md`, or `failed`) and
goes on after a failure; the command exits non-zero if any folder failed. Importing a folder whose
chain already exists is refused. A malformed task line (`- [y] ...`, `- [ ]` without text) or a
task under a later `#` heading is an error naming the file and line.

The daemon reads the folders (`POST /v1/projects/{project}/import-path` with the absolute path),
so the window imports the same way, and each chain remembers its folder as `source`.
`GET /v1/projects/{project}/plans` lists the spec folders under `specs/` and the chain files under
`plans/` of the project's repository and marks the ones already imported.

## Export

`tok chain export <chain> --spec <folder>` patches `<folder>/tasks.md`, it does not rewrite it:

- A checkbox is flipped to match the subtask's state; tasks are matched by their T-id (or, without
  one, by their whole text).
- A card that did not come from the file (for example an approved wildcard card) is appended as a
  new `## Phase N: <card title>` section, its tasks numbered after the highest T-id in the file.
- Every other byte stays, line endings included (LF or CRLF). Nothing is deleted; subtasks added
  in Toknila to a card that is in the file are not written yet.

Exporting right after an import, or twice in a row, leaves the file byte-identical. The file is
only written when it changes.
