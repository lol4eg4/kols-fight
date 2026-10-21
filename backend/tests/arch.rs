//! Architecture rules, checked on every `cargo test -p server`:
//! - the server's plugin list equals the `plugins/*/plugin.toml` set;
//! - a plugin backend depends only on `arena-platform`, its own API crate and the API crates
//!   of plugins it may call (`uses`, emitters of backend events it listens to, hosts of API
//!   slots it fills), all in the same or a lower layer; never on another backend;
//! - an API crate depends only on API crates of plugins in its `uses`;
//! - no plugin cycles;
//! - every route is registered with a tag from its `plugin.toml`, and only through `Route`;
//! - SQL in `plugins/*/backend` names only the plugin's own schema.

use arena_platform::{Config, Ctx, Manifest, construction_order};
use serde_json::Value;
use server::{APP, PLUGINS, TAGS};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    process::Command,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn disk_manifests() -> Vec<Manifest> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(root().join("plugins")).unwrap() {
        let path = entry.unwrap().path().join("plugin.toml");
        if path.is_file() {
            let text = std::fs::read_to_string(&path).unwrap();
            let m = Manifest::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let dir = path.parent().unwrap().file_name().unwrap().to_str().unwrap();
            assert_eq!(m.name, dir, "{}: name differs from folder", path.display());
            out.push(m);
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[test]
fn plugin_list_equals_manifests() {
    let on_disk: BTreeSet<String> = disk_manifests().into_iter().map(|m| m.name).collect();
    let listed: Vec<String> = PLUGINS
        .iter()
        .map(|e| Manifest::parse(e.manifest).unwrap().name)
        .collect();
    let listed_set: BTreeSet<String> = listed.iter().cloned().collect();
    assert_eq!(listed.len(), listed_set.len(), "plugin listed twice: {listed:?}");
    assert_eq!(listed_set, on_disk);
}

#[test]
fn no_plugin_cycles() {
    let manifests = disk_manifests();
    if let Err(e) = construction_order(&manifests) {
        panic!("{e}");
    }
}

/// Plugins whose API crates `m`'s backend may depend on (besides its own).
fn allowed_backend_apis(m: &Manifest, all: &[Manifest]) -> BTreeSet<String> {
    let mut allowed: BTreeSet<String> = m.uses.iter().cloned().collect();
    for event in m.backend_listens() {
        for emitter in all.iter().filter(|o| o.emits.iter().any(|e| e == event)) {
            allowed.insert(emitter.name.clone());
        }
    }
    for host in m.api_slot_hosts(all) {
        allowed.insert(host.to_string());
    }
    allowed.remove(&m.name);
    allowed
}

fn workspace_crates() -> BTreeMap<String, Vec<String>> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let out = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps", "--offline"])
        .current_dir(root())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "cargo metadata: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let meta: Value = serde_json::from_slice(&out.stdout).unwrap();
    let mut crates = BTreeMap::new();
    for p in meta["packages"].as_array().unwrap() {
        let deps = p["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["name"].as_str().unwrap().to_string())
            .collect();
        crates.insert(p["name"].as_str().unwrap().to_string(), deps);
    }
    crates
}

#[test]
fn plugin_dependencies_follow_uses_and_layers() {
    let all = disk_manifests();
    let by_name: BTreeMap<&str, &Manifest> = all.iter().map(|m| (m.name.as_str(), m)).collect();
    let crates = workspace_crates();
    let mut errors = Vec::new();
    for m in &all {
        let backend = format!("plugin-{}", m.name);
        let Some(deps) = crates.get(&backend) else {
            errors.push(format!("{}: no crate {backend}", m.name));
            continue;
        };
        let allowed = allowed_backend_apis(m, &all);
        for dep in deps {
            if dep == "server" || dep.starts_with("plugin-") {
                errors.push(format!("{backend} depends on {dep}"));
            } else if let Some(other) = dep.strip_prefix("api-") {
                if other == m.name {
                    continue;
                }
                if !allowed.contains(other) {
                    errors.push(format!("{backend} depends on {dep}, but does not use {other}"));
                } else if by_name.get(other).is_some_and(|o| o.layer > m.layer) {
                    errors.push(format!("{backend} depends on {dep} from a higher layer"));
                }
            }
        }
        let api = format!("api-{}", m.name);
        if let Some(deps) = crates.get(&api) {
            for dep in deps {
                if dep == "server" || dep.starts_with("plugin-") {
                    errors.push(format!("{api} depends on {dep}"));
                } else if let Some(other) = dep.strip_prefix("api-")
                    && !m.uses.iter().any(|u| u == other)
                {
                    errors.push(format!("{api} depends on {dep}, but does not use {other}"));
                }
            }
        }
    }
    // Every api-* and plugin-* crate belongs to a plugin with a manifest.
    for name in crates.keys() {
        let plugin = name
            .strip_prefix("api-")
            .or_else(|| name.strip_prefix("plugin-"));
        if let Some(p) = plugin
            && !by_name.contains_key(p)
        {
            errors.push(format!("crate {name} has no plugins/{p}/plugin.toml"));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[tokio::test]
async fn every_route_has_a_tag_from_its_manifest() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://x@127.0.0.1:1/x")
        .unwrap();
    let ctx = Ctx::new(pool, Config::default(), APP).unwrap();
    let plugins = server::build_plugins(&ctx, &PLUGINS).unwrap();
    let (_, mounted) = server::plugin_routes(&ctx, &plugins, &TAGS).unwrap();
    for r in &mounted {
        let b = plugins.iter().find(|b| b.manifest.name == r.plugin).unwrap();
        assert!(b.manifest.tags.contains(&r.tag), "{r:?}");
    }
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn backend_sources() -> Vec<(String, PathBuf, String)> {
    let mut out = Vec::new();
    for m in disk_manifests() {
        let mut files = Vec::new();
        rust_files(&root().join("plugins").join(&m.name).join("backend"), &mut files);
        files.sort();
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap();
            out.push((m.name.clone(), f, text));
        }
    }
    out
}

#[test]
fn routes_only_through_route() {
    let mut errors = Vec::new();
    for (_, path, text) in backend_sources() {
        let code = strip_comments_and_strings(&text);
        for bad in ["Router::new", "Router::<", ".nest(", ".nest_service(", "axum::serve"] {
            if code.contains(bad) {
                errors.push(format!("{}: {bad} (use arena_platform::Route)", path.display()));
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn sql_names_only_own_schema() {
    let plugins: Vec<String> = disk_manifests().into_iter().map(|m| m.name).collect();
    let mut errors = Vec::new();
    for (own, path, text) in backend_sources() {
        for lit in string_literals(&text) {
            for e in sql_violations(&lit, &own, &plugins) {
                errors.push(format!("{}: {e}", path.display()));
            }
        }
    }
    // Migrations run with search_path = own schema; they must not name another plugin's.
    for own in &plugins {
        let dir = root().join("plugins").join(own).join("migrations");
        for entry in std::fs::read_dir(&dir).into_iter().flatten() {
            let path = entry.unwrap().path();
            let sql = std::fs::read_to_string(&path).unwrap();
            for e in foreign_prefixes(&sql, own, &plugins) {
                errors.push(format!("{}: {e}", path.display()));
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn sql_checker_catches_bad_sql() {
    let plugins: Vec<String> = ["roster", "board", "wallets"].map(String::from).to_vec();
    let bad = |sql: &str| !sql_violations(sql, "roster", &plugins).is_empty();
    assert!(bad("SELECT * FROM board.rows"));
    assert!(bad("SELECT * FROM members"));
    assert!(bad("INSERT INTO public.x (a) VALUES (1)"));
    assert!(bad("UPDATE wallets.candidates SET a = 1"));
    assert!(bad("SELECT m.id FROM roster.members m JOIN wallets.candidates c ON c.kol = m.kol"));
    assert!(bad("DELETE FROM pg_catalog.pg_class"));
    assert!(bad("SELECT 1 WHERE EXISTS (SELECT 1 FROM board.x)"));
    assert!(bad("CREATE TABLE things (id int)"));
    assert!(!bad("SELECT m.id, m.kol FROM roster.members m JOIN roster.rates r ON r.kol = m.kol"));
    assert!(!bad("INSERT INTO roster.members (kol) VALUES ($1) ON CONFLICT (kol) DO UPDATE SET kol = EXCLUDED.kol"));
    assert!(!bad("SELECT extract(epoch FROM now()), x FROM roster.t, unnest($1::bigint[]) u"));
    assert!(!bad("SELECT k FROM roster.t WHERE k = ANY($1)"));
    assert!(!bad("member not found in the table"));
    assert!(!bad("SELECT 1"));
    assert!(!bad("pulled from board"));

    let lits = string_literals(
        "let a = \"x\\\"y\"; // \"no\"\n let b = r#\"SELECT \"q\" FROM t\"#; let c = 'z'; fn f<'a>() {}",
    );
    assert_eq!(lits, ["x\\\"y", "SELECT \"q\" FROM t"]);
    assert!(!foreign_prefixes("CREATE TABLE members (id int)", "roster", &plugins).iter().any(|_| true));
    assert!(!foreign_prefixes("SELECT * FROM board.x", "roster", &plugins).is_empty());
}

/// Code with comments removed and string/char literal contents blanked.
fn strip_comments_and_strings(src: &str) -> String {
    let mut out = String::new();
    scan(src, &mut |piece| match piece {
        Piece::Code(c) => out.push(c),
        Piece::Literal(_) => out.push_str("\"\""),
    });
    out
}

/// Contents of every string literal (raw contents, escapes kept as written).
fn string_literals(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    scan(src, &mut |piece| {
        if let Piece::Literal(s) = piece {
            out.push(s);
        }
    });
    out
}

enum Piece {
    Code(char),
    Literal(String),
}

fn scan(src: &str, emit: &mut dyn FnMut(Piece)) {
    let c: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < c.len() {
        let at = |j: usize| c.get(j).copied();
        if c[i] == '/' && at(i + 1) == Some('/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if c[i] == '/' && at(i + 1) == Some('*') {
            i += 2;
            while i < c.len() && !(c[i] == '*' && at(i + 1) == Some('/')) {
                i += 1;
            }
            i += 2;
        } else if c[i] == 'r'
            && (at(i + 1) == Some('"') || at(i + 1) == Some('#'))
            && (i == 0 || !(c[i - 1].is_alphanumeric() || c[i - 1] == '_'))
        {
            let mut j = i + 1;
            let mut hashes = 0;
            while at(j) == Some('#') {
                hashes += 1;
                j += 1;
            }
            if at(j) != Some('"') {
                emit(Piece::Code(c[i]));
                i += 1;
                continue;
            }
            j += 1;
            let start = j;
            loop {
                if j >= c.len() {
                    break;
                }
                if c[j] == '"' && (1..=hashes).all(|k| at(j + k) == Some('#')) {
                    break;
                }
                j += 1;
            }
            emit(Piece::Literal(c[start..j.min(c.len())].iter().collect()));
            i = j + 1 + hashes;
        } else if c[i] == '"' {
            let mut j = i + 1;
            while j < c.len() && c[j] != '"' {
                if c[j] == '\\' {
                    j += 1;
                }
                j += 1;
            }
            emit(Piece::Literal(c[i + 1..j.min(c.len())].iter().collect()));
            i = j + 1;
        } else if c[i] == '\'' {
            if at(i + 1) == Some('\\') {
                let mut j = i + 2;
                while j < c.len() && c[j] != '\'' {
                    j += 1;
                }
                emit(Piece::Code('\''));
                emit(Piece::Code('\''));
                i = j + 1;
            } else if at(i + 2) == Some('\'') {
                emit(Piece::Code('\''));
                emit(Piece::Code('\''));
                i += 3;
            } else {
                emit(Piece::Code('\''));
                i += 1;
            }
        } else {
            emit(Piece::Code(c[i]));
            i += 1;
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Word(String),
    Dot,
    Open,
    Close,
    Other,
}

fn tokens(sql: &str) -> Vec<Tok> {
    let c: Vec<char> = sql.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_alphabetic() || ch == '_' {
            let start = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_' || c[i] == '$') {
                i += 1;
            }
            out.push(Tok::Word(c[start..i].iter().collect::<String>().to_lowercase()));
            continue;
        }
        if ch == '\'' {
            i += 1;
            while i < c.len() && c[i] != '\'' {
                i += 1;
            }
            out.push(Tok::Other);
        } else if ch == '.' {
            out.push(Tok::Dot);
        } else if ch == '(' {
            out.push(Tok::Open);
        } else if ch == ')' {
            out.push(Tok::Close);
        } else if !ch.is_whitespace() {
            out.push(Tok::Other);
        }
        i += 1;
    }
    out
}

fn looks_like_sql(t: &[Tok]) -> bool {
    let first = t.iter().find_map(|t| match t {
        Tok::Word(w) => Some(w.as_str()),
        _ => None,
    });
    matches!(
        first,
        Some(
            "select" | "insert" | "update" | "delete" | "with" | "create" | "alter" | "drop"
                | "truncate" | "copy" | "lock" | "merge"
        )
    )
}

/// `<plugin>.` or `public.` prefixes naming a schema other than `own`.
fn foreign_prefixes(sql: &str, own: &str, plugins: &[String]) -> Vec<String> {
    let t = tokens(sql);
    let mut out = Vec::new();
    for w in t.windows(2) {
        if let [Tok::Word(name), Tok::Dot] = w
            && name != own
            && (name == "public" || plugins.iter().any(|p| p == name))
        {
            out.push(format!("names schema {name}: {sql}"));
        }
    }
    out
}

/// Violations of «name only your own schema» in one string literal.
fn sql_violations(sql: &str, own: &str, plugins: &[String]) -> Vec<String> {
    let t = tokens(sql);
    if !looks_like_sql(&t) {
        return Vec::new();
    }
    let mut out = foreign_prefixes(sql, own, plugins);
    // Words that may follow a table keyword without being a table.
    const SKIP: &[&str] = &[
        "set", "select", "lateral", "only", "if", "not", "exists", "nothing", "temp",
        "temporary", "unique", "index", "of",
    ];
    // `FROM` inside these functions is not a table position.
    const FROM_FUNCS: &[&str] = &["extract", "substring", "trim", "overlay", "position"];
    let mut opened_by: Vec<Option<String>> = Vec::new();
    for i in 0..t.len() {
        match &t[i] {
            Tok::Open => {
                let before = match i.checked_sub(1).map(|j| &t[j]) {
                    Some(Tok::Word(w)) => Some(w.clone()),
                    _ => None,
                };
                opened_by.push(before);
            }
            Tok::Close => {
                opened_by.pop();
            }
            Tok::Word(kw)
                if matches!(kw.as_str(), "from" | "join" | "into" | "update" | "table") =>
            {
                if kw == "from"
                    && opened_by
                        .last()
                        .and_then(|o| o.as_deref())
                        .is_some_and(|f| FROM_FUNCS.contains(&f))
                {
                    continue;
                }
                // `ON CONFLICT ... DO UPDATE` and `FOR UPDATE` are not table positions.
                if kw == "update"
                    && matches!(
                        i.checked_sub(1).map(|j| &t[j]),
                        Some(Tok::Word(w)) if w == "do" || w == "for"
                    )
                {
                    continue;
                }
                let mut j = i + 1;
                while let Some(Tok::Word(w)) = t.get(j) {
                    if SKIP.contains(&w.as_str()) {
                        j += 1;
                    } else {
                        break;
                    }
                }
                match (t.get(j), t.get(j + 1), t.get(j + 2)) {
                    (Some(Tok::Word(schema)), Some(Tok::Dot), Some(Tok::Word(_))) => {
                        if schema != own {
                            out.push(format!("{kw} {schema}.…: {sql}"));
                        }
                    }
                    // Function call; after INTO, UPDATE or TABLE the parenthesis is a column list.
                    (Some(Tok::Word(_)), Some(Tok::Open), _) if kw == "from" || kw == "join" => {}
                    (Some(Tok::Word(name)), _, _) => {
                        out.push(format!("{kw} {name} without schema {own}: {sql}"))
                    }
                    _ => {} // subquery, parameter
                }
            }
            _ => {}
        }
    }
    out.sort();
    out.dedup();
    out
}
