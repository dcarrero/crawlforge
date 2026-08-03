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

## Performance

Two invariants that are easy to break by accident, both measured:

**Nobody writes to SQLite but the writer thread.** The engine only holds a sender.

**The writer's channel is bounded on purpose.** Backpressure is the feature. With an unbounded
channel the queue becomes the store and peak memory went from 170 MB to 387 MB.

And when you add a site rule, check `EXPLAIN QUERY PLAN` **against a real crawl**, not a fixture.
At a thousand rows every query plan looks fine; at two hundred thousand a missing index cost eight
hours in a single rule.
