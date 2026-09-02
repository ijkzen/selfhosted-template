## Agent skills

### Issue tracker

Issues and specs are tracked in this repository's GitHub Issues. See `docs/agents/issue-tracker.md`.

### Triage labels

The repository uses the five canonical triage labels. See `docs/agents/triage-labels.md`.

### Domain docs

The repository uses a single-context domain-doc layout. See `docs/agents/domain.md`.

## CI tracking

- Hosting platform: GitHub Actions (`.github/workflows/ci.yml`).
- After every push, run `gh run watch` (or `gh run list --limit 5` / `gh run view <run-id>`) and report the outcome; do not assume success.
