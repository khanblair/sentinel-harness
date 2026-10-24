# Sentinel Docs

Sentinel is a Rust harness that runs many coding agents in one shared working tree without collisions, verifies every "done" claim, and is fully controllable from a phone over Tailscale.

## Doc Map

| Doc | Question it answers |
|---|---|
| [project-overview.md](project-overview.md) | What is this, in one page? |
| [product-scope.md](product-scope.md) | What exactly are we building, for whom, and why? |
| [architecture.md](architecture.md) | How is the system built? |
| [project-structure.md](project-structure.md) | Which folders and files exist, and what is each for? |
| [build-plan.md](build-plan.md) | What do we build, in what order, and when is it done? |
| [code-standards.md](code-standards.md) | How is code written and reviewed? |
| [development.md](development.md) | How do I run, test, and ship it? |
| [library-docs.md](library-docs.md) | Which libraries are approved, and which are banned? |
| [ui-tokens.md](ui-tokens.md) | What are the exact design values? |
| [ui-rules.md](ui-rules.md) | How must the UI look, read, and behave? |
| [ui-registry.md](ui-registry.md) | Which UI components exist? |
| [mobile.md](mobile.md) | How does the phone experience work? |
| [progress-tracker.md](progress-tracker.md) | Where is the project right now? |

## Reading Order

**Humans:** project-overview, product-scope, architecture, build-plan, progress-tracker.

**Interactive prototype:** `design/sentinel-prototype.html` shows the UI, layout, settings, setup, and the user flow, with simulated data. It is the visual reference for the three UI docs. `design/site-structure.html` shows the structure of the how-to site that will be built on GitHub Pages.

**AI agents:** project-overview, code-standards, architecture, project-structure, library-docs, build-plan, progress-tracker. Add ui-tokens, ui-rules, ui-registry, and mobile before any UI task.

## Rule for Keeping Docs True

1. A change that alters behavior, structure, a library, or a design value updates the matching doc in the same change.
2. Every work session ends with an update to `progress-tracker.md`.
3. `cargo xtask doc-check` fails the build when a file listed in `project-structure.md` is missing or an unlisted file exists.
4. The doc that owns a fact is the only place that states it. Other docs link to it.
5. Docs follow the writing rules in `project-overview.md`. `cargo xtask slop-check` enforces them.
6. A change to the UI updates `design/sentinel-prototype.html`, `ui-tokens.md`, `ui-rules.md`, and `ui-registry.md` together. A change to the prototype updates those docs in the same change.
