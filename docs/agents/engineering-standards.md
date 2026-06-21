# Engineering Standards & Operating Contract

These standards are **enforced on every coding-agent session** working in this
repository. The root `AGENTS.md` carries the condensed, always-on summary; this
file is the authoritative, full version. Load it before any non-trivial Rust
change or refactor.

> Scope note: The upstream system prompt also covered C# and Haskell. Lynx is a
> **Rust + TypeScript/React** codebase, so only the Rust and language-agnostic
> directives are retained here. C#/Haskell-specific rules are intentionally
> omitted.

## 1. Core Operating Rules

1. **Autonomy with documented assumptions.** Operate without asking for
   permission to proceed on routine steps. When faced with ambiguity, make the
   most reasonable technical assumption, document it in your output, and
   continue. Reserve questions for genuinely destructive or irreversible
   actions (see `AGENTS.md` → Boundaries).
2. **State management.** Maintain and actively update a TODO list for any
   multi-step task so you never lose your place in complex workflows.
3. **Context optimization.** When a subtask risks overwhelming the context
   window, delegate it to a read-only exploration subagent rather than loading
   everything into the main thread.
4. **Termination protocol.** Do not stop silently. When the objective is
   verifiably complete (build, lint, and tests green), emit an explicit final
   status message and halt.

## 2. Universal Engineering Directives

- **Make invalid states unrepresentable.** Use the type system to prevent
  invalid states at compile time rather than validating at runtime.
- **Ruthless refactoring — but data integrity is sacred.** You may break,
  rename, or delete *code* interfaces freely (no backward-compatibility
  requirement for internal APIs) and prune dead code aggressively. **However,
  the database is exempt from "break it and move on":** every schema-affecting
  change MUST ship a correct, idempotent, non-destructive migration that
  preserves existing data. See `docs/agents/database.md`.
- **Aggressive modularization (500-line soft limit).** No source file should
  exceed ~500 lines. Split files approaching the limit into cohesive submodules.
- **Idiomatic error handling.** Never swallow errors. Use `Result`/`Option`
  with `?`, `thiserror` for typed errors, and `anyhow` at boundaries.

## 3. Rust-Specific Execution

- **Design for the borrow checker.** Pre-calculate ownership and lifetime
  hierarchies; design data flow so the compiler is satisfied by the
  architecture, not by escape hatches.
- **Avoid reflexive `.clone()` / `Rc` / `Arc` / `Copy`.** Do not reach for these
  merely to appease the borrow checker. If lifetimes clash, redesign the data
  flow. (Legitimate shared-ownership needs — e.g. `Arc<Pool>` shared across
  async tasks, as already used in `src/storage/` — remain acceptable when the
  ownership requirement is real.)
- **Zero-cost abstractions.** Prefer traits, generics, and monomorphization over
  dynamic dispatch (`Box<dyn _>`) unless runtime polymorphism is genuinely
  required (the `Storage` trait in `src/storage/trait_def.rs` is a deliberate,
  justified use of dynamic dispatch).
- **Concurrency.** Favor message passing and the existing async primitives over
  ad-hoc shared mutable state.

## 4. Output Contract (per step)

For each step of a non-trivial task, structure your progress as:

- **Current State** — what was just completed.
- **Assumptions Made** — independent technical decisions.
- **TODO Update** — items added or checked off.
- **Architectural Plan** — when writing/modifying code, briefly state:
  1. *Resource/Effect strategy* — ownership, lifetimes, borrowing.
  2. *Type-state / PLT plan* — how the type system blocks invalid states here.
  3. *Pruning targets* — legacy code being deleted or large files being split.
  (Write `N/A` when not coding.)
- **Next Action** — the exact command, edit, or subagent being executed now.

## 5. Definition of Done

A task is complete only when **all** of the following pass locally
(see `docs/agents/testing.md` for commands):

- `cargo fmt --all -- --check`
- `cargo clippy --all-features -- -D warnings -A clippy::too_many_arguments`
- `cargo test` and the SQLite integration suite
- Frontend `npm run build` and `npm run lint` (when frontend changed)
- Any schema change validated against **both** SQLite and PostgreSQL backends
