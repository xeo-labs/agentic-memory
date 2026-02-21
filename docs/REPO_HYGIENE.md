# Repository Hygiene Policy

This repository follows a public-source hygiene rule set.

## Do Not Commit

- AI planning scratchpads and prompt session artifacts.
- Internal-only strategy documents that are not part of product docs.
- Local runtime state and generated caches.
- Secrets, tokens, private keys, and local env files.

## Required Controls

- `.gitignore` MUST block planning/prompt/internal folders.
- Public docs MUST live under `docs/` and be product-meaningful.
- Release and publish workflows MUST only ship production artifacts.

## Pre-Push Checklist

- Confirm no `planning` or `prompt` files are staged.
- Confirm no `.env*`, credentials, or local machine artifacts are staged.
- Run project verification (`fmt`, `clippy/lint`, `test`) before publishing.
