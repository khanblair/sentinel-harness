# Code Standards

## Repo Layout

See [project-structure.md](project-structure.md). One Cargo workspace. Crates under `crates/`. Repo automation in `xtask/`.

## General Rules

1. Rust only. No Python, Node, or shell scripts in the repo except fixtures.
2. Small functions. One job each.
3. Name things for what they are. No `data`, `info`, `handler`, `manager`, `util`.
4. Errors are values. Return `Result`. Add context.
5. No hidden state. Pass dependencies in.
6. Comments explain why, never what.
7. A TODO needs a task ID: `TODO(P3-05)`. Bare TODOs fail the check.
8. Money is integer micro-dollars. Never floats.
9. Time uses the `time` crate in UTC.
10. Never call `git worktree`. `ban-check` fails the build.
11. Anything fed back to a model is compacted first. Never pass a raw full log.
12. Lesson notes hold typed front matter only. Task titles and backlog text never enter lesson notes, the Telegram messages, or comments. Notes a user wrote are never rewritten by code.

## Rust Rules

| Area | Rule |
|---|---|
| Edition and toolchain | Pinned in `rust-toolchain.toml` |
| Formatting | `rustfmt` defaults plus `rustfmt.toml` |
| Lints | `clippy` with `-D warnings`. Pedantic group on, a short allow list in `clippy.toml` |
| Unsafe | Forbidden (`#![forbid(unsafe_code)]`) |
| Panics | No `unwrap` or `expect` outside tests. Startup invariants may use `expect` with a message |
| Errors | `thiserror` in libraries. `anyhow` only in the binary |
| Async | `tokio`. Never block the runtime. Use `spawn_blocking` for SQLite and CPU work |
| Cancellation | Every long task selects on the kill signal |
| Ownership | Prefer owned values and clones over lifetime puzzles. Optimize when measured |
| Types | Newtypes for ids. Enums for states. No stringly typed status |
| Public API | Small. `pub(crate)` by default |
| Logging | `tracing` with fields. No `println!` outside the CLI |

## UI Code (Leptos)

- One component per file, named as in `ui-registry.md`.
- Props are typed. No untyped JSON in components.
- Styles use tokens from `ui-tokens.md` through CSS variables. No raw hex in components.
- No inline copy that breaks `ui-rules.md` writing rules.
- Appearance preferences are stored per device and applied as CSS variables on the root. Components read tokens only.
- Sizes are in `em` so the text size setting scales the whole interface.
- A UI change updates `design/sentinel-prototype.html`, `ui-tokens.md`, `ui-rules.md`, and `ui-registry.md` in the same change.
- Slash commands are parsed in one place (`slash.rs`). Each command maps to one API route. A command never does more than its route.

## Styling

CSS variables only for values that tokens define. Class names follow `block__element--modifier`. No `!important`.

## Testing and Coverage

| Rule | Detail |
|---|---|
| Runner | `cargo nextest` |
| Coverage | `cargo llvm-cov`. Target 80% lines for core, reserve, tree, verify. No target for UI glue |
| Every bug | Gets a failing test first |
| Test names | State behavior |
| Assertions | Every test asserts something real |
| Property tests | Scope math and wave building |
| No sleeps | Use channels and fake clocks |
| Real tools | Only in the dev-PC check task and the opt-in nightly smoke workflow on a self-hosted runner. Never in `ci.yml` |
| Brain | A test seeds a task title and fails if it reaches a lesson note. A test edits a note by hand and checks Sentinel keeps the edit |
| Telegram | A test checks that Summary messages hold no file contents, keys, or note text, and that expired buttons fail |
| Compaction | A test feeds a huge log and checks the retry prompt cap |

## Security Rules

- Bind only to Tailscale. Test refuses `0.0.0.0`.
- Secrets only in the OS keychain.
- Redact keys and tokens in logs. Test with a seeded secret.
- Every route checks the device token.
- No route runs arbitrary commands, edits files, or edits tests.
- The cap can be lowered, never raised, by any route.
- Validate all path input. Reject `..` and absolute paths outside the repo.

## Performance Rules

- Budgets in `architecture.md` are tests where possible.
- No allocation in the reservation hot path beyond the path set.
- Batch SQLite writes in a transaction.
- Cap output buffers per agent. Spill to the log file.
- UI virtualizes long lists.

## Git Workflow

- Small commits with a task ID: `P2-02: checkpoint with private index`.
- Branch per phase task, merged when `cargo xtask ci` passes.
- Never rewrite history on shared branches.
- This is the project's own workflow. Sentinel itself never creates worktrees.

## Code Smells

### Watch List and Limits

| Smell | Limit | Caught by |
|---|---|---|
| Long function | 40 lines | `smell-check` |
| Big file | 400 lines of code. Test modules and files under `tests/` do not count | `smell-check` |
| Many parameters | 4 (use a struct beyond) | clippy |
| Deep nesting | 3 levels | clippy cognitive complexity and `smell-check` |
| Large enum match with `_` catch-all hiding cases | Avoid | clippy `wildcard_enum_match_arm` |
| Duplicate code | 3 or more copies of a 6-line block | `xtask dup-check` |
| Needless `clone` | Avoid in hot paths | clippy |
| Dead code | None | compiler, `cargo machete` |
| Magic numbers | Name them | clippy |
| Unused dependencies | None | `cargo machete` |
| Data clumps | Same 3 or more parameters travel together in 2 or more signatures. Group them in a struct | `smell-check` |
| Shotgun surgery | One change touches more than 5 non-test files | Review. Checkpoint stats show file counts |
| Divergent change | One module changed for unrelated reasons (3 unrelated task IDs in its history). Split it | Review |
| Feature envy | A method uses another type's fields more than its own. Move it | Review |
| Inappropriate intimacy | Public fields on types shared across crates, except plain data in `sentinel-core` | `smell-check` public field scan |
| Message chains | 3 or more chained field or accessor calls such as `a.b().c().d()`. Iterator, builder, `Option`, and `Result` chains are exempt | `smell-check` |
| Switching on codes | `match` on a status string or integer code. Use an enum | clippy and review |

### Review Checklist

1. Does each function do one thing?
2. Can a state be wrong by type? If yes, use an enum.
3. Is every error handled or passed with context?
4. Is every new file listed in `project-structure.md`?
5. Does a test fail without this change?
6. Did a doc change where behavior changed?
7. Do three parameters keep traveling together? Group them.
8. Does one reason for change touch many files? Check for shotgun surgery.
9. Does a method envy another type? Move it.

### Refactoring Rule

Refactor only with tests green before and after. A refactor commit changes no behavior and is named `refactor: ...`. Never edit a test inside a refactor commit.

### Smells in Agent Code

- The verifier reports smells and slop in agent diffs as quality notes. They are warnings. They never change VERIFIED and never trigger a retry on their own.
- Worker prompts include the size limits as guidance.
- Agents are told to fix smells only inside their own scope.
- The report lists notes per task so the owner can decide later.
- Reason: a smell is not a bug, and a retry costs money and time.

## AI Slop

### In Code

| Slop | Do instead | Caught by |
|---|---|---|
| Comment restating the next line | Delete it, or explain why | `slop-check` and review |
| Names like `data`, `result2`, `helper` | Name by meaning | `smell-check` name list |
| Needless trait or generic with one user | Use the concrete type | review |
| Catch-all that swallows errors | Return the error | clippy, review |
| Invented crate APIs | Compile, read the docs | compiler |
| Placeholder code (`todo!()`, `unimplemented!()`) | Finish it or leave out | `smell-check` |
| Unused parameters kept "for later" | Remove | clippy |

### In Tests

| Slop | Do instead | Caught by |
|---|---|---|
| Test with no assertion | Assert the behavior | `smell-check` |
| Test that mirrors the implementation | Test inputs and outputs | review |
| Snapshot of everything | Snapshot only stable output | review |
| Assertion that always passes | Remove or fix | review |

### In Writing

Docs, UI copy, commit messages, and comments follow the banned list in [project-overview.md](project-overview.md). Caught by `slop-check`.

## Rules for AI Agents

1. Read the docs in the order given in `project-overview.md` before changing anything.
2. Work one task ID at a time. Stay inside its files.
3. Do not edit existing tests to make code pass. Fix the code.
4. Do not add a crate that `library-docs.md` does not approve. Propose it in the tracker first.
5. Do not invent APIs. Check the crate docs.
6. Update `project-structure.md` for every file you add.
7. Update `progress-tracker.md` at the end of a session.
8. If a rule blocks the task, stop and log an open question. Do not work around it.
9. No filler comments, no marketing language, no banned words.
10. Never copy task titles or backlog text into lesson notes, Telegram messages, or comments.
11. Do not add free text to a lesson note. Keep the typed fields. Never send a note or file contents to a chat.
