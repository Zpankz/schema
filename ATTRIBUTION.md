# Attribution & Provenance

## Upstream: jcode

This repository is a derived work of **jcode** — a coding-agent harness by
Jeremy Huang — imported verbatim at upstream commit `1919df4` from
<https://github.com/1jehuang/jcode> and augmented with the `schema` crate.
jcode is licensed under the **MIT License** (Copyright (c) 2025 Jeremy
Huang); the upstream `LICENSE` is preserved unchanged at the repository
root and continues to govern the imported code. All credit for the jcode
harness — its agent runtime, providers, TUI, and tooling — belongs to the
upstream project and its contributors.

Two mechanical deviations from a verbatim import, both documented in the
import commit: (1) four embedded Google OAuth installed-app client
ID/secret literals in `crates/jcode-base/src/auth/{gemini,antigravity}.rs`
are redacted to empty strings because GitHub push protection blocks them —
upstream ships them publicly (marked `gitleaks:allow`) and supports env-var
overrides, which remain the way to use those providers here; (2)
`Cargo.lock` enters tracking at the "lockfile" commit in the augmentation
series (the fresh import honored `.gitignore` where upstream had the file
force-tracked); its content is the upstream lockfile plus the `schema`
package entry.

## The `schema` augmentation

`crates/schema` (binary: `schema`) implements the **Schema harness**
control loop — certified executable world models, an append-only Timeline,
exact backtesting, in-model BFS planning, and gated action commitment —
reverse-engineered from the methodology and reasoning traces published by
Impossible Research at <https://schema-harness.github.io/> (ARC-AGI-3).
No code from that publication was available or used; the reconstruction is
from documented observable behavior only, and any errors of interpretation
are ours, not the publication's. See `docs/SCHEMA_HARNESS.md` for the
architecture and the integration seam into jcode's runtime.

The Rust port follows a Python reference implementation that passed six
independently countersigned verification runs (regulate protocol; the
countersigner ran the Python reference as a differential probe against this
crate's binary and found number-for-number agreement on both demos).

## Authorship

The `schema` crate and this repository's assembly were produced with
Claude (Anthropic), session-attributed in each commit trailer:

    Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>

Published by @Zpankz. Nothing in this repository is affiliated with or
endorsed by the upstream jcode project, Impossible Research, or the ARC
Prize Foundation.
