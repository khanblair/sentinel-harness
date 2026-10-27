# Collision Course write-up: Sentinel

Solo entry by Khan Blair. Repository: https://github.com/khanblair/sentinel-harness. Run it with one command, shown in the README under "Run a backlog from the terminal".

## What it does

Sentinel is a harness written in Rust. It runs several coding agents in one shared git working tree without collisions.

- **Plan.** The strongest agent turns the backlog into tasks. Every task gets a scope: the files it may write, plus one new test file. A plan that cannot be read is said in the stream with its reason, and the same agent is asked once more.
- **Reserve.** Tasks that list the same file never run together. A write guard watches the tree and restores any write to a path that no running task holds.
- **Waves and a barrier.** Tasks with different files run in waves. After each wave the full suite runs. If it fails, each task is checked alone on the wave's starting tree, and only the task that breaks the suite goes back.
- **Verify.** A task is Verified only when the suite passes, its new tests fail without the change, no protected file changed, and a second, different agent approved the diff. Anything less is Partial, Blocked or Skipped, with the reason.
- **Traps.** Two tasks that contradict each other are both skipped, with the reason. A ticket that says it is finished is checked, not trusted. Files in `tests.lock.json` can never change.
- **Budget.** A hard cap in dollars: no new task starts at 70 percent, and workers are killed at 95 percent. Free models cost nothing, Claude reports its cost, and OpenRouter returns usage.
- **Evidence.** Every run writes `report.md`, `evidence.json`, `log.jsonl`, and `sentinel report --page` makes one HTML page.

`sentinel selftest` runs seven scripted bad agents (a collider, a tamperer, a liar, an overspender, a crasher, one that needs a file outside its scope, one that breaks the suite) and prints PASS or FAIL. It costs nothing.

## What it got wrong

These are real findings from the rehearsals on the practice repository, each now fixed unless it says otherwise. Rehearsal 8, with all of them fixed, called 9 of 12 tasks Verified for $0.25 in about 18 minutes, and the practice grader passed 10 of 12 (the two tasks that contradict each other were skipped on purpose).

- A failed suite sent every task of the wave back, although one task caused it.
- A task whose agent needed a file outside its scope lost its work to the guard and kept failing.
- A plan of 12 tasks was one long line that was cut at 4096 characters and could not be read.
- The planner was allowed to say "no request came through", and it did.
- Tasks had no test file in their scope, so none could be Verified. The practice repo also locks `test/`, so new tests go to `sentinel-tests/`.
- A free model as reviewer printed only colour codes, which counted as "did not approve".
- Tasks whose prerequisite failed were skipped, so five tasks never started.
- The OpenRouter account had no credit, and the harness did not pause that agent.
- The real git index went stale after checkpoints, so `git status` showed staged changes after a run. The index is now brought up to date after every wave.
- A CLI that reports its cost only at the end was counted by a time guess, and the guess stayed in the total: the first rehearsal showed an estimated $2.37, the real spend was far lower. A reported cost now replaces the guess.
- A later task that changed behaviour broke the own tests of an earlier task, and could not update them, so it failed three times. It may now update those tests on its next try.

## What is still weak

- The write guard is a file watcher. It cannot tell which agent wrote, so a CLI agent that writes into a file held by another running task is not caught. The OpenRouter loop checks its scope in its own file tools.
- Scopes come from an agent that reads file names, not file contents. A wide scope costs parallelism, and a narrow one costs a retry.
- The reviewer is strict. In the first rehearsal Sentinel called 2 tasks Verified while the practice grader passed 7. It under-claims, which is safe, but it leaves points on the table.
- Free models are slow, and a task that times out wastes its whole timeout.
- Tasks that list the same file run one after another. On the practice repository that was nearly every task, so no wave held more than one task and the parallel share stays low. Safe, but it leaves collision-efficiency points on the table.
- The reviewer of a Claude task can be the other Claude agent (another model string, same model family). It is a second agent, but not an independent one.

## What I would fix next

1. Let finished tasks free their slot at once, instead of waiting for the whole wave.
2. Use the Claude hook and the OpenRouter file tools to enforce the scope per task, not per tree.
3. Read file contents, not only names, when planning scopes.
4. Ask the reviewer for a verdict with the failing check, so a rejection can be fixed in the next try.
