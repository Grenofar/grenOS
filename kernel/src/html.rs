//! A web page turned into the lines the browser draws
//! (docs/specs/browser-search.md §2.3 and §3).
//!
//! The browser draws its own pages from a small line format (`web::parse`):
//! `# ` a title, `## ` a heading, `- ` an item, `[text](url)` a link on its
//! own line, `---` a rule, an empty line a gap, anything else text. A page from
//! the network is brought to that same format in one pass over its characters:
//! scripts and styles skipped, headings, items and links kept, every link made
//! absolute, entities decoded, and every character the font cannot draw (it
//! stops at U+00FF) replaced by its nearest Latin-1 spelling or `?`.
//!
//! Everything here is pure, so the boot self-test and the host can check it.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Elements whose contents are never shown.
const HIDDEN: [&str; 8] = ["script", "style", "head", "noscript", "svg", "template", "iframe", "select"];
/// Elements that are the page's furniture rather than its text — menus,
/// footers, asides — skipped with everything inside them, as a reader view
/// does. A page made only of them keeps them (see `lines`).
const FURNITURE: [&str; 5] = ["nav", "footer", "aside", "dialog", "button"];
/// Words in a `class` that mark the same furniture: Wikipedia's language
/// dropdown and navigation boxes, edit links, cookie banners.
const FURNITURE_CLASSES: [&str; 7] = ["dropdown", "navbox", "sidebar", "editsection", "cookie", "vector-menu", "noprint"];
/// Elements that end a line of text.
const BREAKS: [&str; 20] = [
    "br", "p", "div", "tr", "table", "ul", "ol", "dl", "dt", "dd", "section", "article", "header", "footer", "nav",
    "form", "blockquote", "pre", "main", "aside",
];
/// Said on Google's pages, whose search box needs JavaScript.
const GOOGLE_NOTE: &str = "Google ne cherche pas sans JavaScript : tapez votre recherche dans la barre d'adresse, elle passera par DuckDuckGo.";
/// How much of a page is kept, in lines, and how long a link's words may be.
const MAX_LINES: usize = 1_500;
const MAX_LINK_WORDS: usize = 140;

/// A page ready for the browser: what its tab says, and its lines.
pub struct Page {
    pub title: String,
    pub lines: String,
}

/// A fetched body, whatever it is, as a browser page. `content_type` is the
/// response's `Content-Type` header; `url` is where the body finally came
/// from, redirects followed, which relative links are resolved against.
pub fn page(body: &[u8], content_type: Option<&str>, url: &str) -> Page {
    let text = decode(body, content_type);
    let kind = content_type.unwrap_or("text/html").to_ascii_lowercase();
    if !kind.contains("html") {
        // Plain text, JSON, anything readable: shown line by line, with the
        // markers of the line format defused by a leading dot.
        let mut lines = String::new();
        for line in text.lines().take(MAX_LINES) {
            let marked = ["#", "- ", "[", "---"].iter().any(|mark| line.trim_start().starts_with(mark));
            if marked {
                lines.push_str("· ");
            }
            push_drawable(&mut lines, line);
            lines.push('\n');
        }
        return Page { title: host_of(url).to_string(), lines };
    }
    let title = title(&text).unwrap_or_else(|| host_of(url).to_string());
    let host = host_of(url);
    let lines = if host == "html.duckduckgo.com" {
        duckduckgo(&text, &query_of(url))
    } else if host.ends_with("google.com") || host.ends_with("google.fr") {
        format!("{GOOGLE_NOTE}\n\n{}", lines(&text, url))
    } else {
        lines(&text, url)
    };
    Page { title, lines }
}

/// The body as text: ISO-8859-1 and its Windows cousin byte for character,
/// anything else as UTF-8 with broken sequences shown as `?`. The charset comes
/// from the header, or else from a `<meta>` near the top of the page.
pub fn decode(body: &[u8], content_type: Option<&str>) -> String {
    let declared = content_type.and_then(charset_of).or_else(|| {
        let top = String::from_utf8_lossy(&body[..body.len().min(2048)]).to_ascii_lowercase();
        let at = top.find("charset=")?;
        let value: String = top[at + 8..]
            .trim_start_matches(['"', '\''])
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        Some(value)
    });
    let single_byte = declared.as_deref().is_some_and(|name| {
        matches!(name, "iso-8859-1" | "iso8859-1" | "latin1" | "latin-1" | "windows-1252" | "cp1252" | "us-ascii")
    });
    if single_byte {
        body.iter().map(|&byte| windows_1252(byte)).collect()
    } else {
        String::from_utf8_lossy(body).into_owned()
    }
}

/// The `charset` parameter of a Content-Type value, in lower case.
fn charset_of(content_type: &str) -> Option<String> {
    let lower = content_type.to_ascii_lowercase();
    let at = lower.find("charset=")?;
    let value = lower[at + 8..].split(';').next()?.trim().trim_matches(['"', '\'']);
    (!value.is_empty()).then(|| value.to_string())
}

/// A byte of Windows-1252, which servers that say ISO-8859-1 really send: the
/// 0x80..0x9F row holds typographic marks in place of control codes.
fn windows_1252(byte: u8) -> char {
    match byte {
        0x80 => '\u{20AC}',
        0x85 => '\u{2026}',
        0x91 => '\u{2018}',
        0x92 => '\u{2019}',
        0x93 => '\u{201C}',
        0x94 => '\u{201D}',
        0x95 => '\u{2022}',
        0x96 => '\u{2013}',
        0x97 => '\u{2014}',
        _ => char::from(byte),
    }
}

/// Appends `text` with every character the font cannot draw replaced.
fn push_drawable(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '\t' => out.push(' '),
            '\u{0}'..='\u{1F}' | '\u{7F}'..='\u{9F}' | '\u{AD}' => {}
            '\u{20}'..='\u{FF}' => out.push(c),
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{2032}' => out.push('\''),
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{2033}' => out.push('"'),
            '\u{2010}'..='\u{2015}' | '\u{2212}' => out.push('-'),
            '\u{2026}' => out.push_str("..."),
            '\u{2022}' | '\u{2027}' | '\u{25CF}' => out.push('·'),
            '\u{2190}' => out.push_str("<-"),
            '\u{2192}' => out.push_str("->"),
            '\u{20AC}' => out.push_str("EUR"),
            '\u{0152}' => out.push_str("OE"),
            '\u{0153}' => out.push_str("oe"),
            '\u{0178}' => out.push('Y'),
            '\u{2122}' => out.push_str("TM"),
            '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}' => out.push(' '),
            '\u{200B}'..='\u{200F}' | '\u{2060}' | '\u{FEFF}' => {}
            _ => out.push('?'),
        }
    }
}

/// The page's title, from its `<title>`.
pub fn title(html: &str) -> Option<String> {
    let start = find_ci(html, 0, "<title")?;
    let open_end = start + html[start..].find('>')? + 1;
    let close = find_ci(html, open_end, "</title")?;
    let mut text = String::new();
    push_drawable(&mut text, &collapse(&decode_entities(&html[open_end..close])));
    (!text.is_empty()).then_some(text)
}

/// What is being built: the lines so far, the words of the line in progress
/// with their marker, and the link being read, if any.
struct Builder {
    lines: Vec<String>,
    text: String,
    prefix: &'static str,
    /// The link being read: its address, its words so far, and the label its
    /// `aria-label` or `title` gives, for a link made of an image alone.
    link: Option<(String, String, Option<String>)>,
    /// Whether the last thing on the line is a link, so that two links in a
    /// row do not run together.
    after_link: bool,
}

impl Builder {
    /// Ends the line in progress, with its heading or item marker.
    fn flush(&mut self) {
        let words = collapse(&self.text);
        self.text.clear();
        if !words.is_empty() {
            let mut line = String::from(self.prefix);
            push_drawable(&mut line, &words);
            self.lines.push(line);
        }
        self.prefix = "";
        self.after_link = false;
    }

    /// Ends the link being read: `[words](address)` goes into the sentence it
    /// sits in, where the browser finds it again (`web::layout`).
    fn close_link(&mut self) {
        let Some((url, words, label)) = self.link.take() else {
            return;
        };
        let mut shown = String::new();
        push_drawable(&mut shown, &collapse(&words));
        if shown.is_empty() {
            push_drawable(&mut shown, &collapse(&label.unwrap_or_default()));
        }
        // Nothing to read, nothing to click: an icon, a voting arrow.
        if shown.is_empty() {
            return;
        }
        if let Some((cut, _)) = shown.char_indices().nth(MAX_LINK_WORDS) {
            shown.truncate(cut);
            shown.push_str("...");
        }
        // Brackets in the words would end them early.
        let shown = shown.replace('[', "(").replace(']', ")");
        let before = words.starts_with(char::is_whitespace) || self.after_link;
        let after = words.ends_with(char::is_whitespace);
        self.after_link = true;
        self.text.push_str(if before { " [" } else { "[" });
        self.text.push_str(&shown);
        self.text.push_str("](");
        self.text.push_str(&url);
        self.text.push_str(if after { ") " } else { ")" });
    }

    /// An empty line, unless there already is one.
    fn gap(&mut self) {
        if self.lines.last().is_some_and(|line| !line.is_empty()) {
            self.lines.push(String::new());
        }
    }
}

/// `html`, fetched from `base`, as browser lines: without the page's
/// furniture, unless leaving it out leaves almost nothing.
pub fn lines(html: &str, base: &str) -> String {
    let lean = lines_with(html, base, true);
    if lean.len() < 400 {
        let full = lines_with(html, base, false);
        if full.len() > lean.len() * 4 {
            return full;
        }
    }
    lean
}

/// The `>` that ends the tag opening at `start`, outside quoted attribute
/// values: Wikipedia puts JSON full of `>` in its attributes. A quote never
/// closed falls back to the first `>`.
fn tag_end(html: &str, start: usize) -> Option<usize> {
    let bytes = html.as_bytes();
    let mut quote = None;
    for (offset, &byte) in bytes[start..].iter().enumerate() {
        match (quote, byte) {
            (None, b'>') => return Some(start + offset),
            // A quote only opens a value right after `=`, spaces allowed.
            (None, b'"' | b'\'') if html[start..start + offset].trim_end().ends_with('=') => quote = Some(byte),
            (Some(open), _) if byte == open => quote = None,
            _ => {}
        }
    }
    html[start..].find('>').map(|offset| start + offset)
}

/// Whether a tag opens the page's furniture.
fn furniture(name: &str, tag: &str) -> bool {
    if FURNITURE.contains(&name) {
        return true;
    }
    if attribute(tag, "hidden").is_some() || tag.contains("aria-hidden=\"true\"") {
        return !matches!(name, "html" | "body" | "main" | "article");
    }
    attribute(tag, "class").is_some_and(|class| {
        let class = class.to_ascii_lowercase();
        FURNITURE_CLASSES.iter().any(|word| class.contains(word))
    }) && !matches!(name, "html" | "body" | "main" | "article")
}

/// Where the element `name` that opened just before `at` ends: after its
/// matching end tag, counting the same elements nested inside it.
fn end_of_element(html: &str, at: usize, name: &str) -> usize {
    let mut depth = 1;
    let mut from = at;
    while let Some(offset) = html[from..].find('<') {
        let start = from + offset;
        let Some(close) = tag_end(html, start) else {
            return html.len();
        };
        let tag = &html[start + 1..close];
        from = close + 1;
        if tag_name(tag) != name {
            continue;
        }
        if tag.starts_with('/') {
            depth -= 1;
            if depth == 0 {
                return from;
            }
        } else if !tag.ends_with('/') {
            depth += 1;
        }
    }
    html.len()
}

fn lines_with(html: &str, base: &str, lean: bool) -> String {
    let mut page = Builder { lines: Vec::new(), text: String::new(), prefix: "", link: None, after_link: false };
    let mut at = 0;
    while at < html.len() && page.lines.len() < MAX_LINES {
        // Text up to the next tag.
        let next = html[at..].find('<').map_or(html.len(), |offset| at + offset);
        if next > at {
            let piece = decode_entities(&html[at..next]);
            match &mut page.link {
                Some((_, words, _)) => words.push_str(&piece),
                None => {
                    page.after_link &= piece.trim().is_empty();
                    page.text.push_str(&piece);
                }
            }
            at = next;
            continue;
        }
        if html[at..].starts_with("<!--") {
            at = html[at..].find("-->").map_or(html.len(), |offset| at + offset + 3);
            continue;
        }
        let Some(close) = tag_end(html, at) else {
            break;
        };
        let tag = &html[at + 1..close];
        at = close + 1;
        let closing = tag.starts_with('/');
        let name = tag_name(tag);

        if !closing && HIDDEN.contains(&name.as_str()) && !tag.ends_with('/') {
            let end = format!("</{name}");
            match find_ci(html, at, &end) {
                Some(found) => at = html[found..].find('>').map_or(html.len(), |offset| found + offset + 1),
                // A page may leave its head unclosed; the rest is still the page.
                None if name == "head" => {}
                None => break,
            }
            continue;
        }
        const VOID: [&str; 6] = ["input", "img", "br", "hr", "meta", "link"];
        if lean && !closing && !VOID.contains(&name.as_str()) && !tag.ends_with('/') && furniture(&name, tag) {
            at = end_of_element(html, at, &name);
            continue;
        }

        match name.as_str() {
            "a" if closing => page.close_link(),
            "a" => {
                page.close_link();
                let target = attribute(tag, "href").and_then(|href| resolve(base, &decode_entities(&href)));
                let label = attribute(tag, "aria-label").or_else(|| attribute(tag, "title")).map(|label| decode_entities(&label));
                page.link = target.map(|url| (url, String::new(), label));
            }
            "img" => {
                if let (Some((_, words, _)), Some(alt)) = (&mut page.link, attribute(tag, "alt")) {
                    if words.trim().is_empty() {
                        words.push_str(&decode_entities(&alt));
                    }
                }
            }
            "td" | "th" => page.text.push(' '),
            // A heading or an item may sit inside a link (`<a><h3>...</h3></a>`
            // on result pages): the link stays open and ends with the line.
            "h1" if !closing => {
                page.flush();
                page.gap();
                page.prefix = "# ";
            }
            "h2" | "h3" | "h4" | "h5" | "h6" if !closing => {
                page.flush();
                page.gap();
                page.prefix = "## ";
            }
            "li" if !closing => {
                page.flush();
                page.prefix = "- ";
            }
            "hr" => {
                page.close_link();
                page.flush();
                page.lines.push("---".to_string());
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" => {
                page.close_link();
                page.flush();
            }
            "p" if closing => {
                if page.link.is_none() {
                    page.flush();
                    page.gap();
                }
            }
            _ if BREAKS.contains(&name.as_str()) => {
                if page.link.is_none() {
                    page.flush();
                }
            }
            _ => {}
        }
    }
    page.close_link();
    page.flush();
    let mut lines = page.lines;
    lines.dedup();
    while lines.first().is_some_and(String::is_empty) {
        lines.remove(0);
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines.join("\n")
}

/// DuckDuckGo's HTML results laid out as results: the title as a link to the
/// real address, the snippet, the host.
pub fn duckduckgo(html: &str, query: &str) -> String {
    let mut out = String::from("# Recherche : ");
    push_drawable(&mut out, query);
    out.push_str("\nGoogle exige JavaScript pour chercher ; ces résultats viennent de DuckDuckGo.\n\n");
    let mut found = 0;
    let mut at = 0;
    while let Some(offset) = html[at..].find("class=\"result__a\"") {
        let class = at + offset;
        let tag_start = html[..class].rfind('<').unwrap_or(class);
        let Some(tag_end) = html[class..].find('>').map(|offset| class + offset) else {
            break;
        };
        let words_end = html[tag_end..].find("</a>").map_or(html.len(), |offset| tag_end + offset);
        at = words_end;
        let Some(target) = attribute(&html[tag_start + 1..tag_end], "href").map(|href| real_url(&decode_entities(&href)))
        else {
            continue;
        };
        let mut words = String::new();
        push_drawable(&mut words, &collapse(&strip_tags(&decode_entities(&html[tag_end + 1..words_end]))));
        if words.is_empty() {
            words = target.clone();
        }
        // The snippet, if this result has one before the next result starts.
        let until = html[at..].find("class=\"result__a\"").map_or(html.len(), |offset| at + offset);
        let snippet = html[at..until].find("class=\"result__snippet\"").and_then(|offset| {
            let start = at + offset;
            let open = start + html[start..until].find('>')? + 1;
            let close = open + html[open..until].find("</a>").or_else(|| html[open..until].find("</div>"))?;
            let mut text = String::new();
            push_drawable(&mut text, &collapse(&strip_tags(&decode_entities(&html[open..close]))));
            Some(text)
        });
        out.push_str(&format!("[{}]({})\n", words.replace(']', ")"), target));
        if let Some(snippet) = snippet.filter(|text| !text.is_empty()) {
            out.push_str(&snippet);
            out.push('\n');
        }
        out.push_str(host_of(&target));
        out.push_str("\n\n");
        found += 1;
    }
    if found == 0 {
        out.push_str("Aucun résultat, ou DuckDuckGo a demandé une vérification que ce navigateur ne sait pas faire.\n");
    }
    out
}

/// The address a DuckDuckGo redirect link leads to: its `uddg` parameter.
fn real_url(href: &str) -> String {
    let absolute = if href.starts_with("//") { format!("https:{href}") } else { href.to_string() };
    if let Some((_, rest)) = absolute.split_once("uddg=") {
        let decoded = percent_decode(rest.split('&').next().unwrap_or(""));
        if decoded.starts_with("http://") || decoded.starts_with("https://") {
            return escape_url(&decoded);
        }
    }
    absolute
}

/// The `q` parameter of an address, decoded: what was searched for.
fn query_of(url: &str) -> String {
    let Some((_, query)) = url.split_once('?') else {
        return String::new();
    };
    query.split('&').find_map(|pair| pair.strip_prefix("q=")).map(percent_decode).unwrap_or_default()
}

/// Runs of white space become one space; the ends are trimmed.
fn collapse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for word in text.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

fn strip_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut inside = false;
    for c in text.chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => out.push(c),
            _ => {}
        }
    }
    out
}

/// Where `needle` (ASCII, lower case) starts in `haystack` at or after `from`,
/// ignoring ASCII case, without copying the page.
fn find_ci(haystack: &str, from: usize, needle: &str) -> Option<usize> {
    let bytes = haystack.as_bytes();
    let needle = needle.as_bytes();
    let first = *needle.first()?;
    let mut at = from;
    while at + needle.len() <= bytes.len() {
        at += bytes[at..].iter().position(|byte| byte.to_ascii_lowercase() == first)?;
        if at + needle.len() > bytes.len() {
            return None;
        }
        if bytes[at..at + needle.len()].eq_ignore_ascii_case(needle) {
            return Some(at);
        }
        at += 1;
    }
    None
}

/// The lower-case name of a tag from what is between `<` and `>`.
fn tag_name(tag: &str) -> String {
    tag.trim_start_matches('/')
        .split(|c: char| c.is_ascii_whitespace() || c == '/')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// The value of attribute `name` (lower case) in a tag: quoted with `"` or
/// `'`, or bare.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let mut from = 0;
    while let Some(at) = find_ci(tag, from, name) {
        from = at + name.len();
        if at == 0 || !bytes[at - 1].is_ascii_whitespace() {
            continue;
        }
        let Some(value) = tag[from..].trim_start().strip_prefix('=') else {
            continue;
        };
        let value = value.trim_start();
        return Some(match value.as_bytes().first() {
            Some(&quote @ (b'"' | b'\'')) => value[1..].split(char::from(quote)).next().unwrap_or("").to_string(),
            _ => value.split(|c: char| c.is_ascii_whitespace()).next().unwrap_or("").to_string(),
        });
    }
    None
}

/// `href` made absolute against the address of the page it is on, or None for
/// what the browser cannot follow: other schemes (`javascript:`, `mailto:`)
/// and anchors within the page.
pub fn resolve(base: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() || href.starts_with('#') {
        return None;
    }
    let lower = href.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(escape_url(href));
    }
    // Any other scheme: letters, digits and `+ - .` before a colon.
    if let Some(colon) = href.find(':') {
        let before = &href[..colon];
        if !before.is_empty() && before.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
            return None;
        }
    }
    let (scheme, rest) = base.split_once("://").unwrap_or(("https", base));
    if let Some(rest) = href.strip_prefix("//") {
        return Some(escape_url(&format!("{scheme}://{rest}")));
    }
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let host = &rest[..host_end];
    let path = &rest[host_end..];
    let path = &path[..path.find(['?', '#']).unwrap_or(path.len())];
    let joined = if href.starts_with('/') {
        href.to_string()
    } else if href.starts_with('?') {
        format!("{}{href}", if path.is_empty() { "/" } else { path })
    } else {
        let directory = &path[..path.rfind('/').map_or(0, |slash| slash + 1)];
        format!("{}{href}", if directory.is_empty() { "/" } else { directory })
    };
    Some(escape_url(&format!("{scheme}://{host}{}", normalise(&joined))))
}

/// `.` and `..` segments of a path taken out; the query is left alone.
fn normalise(path: &str) -> String {
    let (path, query) = path.split_at(path.find(['?', '#']).unwrap_or(path.len()));
    if !path.contains("/.") {
        return format!("{path}{query}");
    }
    let mut segments: Vec<&str> = Vec::new();
    for segment in path.split('/').skip(1) {
        match segment {
            "." => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }
    let mut out = String::new();
    for segment in &segments {
        out.push('/');
        out.push_str(segment);
    }
    if path.ends_with("/.") || path.ends_with("/..") || out.is_empty() {
        out.push('/');
    }
    format!("{out}{query}")
}

/// Spaces, quotes, brackets and non-ASCII characters percent-encoded, so the
/// address can go into a request line as it is and a `)` never ends a link
/// of the line format early.
fn escape_url(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    for byte in url.bytes() {
        let unsafe_byte = matches!(byte, b'"' | b'<' | b'>' | b'\\' | b'^' | b'`' | b'{' | b'|' | b'}' | b'(' | b')' | b'[' | b']');
        if byte > b' ' && byte < 0x7F && !unsafe_byte {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The host of an absolute address.
pub fn host_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    authority.split(':').next().unwrap_or(authority)
}

/// `%XX` escapes and `+` decoded, as a query string writes them.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let digit = |index: usize| bytes.get(index).and_then(|&byte| char::from(byte).to_digit(16));
        match (bytes[at], digit(at + 1), digit(at + 2)) {
            (b'%', Some(high), Some(low)) => {
                out.push((high * 16 + low) as u8);
                at += 3;
                continue;
            }
            (b'+', _, _) => out.push(b' '),
            (byte, _, _) => out.push(byte),
        }
        at += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// HTML entities: the XML five, `&nbsp;`, the named ones French pages use,
/// and numeric ones in decimal or hexadecimal. Anything else stays as written.
pub fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let decoded = rest[1..].find(';').filter(|&end| end <= 10).and_then(|end| {
            let name = &rest[1..=end];
            let c = match name.strip_prefix('#') {
                Some(number) => match number.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => number.parse::<u32>().ok(),
                }
                .and_then(char::from_u32)?,
                None => named(name)?,
            };
            Some((c, end + 2))
        });
        match decoded {
            Some((c, length)) => {
                out.push(c);
                rest = &rest[length..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn named(name: &str) -> Option<char> {
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        "eacute" => 'é',
        "egrave" => 'è',
        "ecirc" => 'ê',
        "euml" => 'ë',
        "agrave" => 'à',
        "acirc" => 'â',
        "ccedil" => 'ç',
        "icirc" => 'î',
        "iuml" => 'ï',
        "ocirc" => 'ô',
        "ouml" => 'ö',
        "ucirc" => 'û',
        "ugrave" => 'ù',
        "uuml" => 'ü',
        "Eacute" => 'É',
        "Agrave" => 'À',
        "Ccedil" => 'Ç',
        "laquo" => '«',
        "raquo" => '»',
        "copy" => '©',
        "reg" => '®',
        "deg" => '°',
        "middot" => '·',
        "times" => '×',
        "hellip" => '\u{2026}',
        "mdash" => '\u{2014}',
        "ndash" => '\u{2013}',
        "lsquo" => '\u{2018}',
        "rsquo" => '\u{2019}',
        "ldquo" => '\u{201C}',
        "rdquo" => '\u{201D}',
        "bull" => '\u{2022}',
        "euro" => '\u{20AC}',
        _ => return None,
    })
}

/// Checks the decoders on fixed inputs (docs/specs/browser-search.md §5):
/// `Ok`, or the name of the first case that failed.
pub fn self_test() -> Result<(), &'static str> {
    if decode(b"caf\xE9", Some("text/html; charset=ISO-8859-1")) != "café" {
        return Err("iso-8859-1");
    }
    if decode("café".as_bytes(), Some("text/html; charset=utf-8")) != "café" {
        return Err("utf-8");
    }
    let mut drawn = String::new();
    push_drawable(&mut drawn, "l\u{2019}été \u{2014} fin\u{2026} \u{4E2D}");
    if drawn != "l'été - fin... ?" {
        return Err("latin-1 folding");
    }
    if decode_entities("&eacute;t&#233; &#xE9;&amp;&lt;b&gt; &bogus; & fin") != "été é&<b> &bogus; & fin" {
        return Err("entities");
    }
    let page = page(
        b"<html><head><title>Essai &amp; page</title><script>var a = '<p>no</p>';</script></head>\
          <body><h1>Bonjour</h1><p>Un <b>texte</b> et <a href=\"../autre.html?x=1&amp;y=2\">un lien</a>.</p>\
          <ul><li>premier</li><li>second</li></ul><a href=\"javascript:void(0)\">rien</a></body></html>",
        Some("text/html; charset=utf-8"),
        "https://exemple.fr/dossier/page.html",
    );
    if page.title != "Essai & page" {
        return Err("title");
    }
    let expected = "# Bonjour\nUn texte et [un lien](https://exemple.fr/autre.html?x=1&y=2).\n\n- premier\n- second\nrien";
    if page.lines != expected {
        return Err("html lines");
    }
    let results = self::page(
        "<div><h2 class=\"result__title\"><a rel=\"nofollow\" class=\"result__a\" \
         href=\"//duckduckgo.com/l/?uddg=https%3A%2F%2Fexemple.org%2Fnoyau&amp;rut=abc\">Le <b>noyau</b></a></h2>\
         <a class=\"result__snippet\" href=\"//duckduckgo.com/l/?uddg=x\">Un <b>noyau</b> écrit en Rust</a></div>"
            .as_bytes(),
        Some("text/html; charset=UTF-8"),
        "https://html.duckduckgo.com/html/?q=noyau+rust",
    );
    if !results.lines.starts_with("# Recherche : noyau rust\n")
        || !results.lines.contains("[Le noyau](https://exemple.org/noyau)\nUn noyau écrit en Rust\nexemple.org\n")
    {
        return Err("duckduckgo results");
    }
    Ok(())
}
