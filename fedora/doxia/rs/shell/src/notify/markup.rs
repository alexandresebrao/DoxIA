// Notification bodies into Pango markup. The spec's body markup is a small
// HTML subset (b, i, u, a href, img); only b/i/u/a survive here, br becomes a
// newline and every other tag goes. Like the QML stripper, a `<` opens a tag
// that runs to the next `>`, so dropping one can never splice its neighbours
// into a new tag; GTK never fetches anything, so img is simply dropped.

use super::model::is_chromium;

fn tag_name(tag: &str) -> (bool, String) {
  let inner = tag.trim_start_matches('<');
  let closing = inner.trim_start().starts_with('/');
  let name: String = inner
    .trim_start()
    .trim_start_matches('/')
    .chars()
    .take_while(|c| c.is_ascii_alphanumeric())
    .collect::<String>()
    .to_lowercase();
  (closing, name)
}

fn attr(tag: &str, name: &str) -> Option<String> {
  let lower = tag.to_lowercase();
  let at = lower.find(&format!("{name}="))? + name.len() + 1;
  let rest = &tag[at..];
  let quote = rest.chars().next()?;
  if quote == '"' || quote == '\'' {
    let body = &rest[1..];
    body.find(quote).map(|end| body[..end].to_string())
  } else {
    Some(rest.chars().take_while(|c| !c.is_whitespace() && *c != '>').collect())
  }
}

fn escape(text: &str) -> String {
  gtk::glib::markup_escape_text(text).to_string()
}

// Text between tags may hold entities; keep the valid ones, escape the rest.
fn text_markup(text: &str) -> String {
  let mut out = String::new();
  let mut rest = text;
  while let Some(amp) = rest.find('&') {
    out += &escape(&rest[..amp]);
    let after = &rest[amp..];
    let entity_end = after.find(';').filter(|end| *end <= 10);
    let valid = entity_end.is_some_and(|end| {
      let name = &after[1..end];
      matches!(name, "amp" | "lt" | "gt" | "quot" | "apos")
        || name.strip_prefix("#x").is_some_and(|h| !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit()))
        || name.strip_prefix('#').is_some_and(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()))
    });
    if valid {
      let end = entity_end.unwrap();
      out += &after[..=end];
      rest = &after[end + 1..];
    } else {
      out += "&amp;";
      rest = &after[1..];
    }
  }
  out + &escape(rest)
}

// Chromium-family senders lead the body with the site's address.
fn drop_leading_site(body: &str) -> String {
  let trimmed = body.trim_start();
  let (first, rest) = match trimmed.find(char::is_whitespace) {
    Some(i) => (&trimmed[..i], &trimmed[i..]),
    None => (trimmed, ""),
  };
  let link_text = if first.to_lowercase().starts_with("<a") {
    // <a href=…>example.com</a> as the first thing.
    match trimmed.find("</a>").or_else(|| trimmed.find("</A>")) {
      Some(end) => {
        let open_end = trimmed.find('>').unwrap_or(0);
        let text = &trimmed[open_end + 1..end];
        if looks_like_site(text.trim()) {
          return trimmed[end + 4..].trim_start().to_string();
        }
        return body.to_string();
      }
      None => return body.to_string(),
    }
  } else {
    first
  };
  if !rest.is_empty() && looks_like_site(link_text) { rest.trim_start().to_string() } else { body.to_string() }
}

fn looks_like_site(s: &str) -> bool {
  let s = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://")).unwrap_or(s);
  let host = s.split(['/', ':']).next().unwrap_or("");
  let parts: Vec<&str> = host.split('.').collect();
  parts.len() >= 2
    && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
    && parts.last().is_some_and(|tld| tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()))
}

pub struct Body {
  // Pango markup for the label.
  pub markup: String,
  // Whether anything visible is left.
  pub empty: bool,
}

pub fn body(raw: &str, app: &str, app_icon: &str) -> Body {
  let source = if is_chromium(app, app_icon) { drop_leading_site(raw) } else { raw.to_string() };
  let mut out = String::new();
  let mut plain = String::new();
  let mut open: Vec<&'static str> = Vec::new();
  let mut rest = source.as_str();
  while !rest.is_empty() {
    let Some(lt) = rest.find('<') else {
      out += &text_markup(rest);
      plain += rest;
      break;
    };
    out += &text_markup(&rest[..lt]);
    plain += &rest[..lt];
    let after = &rest[lt..];
    let (tag, next) = match after.find('>') {
      Some(gt) => (&after[..=gt], &after[gt + 1..]),
      None => (after, ""),
    };
    rest = next;
    let (closing, name) = tag_name(tag);
    let known: Option<&'static str> = match name.as_str() {
      "b" | "strong" => Some("b"),
      "i" | "em" => Some("i"),
      "u" => Some("u"),
      "a" => Some("a"),
      _ => None,
    };
    if name == "br" {
      out.push('\n');
      plain.push('\n');
      continue;
    }
    let Some(t) = known else { continue };
    if closing {
      if open.last() == Some(&t) {
        open.pop();
        out += &format!("</{t}>");
      }
    } else if t == "a" {
      match attr(tag, "href").filter(|h| h.starts_with("http://") || h.starts_with("https://")) {
        Some(href) => {
          out += &format!("<a href=\"{}\">", escape(&href));
          open.push("a");
        }
        // A link without a usable target is just its text.
        None => {}
      }
    } else {
      out += &format!("<{t}>");
      open.push(t);
    }
  }
  while let Some(t) = open.pop() {
    out += &format!("</{t}>");
  }
  let out = out.replace("\r\n", "\n").replace('\r', "\n");
  let empty = plain.trim().is_empty();
  // Anything Pango still refuses becomes plain text. Links are GtkLabel's,
  // not Pango's, so check them as spans.
  let mut check = String::new();
  let mut rest = out.as_str();
  while let Some(start) = rest.find("<a href=\"") {
    check += &rest[..start];
    let after = &rest[start..];
    let end = after.find('>').map(|e| e + 1).unwrap_or(after.len());
    check += "<span>";
    rest = &after[end..];
  }
  check += rest;
  let check = check.replace("</a>", "</span>");
  if gtk::pango::parse_markup(&check, '\0').is_err() {
    return Body { markup: escape(plain.trim()), empty };
  }
  Body { markup: out.trim().to_string(), empty }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn m(raw: &str) -> String {
    body(raw, "App", "").markup
  }

  #[test]
  fn keeps_the_spec_subset() {
    assert_eq!(m("<b>Oi</b> <i>mundo</i>"), "<b>Oi</b> <i>mundo</i>");
    assert_eq!(m("a<br/>b"), "a\nb");
    assert_eq!(m(r#"<a href="https://x.com/a?b=1&c=2">link</a>"#), r#"<a href="https://x.com/a?b=1&amp;c=2">link</a>"#);
  }

  #[test]
  fn drops_images_and_unknown_tags() {
    assert_eq!(m(r#"x<img src="http://a/b.png">y"#), "xy");
    // The QML splice case: one malformed tag, nothing manufactured.
    assert_eq!(m(r#"<im<img src="http://a/d.png">g src="http://a/b.png">ok"#), "g src=&quot;http://a/b.png&quot;&gt;ok");
    assert_eq!(m("<script>alert(1)</script>"), "alert(1)");
    assert_eq!(m(r#"<a href="javascript:x">t</a>"#), "t");
  }

  #[test]
  fn balances_and_escapes() {
    assert_eq!(m("<b>open"), "<b>open</b>");
    assert_eq!(m("</i>x"), "x");
    assert_eq!(m("Tom & Jerry &amp; co &#233;"), "Tom &amp; Jerry &amp; co &#233;");
    assert_eq!(m("1 < 2"), "1");
  }

  #[test]
  fn chromium_site_prefix() {
    assert_eq!(body("web.whatsapp.com Oi, tudo bem?", "Google Chrome", "").markup, "Oi, tudo bem?");
    assert_eq!(body("<a href=\"https://mail.google.com\">mail.google.com</a> Nova mensagem", "chromium", "").markup, "Nova mensagem");
    assert_eq!(body("Só texto", "chromium", "").markup, "Só texto");
  }

  #[test]
  fn empty_bodies() {
    assert!(body("", "App", "").empty);
    assert!(body("<img src=\"x\">", "App", "").empty);
    assert!(!body("a", "App", "").empty);
  }
}
