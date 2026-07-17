# Schema Harness (`crates/schema`)

`schema` is a workspace crate + binary that augments the jcode harness with
the **Schema** control loop — the certified-world-model architecture
reverse-engineered from the methodology and reasoning traces published at
<https://schema-harness.github.io/> (ARC-AGI-3, ~99% public-set claim). The
port follows a Python reference that passed five independently countersigned
verification runs; this crate reproduces its behaviors under `cargo test -p
schema`.

## The architecture in one paragraph

The agent's understanding of an environment must live as an **executable
program** (`step(state, action)` + `is_goal(state)`), certified by exact
replay against an **append-only Timeline** of every real transition before
it may drive action. Planning (**BFS**) happens entirely inside the
certified model, spending zero environment actions. Actions reach the
environment only through **commit_actions**, which fixes the model's
prediction *before* each real step, records the transition *before* any
halt, and on the first mismatch halts, discards the rest of the plan, and
returns the mismatched transition as a **counterexample** that drives the
next revision — jointly over transition rules *and* state representation.

## Split of planes

| Plane | Owner | In this crate |
|---|---|---|
| Ground truth (append-only Timeline) | code | `timeline.rs` — immutability is type-enforced: no `&mut` access to records exists |
| Certification (exact backtest, N/N accounting) | code | `backtest.rs` |
| Planning (in-model search) | code | `planner.rs` |
| Action gating (predict → act → record → halt-on-mismatch) | code | `executor.rs` |
| Loop control (observe → deliberate → execute → record, backtest gate) | code | `agent.rs` |
| Hypotheses, experiments, revision | inference | the `Deliberator` trait (`agent.rs`); demo implementations in `deliberators.rs` |

## How this augments jcode

jcode's agent runtime executes provider turns against tools with
permissioning and session state; what it does not have is a **certification
gate between the model's beliefs and its actions**. The Schema loop adds
exactly that discipline, and the seam is now a **working JSON protocol**, not
just a Rust trait.

### The world model is a program (`program.rs`)

The publication is explicit that the world model is "an interpretable
program … readable as text, diffable across versions … searchable." A
`RuleProgram` makes that literally true: an ordered list of transition rules
over the grid, executed by an interpreter (`step`), serialized to/from JSON,
rendered as readable text (`to_pretty`), and diffed structurally between
revisions (`diff`). It is expressive enough to reproduce both demo
environments *exactly* — including CartDock's cart-push with a
dock-conditional board reflow — which the tests cross-check against the
native environments over full playthroughs. Because the model is plain data,
it can be **authored by an LLM as text**, which the earlier closure-based
model could not.

### The deliberation protocol (`protocol.rs`)

A deliberation turn is: `DeliberationRequest` (observation + Timeline digest
+ current program, all JSON) → a `Fn(&str) -> String` call →
`DeliberationResponse` (an optional revised `RuleProgram` + intent + actions
+ note, all JSON). That function signature *is* a provider turn — prompt in,
completion out. `ProtocolDeliberator` adapts any such function into the
harness's `Deliberator` trait, so a real `jcode-provider-*` runtime drops in
unchanged; `demo_provider.rs` is a scripted stand-in that reasons only from
the JSON observation (never the hidden rules) and emits the world program as
JSON. `schema demo protocol` runs ToggleMaze discovery to a certified,
zero-misprediction WIN with the world program authored entirely over the
protocol.

### Sessions persist (`session.rs`)

`Session::capture` / `restore_*` round-trip a running session (Timeline +
program) through JSON, so it survives a turn or process boundary — the
prerequisite for a jcode agent that deliberates across many turns.
Environments implement `executor::Environment`
(`reset`/`step`/`actions_taken`/`legal_actions`) — grid games here, but any
tool-mediated world with observable state fits (a build/test cycle is an
environment whose observations are command outputs).

Per self-dev conventions (AGENTS.md): small focused commits, `cargo check`
iteration, clippy clean, full build at the end. The crate depends only on
`serde`/`serde_json`, which are already in the jcode lockfile — so this adds
**zero** new crates to the workspace supply chain (a deliberate relaxation of
the initial std-only choice, justified because serialization is required to
make the model LLM-authorable and sessions persistable).

## Using it

```
cargo build -p schema --release
target/release/schema demo toggle    # mechanism discovery: probe → halt →
                                     # revise → certify → 0-misprediction WIN
target/release/schema demo protocol  # the SAME discovery, driven entirely via
                                     # the JSON deliberation protocol — the
                                     # world program authored as text
target/release/schema demo cart      # representation revision: the dock
                                     # counterexample forces cart-as-board-cell
                                     # unification; the reflow cell is learned
                                    # from the observed diff
target/release/schema selftest      # both demos, exit code 0 on success
```

`cargo test -p schema` runs the ported criterion suite: append-only
timeline; exact-vs-first-mismatch backtests; halt/discard/counterexample
semantics with an environment action-count proof; planner isolation;
ToggleMaze end-to-end discovery; CartDock representation revision learning
the hidden gate cell it was never shown.

## Honest scope

The provider-backed deliberator is a seam, not shipped code — wiring a live
jcode provider session into the seat needs credentials and a runtime this
crate deliberately does not depend on. Demo environments are stand-ins with
the same observable character as the paper's games, not ARC-AGI-3 itself.
