//! Find in page.
//!
//! The web engine has no find-in-page API, so matches are highlighted by a script run
//! inside the page: text nodes are split and the matches wrapped in `<mark>` elements.
//! This module builds that script and parses its answer; it never runs anything itself.
//!
//! Every script evaluates to the string `"<current>,<total>"` (1-based `current`,
//! `0,0` when there is no match).

/// What to do in the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindCommand<'a> {
    /// Highlight every match of `query` and select the first.
    Search { query: &'a str, case_sensitive: bool },
    /// Select the next match (wraps around).
    Next,
    /// Select the previous match (wraps around).
    Previous,
    /// Remove all highlighting.
    Clear,
}

/// Where the search stands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FindStatus {
    /// 1-based index of the selected match; 0 when there is none.
    pub current: usize,
    pub total: usize,
}

/// Installs the helper object once per page, then runs one command against it.
const TEMPLATE: &str = r"(function () {
  var F = window.__nitiumFind;
  if (!F) {
    F = window.__nitiumFind = { marks: [], index: -1 };
    F.clear = function () {
      for (var i = 0; i < F.marks.length; i++) {
        var m = F.marks[i], p = m.parentNode;
        if (!p) continue;
        p.replaceChild(document.createTextNode(m.textContent), m);
        p.normalize();
      }
      F.marks = [];
      F.index = -1;
      return '0,0';
    };
    F.select = function (i) {
      var n = F.marks.length;
      if (!n) { F.index = -1; return '0,0'; }
      if (F.index >= 0 && F.marks[F.index]) F.marks[F.index].style.background = '#ffe066';
      F.index = ((i % n) + n) % n;
      var m = F.marks[F.index];
      m.style.background = '#ff9632';
      try { m.scrollIntoView({ block: 'center', inline: 'nearest' }); } catch (e) { m.scrollIntoView(); }
      return (F.index + 1) + ',' + n;
    };
    F.step = function (d) { return F.select(F.index + d); };
    F.search = function (q, cs) {
      F.clear();
      if (!q || !document.body) return '0,0';
      var needle = cs ? q : q.toLowerCase();
      var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, {
        acceptNode: function (n) {
          var p = n.parentNode;
          if (!p || !n.nodeValue) return NodeFilter.FILTER_REJECT;
          var t = p.nodeName;
          if (t === 'SCRIPT' || t === 'STYLE' || t === 'NOSCRIPT' || t === 'TEXTAREA') return NodeFilter.FILTER_REJECT;
          return NodeFilter.FILTER_ACCEPT;
        }
      });
      var nodes = [], n;
      while ((n = walker.nextNode())) nodes.push(n);
      for (var k = 0; k < nodes.length; k++) {
        var node = nodes[k], text = node.nodeValue;
        var hay = cs ? text : text.toLowerCase();
        var from = 0, pos, last = 0, frag = null;
        while ((pos = hay.indexOf(needle, from)) !== -1) {
          if (!frag) frag = document.createDocumentFragment();
          if (pos > last) frag.appendChild(document.createTextNode(text.slice(last, pos)));
          var mark = document.createElement('mark');
          mark.setAttribute('data-nitium-find', '');
          mark.style.cssText = 'background:#ffe066;color:#000;';
          mark.textContent = text.slice(pos, pos + needle.length);
          frag.appendChild(mark);
          F.marks.push(mark);
          last = from = pos + needle.length;
        }
        if (frag) {
          if (last < text.length) frag.appendChild(document.createTextNode(text.slice(last)));
          node.parentNode.replaceChild(frag, node);
        }
      }
      return F.select(0);
    };
  }
  return @COMMAND@;
})()";

/// The script for `command`.
pub fn script(command: FindCommand<'_>) -> String {
    let call = match command {
        FindCommand::Search {
            query,
            case_sensitive,
        } => format!("F.search({}, {case_sensitive})", js_string_literal(query)),
        FindCommand::Next => "F.step(1)".to_string(),
        FindCommand::Previous => "F.step(-1)".to_string(),
        FindCommand::Clear => "F.clear()".to_string(),
    };
    TEMPLATE.replace("@COMMAND@", &call)
}

/// Parses the `"<current>,<total>"` answer. Anything else counts as "no match".
pub fn parse_status(answer: &str) -> FindStatus {
    let parse = || {
        let (current, total) = answer.trim().split_once(',')?;
        Some(FindStatus {
            current: current.parse().ok()?,
            total: total.parse().ok()?,
        })
    };
    parse().unwrap_or_default()
}

/// A double-quoted JavaScript string literal that evaluates to exactly `text`.
///
/// Escapes everything that could end the literal or the script: quotes, backslashes,
/// line terminators (including U+2028/2029, which are line terminators in JavaScript),
/// other control characters and `<` (so the literal is also safe inside a `<script>`).
fn js_string_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '<' => out.push_str("\\u003c"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\u{:04x}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_escape_everything_dangerous() {
        assert_eq!(js_string_literal("plain"), r#""plain""#);
        assert_eq!(js_string_literal(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(js_string_literal("a\nb\r\tc"), r#""a\nb\r\tc""#);
        // `<` becomes a backslash, `u003c` (spelled out to keep this line escape-free).
        assert_eq!(
            js_string_literal("</script>"),
            format!("\"{}u003c/script>\"", '\\')
        );
        // U+2028 / U+2029 are line terminators in JavaScript: the literal must spell them out.
        assert_eq!(
            js_string_literal("a\u{2028}b\u{2029}"),
            format!("\"a{0}u2028b{0}u2029\"", '\\')
        );
        assert_eq!(js_string_literal("\u{0}\u{1f}\u{7f}"), r#""\u0000\u001f\u007f""#);
        assert_eq!(js_string_literal("日本語"), "\"日本語\"");
    }

    #[test]
    fn a_hostile_query_stays_inside_its_literal() {
        let query = r#"");alert(1);//"#;
        let script = script(FindCommand::Search {
            query,
            case_sensitive: false,
        });
        // The quote that would close the literal is escaped, so the payload stays data.
        assert!(script.contains(r#"return F.search("\");alert(1);//", false);"#));
        assert_eq!(script.matches("alert(1)").count(), 1);
    }

    #[test]
    fn commands_are_embedded() {
        assert!(script(FindCommand::Next).contains("return F.step(1);"));
        assert!(script(FindCommand::Previous).contains("return F.step(-1);"));
        assert!(script(FindCommand::Clear).contains("return F.clear();"));
        let search = script(FindCommand::Search {
            query: "needle",
            case_sensitive: true,
        });
        assert!(search.contains(r#"return F.search("needle", true);"#));
    }

    #[test]
    fn template_has_no_leftover_placeholder_and_balanced_braces() {
        let s = script(FindCommand::Clear);
        assert!(!s.contains("@COMMAND@"));
        let count = |c: char| s.chars().filter(|&x| x == c).count();
        assert_eq!(count('{'), count('}'));
        assert_eq!(count('('), count(')'));
        assert!(s.starts_with("(function () {") && s.ends_with("})()"));
    }

    #[test]
    fn parses_answers() {
        assert_eq!(parse_status("3,12"), FindStatus { current: 3, total: 12 });
        assert_eq!(parse_status(" 1,1 "), FindStatus { current: 1, total: 1 });
        assert_eq!(parse_status("0,0"), FindStatus::default());
        assert_eq!(parse_status(""), FindStatus::default());
        assert_eq!(parse_status("garbage"), FindStatus::default());
        assert_eq!(parse_status("1,x"), FindStatus::default());
        assert_eq!(parse_status("-1,3"), FindStatus::default());
    }
}
