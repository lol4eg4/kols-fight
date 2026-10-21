//! Fight image for X: an SVG drawn from the result, rendered by `resvg` with the committed
//! OFL fonts (`../fonts`) to a 1200x630 PNG. Rendered images are kept in a small LRU.

use crate::{avatar, resolve::points_text};
use api_fight::{FightResult, HP_SEGMENTS, Tier};
use chrono::{DateTime, NaiveDate, Utc};
use resvg::{tiny_skia, usvg};
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

pub const WIDTH: u32 = 1200;
pub const HEIGHT: u32 = 630;
/// Most images kept in memory.
pub const CACHE_SIZE: usize = 200;

const FONTS: [&[u8]; 3] = [
    include_bytes!("../fonts/PressStart2P-Regular.ttf"),
    include_bytes!("../fonts/GolosText-Variable.ttf"),
    include_bytes!("../fonts/JetBrainsMono-Variable.ttf"),
];

const BG: &str = "#110d1c";
const SURFACE: &str = "#1a1430";
const BORDER: &str = "#3a3160";
const TEXT: &str = "#f4f1ff";
const MUTED: &str = "#a79fc6";
const COIN: &str = "#ffc93c";
const PINK: &str = "#ff4d8d";
const SHADOW: &str = "#05030a";

fn options() -> &'static usvg::Options<'static> {
    static OPTIONS: OnceLock<usvg::Options<'static>> = OnceLock::new();
    OPTIONS.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        for font in FONTS {
            db.load_font_data(font.to_vec());
        }
        usvg::Options {
            font_family: "Golos Text".into(),
            fontdb: Arc::new(db),
            ..usvg::Options::default()
        }
    })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// At most `max` characters, with `…` when cut.
fn cut(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max - 1).collect();
        t.push('…');
        t
    }
}

fn tier(t: Tier) -> (&'static str, &'static str) {
    match t {
        Tier::Goat => ("GOAT", COIN),
        Tier::Diamond => ("DIAMOND", "#6cc6ff"),
        Tier::Sendoor => ("SENDOOR", PINK),
        Tier::Degen => ("DEGEN", "#b79cff"),
    }
}

fn text(x: f64, y: f64, font: &str, size: u32, fill: &str, anchor: &str, body: &str) -> String {
    format!(
        "<text x=\"{x}\" y=\"{y}\" font-family=\"'{font}'\" font-size=\"{size}\" fill=\"{fill}\" \
         text-anchor=\"{anchor}\">{}</text>",
        esc(body)
    )
}

/// The image as SVG source.
pub fn svg(result: &FightResult) -> String {
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{WIDTH}\" height=\"{HEIGHT}\" \
         viewBox=\"0 0 {WIDTH} {HEIGHT}\"><rect width=\"100%\" height=\"100%\" fill=\"{BG}\"/>"
    );
    // Header: KOL ARENA with its hard pink shadow; fight and snapshot date on the right.
    s += &text(64.0, 92.0, "Press Start 2P", 36, PINK, "start", "KOL ARENA");
    s += &text(60.0, 88.0, "Press Start 2P", 36, COIN, "start", "KOL ARENA");
    s += &text(1140.0, 66.0, "Press Start 2P", 20, PINK, "end", "KOL FIGHT");
    s += &text(
        1140.0,
        96.0,
        "JetBrains Mono",
        20,
        MUTED,
        "end",
        &format!("Snapshot {}", result.date),
    );

    let n = result.fighters.len().max(1) as f64;
    let gap = 24.0;
    let w = (1080.0 - gap * (n - 1.0)) / n;
    let top = 136.0;
    let h = 300.0;
    for (i, f) in result.fighters.iter().enumerate() {
        let x = 60.0 + i as f64 * (w + gap);
        let cx = x + w / 2.0;
        let is_winner = result.winner.as_deref() == Some(f.kol.as_str());
        let (border, stroke) = if is_winner { (COIN, 4) } else { (BORDER, 2) };
        s += &format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{w}\" height=\"{h}\" fill=\"{SHADOW}\"/>\
             <rect x=\"{x}\" y=\"{top}\" width=\"{w}\" height=\"{h}\" fill=\"{SURFACE}\" \
             stroke=\"{border}\" stroke-width=\"{stroke}\"/>",
            x + 6.0,
            top + 6.0
        );
        s += &avatar::svg(&f.kol, cx - 40.0, top + 20.0, 80.0);
        let max_name = ((w - 24.0) / 15.0).max(6.0) as usize;
        s += &text(
            cx,
            top + 136.0,
            "Golos Text",
            28,
            TEXT,
            "middle",
            &cut(&f.name, max_name),
        );
        let (tier_name, tier_color) = tier(f.card.tier);
        s += &text(
            cx,
            top + 172.0,
            "JetBrains Mono",
            20,
            tier_color,
            "middle",
            &format!("Score {} · {tier_name}", f.card.score),
        );
        s += &text(
            cx,
            top + 236.0,
            "JetBrains Mono",
            48,
            if is_winner { COIN } else { TEXT },
            "middle",
            &points_text(f.points),
        );
        // HP bar: one segment per HP point.
        let seg_w = ((w - 48.0) / f64::from(HP_SEGMENTS)).min(24.0);
        let bar_x = cx - seg_w * f64::from(HP_SEGMENTS) / 2.0;
        for k in 0..HP_SEGMENTS {
            let fill = if k < f.hp { "#2fe5a7" } else { BORDER };
            s += &format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"14\" fill=\"{fill}\"/>",
                bar_x + f64::from(k) * seg_w + 1.0,
                top + 266.0,
                seg_w - 2.0
            );
        }
    }

    // Result: K.O. or DRAW, the outcome line, the counted rounds.
    let verdict = if result.winner.is_some() {
        "K.O."
    } else {
        "DRAW"
    };
    s += &text(60.0, 500.0, "Press Start 2P", 28, PINK, "start", verdict);
    let outcome = result.summary.first().map(String::as_str).unwrap_or("");
    s += &text(
        200.0,
        500.0,
        "Golos Text",
        30,
        TEXT,
        "start",
        &cut(outcome, 60),
    );
    s += &text(
        60.0,
        548.0,
        "JetBrains Mono",
        22,
        MUTED,
        "start",
        &result.counted_line,
    );
    s += &text(
        1140.0,
        590.0,
        "JetBrains Mono",
        18,
        MUTED,
        "end",
        "NFA · DYOR",
    );
    s += "</svg>";
    s
}

/// Renders `result` to a 1200x630 PNG.
pub fn render(result: &FightResult) -> Result<Vec<u8>, String> {
    let tree = usvg::Tree::from_str(&svg(result), options()).map_err(|e| e.to_string())?;
    let mut pixmap = tiny_skia::Pixmap::new(WIDTH, HEIGHT).ok_or("pixmap")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(|e| e.to_string())
}

/// Cache key: requested ids, snapshot date and that snapshot's close time, so a re-collected
/// date never serves an old image.
pub type Key = (Vec<String>, NaiveDate, Option<DateTime<Utc>>);

/// Least recently used images, at most `cap`.
pub struct Cache {
    cap: usize,
    tick: u64,
    map: HashMap<Key, (u64, Arc<Vec<u8>>)>,
}

impl Cache {
    pub fn new(cap: usize) -> Self {
        Cache {
            cap,
            tick: 0,
            map: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn get(&mut self, key: &Key) -> Option<Arc<Vec<u8>>> {
        self.tick += 1;
        let tick = self.tick;
        self.map.get_mut(key).map(|(t, png)| {
            *t = tick;
            png.clone()
        })
    }

    pub fn put(&mut self, key: Key, png: Arc<Vec<u8>>) {
        self.tick += 1;
        if !self.map.contains_key(&key) && self.map.len() >= self.cap {
            let oldest = self
                .map
                .iter()
                .min_by_key(|(_, (t, _))| *t)
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                self.map.remove(&k);
            }
        }
        self.map.insert(key, (self.tick, png));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(id: &str) -> Key {
        (
            vec![id.into()],
            NaiveDate::from_ymd_opt(2026, 10, 18).unwrap(),
            None,
        )
    }

    #[test]
    fn lru_drops_the_least_recently_used() {
        let mut c = Cache::new(2);
        c.put(key("a"), Arc::new(vec![1]));
        c.put(key("b"), Arc::new(vec![2]));
        assert!(c.get(&key("a")).is_some());
        c.put(key("c"), Arc::new(vec![3]));
        assert_eq!(c.len(), 2);
        assert!(c.get(&key("b")).is_none());
        assert!(c.get(&key("a")).is_some());
        assert!(c.get(&key("c")).is_some());
    }

    #[test]
    fn names_are_escaped_and_cut() {
        assert_eq!(esc("<a&\"b\">"), "&lt;a&amp;&quot;b&quot;&gt;");
        assert_eq!(cut("abcdef", 4), "abc…");
        assert_eq!(cut("abc", 4), "abc");
    }
}
