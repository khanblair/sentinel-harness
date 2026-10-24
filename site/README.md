# Sentinel how-to site

This folder holds the how-to site. It is an mdBook project. The feature scope is in `docs/product-scope.md` (F22), and the site map is in `design/site-structure.html`.

## Build locally

Install mdBook once with `cargo install mdbook`. Then run these commands from the repository root:

1. Build the site: `mdbook build site`. The output goes to `site/book/`.
2. Copy the prototype into the built site: `mkdir -p site/book/prototype && cp design/sentinel-prototype.html site/book/prototype/`. The Try it page loads it from there. The Pages workflow does not copy it yet. This step runs after `mdbook build` in that workflow.
3. Preview the site with live reload: `mdbook serve site`. Run step 2 against the served output too, or the Try it page stays empty.
4. Check the relative links: `sh site/check-links.sh`. It prints each missing target and exits with 1.
5. Run the slop check: `cargo xtask slop-check`. It scans the `.md` files in this folder.

## Where the pages are

- `book.toml` holds the title, the theme, and the repository link.
- `theme/sentinel.css` maps the colors and fonts from `docs/ui-tokens.md` to mdBook. Light is the default. The dark themes (navy and coal) use the dark values.
- `src/SUMMARY.md` sets the order and the menu. A page must be listed there to appear in the book.
- `src/` holds one folder per top-level section. Each section has an `index.md` page and its sub pages.
- `theme/figures.js` turns each `<figure class="fig" data-fig="ID">` into a live view of the prototype. The prototype file is copied to `prototype/` when the site is built, and the view is its `?fig=ID` screen. There are no screenshot files.
- To add a view, define `ID` in the `FIGS` table in `design/sentinel-prototype.html` (its setup, the crop, and the marks that get outlines and arrows), then add the figure block to the page.

## Page template

Every content page uses the same parts, in this order:

```
# Page title

## What it is

One sentence that says what the feature is.

## Steps

1. One action per step.
2. Next action.

## See it

<figure class="fig" data-fig="view-id" data-ratio="16 / 10" data-title="What the view shows">
<figcaption>What the view shows.<ol><li>The first marked part.</li><li>The second marked part.</li></ol></figcaption>
</figure>

Leave the section out when a picture adds nothing (overview pages, tables).

## Tip

One line of advice.

## Related

- [Other page](other-page.md)
```

Link to other pages with relative paths. The image path starts with `../images/` on pages inside a section folder. The Home page uses `images/` because it sits at the top of `src/`.

## Writing rules

Write short sentences, plain words, and active voice. Do not use em dashes, emoji, or the banned words in `docs/project-overview.md`. Use the terms from `docs/product-scope.md`.
