//! Page meta: tags a plugin adds to `index.html` for one SPA path (e.g. `og:image` for `/fight`).

/// `<meta name=…>` or `<meta property=…>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetaAttr {
    Name,
    Property,
}

/// One `<meta>` tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetaTag {
    pub attr: MetaAttr,
    pub key: String,
    pub content: String,
}

impl MetaTag {
    /// `<meta name="key" content="…">`, e.g. `twitter:card`.
    pub fn name(key: impl Into<String>, content: impl Into<String>) -> Self {
        MetaTag {
            attr: MetaAttr::Name,
            key: key.into(),
            content: content.into(),
        }
    }

    /// `<meta property="key" content="…">`, e.g. `og:image`.
    pub fn property(key: impl Into<String>, content: impl Into<String>) -> Self {
        MetaTag {
            attr: MetaAttr::Property,
            key: key.into(),
            content: content.into(),
        }
    }
}

/// What a plugin injects into `index.html` for one path.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageMeta {
    /// Replaces the `<title>` text when set.
    pub title: Option<String>,
    pub tags: Vec<MetaTag>,
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Puts `meta` into `html`: replaces the title text and adds the tags before `</head>`.
/// Every value is HTML-escaped.
pub fn inject(html: &str, meta: &PageMeta) -> String {
    let mut out = html.to_string();
    if let Some(title) = &meta.title
        && let (Some(start), Some(end)) = (out.find("<title>"), out.find("</title>"))
        && start < end
    {
        out.replace_range(start + "<title>".len()..end, &escape(title));
    }
    let tags: String = meta
        .tags
        .iter()
        .map(|t| {
            let attr = match t.attr {
                MetaAttr::Name => "name",
                MetaAttr::Property => "property",
            };
            format!(
                "<meta {attr}=\"{}\" content=\"{}\">\n",
                escape(&t.key),
                escape(&t.content)
            )
        })
        .collect();
    match out.find("</head>") {
        Some(at) => out.insert_str(at, &tags),
        None => out.insert_str(0, &tags),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_escaped_tags_and_title() {
        let html = "<html><head><title>KOL Arena</title></head><body></body></html>";
        let meta = PageMeta {
            title: Some("Decu vs \"Tekkerrss\"".into()),
            tags: vec![
                MetaTag::property("og:image", "https://x/api/fight/image?ids=a,b&date=2026-10-18"),
                MetaTag::name("twitter:card", "summary_large_image"),
            ],
        };
        let out = inject(html, &meta);
        assert!(out.contains("<title>Decu vs &quot;Tekkerrss&quot;</title>"), "{out}");
        assert!(out.contains(
            "<meta property=\"og:image\" content=\"https://x/api/fight/image?ids=a,b&amp;date=2026-10-18\">\n<meta name"
        ), "{out}");
        assert!(out.contains("<meta name=\"twitter:card\" content=\"summary_large_image\">\n</head>"), "{out}");
    }

    #[test]
    fn empty_meta_keeps_page() {
        let html = "<html><head></head></html>";
        assert_eq!(inject(html, &PageMeta::default()), html);
    }
}
