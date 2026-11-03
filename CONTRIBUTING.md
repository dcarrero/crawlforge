# Contributing

Thanks for looking. The most useful thing you can contribute is a **rule**, and this document
exists mostly to explain what a rule needs before it can be merged.

Read [`docs/CONVENTIONS.md`](docs/CONVENTIONS.md) first. It carries the decisions the code takes
for granted, and it will answer most of the "why is this like that" questions before you ask them.

## The one requirement that is not negotiable

**A rule arrives with its fixture, its test and its text in English and Spanish.**

A rule with no test case is a rule nobody can trust — including whoever wrote it. This project has
shipped three systematic false positives, and every one of them was found by crawling a real site
rather than by reading code. That is why the bar is where it is.

There is a test that fails when a rule has no fixture, so this is not a matter of reviewer
discipline: it will not merge.

## Adding a rule

1. **Declare its `RuleMeta`** as a `pub static` in the module for its category. **The ID is
   forever**: a historical diff between two crawls depends on it not changing meaning.
   Its name and description go in `crates/crawlforge-rules/locales/en/rules.ftl` and
   `crates/crawlforge-rules/locales/es/rules.ftl`, one message
   per rule ID (see [Adding or improving a language](#adding-or-improving-a-language)); every
   other language that has been translated needs it too, or the completeness test fails — write
   the English there and say so in the pull request, and a translator will pick it up.
2. **Implement `PageRule` or `SiteRule`** on an empty struct. `PageRule` is evaluated during the
   crawl, on one page at a time, and must be cheap. `SiteRule` runs once at the end, in SQL over
   the whole store — use it when the answer needs the complete picture: duplicates, orphans,
   redirect chains.
3. **Register it** in the module's `page_rules()` or `site_rules()`.
4. **Write its fixture** in `fixtures/<RULE-ID>.html`, or `fixtures/<RULE-ID>/` when the case needs
   several pages.
5. **Write its test.**

`MetaTitleMissing` and `MetaTitleDuplicate` are the two to copy from: one page-level, one
site-level, each with its metadata, its evaluation and its tests.

## Adding or improving a language

Every text the user reads lives in [Fluent](https://projectfluent.org/fluent/guide/) files, two per
language, compiled into the binary:

```
crates/crawlforge-rules/locales/<code>/cli.ftl     messages of the command line: report, inspect, diff…
crates/crawlforge-rules/locales/<code>/rules.ftl   the name and description of every rule
```

Both live in `crawlforge-rules`, the CLI's messages too, because the crates are published on
crates.io and a published crate contains only its own folder. `crawlforge-rules` embeds both files
and the CLI takes its texts from it. Paths below are relative to `crates/crawlforge-rules/`.

The codes are `en`, `es`, `de`, `fr`, `it`, `nl`, `tr`, `pt`, `ja` and `pl`. **English is the
source and the authority**: when a translation and English disagree, English is right and the
translation is fixed. A folder whose files have no messages (only the header comment) is a
language not translated yet: `--lang <code>` is accepted and shows English. As soon as a file has
one message it must have all of them.

To translate one:

1. Copy `locales/en/cli.ftl` and `locales/en/rules.ftl` over the two files of your folder, and
   translate the text after each `=`. Translate both files in the same change: a language with
   one file translated and the other not fails the test.
2. Run the completeness test and fix what it lists:

   ```bash
   cargo test -p crawlforge-rules -p crawlforge -- translated
   ```

   It names every problem at once, with file and key: a key missing, a key English does not have,
   a variable that differs from English, a syntax error, a suffix that lost its leading space.
3. Look at the result: `crawlforge rules --lang <code> --detail`, and `report`/`inspect` on a
   crawl file.
4. Run `tools/verificar.sh` before opening the pull request. Translating `rules.ftl` adds your
   language to `rules.json`; the script regenerates it and asks you to commit it.

What the parts of a message mean:

- **The key** (`crawl_finished`, `META-TITLE-MISSING`) is an identifier. Never translate or
  rename it.
- **Variables** (`{ $urls }`, `{ $path }`) are filled in by the program. Keep every one English
  has, with the same name; move them wherever your grammar needs them. Do not add new ones.
- **Attributes** (`.description` under each rule) are part of the message: translate their text,
  keep the line that starts with `.description =`.
- **Plurals** use selectors over the [CLDR plural categories](https://www.unicode.org/cldr/charts/latest/supplemental/language_plural_rules.html)
  of your language:

  ```ftl
  # English: pages = { $n -> [one] { $n } page *[other] { $n } pages }
  pages = { $n ->
          [one] { $n } strona
          [few] { $n } strony
          [many] { $n } stron
         *[other] { $n } strony
      }
  ```

  English uses `[one]`/`*[other]`; Polish needs `[one]`, `[few]`, `[many]` and `*[other]`;
  Japanese needs no selector at all. You may add a selector where English has none, as long as
  the selector is a variable English uses and it holds a count (a selector over text always takes
  the `*` default). The variant marked `*` is mandatory and is the one used when no other
  matches.
- **Spaces at the ends** are trimmed by Fluent. A message that starts or ends with a space in
  English starts with `{" "}` there, because it is glued to another text (`crawl_failed_suffix =
  {" "}({ $n } failed)`). Keep it.
- **Literal text** that Fluent would read as syntax is written as a string: `{"{"}`, and `{"["}`,
  `{"*"}` or `{"."}` at the start of a continuation line. A continuation line is indented; to keep
  spaces at its start, write them as `{"    "}`.
- **What not to translate**: rule IDs, command names and flags (`crawlforge resume`,
  `--max-external`), file names, HTTP status codes and anything in backticks that is a value
  (`noindex`, `max_urls`). Backticks and `<tags>` in descriptions are literal: keep them.
- Comments (`#`, `##`, `###`) are context for you and are not shown. Many are in Spanish, from when
  the code was; English is the reference text.

Numbers are formatted by the program with your language's separators (`1.234` in German, `1 234`
in Polish); do not write them by hand.

## Three things that will get a rule sent back

**It fires on something it does not name.** A rule called "orphan page" that does not check the
thing is a page will report images. That one cost 1,867 false positives out of 1,912 before anyone
noticed.

**It asserts something the data cannot support.** If a crawl was truncated, the link graph has
holes, and any rule whose conclusion depends on a complete graph must stay silent rather than
guess. `REQUIERE_GRAFO_COMPLETO` is the list of rules that do this — add yours if it belongs there.

**Its severity does not match reality.** A `critical` that is right every time and useful none
teaches people to skip the severity column. `noindex` on a tag archive is deliberate configuration,
not an emergency; `noindex` on the home page is.

## Before opening a pull request

```bash
tools/verificar.sh
```

It runs the tests, clippy with warnings denied, the fixture bench and the performance regression in
release. That last one **only asserts in `--release`** — a regression once shipped because it was
run in debug.

If you touched a rule, also crawl a real site with it and check that what it says is true. Nothing
in the test suite substitutes for that.

## Conventions in one paragraph

Decision comments in Spanish; identifiers, function names, types, database columns, rule IDs and
commit messages in English. Test names in Spanish, like the comments — they describe a claim, not
an API. No `unwrap()` or `expect()` outside tests and `main`. Conventional Commits. Migrations are
numbered and forward-only, and a published one is never edited: a year-old crawl file must still
open.

## Versioning

Do not bump the version in a pull request — it is set when a release is cut. But it helps to know
what your change implies, because it decides when it ships:

- **A new rule, or a rule that starts or stops firing, is a minor bump.** Someone's CI gate depends
  on that behaviour, so it is never a patch.
- **A change to the crawl file schema that older builds cannot open is major.**

Rule IDs never change meaning. A historical diff between two crawls depends on it.

## Cutting a release

For the maintainer. A release is a tag `vX.Y.Z` on `main`; the release workflow (generated by
`dist` into `.github/workflows/release.yml`) builds the binaries, publishes the Homebrew formula
and, through `.github/workflows/publish-crates.yml`, publishes `crawlforge-rules`,
`crawlforge-core` and `crawlforge` to crates.io, in that order.

1. Bump the version in **four places**, all to the same number:
   - `[workspace.package] version` in `Cargo.toml`;
   - `version` of `crawlforge-core` and of `crawlforge-rules` in `[workspace.dependencies]` of the
     same file — the published packages keep only this, not the path;
   - `version` in `package.json` (the web installs this repository by tag).

   `cargo test` fails while any of them disagrees (`tests/workspace_versions.rs` in the CLI), and
   `tools/verificar.sh` regenerates `meta.json` and asks you to commit it.
2. Write the `CHANGELOG.md` entry, run `tools/verificar.sh`, commit, tag and push the tag.
3. If dist changes, regenerate the workflow with `dist generate --mode=ci` (never edit
   `release.yml` by hand: the plan job fails when it differs from what dist would write).

The crates.io job authenticates with **Trusted Publishing** (OIDC), so there is no token in the
repository's secrets. It skips any crate whose version is already on crates.io, so re-running a
release is safe. It needs a one-time setup that only the owner of the crates can do:

1. **Publish the first version by hand**, from a clean checkout of the release commit, logged in
   with `cargo login` (crates.io → Account Settings → API Tokens, scope `publish-new`):

   ```bash
   cargo publish -p crawlforge-rules -p crawlforge-core -p crawlforge
   ```

   Cargo publishes them in dependency order and waits for each one to reach the index. On a cargo
   older than 1.90, run three `cargo publish -p <crate>` in that order.
2. **On crates.io, for each of the three crates**, Settings → Trusted Publishing → Add: publisher
   GitHub, owner `dcarrero`, repository `crawlforge`, workflow filename **`release.yml`** (the
   workflow that starts the run, not the reusable `publish-crates.yml`), environment empty.
3. Optionally revoke the API token from step 1, and in each crate's settings require Trusted
   Publishing so that a leaked token cannot publish.

Until step 2 is done for all three, the crates.io job of a tagged release fails at its
authentication step. The GitHub release and the Homebrew formula are already out by then (they do
not depend on it), but the run ends red and its `announce` job does not run. Publish that version
by hand as in step 1 and re-run the failed jobs: the crates.io job then finds the versions published
and skips them.

## Performance

Two invariants that are easy to break by accident, both measured:

**Nobody writes to SQLite but the writer thread.** The engine only holds a sender.

**The writer's channel is bounded on purpose.** Backpressure is the feature. With an unbounded
channel the queue becomes the store and peak memory went from 170 MB to 387 MB.

And when you add a site rule, check `EXPLAIN QUERY PLAN` **against a real crawl**, not a fixture.
At a thousand rows every query plan looks fine; at two hundred thousand a missing index cost eight
hours in a single rule.
