//! Turns whatever the user typed into the address bar into a URL.

use std::path::Path;

/// URL schemes accepted verbatim. Anything else that looks like `word:` is more
/// likely `host:port` (e.g. `localhost:3000`) than a scheme.
const KNOWN_SCHEMES: &[&str] = &[
    "http",
    "https",
    "file",
    "about",
    "data",
    "blob",
    "view-source",
    "ftp",
];

/// The URL a blank tab shows.
pub const BLANK_URL: &str = "about:blank";

/// Resolves address bar input to an absolute URL.
///
/// `search_template` is a URL containing `%s`, replaced by the percent-encoded query.
///
/// * `https://example.com`, `about:blank`, `file:///tmp/a.html` are used as typed.
/// * `example.com/path` becomes `https://example.com/path`.
/// * `localhost:3000`, `127.0.0.1:8080` become `http://…` (local development servers
///   rarely speak TLS).
/// * An existing absolute path becomes a `file://` URL.
/// * Everything else is a web search.
pub fn resolve(input: &str, search_template: &str) -> String {
    let input = input.trim();
    if input.is_empty() {
        return BLANK_URL.to_string();
    }
    if has_known_scheme(input) {
        return input.to_string();
    }
    if input.starts_with('/') && !input.contains(' ') && Path::new(input).exists() {
        return format!("file://{}", encode_path(input));
    }
    if let Some(url) = as_host(input) {
        return url;
    }
    search_template.replace("%s", &encode_query(input))
}

fn has_known_scheme(input: &str) -> bool {
    let Some((scheme, rest)) = input.split_once(':') else {
        return false;
    };
    if !KNOWN_SCHEMES.iter().any(|s| scheme.eq_ignore_ascii_case(s)) {
        return false;
    }
    // `http:3000` is not a URL; hierarchical schemes need `//` (or be one of the opaque ones).
    match scheme.to_ascii_lowercase().as_str() {
        "http" | "https" | "ftp" | "file" => rest.starts_with("//"),
        _ => true,
    }
}

/// Recognizes `host[:port][/path][?query][#fragment]` and returns it with a scheme.
fn as_host(input: &str) -> Option<String> {
    if input.contains(char::is_whitespace) {
        return None;
    }
    let authority_end = input.find(['/', '?', '#']).unwrap_or(input.len());
    let authority = &input[..authority_end];
    // Userinfo is not something a person types into an address bar on purpose.
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let (host, port) = if authority.starts_with('[') {
        // Bracketed IPv6: `[::1]` or `[::1]:8080`.
        let close = authority.find(']')?;
        let (host, rest) = authority.split_at(close + 1);
        if rest.is_empty() {
            (host, None)
        } else {
            (host, Some(rest.strip_prefix(':')?))
        }
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };
    if let Some(port) = port {
        if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }

    let local = host.eq_ignore_ascii_case("localhost")
        || host.parse::<std::net::Ipv4Addr>().is_ok()
        || (host.starts_with('[') && host.ends_with(']'));
    if local {
        return Some(format!("http://{input}"));
    }
    if is_domain_name(host) {
        return Some(format!("https://{input}"));
    }
    None
}

/// `a.b` where every label is a valid hostname label and the last one is alphabetic
/// (so `3.14` and `v1.2.3` are searches, `example.com` and `foo.dev` are hosts).
fn is_domain_name(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    let valid_label = |label: &&str| {
        !label.is_empty()
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.chars().all(|c| c.is_alphanumeric() || c == '-')
    };
    if !labels.iter().all(valid_label) {
        return false;
    }
    let tld = labels[labels.len() - 1];
    tld.chars().count() >= 2 && tld.chars().all(char::is_alphabetic)
}

/// Percent-encodes a search query (`application/x-www-form-urlencoded`-style).
fn encode_query(query: &str) -> String {
    let mut out = String::with_capacity(query.len());
    for byte in query.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Whether a URL is a real page worth remembering (history, bookmarks, session).
/// Blank pages, `data:` URLs and the like are not.
pub fn is_web_url(url: &str) -> bool {
    ["http://", "https://", "file://"].iter().any(|prefix| {
        // `get` (not indexing): the cut may fall inside a multi-byte character.
        url.len() > prefix.len()
            && url
                .get(..prefix.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
    })
}

/// The URL without its `#fragment`; two URLs differing only there are the same page
/// for bookmarking and history.
pub fn without_fragment(url: &str) -> &str {
    url.split_once('#').map_or(url, |(base, _)| base)
}

/// What the address bar shows for a page: nothing for a blank page, so the user can type.
pub fn display_url(url: &str) -> &str {
    if url == BLANK_URL { "" } else { url }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEARCH: &str = "https://search.example/?q=%s";

    fn r(input: &str) -> String {
        resolve(input, SEARCH)
    }

    #[test]
    fn empty_is_blank() {
        assert_eq!(r(""), BLANK_URL);
        assert_eq!(r("   "), BLANK_URL);
    }

    #[test]
    fn explicit_urls_are_kept() {
        assert_eq!(r("https://example.com/a?b=c"), "https://example.com/a?b=c");
        assert_eq!(r("http://example.com"), "http://example.com");
        assert_eq!(r("about:blank"), "about:blank");
        assert_eq!(r("file:///tmp/a.html"), "file:///tmp/a.html");
        assert_eq!(r("view-source:https://example.com"), "view-source:https://example.com");
    }

    #[test]
    fn bare_domains_get_https() {
        assert_eq!(r("example.com"), "https://example.com");
        assert_eq!(r("example.com/path?x=1"), "https://example.com/path?x=1");
        assert_eq!(r("docs.rs"), "https://docs.rs");
        assert_eq!(r("example.com:8443"), "https://example.com:8443");
    }

    #[test]
    fn local_servers_get_http() {
        assert_eq!(r("localhost"), "http://localhost");
        assert_eq!(r("localhost:3000"), "http://localhost:3000");
        assert_eq!(r("localhost:3000/app"), "http://localhost:3000/app");
        assert_eq!(r("127.0.0.1:8080"), "http://127.0.0.1:8080");
        assert_eq!(r("[::1]:8080"), "http://[::1]:8080");
        assert_eq!(r("[::1]"), "http://[::1]");
    }

    #[test]
    fn host_port_is_not_mistaken_for_a_scheme() {
        // `localhost:` would parse as a URL with scheme "localhost".
        assert_eq!(r("localhost:3000"), "http://localhost:3000");
        assert_eq!(r("http:3000"), format!("https://search.example/?q=http%3A3000"));
    }

    #[test]
    fn everything_else_is_a_search() {
        assert_eq!(r("rust lifetimes"), "https://search.example/?q=rust+lifetimes");
        assert_eq!(r("hello"), "https://search.example/?q=hello");
        assert_eq!(r("3.14"), "https://search.example/?q=3.14");
        assert_eq!(r("v1.2.3"), "https://search.example/?q=v1.2.3");
        assert_eq!(r("what is example.com?"), "https://search.example/?q=what+is+example.com%3F");
    }

    #[test]
    fn search_queries_are_percent_encoded() {
        assert_eq!(r("a&b=c#d"), "https://search.example/?q=a%26b%3Dc%23d");
        assert_eq!(r("日本語"), "https://search.example/?q=%E6%97%A5%E6%9C%AC%E8%AA%9E");
        assert_eq!(r("100%"), "https://search.example/?q=100%25");
    }

    #[test]
    fn userinfo_is_not_a_host() {
        assert_eq!(r("me@example.com"), "https://search.example/?q=me%40example.com");
    }

    #[test]
    fn existing_absolute_paths_become_file_urls() {
        let dir = std::env::temp_dir();
        let dir = dir.to_str().expect("utf-8 temp dir");
        assert_eq!(r(dir), format!("file://{}", encode_path(dir)));
        assert!(r("/definitely/not/here/xyz").starts_with("https://search.example/"));
    }

    #[test]
    fn web_urls() {
        assert!(is_web_url("https://example.com"));
        assert!(is_web_url("HTTP://example.com"));
        assert!(is_web_url("file:///tmp/a.html"));
        assert!(!is_web_url("https://"));
        assert!(!is_web_url("about:blank"));
        assert!(!is_web_url("日本語日本語"), "must not panic on multi-byte input");
    }

    #[test]
    fn fragments_are_stripped() {
        assert_eq!(without_fragment("https://a.test/x#y"), "https://a.test/x");
        assert_eq!(without_fragment("https://a.test/x"), "https://a.test/x");
    }

    #[test]
    fn display_hides_blank() {
        assert_eq!(display_url(BLANK_URL), "");
        assert_eq!(display_url("https://example.com"), "https://example.com");
    }
}
