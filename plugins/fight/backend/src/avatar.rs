//! Pixel avatar for the fight image: the same sprite as `avatarSpec` in
//! `platform/design/src/avatar.ts` (5x5 mirrored grid, one palette colour, from the id only).

/// Palette colours in `AVATAR_COLORS` order: coin, pink, blue, lavender, plus.
const COLORS: [&str; 5] = ["#ffc93c", "#ff4d8d", "#6cc6ff", "#b79cff", "#2fe5a7"];

/// FNV-1a over UTF-16 code units, as `hashId`.
fn hash_id(id: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for unit in id.encode_utf16() {
        h ^= u32::from(unit);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// mulberry32, as `rng`.
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

type Cells = [[bool; 5]; 5];

fn build(seed: u32) -> Cells {
    let mut rng = Rng(seed);
    let mut cells = [[false; 5]; 5];
    for row in cells.iter_mut() {
        for c in 0..3 {
            let on = rng.next() < 0.55;
            row[c] = on;
            row[4 - c] = on;
        }
    }
    cells
}

fn good(cells: &Cells) -> bool {
    let count = cells.iter().flatten().filter(|&&on| on).count();
    (9..=17).contains(&count) && cells.iter().all(|row| row.iter().any(|&on| on))
}

/// Sprite cells and colour for `id`.
pub fn spec(id: &str) -> (Cells, &'static str) {
    let seed = hash_id(id);
    let mut cells = build(seed);
    let mut salt = 1u32;
    while salt <= 32 && !good(&cells) {
        cells = build((seed ^ salt).wrapping_mul(0x9e37_79b1));
        salt += 1;
    }
    (cells, COLORS[((seed >> 11) % 5) as usize])
}

/// SVG group drawing the avatar at (`x`, `y`) with side `size`.
pub fn svg(id: &str, x: f64, y: f64, size: f64) -> String {
    let (cells, color) = spec(id);
    let mut d = String::new();
    for (r, row) in cells.iter().enumerate() {
        for (c, &on) in row.iter().enumerate() {
            if on {
                d.push_str(&format!("M{} {}h1v1h-1z", c + 1, r + 1));
            }
        }
    }
    let k = size / 7.0;
    format!(
        "<g transform=\"translate({x} {y}) scale({k})\" shape-rendering=\"crispEdges\">\
         <rect x=\"0.1\" y=\"0.1\" width=\"6.8\" height=\"6.8\" rx=\"0.6\" fill=\"#0e0b18\" \
         stroke=\"#3a3160\" stroke-width=\"0.2\"/><path d=\"{d}\" fill=\"{color}\"/></g>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_matches_fnv1a() {
        // FNV-1a 32 of "a" is 0xe40c292c.
        assert_eq!(hash_id("a"), 0xe40c_292c);
        assert_eq!(hash_id(""), 0x811c_9dc5);
    }

    /// Values printed by `avatarSpec` in `platform/design/src/avatar.ts`.
    #[test]
    fn same_sprite_as_the_web() {
        for (id, color, want) in [
            ("tekkerrss", "#ffc93c", "11111/01010/10001/11111/01110"),
            ("decu", "#ff4d8d", "01110/10001/10101/11111/10101"),
            ("cooker", "#b79cff", "11011/10101/10001/10101/10101"),
        ] {
            let (cells, c) = spec(id);
            let got: Vec<String> = cells
                .iter()
                .map(|r| r.iter().map(|&on| if on { '1' } else { '0' }).collect())
                .collect();
            assert_eq!((got.join("/").as_str(), c), (want, color), "{id}");
        }
    }

    #[test]
    fn sprite_is_mirrored_and_stable() {
        let (cells, color) = spec("tekkerrss");
        assert_eq!(spec("tekkerrss"), (cells, color));
        for row in cells {
            assert_eq!(row[0], row[4]);
            assert_eq!(row[1], row[3]);
        }
    }
}
