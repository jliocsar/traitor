# `traitor check --format json`

This is the report for tools: editor and linter plugins, bots, CI steps. Its shape is a stable contract. traitor knows nothing about the tool that reads it.

## Running it

```sh
traitor check --format json                           # what you haven't pushed, like plain `check`
traitor check --format json origin/main               # what changed since origin/main
traitor check --format json --files src/a.ts src/b.ts # these files, whole, as they are on disk
```

- **stdout** holds the report as one line of JSON when the exit code is `0` or `1`.
- **The exit codes** are `check`'s own:
  - `0`: nothing failed a threshold;
  - `1`: a finding has `failsCheck: true`;
  - `2`: traitor couldn't run. stdout is then empty and stderr says why.
- **stderr** can also hold warnings, such as a cache that couldn't be written. They never change the report.
- **`--files`**:
  - It takes paths that are absolute or relative to the current directory. Each one has to be inside the repository.
  - It skips files whose path no trait matches (or that a trait exempts), and binary files.
  - It cuts each file into snippets the way `check` does: functions, top-level statements and doc sections. So a finding points at a snippet, not the whole file.
  - It can't be combined with a revision.
- **Verdicts are cached** by snippet and trait. Running it on every lint run only asks Jev about snippets that changed since the last one.
- It needs `TYPESAFE_API_KEY`, like any `check`.

## The report

```json
{
  "version": 1,
  "root": "/work/repo",
  "snippetCount": 2,
  "failsCheck": true,
  "findings": [
    {
      "path": "src/config.ts",
      "firstLine": 12,
      "lastLine": 31,
      "trait": "railway-not-guards",
      "title": "Railway, not guards",
      "why": "Guards hide how a value flows.",
      "verdict": "fail",
      "pFail": 0.91,
      "threshold": 0.5,
      "failsCheck": true,
      "fromCache": false
    }
  ],
  "nonUtf8Paths": []
}
```

| Field | Type | Meaning |
|---|---|---|
| `version` | integer | The contract's version, `1` for this shape |
| `root` | string | The repository's root, absolute. Each `path` is relative to it |
| `snippetCount` | integer | How many snippets were judged |
| `failsCheck` | boolean | Whether any finding has `failsCheck: true`, which is when the exit code is `1` |
| `findings` | array | One per snippet and trait that applies to it, passes included |
| `nonUtf8Paths` | array of strings | Paths that aren't valid UTF-8, shown as near as they can be. They weren't judged |

Each finding:

| Field | Type | Meaning |
|---|---|---|
| `path` | string | The file, relative to `root`, with `/` between its parts |
| `firstLine` | integer | The snippet's first line, counting from 1 |
| `lastLine` | integer | The snippet's last line, included |
| `trait` | string | The trait's slug: its folder's name under `.agents/traits/` |
| `title` | string | The trait's title, from its `TRAIT.md` |
| `why` | string | Why the trait matters, from its `TRAIT.md` |
| `verdict` | `"pass"`, `"fail"`, `"n/a"` or `"unsure"` | Jev's answer |
| `pFail` | number, 0 to 1 | How likely the snippet fails the trait |
| `threshold` | number or `null` | The trait's threshold in `.agents/traitor.json`, or `null` when its fails are only reported |
| `failsCheck` | boolean | Whether `pFail` reached `threshold` |
| `fromCache` | boolean | Whether the verdict came from traitor's cache instead of a new request |

`unsure` isn't a fail: Jev couldn't tell from the snippet alone, so a person should read it. A tool that turns findings into diagnostics usually shows `fail` and `unsure`. It marks `failsCheck: true` as an error and everything else as a warning.

## Versioning

`version` changes only for a change that could break a reader: a field renamed, removed, or with a new type or meaning. A new field doesn't change it, so ignore fields you don't know. A reader should refuse a `version` it wasn't written for.

The JSON Schema (draft 2020-12) is [`schema/check-report.schema.json`](../schema/check-report.schema.json). A test keeps it in step with the code.
