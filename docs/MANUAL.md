# CrawlForge Manual (CLI)

> Versión en español: [leer el manual en español](es/MANUAL.md)

> The tool is command-line only today. A graphical interface is planned and does not exist yet.
>
> This does not repeat `--help`, which is already complete and in English. This is **which
> command to use in each real situation**, with real sites.

---

## 0. Before you start

The binary is already installed and on the `PATH`:

```bash
crawlforge --version
```

If it does not respond, reinstall from the repository:

```bash
cd ~/Desarrollos/proyectos/proyectos-mini/crawlforge
cargo build --release -p crawlforge-cli && cp target/release/crawlforge ~/.cargo/bin/
```

**The tool's output is in English.** That is the source language; Spanish exists where there is
real text to translate — the rule catalog and the reports — and you ask for it with `--lang es`:

```bash
crawlforge rules  --lang es
crawlforge report crawl.sqlite --lang es
```

To avoid repeating it on every command, in your `~/.zshrc`:

```bash
export CRAWLFORGE_LANG=es
```

---

## 1. The flow of a normal day

Four commands. If you only read one section of this manual, make it this one.

```bash
# 1. Crawl
crawlforge crawl https://tusitio.com/

# 2. See what came out, right here
crawlforge report crawl-tusitio-com.sqlite --lang es

# 3. Take it to someone who doesn't use a terminal
crawlforge export crawl-tusitio-com.sqlite --format xlsx --out auditoria.xlsx

# 4. A report to paste into a ticket or send by email
crawlforge report crawl-tusitio-com.sqlite --format html --out informe.html --lang es
```

Every command ends by telling you which one comes next, ready to copy. There is nothing to
memorize.

### The files that show up

| File | What it is |
|---|---|
| `crawl-<site>.sqlite` | **The crawl.** Everything is here: URLs, links, images, findings, headers. It is a plain SQLite file; any viewer can open it. |
| `crawl-<site>.prev.sqlite` | The **previous** crawl, set aside automatically when you crawl again. It is what `diff` needs. Don't delete it. |

A crawl can be revisited months later without crawling again: `report`, `export` and `diff` work
on the file, not on the network.

---

## 2. Recipes

### Crawl a whole site

```bash
crawlforge crawl https://cliente.com/
```

It discovers the sitemaps, respects `robots.txt` and stops when it exhausts the site. It runs 5
concurrent requests, which is prudent for a WordPress. On your own, well-provisioned site:

```bash
crawlforge crawl https://tusitio.com/ --concurrency 10
```

**Do not raise the concurrency on third-party sites or shared hosting.** A server with per-IP
limits starts returning 503 after a few crawls in a row, and from then on the measurements are
worthless: what you are measuring is its defenses, not your site.

### Only part of the site

```bash
# Only the blog
crawlforge crawl https://cliente.com/ --include "/blog/"

# Everything except the admin area and the comment-reply URLs
crawlforge crawl https://cliente.com/ --exclude "/wp-admin/" --exclude "\?replytocom="
```

These are unanchored regular expressions: a plain string works as "contains". **`--exclude` wins
over `--include`**, and whatever is excluded is recorded as excluded in the report: it does not
vanish without a trace.

### A quick test, without crawling all 176,000 URLs

```bash
crawlforge crawl https://tumedio.com/ --max-urls 500
```

Careful with this one: **a truncated crawl cannot judge the link graph.** The tool knows it and
silences the rules that depend on having it complete — orphan pages, excessive depth — instead
of making up a verdict. If the report does not mention them, it is not that they are fine; it is
that they could not be evaluated.

### Audit before deploying

Over the built folder, without uploading anything anywhere:

```bash
crawlforge audit ./dist --base https://tusitio.com/
```

`--base` is mandatory for a reason: the site's absolute `canonical`s are compared against that
URL. With a fake one, the indexability audit means nothing, and the tool warns you if 80% of the
canonicals contradict what you told it.

It is the fastest mode of all — there is no network — and it is the one used in CI.

### Compare two crawls: did the deploy make anything worse?

```bash
crawlforge diff crawl-sitio.prev.sqlite crawl-sitio.sqlite --lang es
```

This is what Screaming Frog does not give you. A crawl is a snapshot; the diff tells you whether
yesterday's still holds. As a CI gate, make it fail if something serious shows up:

```bash
crawlforge diff antes.sqlite despues.sqlite --fail-on high
```

It exits non-zero if a new finding of severity `high` or worse appears. A
`--fail-on INDEX-NOINDEX` watches one specific rule.

### A password-protected site (staging)

```bash
CRAWLFORGE_AUTH='user:password' crawlforge crawl https://pre.cliente.es/
```

It is also accepted in the URL as a shortcut, though that way it stays in your shell history:

```bash
crawlforge crawl https://user:password@pre.cliente.es/
```

In both cases, **the credential is not stored in the crawl file** and never travels to any host
other than the seed's, not even if the site links to another domain. That is why `resume` also
needs the variable: the file deliberately does not carry it.

It also applies to `robots.txt` and the sitemaps — otherwise a protected staging would return
401 when they are fetched and the crawl would behave strangely without saying why.

### An exact list of URLs

```bash
crawlforge list urls.txt
```

A file with one URL per line. Useful for reviewing a specific set — the 40 landing pages of a
campaign — and it is also what makes a comparison with another tool fair: both receive exactly
the same set.

### The crawl was interrupted

```bash
crawlforge resume crawl-tumedio-com.sqlite
```

It continues exactly where it left off, with the configuration stored in the file itself. It
does not repeat what was already crawled. A finished crawl cannot be resumed: for that you crawl
again.

### Recurring crawls: save the configuration

```bash
cp docs/crawl-config.example.yaml cliente.yaml   # then edit it
crawlforge crawl https://cliente.com/ --config cliente.yaml
```

The file describes **the site**, the command line describes **the run**: flags win over the
YAML. A misspelled field is an error, not an option silently ignored.

### See what the tool checks

```bash
crawlforge rules --lang es                    # all 59, as a table
crawlforge rules INDEX-ORPHAN-PAGE            # one rule's card, by its ID
crawlforge rules --lang es --category canonical
crawlforge rules --lang es --detail           # with each rule's full explanation
crawlforge rules --format json                # the whole catalog as data, both languages
```

The JSON is for anything that consumes the catalog as data — a CI script checking that a
rule ID still exists, or a page generated from the catalog instead of copying it. It always
carries both languages, and its envelope states the catalog version and the rule count.

---

## 3. How to read the result

### In the terminal

`report` on its own gives the summary: how many URLs, how many indexable, and the findings
grouped by severity. It is for answering "how is this doing?" in ten seconds.

### The XLSX

```bash
crawlforge export crawl.sqlite --format xlsx --out auditoria.xlsx
```

Thirteen sheets, each with the header frozen and the autofilter on. Status codes are real
numbers, so a "greater than 399" filter works the way you expect. An empty status cell is a URL
that was recorded but never requested — an internal link beyond the crawl's reach, or an external
one when the check is off.

It is verified to open clean in Microsoft Excel 16, with no repair warning.

### The SQLite, if you want to go further

This is the advantage of a format that is not proprietary. Any question the report does not
answer, SQL does.

The three tables you will use 90% of the time:

| Table | What it holds | Key |
|---|---|---|
| `urls` | Every URL seen: `url`, `status_code`, `depth`, `content_type`, `response_time_ms` | `id` |
| `pages` | What was extracted from the HTML: `title`, `h1`, `word_count`, `canonical`, `is_indexable` | `url_id` → `urls.id` |
| `issues` | One finding per row: `rule_id`, `severity` | `url_id` → `urls.id` |

```bash
# The URLs that fail
sqlite3 crawl-cliente-com.sqlite \
  "SELECT url, status_code FROM urls WHERE status_code >= 400 ORDER BY status_code DESC LIMIT 20;"

# Which pages triggered a specific rule
sqlite3 crawl-cliente-com.sqlite \
  "SELECT u.url FROM issues i JOIN urls u ON u.id = i.url_id
   WHERE i.rule_id = 'CONTENT-H1-MISSING' LIMIT 20;"

# The slowest pages
sqlite3 crawl-cliente-com.sqlite \
  "SELECT url, response_time_ms FROM urls ORDER BY response_time_ms DESC LIMIT 10;"
```

There are ready-made views for the usual questions: `v_orphans`, `v_broken_links`,
`v_indexable_pages` and `v_issue_summary`.

Two warnings that save you a while: the column is **`status_code`**, not `status`, and it is
`NULL` for any URL that was recorded without being requested — an internal link the crawl never
reached, or an external one when `--no-external-check` is in play.

---

## 3.bis How to read a report

The summary is one line per rule, sorted by severity. Three things worth knowing to interpret
it:

### The site share

```
medium  META-TITLE-TOO-LONG  173,654  (80% of the site)
```

When a rule affects 40% or more of the pages, the line adds its share. **It is the difference
between a list of pages to fix one by one and a systemic problem**: 2,193 broken images are
2,193 fixes; 173,654 long titles across 80% of the site are a template or a publishing pattern,
and you fix them in one place.

### Template issues

```
high  ASSET-IMG-EMPTY-ALT-LINK  13 template issues (567 pages) + 90 more findings
      e.g. https://ejemplo.com/a · https://ejemplo.com/b
```

When the same defect appears for the same cause on many pages — the header logo, a footer link,
the `<h4>` of the author signature — it counts as **one issue**, with examples. The rows all
stay in the file: what changes is the report's count, not what is stored.

### See every URL for a rule

The summary never enumerates. For the full list:

```bash
crawlforge report crawl.sqlite --rule HTTP-404-INTERNAL
```

It comes out sorted, template groups first with their cause next to them. It is the command that
replaces the "… and 26 more" that led nowhere.

### A URL's card: who links here?

The question that comes up most in an audit has its own command:

```bash
crawlforge inspect crawl.sqlite 'https://cliente.com/pagina/'
```

The path alone also works (`/pagina/`), so does the domain without the scheme, with or without
the trailing slash. If you get it wrong, the error suggests the closest URLs in the file instead
of just saying "not there".

The card shows the HTTP status, what was extracted (title, meta description, H1, word count,
canonical, indexability), that URL's findings, its redirect chain if it redirects, its images,
and — the star section — **who links to that page**: deduplicated by linking page, with its
anchor text, whether it is `nofollow` and from which region (content links first, the `nav` and
`footer` noise after). Real output, trimmed:

```
$ crawlforge inspect un-diario-completo.sqlite '/un-titular-cualquiera'

── Inlinks (24) ─────────────────────────────
  By region: unknown 13 · main 11 · 0 nofollow
  Linking pages, content links first:
    main     "Dani Simón, nuevo portero llegado del Navalcarnero" — https://tumedio.com/carlos-cano-refuerza-la-defensa-del-calvo-sotelo-puertollano/
    main     "Dani Simón como nuevo portero" — https://tumedio.com/el-calvo-sotelo-presenta-su-nueva-equipacion-para-2026-27/
    unknown  (no anchor text) ×4 — https://tumedio.com/tag/calvo-sotelo-puertollano/

── Outlinks (156: 108 internal, 48 external) ─
     200  https://tumedio.com/quienes-somos/ "¿Quiénes somos?"
```

Outlinks show the **destination's status code** with the broken ones first: a page's card is
also its broken-link triage. And if the inspected URL is an image, the card says which pages use
it — the opposite direction to the images section.

Each list cuts off at 20 rows and the cut tells you the exact command that completes it
(`--limit all`; it also takes a number). `--lang es` translates it, and `--format md` with
`--out ficha.md` produces a card ready to paste into a ticket:

```bash
crawlforge inspect crawl.sqlite '/pagina/' --format md --out ficha.md
```

---

## 3.ter Complete scenarios

### A client audit, start to finish

```bash
crawlforge crawl https://cliente.com/
crawlforge report crawl-cliente-com.sqlite --lang es          # what does it have?
crawlforge report crawl-cliente-com.sqlite --rule CONTENT-H1-MISSING   # where?
crawlforge inspect crawl-cliente-com.sqlite '/esa-pagina/' --lang es   # who links here?
crawlforge export crawl-cliente-com.sqlite --format xlsx --out cliente.xlsx
crawlforge report crawl-cliente-com.sqlite --format html --out cliente.html --lang es
```

The `.xlsx` is for working; the `.html` is for sending.

### Watch a deploy

```bash
crawlforge crawl https://cliente.com/                         # before publishing
# … the deploy happens …
crawlforge crawl https://cliente.com/                         # the previous one becomes .prev.sqlite
crawlforge diff crawl-cliente-com.prev.sqlite crawl-cliente-com.sqlite --lang es
```

And in a CI pipeline, over the built folder and with no network:

```bash
crawlforge audit ./dist --base https://cliente.com/ --out nuevo.sqlite
crawlforge diff referencia.sqlite nuevo.sqlite --fail-on high || exit 1
```

### Review a portfolio of sites

```bash
for s in blog1.com blog2.com blog3.com; do
  crawlforge crawl "https://$s/" --out "portfolio/$s.sqlite"
done
crawlforge portfolio ./portfolio
```

One panel across all the files: what changed since each site's previous crawl, which rules
fail on how many sites, and one line per site. The whole of it in §3.quater.

### A specific set of URLs

```bash
printf '%s\n' https://cliente.com/landing-a https://cliente.com/landing-b > urls.txt
crawlforge list urls.txt --lang es
```

---

## 3.quater The portfolio: many sites at once

A single audit is a snapshot. Whoever runs many sites needs two other answers: **what broke
since last week**, and **what fails on all of them at once**. That is `portfolio`:

```bash
crawlforge portfolio ./crawls/                 # a directory is scanned for *.sqlite
crawlforge portfolio a.sqlite b.sqlite c.sqlite
```

The `.prev.sqlite` files next to your crawls are **not** counted as sites: each one is the
"before" of the crawl next to it, and the panel compares the pair automatically. That is the
same file `diff` uses, produced the same way — by re-crawling onto the same output file.

Real output, trimmed (a five-site test portfolio; two files were crawled by an older
version, one crawl was truncated and one is a list crawl):

```
$ crawlforge portfolio ./cartera

── Portfolio panel ──────────────────────────
  5 sites · crawls from 2026-08-04 to 2026-08-04

── Warnings ─────────────────────────────────
  WARNING   Not every site was crawled with the same rule catalog (0.4.0, 0.6.2). A rule
            can be missing on a site because it did not exist when that site was crawled.

── What changed ─────────────────────────────
  1 of 5 sites has a previous crawl (.prev.sqlite) to compare against.

  New critical and high findings:
    https://alpha.example/
      critical  HTTP-404-INTERNAL                   2
        https://alpha.example/p/000005/
        https://alpha.example/p/000006/

  The rest, site by site:
    https://alpha.example/
      Findings resolved 2 · Status codes that got worse 2

── Failing across the portfolio ─────────────
  A rule firing on most sites is rarely content: it is usually a shared template or
  plugin — one fix that serves them all.

  medium    CANON-CROSS-DOMAIN             3 of 5 sites
  critical  HTTP-NO-HTTPS                  2 of 5 sites
  medium    INDEX-DEEP-PAGE                1 of 5 sites (2 inconclusive)

── The portfolio at a glance ────────────────
       URLs  index.  crit  high   med   low  info  crawled     site
        240       0     2     0   118     0     0  2026-08-04  https://alpha.example/
          8       0     1     1     7     0     0  2026-08-04  http://127.0.0.1:8912/  (truncated)
```

Three things in that output are deliberate, and they are what makes the panel trustworthy:

- **"1 of 5 sites (2 inconclusive)"** — a truncated or list-mode crawl never evaluated the
  rules that need the complete link graph, so for those rules the panel separates three
  states: fires, does not fire, and **could not be evaluated**. A rule that does not appear
  on a truncated site is not a rule that passed there.
- **The catalog warning goes on top.** Files crawled with different rule catalogs are not
  silently comparable: a rule can be "missing" on a site because it did not exist yet.
- **The date range is always stated**, and if the oldest and newest crawls are more than a
  week apart the panel says so: that is not a snapshot of the portfolio, and "what changed"
  would cover a different period on each site.

A file that cannot be opened — not a crawl, another program's database, a schema newer than
the binary — is listed under "Files set aside" with its reason, and the rest of the panel is
still produced. One bad file does not cost you the other eleven.

Everything else works like `report`: `--lang es` translates the panel, and `--format md` or
`--format html` with `--out` produce a file you can paste into a ticket or send:

```bash
crawlforge portfolio ./cartera --format html --out panel.html --lang es
```

The panel is not part of the free tier. The CLI runs as the top tier by default; it only
matters if you set `CRAWLFORGE_TIER` (see §5).

---

## 4. What it does **not** do today

Said upfront, so you do not waste time looking for it:

- **It does not render JavaScript.** A site whose content is assembled in the browser will look
  empty. It is planned.
- **It does not follow external sites.** Outbound links are checked for status —that is what makes
  `HTTP-404-EXTERNAL` fire— but nothing of another domain is parsed or crawled. There is **no
  `--follow-external` flag**: crawling a third-party site whole is not something the tool offers
  from the command line. The `follow_external` key exists in the configuration file and stays off
  by default; a resumed crawl ignores whatever the file says about it, the same way it ignores a
  saved `ignore_robots`.
- **There is no graphical interface** yet.
- **There is no crawl scheduling.** The portfolio panel (§3.quater) reads the files you
  already have; producing them on a schedule is still your cron's job.
- **`HTTP-TEMP-REDIRECT`** does not exist yet: it needs a crawl history that does not exist yet.

Of the free catalog, 59 of 60 rules are implemented.

---

## 5. When something goes wrong

| Symptom | What is happening |
|---|---|
| The crawl ends with far fewer URLs than you expect | Something cut it: `--max-urls`, `--max-depth`, `--max-duration`, or the free tier's 1,000-URL cap if `CRAWLFORGE_TIER=free` is set. The report says the crawl was truncated and by which limit. |
| A `report` mentions findings "not evaluated" | The crawl was truncated and the rules that need the full graph were silenced on purpose. |
| The site returns 429 or 503 | Lower `--concurrency`. The tool respects the `Crawl-delay` in `robots.txt`, but a WAF can be stricter than robots. |
| "no such file" when running `diff` | The `.prev.sqlite` is missing: it only appears when you **repeat** a crawl onto the same output file. |

| The program seems hung after crawling | It is in the final pass: inlinks and set-wide rules. It says which rule it is on (`final pass · rule 7/29 · …`). On large sites this takes minutes. |
| An old crawl cannot be resumed | It is only refused if the file is **newer** than the program, or if it still has to cross a migration that changes what the engine writes. It still opens with `report`, `export` and `diff`. |
| `.sqlite-wal` and `.sqlite-shm` appear next to it | The crawl did not close cleanly. **Do not copy just the `.sqlite`**: you would be missing data. Open it again with `crawlforge report` so it consolidates them. |

Errors say which file is missing and which command generates it. If one does not, that is a
product bug and worth writing down.

---

## 6. Quick reference

```bash
crawlforge crawl  <URL>       # crawl over HTTP
crawlforge audit  <DIR> --base <URL>   # audit a built folder
crawlforge list   <FILE>      # an exact list of URLs
crawlforge resume <FILE>      # continue an interrupted crawl
crawlforge report <FILE>      # summary, or --format md|html
crawlforge export <FILE> --format xlsx --out a.xlsx
crawlforge diff   <BEFORE> <AFTER> [--fail-on high]
crawlforge portfolio <PATH>... [--format md|html --out f]   # panel across many crawls
crawlforge rules  [--category X] [--detail] [--format json]
```

Any of them with `--help` gives the full list of options.
