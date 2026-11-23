//! The font list of the text dialog: each font shows a sample in itself, drawn from the outlines
//! the Text tool makes with it (so the list shows what the sketch will get), and the fonts marked
//! favourite and picked last come first. The app keeps the favourites, the recent fonts and the
//! sample text in its saved settings and hands them to the dialog every frame with [`share`].

use std::collections::HashMap;

use egui::{Color32, ColorImage, TextureHandle, TextureOptions};
use solidcore_record::Vec2;
use solidcraft_engine::sketch::LinkGeom;

/// How many recently picked fonts the list keeps.
pub const MAX_RECENT_FONTS: usize = 8;
/// How many favourite fonts are kept.
pub const MAX_FAVORITE_FONTS: usize = 64;
/// The longest sample text, in characters.
pub const MAX_SAMPLE_CHARS: usize = 40;
/// Previews made in one frame: the rest come in the next frames, so opening a long list never
/// stalls the window.
const PREVIEWS_PER_FRAME: usize = 4;
/// Previews kept; the oldest go first.
const MAX_PREVIEWS: usize = 256;
/// The widest preview, in pixels.
const MAX_WIDTH: usize = 640;

/// What the app saves for the font list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontPicks {
    pub favorites: Vec<String>,
    pub recent: Vec<String>,
    /// Shown in each font instead of the text being edited (empty: that text, or the font's name).
    pub sample: String,
}

impl FontPicks {
    /// From the saved settings: each list capped, its names trimmed and once each ("" is the
    /// built-in font).
    pub fn load(v: &serde_json::Value) -> FontPicks {
        let list = |key: &str, max: usize| -> Vec<String> {
            let mut out: Vec<String> = Vec::new();
            for name in v.get(key).and_then(serde_json::Value::as_array).into_iter().flatten().filter_map(|x| x.as_str()).map(str::trim) {
                if out.len() < max && !out.iter().any(|o| o == name) {
                    out.push(name.to_owned());
                }
            }
            out
        };
        let sample = v.get("sample").and_then(serde_json::Value::as_str).unwrap_or_default().chars().take(MAX_SAMPLE_CHARS).collect();
        FontPicks { favorites: list("favorites", MAX_FAVORITE_FONTS), recent: list("recent", MAX_RECENT_FONTS), sample }
    }

    pub fn store(&self) -> serde_json::Value {
        serde_json::json!({"favorites": self.favorites, "recent": self.recent, "sample": self.sample})
    }

    pub fn is_favorite(&self, font: &str) -> bool {
        self.favorites.iter().any(|f| f == font)
    }

    pub fn toggle_favorite(&mut self, font: &str) {
        if self.is_favorite(font) {
            self.favorites.retain(|f| f != font);
        } else if self.favorites.len() < MAX_FAVORITE_FONTS {
            self.favorites.push(font.to_owned());
        }
    }

    /// Put a picked font at the front of the recent list (once, capped); the built-in font ("") too.
    pub fn remember(&mut self, font: &str) {
        self.recent.retain(|f| f != font);
        self.recent.insert(0, font.to_owned());
        self.recent.truncate(MAX_RECENT_FONTS);
    }
}

#[derive(Clone, Default)]
struct Shared {
    picks: FontPicks,
    /// Changed in the dialog since the app last took the picks for its settings.
    changed: bool,
}

fn shared_id() -> egui::Id {
    egui::Id::unique("sketch_text_font_picks")
}

/// The favourites, recent fonts and sample the dialog shows.
pub fn picks(ctx: &egui::Context) -> FontPicks {
    ctx.data(|d| d.get_temp::<Shared>(shared_id())).map(|s| s.picks).unwrap_or_default()
}

/// Change the picks from the dialog (a star clicked, a font chosen, the sample typed).
pub fn change(ctx: &egui::Context, f: impl FnOnce(&mut FontPicks)) {
    ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_default::<Shared>(shared_id());
        let before = s.picks.clone();
        f(&mut s.picks);
        s.changed |= s.picks != before;
    });
}

/// Keep the saved picks and the dialog's the same; the app calls this every frame. Changes made
/// in the dialog since the last call are written to `saved`; otherwise `saved` is what it shows.
pub fn share(ctx: &egui::Context, saved: &mut FontPicks) {
    ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_default::<Shared>(shared_id());
        if s.changed {
            saved.clone_from(&s.picks);
            s.changed = false;
        } else if s.picks != *saved {
            s.picks.clone_from(saved);
        }
    });
}

type Key = (String, String, Color32);

/// Previews made, by font, sample and ink, and how many were made in the current frame.
#[derive(Clone, Default)]
struct Previews {
    made: HashMap<Key, Option<TextureHandle>>,
    order: Vec<Key>,
    frame: u64,
    this_frame: usize,
}

/// `sample` drawn in `font` ("" is the built-in font), `height` pixels tall at most, in `ink` on
/// clear (draw it untinted). None when the font cannot set the sample, or when this frame has made
/// its share of previews already: the list asks again on the next frame.
pub fn preview(ctx: &egui::Context, font: &str, sample: &str, height: usize, ink: Color32) -> Option<TextureHandle> {
    let key = (font.to_owned(), sample.to_owned(), ink);
    let id = egui::Id::unique("sketch_text_font_previews");
    let frame = ctx.cumulative_frame_nr();
    // Some(made) when it was made before; None when it may be made now.
    let found = ctx.data_mut(|d| {
        let p = d.get_temp_mut_or_default::<Previews>(id);
        if p.frame != frame {
            p.frame = frame;
            p.this_frame = 0;
        }
        if let Some(made) = p.made.get(&key) {
            return Some(made.clone());
        }
        if p.this_frame >= PREVIEWS_PER_FRAME {
            return Some(None);
        }
        p.this_frame += 1;
        None
    });
    match found {
        Some(Some(texture)) => return Some(texture),
        Some(None) => {
            // Made and unreadable, or waiting for a later frame: ask again then.
            if !ctx.data(|d| d.get_temp::<Previews>(id)).is_some_and(|p| p.made.contains_key(&key)) {
                ctx.request_repaint();
            }
            return None;
        }
        None => {}
    }
    let texture = render(font, sample, height, ink).map(|img| ctx.load_texture(format!("font_preview:{font}:{sample}"), img, TextureOptions::LINEAR));
    ctx.data_mut(|d| {
        let p = d.get_temp_mut_or_default::<Previews>(id);
        p.made.insert(key.clone(), texture.clone());
        p.order.push(key);
        if p.order.len() > MAX_PREVIEWS {
            let old = p.order.remove(0);
            p.made.remove(&old);
        }
    });
    texture
}

/// The sample's outlines in `font`, filled (even-odd: a letter's counters stay open) into an
/// image `height` pixels tall at most, in `ink`, as wide as the sample needs up to [`MAX_WIDTH`].
pub fn render(font: &str, sample: &str, height: usize, ink: Color32) -> Option<ColorImage> {
    let font = (!font.trim().is_empty()).then_some(font);
    let geom = solidcraft_engine::sketch::text_geometry_in(font, sample, Vec2::new(0.0, 0.0), 1.0, 0.0).ok()?;
    let edges: Vec<(Vec2, Vec2)> = geom.iter().flat_map(flatten).collect();
    if edges.is_empty() || height < 4 {
        return None;
    }
    let (lo, hi) = edges.iter().fold((Vec2::new(f64::MAX, f64::MAX), Vec2::new(f64::MIN, f64::MIN)), |(lo, hi), (a, b)| {
        (Vec2::new(lo.x.min(a.x).min(b.x), lo.y.min(a.y).min(b.y)), Vec2::new(hi.x.max(a.x).max(b.x), hi.y.max(a.y).max(b.y)))
    });
    // The capital height (1 mm here) fills most of the row; descenders and accents fit in the rest.
    let px = (height as f64 - 2.0) / (hi.y - lo.y).max(1.0);
    let width = (((hi.x - lo.x) * px).ceil() as usize + 2).clamp(1, MAX_WIDTH);
    let to = |p: Vec2| ((p.x - lo.x) * px + 1.0, (hi.y - p.y) * px + 1.0);
    let edges: Vec<((f64, f64), (f64, f64))> = edges.iter().map(|(a, b)| (to(*a), to(*b))).collect();
    let mut cover = vec![0.0f32; width * height];
    const SUB: usize = 4;
    let mut xs: Vec<f64> = Vec::new();
    for row in 0..height {
        for s in 0..SUB {
            let y = row as f64 + (s as f64 + 0.5) / SUB as f64;
            xs.clear();
            for &((x0, y0), (x1, y1)) in &edges {
                if (y0 <= y) != (y1 <= y) {
                    xs.push(x0 + (y - y0) / (y1 - y0) * (x1 - x0));
                }
            }
            xs.sort_by(f64::total_cmp);
            for [a, b] in xs.as_chunks::<2>().0 {
                let [a, b] = [*a, *b].map(|x| x.clamp(0.0, width as f64));
                let mut x = a.floor() as usize;
                while (x as f64) < b && x < width {
                    let inside = (b.min(x as f64 + 1.0) - a.max(x as f64)).max(0.0);
                    if let Some(c) = cover.get_mut(row * width + x) {
                        *c += inside as f32 / SUB as f32;
                    }
                    x += 1;
                }
            }
        }
    }
    let pixels = cover.iter().map(|c| ink.gamma_multiply(c.clamp(0.0, 1.0))).collect();
    Some(ColorImage::new([width, height], pixels))
}

/// An outline piece as straight edges (lines exactly; conics, arcs and Béziers in short steps).
fn flatten(g: &LinkGeom) -> Vec<(Vec2, Vec2)> {
    const STEPS: usize = 8;
    let pts: Vec<Vec2> = match g {
        LinkGeom::Line(a, b) => vec![*a, *b],
        LinkGeom::Conic { a, apex, b, rho } => {
            let w = rho / (1.0 - rho).max(1e-9);
            (0..=STEPS)
                .map(|i| {
                    let t = i as f64 / STEPS as f64;
                    let (p, q, r) = ((1.0 - t) * (1.0 - t), 2.0 * w * t * (1.0 - t), t * t);
                    (*a * p + *apex * q + *b * r) / (p + q + r)
                })
                .collect()
        }
        LinkGeom::Spline { pts, control: true, nurbs: None, .. } if pts.len() >= 2 => (0..=STEPS)
            .map(|i| {
                // De Casteljau on the Bézier's control points.
                let t = i as f64 / STEPS as f64;
                let mut p = pts.clone();
                while p.len() > 1 {
                    p = p
                        .windows(2)
                        .map(|w| w.first().copied().unwrap_or_default() * (1.0 - t) + w.last().copied().unwrap_or_default() * t)
                        .collect();
                }
                p.first().copied().unwrap_or_default()
            })
            .collect(),
        LinkGeom::Spline { pts, .. } => pts.clone(),
        LinkGeom::Arc { c, a, b } => {
            let (r, s) = ((a.x - c.x).hypot(a.y - c.y), (a.y - c.y).atan2(a.x - c.x));
            let mut e = (b.y - c.y).atan2(b.x - c.x);
            if e <= s {
                e += std::f64::consts::TAU;
            }
            (0..=STEPS)
                .map(|i| *c + Vec2::new((s + (e - s) * i as f64 / STEPS as f64).cos(), (s + (e - s) * i as f64 / STEPS as f64).sin()) * r)
                .collect()
        }
        LinkGeom::Circle(c, r) => (0..=STEPS * 2)
            .map(|i| {
                *c + Vec2::new(
                    (i as f64 * std::f64::consts::TAU / (STEPS * 2) as f64).cos(),
                    (i as f64 * std::f64::consts::TAU / (STEPS * 2) as f64).sin(),
                ) * *r
            })
            .collect(),
        LinkGeom::Point(_) | LinkGeom::Ellipse { .. } => Vec::new(),
    };
    pts.windows(2).filter_map(|w| Some((*w.first()?, *w.last()?))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const INK: Color32 = Color32::from_rgb(200, 100, 50);

    /// The built-in font's "O": a ring, so its middle is clear and its stroke is ink; an "I" is
    /// narrower than "WW"; a sample no font can set makes no picture.
    #[test]
    fn a_preview_fills_the_outlines_and_leaves_the_counters_open() {
        let o = render("", "O", 40, INK).unwrap();
        let [w, h] = o.size;
        assert_eq!(h, 40);
        let at = |x: usize, y: usize| o.pixels[y * w + x].a();
        assert_eq!(at(w / 2, h / 2), 0, "the counter of the O is open");
        let row: Vec<u8> = (0..w).map(|x| at(x, h / 2)).collect();
        assert!(row.iter().any(|a| *a > 200), "the stroke of the O is ink: {row:?}");
        assert!(render("", "I", 40, INK).unwrap().size[0] < render("", "WW", 40, INK).unwrap().size[0]);
        assert!(render("", " ", 40, INK).is_none());
        assert!(render("/no/such/font.ttf", "Ab", 40, INK).is_none());
        assert!(render("", "Ab", 2, INK).is_none());
        // A long sample is cut at the widest picture.
        assert_eq!(render("", &"W".repeat(MAX_SAMPLE_CHARS), 40, INK).unwrap().size[0], MAX_WIDTH);
    }

    #[test]
    fn picks_keep_favourites_and_the_recent_fonts_capped_and_read_back() {
        let mut p = FontPicks::default();
        for i in 0..20 {
            p.remember(&format!("F{i}"));
        }
        p.remember("F15");
        assert_eq!(p.recent.len(), MAX_RECENT_FONTS);
        assert_eq!(p.recent[0], "F15");
        assert_eq!(p.recent.iter().filter(|f| *f == "F15").count(), 1);
        p.toggle_favorite("Serif");
        p.toggle_favorite("Mono");
        p.toggle_favorite("Serif");
        assert_eq!(p.favorites, ["Mono"]);
        p.sample = "Ab".into();
        assert_eq!(FontPicks::load(&p.store()), p);
        // What is not a name is left out; the built-in font ("") is a name, kept once.
        let read = FontPicks::load(&serde_json::json!({"favorites": [1, "", "  ", " Sans"], "recent": "x", "sample": "y".repeat(99)}));
        assert_eq!((read.favorites, read.recent.len(), read.sample.chars().count()), (vec![String::new(), "Sans".into()], 0, MAX_SAMPLE_CHARS));
    }
}
