-- 012 · What `LINK-FRAGMENT-BROKEN` compares: the `#fragment` of each link and the places a
-- fragment can land on in each page.
--
-- `links.fragment`: the fragment of an internal `<a href>` without the `#`, percent-encoded as a
-- browser leaves it in the URL (`parse::link_fragment`). NULL without one, for external links and
-- for CDN infrastructure, whose fragments no rule reads. It is a column and not part of
-- `to_url_id` because the URL graph is fragment-free on purpose: `/a#x` and `/a#y` are one page,
-- crawled once.
--
-- `pages.fragment_ids`: every `id` and every `<a name>` of the page, as sorted, deduplicated
-- 32-bit hashes, four little-endian bytes each (`parse::encode_fragment_ids`). Hashes and not the
-- names: a long Wikipedia article has 9,187 ids, 107 KB as text and 36 KB this way, and the rule
-- only asks whether a fragment is among them. A page with no ids stores an empty blob, which is
-- not NULL.
--
-- **NULL in either column means "not known"**, and it is what every row written before this
-- migration has. The rule reads only links with a fragment and target pages with a set, so an old
-- file, or the older half of a resumed crawl, makes it stay silent instead of firing everywhere.
--
-- The partial index is the rule's entry point: links with a fragment are a small fraction of
-- `links`, and the condition `fragment IS NOT NULL` is a literal in the query, which is what
-- lets SQLite use a partial index (the parameterised `element = ?` of 010 could not).
ALTER TABLE links ADD COLUMN fragment TEXT;
ALTER TABLE pages ADD COLUMN fragment_ids BLOB;
CREATE INDEX IF NOT EXISTS idx_links_fragment ON links(to_url_id) WHERE fragment IS NOT NULL;
