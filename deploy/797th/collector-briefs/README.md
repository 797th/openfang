# Collector briefs

One `context.md` per collector instance. The agent re-reads its `context.md`
**every turn** (`agent_context.rs::load_context_md`), so editing one steers the
beat immediately — no rebuild, no restart, no redeploy.

These live here because they are the actual product of each beat, they are the
only place the topic is defined, and until now they existed nowhere but the
PVC. A lost volume meant rewriting them from memory.

## Which file goes where

| brief | workspace on the PVC |
|---|---|
| `underground.context.md` | `/data/workspaces/underground/context.md` |
| `remote-jobs.context.md` | `/data/workspaces/remote-jobs/context.md` |

The workspace directory is named after the hand **instance name**, so activate
the instance first and the directory appears.

## Installing one

```bash
POD=$(kubectl -n openfang get pod -l app=openfang -o jsonpath='{.items[0].metadata.name}')
kubectl -n openfang cp remote-jobs.context.md "$POD:/data/workspaces/remote-jobs/context.md"
```

Takes effect on the agent's next turn.

## Why these are not baked into `HAND.toml`

`HAND.toml` is `include_str!`'d into the binary, so changing a beat there costs
a full rebuild and redeploy. `context.md` is read from disk every turn. Keep
the *generic* collector behaviour in `HAND.toml` and the *specific* beat here.

## What a brief has to get right

The delivery job (`../falkor-sync.yaml`) parses the report, so the report
format in each brief is a contract, not a style preference:

- reports go to `<workspace>/output/collector_report_*.md`
- artifacts are the **indented** `  - **Name** (tags) — description` bullets
  under `## Key Changes`
- a cycle with nothing new must say so in a top-level bullet, so
  `has_new_artifacts()` can stay quiet instead of pinging an empty report
- only the configured `[knowledge]` vocabulary is accepted; the kernel rejects
  anything else and a rejected write costs the agent an iteration

Both current briefs carry a `shell_exec: never use a semicolon` section. Keep
it — the sandbox rejects `;` used for chaining, and each rejection burns an
iteration the agent needs to finish its run.
