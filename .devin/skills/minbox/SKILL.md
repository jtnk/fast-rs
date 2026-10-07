---
name: minbox
description: Run shell commands inside this repo's Minimal sandbox box. Use instead of plain exec for isolated, reproducible execution at /workbench.
---

# minbox — Minimal sandbox execution for this repo

This repo carries a vendored devin-minimal kit at `.devin/minimal/` and a
box declaration in `minimal.toml`. In Devin Cloud, repo hooks do not run —
so shell commands must go through `minbox` explicitly.

## Commands

```sh
sh .devin/minimal/minbox up                 # arm the box (once; idempotent)
sh .devin/minimal/minbox status             # armed? live? session id
sh .devin/minimal/minbox exec '<command>'   # sync changes + run inside box at /workbench
sh .devin/minimal/minbox exec 'host: <cmd>' # explicit host escape
sh .devin/minimal/minbox task <name>        # run a [tasks.*] declared task
sh .devin/minimal/minbox pull [path…]       # pull box artifacts back to checkout
sh .devin/minimal/minbox down               # disarm; --destroy removes the session
```

## Rules of thumb

- Prefer `minbox exec` over a bare exec for any build/test/tooling command:
  the box is reproducible (declared packages in minimal.toml) and disposable.
- Changed files sync into /workbench automatically before each boxed exec.
- If `min` is missing, run `sh .devin/minimal/cloud-bootstrap.sh` first.
- The box's egress policy is authoritative — declare network needs in
  minimal.toml rather than working around blocks.
