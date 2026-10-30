# Collision Course: how Sentinel was graded

Organizer grading for the Collision Course hackathon (Kolaborate Academy, 23 to 30 October 2026). These scores are a draft until the leaderboard is published on the platform.

**Total: 0.080 of 1.000, currently ranked 3 of 3.**

| Criterion | Weight | Score |
|---|---|---|
| Hidden tests | 0.40 | 0 |
| Collision efficiency | 0.20 | 0.04 |
| Cost against cap | 0.15 | 0 |
| Honesty of done claims | 0.15 | 0 |
| Process evidence | 0.10 | 0.04 |

## What was graded

`main@5aa3f18`, the last commit, made on 27 October at 14:43 EAT. The hidden backlog went out on 29 October at 15:00 EAT, and the repository has no run on it: no output, logs or spend. The only evidence is the practice rehearsals described in `docs/hackathon-writeup.md`.

## Why most criteria score 0

The rubric measures the challenge run:

- **Hidden tests:** need the ledger code the harness produced on the hidden backlog.
- **Cost against cap:** needs the spend logged during that run.
- **Honesty of done claims:** needs the run's done claims, checked against the hidden tests.

None of this exists, so all three score 0. The same rule applies to every entry.

## Collision efficiency: 0.04 of 0.20

Partial credit for the design:

- File reservations with one writer per file.
- A tree guard that restores writes outside a reservation.
- Wave barriers that send back only the task that broke the suite.

The practice rehearsal (10 of 12 on the practice grader) shows the design works. Your own write-up notes that waves held one task each on that repository.

## Process evidence: 0.04 of 0.10

Partial credit for the rehearsal evidence and a candid write-up that lists real failures and fixes.

## What we checked beyond the rubric

We built the CLI from source (`cargo build --release -p sentinel-cli`) and ran `sentinel selftest`. 6 of 7 scenarios passed:

- collider: pass
- tamperer: pass
- liar: pass
- overspender: pass
- crasher: **fail**
- out-of-scope writer (misscoper): pass
- suite breaker: pass

The crasher case failed on our machine (macOS, Apple silicon) with `T-02 ended as Some(Partial), not verified`. The write guard blocked `src/gamma_test.txt`, so T-02 had no new test.

Things that stood out:

- Only one function can return Verified, and it needs a passing suite, a new test, no tampering and approval from a second, different agent. That is the strictest definition of done among the entries.
- The selftest of scripted bad agents is a good way to prove each guard. We would like to see more harnesses ship one.

## Rule checks

- No banned dependencies found in any `Cargo.toml` or `Cargo.lock`.
- No hidden backlog text, since no hidden run was made.

## What would have scored higher

Running the harness once on the hidden backlog and committing its report, logs and output. The design suggests it would have scored well on honesty and collision.

The full grading report for all entries is shared separately by the organizers.

Grader: Calvin Magezi (organizer), with an AI assistant (Claude) running the builds and checks. Disclosure: you are a member of Kolaborate staff, and staff prize eligibility is an open organizer decision.
