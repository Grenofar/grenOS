# Spec — Google and web search in the browser

Written by Claude for the agents, 2026-09-16, at the human's request: "add
Google to the browser". Every fact below was measured with the kernel's own TLS
client (`tls.rs`, compiled for the host) against the live servers on
2026-09-16, unless another source is named. English, because models read it.

## 1. What the servers actually answer

| Request (User-Agent `grenOS/0.9 (texte)` unless said) | Answer |
|---|---|
| TLS 1.3 handshake with `www.google.com` | succeeds (certificate 3 774 bytes) |
| `GET /search?q=...` without cookie | `302` to `consent.google.com` (the host's IP is in France: the EU consent page) |
| `GET /search?q=...` with `Cookie: SOCS=CAI` | `200`, but the page says "Votre navigateur n'est plus pris en charge" — the same for Lynx, w3m and Links user agents; with a Chrome user agent, a page that only redirects through JavaScript |
| `GET /webhp?hl=fr` with `Cookie: SOCS=CAI` | `200`, 85 KB, `charset=ISO-8859-1`, `Transfer-Encoding: chunked`: the Google home page, text and links |
| `GET /` | `400 Bad Request` |
| `html.duckduckgo.com` `GET /html/?q=...` | `200`, 35 KB, `charset=UTF-8`, `Transfer-Encoding: chunked`, real results, no JavaScript needed |
| `lite.duckduckgo.com/lite/?q=...` | `202`, a "select the ducks" bot challenge |
| `www.bing.com/search?q=...` | `200`, 360 KB of mostly script |

Conclusion, to be written honestly in the browser: **Google Search refuses
browsers without JavaScript**, and a kernel browser has none. So:

- **Google** is in the browser as a bookmark and a home button that open
  `https://www.google.com/webhp?hl=fr`, which works.
- **Searching** from the address bar goes to DuckDuckGo's HTML page, which
  answers text browsers, and the results page says so in one line:
  "Google exige JavaScript pour chercher ; ces résultats viennent de DuckDuckGo."

`SOCS=CAI` is the cookie Google sets when "Reject all" is chosen on its consent
page. The browser keeps no cookies at all, so rejecting is its true state:
send `Cookie: SOCS=CAI` to hosts ending in `google.com` only, and to no one else.

## 2. `http.rs`

1. **Chunked bodies** (RFC 9112 §7.1): `chunked-body = *chunk last-chunk
   trailer-section CRLF`, `chunk = chunk-size [ chunk-ext ] CRLF chunk-data
   CRLF`, `last-chunk = 1*("0") [ chunk-ext ] CRLF`. The CRLF after `0` ends the
   last-chunk line; the trailer section (zero or more header lines) ends with
   its own CRLF. Two models out of two got this wrong when asked: the simplest
   input `5\r\nhello\r\n0\r\n\r\n` must decode to `hello`. Hex digits in either
   case, leading zeros allowed, reject a size that overflows `usize`, never
   index out of bounds, return `None` on malformed or truncated input.
   Signature: `pub fn dechunk(body: &[u8]) -> Option<Vec<u8>>`.
2. **Redirects**: 301, 302, 303, 307, 308 with a `Location` header — absolute
   (`https://...`), scheme-relative (`//host/path`) or path-relative (`/path`)
   — are followed, at most 5 times, then the page says it stopped.
3. **Character sets**: `charset=ISO-8859-1` (or `latin1`, `windows-1252`): each
   byte is the character with the same code (the font covers U+0000..U+00FF).
   `charset=UTF-8` or none: decode UTF-8, replacing invalid sequences with `?`.
   Characters above U+00FF cannot be drawn: replace them with `?`, except
   typographic ones worth mapping: ’ ‘ → `'`, “ ” → `"`, – — → `-`, … → `...`,
   · stays (it is U+00B7).
4. **Headers** are matched case-insensitively.
5. Keep `Accept-Encoding` absent: no compression is supported.

## 3. HTML to a page (`web.rs` blocks)

The browser draws `web::Block`s: `Title`, `Head`, `Text`, `Item`, `Link(text,
url)`, `Rule`, `Space`. Today a remote page is flattened to one text with no
links. Instead, convert HTML to that same line format that `web::parse`
already reads (`# `, `## `, `- `, `[text](url)` on its own line):

- Skip `<script>`, `<style>`, `<head>`, `<noscript>`, `<svg>`, `<template>` with
  their contents.
- `<title>` becomes the tab title. `<h1>` → `# `, `<h2>`..`<h6>` → `## `,
  `<li>` → `- `, `<p>`, `<br>`, `<div>`, `<tr>` end a line.
- `<a href="...">text</a>` becomes its own `[text](absolute url)` line: resolve
  relative URLs against the page URL. Skip `javascript:` and `#` links.
- Entities: `&amp; &lt; &gt; &quot; &apos; &nbsp;`, named Latin-1 ones
  (`&eacute;` ...), and numeric `&#233;` / `&#xE9;`.
- Linear time: one pass over the bytes. The current `to_text` builds a string
  of 16 characters for every character of the page, which on an 85 KB page is
  millions of allocations.

### DuckDuckGo results (special case, by host `html.duckduckgo.com`)
Each result is:
```html
<a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fgithub.com%2F...&amp;rut=...">Title</a>
...
<a class="result__snippet" href="...">Snippet with <b>bold</b> words</a>
```
Render as: `# Recherche : <query>`, the JavaScript note above, then for each
result a `[Title](real url)` line, the snippet as text, and the host as a small
text line. The real URL is the `uddg` parameter, percent-decoded (after turning
`&amp;` into `&`).

## 4. The address bar
- Text that starts with `http://`, `https://`, `grenos:` or `fichier:` is
  opened as it is.
- Text with no space that contains a dot (`example.com`, `wikipedia.org/wiki/X`)
  is opened as `https://` + text.
- Anything else is a search: `https://html.duckduckgo.com/html/?q=` + the
  query, percent-encoded (space as `+`, every byte outside
  `A-Za-z0-9-._~` as `%XX` of its UTF-8 bytes).

## 5. What CI can see
The browser is not exercised by a network fetch in CI (external servers make
flaky tests). Instead, at boot, `main.rs` runs the pure decoders on fixed
inputs — the chunked cases above, an ISO-8859-1 body with `é`, a small HTML page
with a relative link and an entity, and a DuckDuckGo result block — and prints
`web: decoders verified` or which case failed.
