### CrawlForge — rule catalogue: names and descriptions.
###
### English (locales/en/) is the source and the authority; every other folder is a
### translation of it. CONTRIBUTING.md, "Adding or improving a language", explains the rules.
### The comments above each message are context for translators: they are not shown.

### Each rule is one message: its ID, the short name as the value, and the description
### as the `.description` attribute. Backticks and <tags> are literal text, keep them.

ASSET-BROKEN = Broken CSS or JS
    .description = A CSS or JS file on the site returns 4xx or 5xx. Google renders the page to index it, so a missing stylesheet can make it see an unstyled —hence not mobile-friendly— page, and a missing script can leave client-rendered content empty. It is typical of a deploy with mismatched file hashes.
ASSET-CSS-HEAVY = Oversized stylesheet
    .description = The stylesheet is over 100 KB as delivered. The browser paints nothing until it has it, so these bytes cost more than a script's: they delay the whole first paint. That is why the bar is lower. It is nearly always a full framework shipped to use a tenth of it.
ASSET-FORM-BROKEN = Form posting to a broken URL
    .description = A form's `action` points at a URL of yours returning 4xx or 5xx. The form looks fine, fills in fine and is lost on submit: one of the few defects that costs customers outright, and nobody finds out, because whoever hits it leaves without saying so. Only forms submitted with GET are checked (a search box, a catalogue filter): checking a POST would mean submitting it, and this tool does not submit forms.
ASSET-IFRAME-BROKEN = Broken embedded frame
    .description = An `<iframe>` on the page points at a URL of yours returning 4xx or 5xx, so the visitor sees a blank hole where the map, the video or the form should be. It leaves no trace in the containing page —the HTML is still valid— which is why it goes unnoticed for months.
ASSET-IMG-BROKEN = Broken image
    .description = An image the site points to returns 4xx or 5xx. The gap shows on the page, the image does not exist for image search, and every visit spends a request on an error. It is almost always a migration that left the uploads folder behind, or a hand-written path.
ASSET-IMG-EMPTY-ALT-LINK = Link with only an empty-alt image
    .description = A link whose only content is an image with `alt=""` has no accessible name: someone using a screen reader hears “link” and nothing else, and the search engine gets no signal about where it leads. The typical case is the header logo, usually the most repeated link on the site. Here the `alt` is not decorative: it is the link text.
ASSET-IMG-HEAVY = Oversized image
    .description = The image is over 200 KB. It is the most common cause of a poor mobile LCP: it delays painting the main element and eats the bandwidth of a slow connection. Exporting it at the size it is actually displayed and serving WebP or AVIF is usually enough.
ASSET-IMG-NO-ALT = Image without alt attribute
    .description = Some images have no `alt` attribute. A screen reader has nothing to read and ends up spelling out the file name, and Google loses the only text describing the image: it is what ranks it in image search and what shows up when the picture fails to load. An empty `alt=""` is valid, but only for decorative images.
ASSET-JS-HEAVY = Oversized script
    .description = The script is over 250 KB as delivered. Every page that loads it pays for the download and, more to the point, for parsing and executing it on the main thread, which is where poor responsiveness on an ordinary phone comes from. It is usually a bundle nobody has split: work out what is needed for the first screen and load the rest separately.
CANON-CHAIN = Canonical chain
    .description = A declares B as its canonical, and B declares C. Canonicals are not transitive for Google: when it finds a chain it ignores it and decides on its own which URL is the main one. Every step should point straight at C.
CANON-CROSS-DOMAIN = Cross-domain canonical
    .description = The canonical points to a host other than the page's own. That is legitimate for syndicated content, but when it is not deliberate it hands the ranking to the other domain: the page itself stops being indexed. It usually comes from a half-finished migration or a copied staging environment.
CANON-MISSING = Missing canonical
    .description = The indexable page declares no rel=canonical. Not serious on its own, since Google infers the canonical, but without it any URL parameter — a campaign, a filter, a sort order — can end up indexed as a separate page.
CANON-MULTIPLE = Multiple canonicals
    .description = The page declares more than one link rel=canonical. Google does not pick one: it ignores them all, so the effect is having none at all. It is almost always the theme and an SEO plugin each emitting the tag on their own.
CANON-RELATIVE = Relative canonical
    .description = The canonical is declared as a relative reference instead of an absolute URL. It works while the page is served from one place, but if the HTML is reproduced under another host — a staging environment, a proxy, a scraper — the canonical resolves against that host and stops pointing at the original.
CANON-TO-4XX = Canonical to an error URL
    .description = The canonical points to a URL that answers with an error. The page declares itself a duplicate of something that does not exist, so neither it nor the target can be indexed: the content disappears from results entirely.
CANON-TO-NOINDEX = Canonical to a noindexed page
    .description = The canonical points to a page marked noindex. The two signals contradict each other: one says “index that one” and that one says “do not index me”. The usual outcome is losing both URLs.
CANON-TO-REDIRECT = Canonical to a redirect
    .description = The canonical points to a URL that redirects elsewhere. Google has to choose between the canonical signal and the redirect one, and usually keeps the final destination, which makes the tag pointless. Point it straight at the URL that answers 200.
CONTENT-H1-EMPTY = Empty H1
    .description = There is an H1 in the markup, but it contributes no text: it is empty, or its only content is an image that says nothing. The heading takes the headline's place without doing its job, so neither the search engine nor a screen reader gets the topic of the page. The usual case is a logo inside the H1.
CONTENT-H1-MISSING = Missing H1
    .description = The indexable page has no H1, or an empty one. The H1 tells the search engine what the page is about in the author's own words, and it is the first heading a screen reader announces when entering the content.
CONTENT-H1-MULTIPLE = Multiple H1
    .description = The page has more than one H1. HTML5 allows it, but there is no longer a single main topic: the search engine has to guess what the page is about, and a screen reader announces several level-one headings as if they were separate documents. It is usually the template marking the logo or the site name as an H1 on top of the real headline.
CONTENT-HEADING-SKIP = Heading level skipped
    .description = Heading levels drop by more than one step at once, for example from H2 to H4. The document outline ends up with holes: someone navigating by headings cannot tell whether the H4 belongs to that H2 or to another section, and the search engine loses the hierarchy it uses to understand what belongs to what. It usually comes from picking a heading by its font size instead of its level.
CONTENT-LANG-MISSING = Missing lang attribute
    .description = The <html> element does not declare the language of the content. Without it, a screen reader pronounces the text using the system language's rules, the browser does not know which dictionary to use when translating, and the site's language signals are left to automatic detection. It is a one-line attribute.
CONTENT-THIN = Thin content
    .description = An indexable page with fewer than 300 words of visible text. It rarely has enough substance to answer a query, so it competes poorly and, in bulk, dilutes the site's average quality in the search engine's eyes. Many of these are archives, tag pages or auto-generated stubs that are better left unindexed than expanded.
DUP-CONTENT-EXACT = Identical content
    .description = Two or more indexable URLs return exactly the same HTML, byte for byte. They compete against each other for the same queries and split incoming links instead of adding them up. Fix it by keeping one indexable and canonicalising the rest to it.
DUP-CONTENT-NEAR = Near-duplicate content
    .description = Two or more indexable URLs have almost the same visible text even though their HTML is not identical: a date, a widget or a few sentences change. They are usually tag and category pages listing the same posts, or one post rewritten over another. Google picks one to show and sets the others aside. Merge them, set them apart or canonicalise them to the main one.
HREFLANG-INVALID-CODE = Invalid hreflang code
    .description = The attribute value is neither an ISO 639-1 language with an optional ISO 3166-1 region nor the special x-default. Google silently discards a malformed annotation, so the set is left incomplete with no warning: it is the most frequent failure and the hardest one to spot by eye.
HREFLANG-LANG-MISMATCH = Self hreflang in another language than the page
    .description = The hreflang annotation pointing at the page itself declares a different language from its `<html lang>`: a page in Spanish announcing itself as English, for example. One of the two is wrong, and if it is the hreflang, Google serves the page to people searching in the other language. It is usually a translation plugin that copies the original page's set without changing the code.
HREFLANG-NO-SELF = Hreflang without self-reference
    .description = The page declares language alternates but none points to itself. Google requires every page in the set to include itself; when that link is missing it discards the whole group and none of the translations benefit from the others.
HREFLANG-NOT-RECIPROCAL = Hreflang not reciprocal
    .description = This page declares a language alternate that does not declare it back. Hreflang annotations are votes that only count when they are mutual: Google ignores the whole relationship when one of the two sides does not confirm the other.
HREFLANG-SELF-ONLY = Hreflang with no alternates
    .description = The page declares hreflang, but every annotation points at the page itself: there is no translation in the set, so it does nothing. If the page has no translation, the annotation is unnecessary; if it does, its links are missing, and that is what really needs checking: it is usually a translation the plugin stopped linking.
HREFLANG-TO-4XX = Hreflang to error URL
    .description = A language alternate points to a URL that returns 4xx. The translation the page promises does not exist: a visitor switching language lands on an error page and Google drops the referring page from the set.
HTTP-404-EXTERNAL = Broken external link
    .description = The site links to a URL on another domain that is gone: it answers 404 or 410, or its domain does not resolve. It does not hurt like a 404 of your own, but it sends the visitor to an error page and ages the content: a guide full of dead links stops looking maintained.
HTTP-404-INTERNAL = Broken internal link
    .description = A URL on this site returns 4xx and there are pages linking to it. It wastes crawl budget, cuts the flow of internal links to whatever used to be there, and leaves the visitor on an error page.
HTTP-5XX = Server error
    .description = The URL returns 5xx: the server failed to build the response. Google drops pages that answer this way for long from its index, and slows down crawling of the whole site while it lasts.
HTTP-EXTERNAL-UNVERIFIABLE = Unverifiable external link
    .description = The site links to a URL on another domain that refused the check: it answered 401, 403 or 429. That is usually an anti-bot wall, a paywall or a rate limit, and the page almost always opens fine in a browser. It is not called broken because there is no way to tell from here: open it to confirm it is still there.
HTTP-LARGE-PAGE = Oversized HTML
    .description = The HTML document is over 500 KB, images and scripts aside. It delays first paint on mobile connections and usually means the template dumps content the page never shows, or that inlined CSS and JS should live in their own files.
HTTP-MIXED-CONTENT = Mixed content
    .description = A page served over HTTPS loads images, stylesheets or scripts over HTTP. The browser silently blocks scripts and stylesheets and flags the connection as not secure: the page looks broken and stops inspiring trust.
HTTP-NO-HTTPS = Site answers over HTTP
    .description = Some internal URLs serve content over HTTP without redirecting to HTTPS. The site stays reachable at two different addresses for the same page — duplicate content — and the browser warns the visitor that the connection is not secure.
HTTP-REDIRECT-CHAIN = Redirect chain
    .description = The final destination is reached after two or more consecutive hops. Every hop adds latency for the visitor and crawl cost, and usually means two rewrite rules are stepping on each other. The fix is to point the first hop straight at the end.
HTTP-REDIRECT-LOOP = Redirect loop
    .description = The redirects come back to a URL already visited, so the destination is never reached. The browser gives up with an error and the crawler never sees any content: for all practical purposes that part of the site does not exist.
HTTP-REDIRECT-TO-404 = Redirect to error page
    .description = The redirect ends on a URL that returns 4xx. It is the worst of both worlds: the redirect that suggested the content had moved is still there, but the destination does not exist either, so the link and its authority are lost anyway.
HTTP-SLOW-RESPONSE = Slow response
    .description = The server takes more than a second to send the first byte. That time is added in full to the load time Core Web Vitals measures, and it caps how many pages the crawler gets through on each visit.
INDEX-BLOCKED-IN-SITEMAP = Blocked but in sitemap
    .description = The sitemap presents the URL as content you want indexed while robots.txt forbids crawling it. The two instructions contradict each other, Search Console flags it as a coverage error and the URL stays out of the index. It is almost always a Disallow written for something else that caught a published section along the way.
INDEX-DEEP-PAGE = Too many clicks from home
    .description = It takes more than four clicks from the home page to reach this page. The further away, the less internal authority it gets and the less often the search engine revisits it; in catalogues and archives it is the symptom of pagination with no shortcuts, where page 40 is only reachable through the previous 39.
INDEX-NO-INTERNAL-LINKS-IN = No inbound internal links
    .description = No other page on the site links to this one, even though it is indexable and published. With no inbound links it receives no internal authority, the search engine takes far longer to revisit it and its changes go unnoticed. It is the first thing to check when a new page never quite ranks.
INDEX-NOFOLLOW-INTERNAL = Nofollow internal link
    .description = This page links to another page on the same site with rel=nofollow. The link passes no authority and does not help discover the target, so within one domain it rarely makes sense: PageRank sculpting stopped working in 2009. It usually comes from a plugin or template that adds it to every link without telling internal from external.
INDEX-NOINDEX = Noindex
    .description = The page asks not to be indexed, either through its meta robots tag or the X-Robots-Tag header. Google crawls it and drops it: it will not show up for any query. On archives, tags and utility pages it is usually a deliberate choice of the SEO plugin, which is why the warning is moderate; it escalates to critical on the home page, and the conflict with the sitemap has its own rule.
INDEX-NOINDEX-IN-SITEMAP = Noindex but in sitemap
    .description = The URL is in the sitemap, which is the list of what the site wants indexed, and its own header or meta robots says the opposite. One of the two is wrong: either it does not belong in the sitemap, or the noindex is a leftover nobody removed. While both coexist, the site contradicts itself in front of the search engine.
INDEX-ORPHAN-PAGE = Orphan page
    .description = The site declares this URL in its sitemap but none of its pages links to it. A visitor cannot reach it by browsing and the search engine sees content with no context and no internal authority. It is the finding that comes from comparing what is declared against what is reached, and no single page can reveal it.
INDEX-ROBOTS-BLOCKED = Blocked by robots.txt
    .description = The site links to this URL from its own pages and at the same time forbids it in robots.txt. Google cannot read it, so it does not know what it contains nor does it follow its links: the internal link leads nowhere and the authority it passes is lost. This is not the same as noindex, which still allows reading the page.
INDEX-ROBOTS-TXT-BLOCKS-ALL = robots.txt blocks the whole site
    .description = The robots.txt forbids crawling the site root, so no page can be read or indexed. It is the fastest and quietest way to disappear from Google, and it is nearly always the same accident: the staging file, which carries Disallow: /, shipped to production in a deploy.
INDEX-ROBOTS-TXT-MISSING = Missing robots.txt
    .description = The site does not serve /robots.txt. It does not prevent indexing —with no file, everything is crawlable— but it gives up the place where the sitemap is announced and where you exclude the areas that add nothing for a search engine, such as internal search results or cart pages.
INDEX-SECTION-DISCONNECTED = Section disconnected from internal linking
    .description = A group of pages link to each other but cannot be reached from the home page by following regular links: the bridge joining them to the rest of the site is JavaScript, a form, or missing altogether. Search engines only discover them through the sitemap and little internal authority flows to them. The cause is one —a crawlable link into the section is missing— and it is fixed once, not page by page.
INDEX-SITEMAP-ERROR = Sitemap with errors
    .description = A sitemap does not respond, has malformed XML, or exceeds the protocol limits (50,000 URLs or 50 MB). Search engines stop reading at the error, so everything after it is never discovered through that route, and nothing warns you: the sitemap is still there, seemingly fine.
INDEX-SITEMAP-MISSING = No sitemap
    .description = No sitemap was found: robots.txt does not announce one, it is not at the usual paths, and none declares any URL. Without it the search engine only reaches what is linked, at whatever pace internal linking allows; and you lose the comparison between what the site claims to publish and what a crawl actually reaches.
LINK-ANCHOR-EMPTY = Internal link without text
    .description = Some internal links have no text, no image and no aria-label: an icon drawn with CSS, a whole block turned into an empty link. The search engine gets no hint of where it leads, and someone using a screen reader hears “link” and nothing else. Give it text, even if visually hidden, or an aria-label saying where it goes.
LINK-ANCHOR-GENERIC = Generic link text
    .description = Some internal links consist entirely of a stock phrase such as “read more”, “here” or “click here”. That text says nothing about the target page, which is exactly what the search engine reads in it, and out of context —the way a screen reader lists them— every link says the same. It is enough for the text to name the target: “read more about pricing” instead of “read more”.
META-DESC-DUPLICATE = Duplicate meta description
    .description = Two or more indexable pages repeat the same description, so the search result does not tell them apart and none of them promises anything specific. It usually comes from a site-wide default nobody overrode.
META-DESC-MISSING = Missing meta description
    .description = The indexable page declares no description, so Google clips an arbitrary snippet of the body for the search result. It is not a ranking factor, but it is the text that decides the click, and not writing it means not controlling it.
META-DESC-TOO-LONG = Meta description too long
    .description = The description does not fit in the search result and gets truncated, so the call to action at the end is never read. It is measured in pixels, not characters, because the cut is decided by rendered width.
META-DESC-TOO-SHORT = Meta description too short
    .description = The description is so brief that Google tends to ignore it and build the snippet from body text instead. With so little space used there is room for neither the argument nor the query variant that would justify the click.
META-OUTSIDE-HEAD = Metadata outside the head
    .description = The canonical, meta robots, hreflang, title or description end up outside the <head>. An element that does not belong in the head —a tracking <img>, a <div>, stray text left by a plugin— closes it at that point, and Google ignores everything after it even though it is still in the source. The effect is the same as not having them.
META-REFRESH = Meta refresh redirect
    .description = The page redirects with <meta http-equiv=refresh> instead of a 301. Google reads it with reservations, does not pass authority as reliably, and the user sees an abrupt jump; with a non-zero delay it also traps the browser's back button.
META-TITLE-DUPLICATE = Duplicate title
    .description = Two or more indexable pages share the same <title>. Google has to pick which one to show for the same query, so they compete with each other instead of adding up. It is the most common symptom of pagination or misconfigured archives.
META-TITLE-MISSING = Missing title
    .description = The page is indexable and has no <title>, or an empty one. It is the on-page factor with the most weight that you control directly, and it is the blue text in the search result: without it, Google makes one up from whatever it finds.
META-TITLE-MULTIPLE = Multiple title tags
    .description = The page carries more than one <title>. Browsers and Google keep the first and silently discard the rest, so the title that was written with care may not be the one published. It is nearly always a template printing it twice, or an SEO plugin adding its own without removing the theme's.
META-TITLE-TOO-LONG = Title too long
    .description = The title does not fit in the search result and Google truncates it with an ellipsis, so the tail —often the brand, or the keyword left for last— is never read. It is measured in pixels, not characters: the cut is decided by rendered width.
META-TITLE-TOO-SHORT = Title too short
    .description = The title leaves most of the space the search result offers unused. It is not an error but wasted space: there is room for a query variant or a reason to click, and it often points to a template that only prints the section name.
META-VIEWPORT-MISSING = Missing meta viewport
    .description = Without <meta name=viewport> a phone lays the page out at 980 px wide and scales it down, leaving text unreadable and forcing pinch-zoom. Indexing is mobile-first: what Google evaluates is that shrunken version.
SOCIAL-OG-IMAGE-BROKEN = Broken Open Graph image
    .description = The image declared in og:image returns an error, directly or at the end of its redirects. The tag is there and no tool that only reads the HTML notices, but when the page is shared on WhatsApp, LinkedIn or Slack the card shows no image, or an empty frame. It is usually an image deleted from the media library or a path that did not survive a migration.
SOCIAL-OG-MISSING = Incomplete Open Graph
    .description = og:title, og:description or og:image is missing. When someone shares the page on WhatsApp, LinkedIn or Slack the network cannot build the preview card and the link shows up as a bare URL: it does not affect ranking, but the click is lost.
