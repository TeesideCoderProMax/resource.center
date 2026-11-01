# Completion checks
- For application/infrastructure changes, run appropriate Makefile checks; make check runs the full local suite. Documentation-only edits need reference/config/whitespace checks, not cloud deployment.
- For authorized deployed changes, check intended environment drift and smoke tests; do not treat an old success as a fresh check.
- Reconcile affected memories under `mem:memory_maintenance`: replace stale claims and delete obsolete context, retaining unresolved work and evidence links. Run serena memories check.
- Finish the current AI_LOG.md entry with actual changes, checks and limits.
