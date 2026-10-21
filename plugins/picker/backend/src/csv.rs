//! CSV for the exports: quoted where needed, and safe to open in a spreadsheet.

/// One cell. A value a spreadsheet would read as a formula gets a leading `'`.
pub fn cell(value: &str) -> String {
    let mut v = value.to_string();
    if v.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        v.insert(0, '\'');
    }
    if v.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v
    }
}

/// Lines joined by CRLF, ending with one.
pub fn table(header: &[&str], rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    let line = |cells: Vec<String>| cells.join(",") + "\r\n";
    out.push_str(&line(header.iter().map(|h| cell(h)).collect()));
    for r in rows {
        out.push_str(&line(r.iter().map(|c| cell(c)).collect()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_are_quoted_and_formulas_defused() {
        assert_eq!(cell("plain"), "plain");
        assert_eq!(cell("a, b"), "\"a, b\"");
        assert_eq!(cell("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(cell("=HYPERLINK(\"x\")"), "\"'=HYPERLINK(\"\"x\"\")\"");
        assert_eq!(cell("+1"), "'+1");
        assert_eq!(cell("@x"), "'@x");
        assert_eq!(
            table(&["A", "B"], &[vec!["1".into(), "2".into()]]),
            "A,B\r\n1,2\r\n"
        );
    }
}
