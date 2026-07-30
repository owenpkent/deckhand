---
description: Run the docs CI gates locally before pushing
---

Reproduce `.github/workflows/docs.yml` against the working tree. Report
only. Do not fix, commit, or push.

1. Run `pwsh -NoProfile -File scripts/check-docs.ps1 -All`. That is the
   same script the CI style job runs, so its verdict is the CI verdict for
   the dash rule, the status line rule, ADR numbering, and the
   Constellation contract. Quote any failing line it prints. Its 80 column
   output is a warning and never a failure; do not report it as one.
   If `pwsh` is missing, `powershell -NoProfile -File` runs it too.

2. Link gate. `lychee` is not installed on this machine, so approximate the
   offline job: extract every relative markdown link and anchor from
   tracked `*.md` with
   `rg -n "\]\(([^)h][^)]*)\)" --glob "*.md"`, then confirm each target
   file exists and each `#fragment` matches a real heading, or an explicit
   `<a id="...">` anchor, in that file. `docs/DECISIONS.md` uses explicit
   anchors, so heading-slug matching alone will produce false failures
   there. Report unresolved targets. State plainly that this is an
   approximation of the offline lychee run, not the same check.

3. The changelog job runs on pull requests only: if this branch touches
   anything under `docs/`, confirm `CHANGELOG.md` is touched too, since CI
   will fail the PR otherwise.

End with PASS or FAIL and the exact failing lines.
