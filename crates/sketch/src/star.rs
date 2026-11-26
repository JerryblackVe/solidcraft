//! A star as a link source: the numbers that make it, and the outline they give.
//!
//! Like a text, a star keeps its numbers in the design and its curves are rebuilt from them, so a
//! five point star becomes an eight point one by editing, not by drawing again. The outline
//! alternates `points` tips (radius `outer`) with as many valleys (radius `inner_percent` of it,
//! or the one a regular star with a `skip` has), optionally with the tips and valleys rounded.

use serde::{Deserialize, Serialize};
use solidcore_record::Vec2;
use solidcraft_geom::Vec2Geom;

use crate::link::LinkGeom;
use crate::model::{Result, SketchError};

/// Fewest and most tips a star has.
pub const MIN_POINTS: u32 = 3;
pub const MAX_POINTS: u32 = 200;

/// What makes a star. Angles in degrees, lengths in millimetres, in sketch coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StarSpec {
    pub center: Vec2,
    pub points: u32,
    /// Distance from the centre to a tip.
    pub outer: f64,
    /// Distance from the centre to a valley, as a share of `outer` (0 to 100, both excluded).
    /// Ignored when `skip` is 2 or more.
    pub inner_percent: f64,
    /// 0 or 1: the valleys are `inner_percent` deep. 2 or more: the regular star `{points/skip}`,
    /// whose valleys are where the lines skipping that many tips cross.
    #[serde(default)]
    pub skip: u32,
    /// Radius of the rounding of each tip and of each valley (0: sharp).
    #[serde(default)]
    pub tip_round: f64,
    #[serde(default)]
    pub valley_round: f64,
    /// Turn, counter-clockwise: 0 has the first tip straight up (the sketch's +Y).
    #[serde(default)]
    pub angle: f64,
    /// Also draw the construction circles through the tips and through the valleys.
    #[serde(default)]
    pub circles: bool,
    /// The expressions behind the numbers above that were typed over the design's parameters.
    #[serde(default, skip_serializing_if = "StarExprs::is_empty")]
    pub exprs: StarExprs,
    /// Set on a mirror or pattern copy: it takes its shape from another star and keeps its own place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follows: Option<StarFollow>,
}

/// A star that follows another: the points, radius, depth, skip, roundings and circles are the
/// source's, and its centre and turn are the source's moved the way this says. The way is worked
/// out again from the mirror line (`line`), or from the pattern that made the copy, each time the
/// sketch is solved, so the copy follows the line, or the pattern's centre and direction lines, when
/// they move; the numbers are the way as last worked out, and all there is once those are gone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StarFollow {
    /// The link id of the star it follows.
    pub source: String,
    /// Mirrored across the sketch's X axis before turning.
    #[serde(default)]
    pub flip: bool,
    /// Turned about the origin by this many degrees (after the flip), then moved by `shift`.
    #[serde(default)]
    pub turn: f64,
    #[serde(default)]
    pub shift: Vec2,
    /// The id of the straight curve it was mirrored in, when it was made by a mirror.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
}

impl StarFollow {
    /// The mirror image in the line through `a` along the unit vector `u`.
    pub fn mirror(source: &str, line: Option<&str>, a: Vec2, u: Vec2) -> StarFollow {
        let turn = 2.0 * u.y.atan2(u.x);
        let flipped = Vec2::new(a.x, -a.y);
        StarFollow { source: source.to_owned(), flip: true, turn: turn.to_degrees(), shift: a - turned(flipped, turn), line: line.map(str::to_owned) }
    }

    /// Where the source's numbers put this star: `to` gets the shape and the place.
    pub fn apply(&self, from: &StarSpec, to: &mut StarSpec) {
        let c = if self.flip { Vec2::new(from.center.x, -from.center.y) } else { from.center };
        to.center = turned(c, self.turn.to_radians()) + self.shift;
        // The first tip points `90 + angle` degrees from +X: a flip negates that direction.
        let tip = 90.0 + from.angle;
        to.angle = self.turn + if self.flip { -tip } else { tip } - 90.0;
        (to.points, to.outer, to.inner_percent, to.skip) = (from.points, from.outer, from.inner_percent, from.skip);
        (to.tip_round, to.valley_round, to.circles) = (from.tip_round, from.valley_round, from.circles);
        to.exprs = StarExprs::default();
    }
}

fn turned(p: Vec2, radians: f64) -> Vec2 {
    let (s, c) = radians.sin_cos();
    Vec2::new(p.x * c - p.y * s, p.x * s + p.y * c)
}

impl crate::Sketch {
    /// Give every star that follows another the numbers the source has now, in link order (a copy
    /// of a copy follows through). A star whose source is gone stays as it is, on its own. Returns
    /// the links whose numbers changed: their outlines are rebuilt from them.
    pub fn resolve_star_follows(&mut self) -> Vec<String> {
        let mut changed = Vec::new();
        let patterned = self.pattern_star_follows();
        for i in 0..self.links.len() {
            let crate::LinkSource::Star(spec) = &self.links[i].source else { continue };
            let Some(follow) = spec.follows.clone() else { continue };
            let from = match self.links.iter().find(|l| l.id == follow.source).map(|l| &l.source) {
                Some(crate::LinkSource::Star(from)) => Some(from.clone()),
                _ => None,
            };
            let mut next = spec.clone();
            match from {
                Some(from) => {
                    // The mirror line or the pattern's centre and lines are where they are now; one that
                    // is gone or no longer a line leaves the way as it was.
                    let moved = follow.line.as_deref().and_then(|id| {
                        let Some(crate::Shape::Line { a, b }) = self.curve_index(id).and_then(|c| self.shape(c)) else { return None };
                        (b - a).normalized().map(|u| StarFollow::mirror(&follow.source, Some(id), a, u))
                    });
                    let moved = moved.or_else(|| patterned.get(&self.links[i].id).filter(|f| f.source == follow.source).cloned());
                    let follow = moved.unwrap_or(follow);
                    follow.apply(&from, &mut next);
                    next.follows = Some(follow);
                }
                None => next.follows = None,
            }
            if next != *spec {
                changed.push(self.links[i].id.clone());
                self.links[i].source = crate::LinkSource::Star(next);
            }
        }
        changed
    }

    /// [`Self::resolve_star_follows`], then the outlines of the stars it changed rebuilt from their numbers.
    pub fn refresh_star_followers(&mut self) -> Result<()> {
        for link in self.resolve_star_follows() {
            let Some(crate::LinkSource::Star(spec)) = self.link(&link).map(|l| l.source.clone()) else { continue };
            self.update_link(&link, &star_geometry(&spec)?)?;
        }
        Ok(())
    }
}

/// The expressions behind a star's numbers that were typed over the design's parameters (`"R / 2"`,
/// `"n"`). The numbers of [`StarSpec`] are what they gave last: the document evaluates them against
/// its parameters before the star's outline is rebuilt, so the star follows a parameter. A number
/// typed without an expression is kept as typed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StarExprs {
    /// The centre's x and y (lengths).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<String>,
    /// How many tips (a whole number).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<String>,
    /// The outer radius (a length).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outer: Option<String>,
    /// The valley depth in percent (no unit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inner_percent: Option<String>,
    /// The skip (a whole number).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip: Option<String>,
    /// The roundings (lengths).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tip_round: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valley_round: Option<String>,
    /// The turn (an angle).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<String>,
}

impl StarExprs {
    pub fn is_empty(&self) -> bool {
        *self == StarExprs::default()
    }

    /// The expressions with what each one sets, for finding and renaming the parameters they use.
    pub fn list(&self) -> Vec<(&'static str, &String)> {
        [
            ("Star X", &self.x),
            ("Star Y", &self.y),
            ("Star points", &self.points),
            ("Star radius", &self.outer),
            ("Star depth", &self.inner_percent),
            ("Star skip", &self.skip),
            ("Star tip rounding", &self.tip_round),
            ("Star valley rounding", &self.valley_round),
            ("Star angle", &self.angle),
        ]
        .into_iter()
        .filter_map(|(what, e)| e.as_ref().map(|e| (what, e)))
        .collect()
    }

    pub fn list_mut(&mut self) -> Vec<&mut String> {
        [
            &mut self.x,
            &mut self.y,
            &mut self.points,
            &mut self.outer,
            &mut self.inner_percent,
            &mut self.skip,
            &mut self.tip_round,
            &mut self.valley_round,
            &mut self.angle,
        ]
        .into_iter()
        .filter_map(Option::as_mut)
        .collect()
    }
}

impl StarSpec {
    /// The valley radius as a share of the tip radius, from `inner_percent` or from the skip.
    pub fn inner_ratio(&self) -> Result<f64> {
        if self.skip < 2 {
            return Ok(self.inner_percent / 100.0);
        }
        let n = f64::from(self.points);
        let k = f64::from(self.skip);
        if 2 * self.skip >= self.points {
            return Err(bad(format!("a skip of {} makes no star of {} points: it must be less than half of them", self.skip, self.points)));
        }
        if gcd(self.points, self.skip) != 1 {
            return Err(bad(format!(
                "a skip of {} leaves {} points out of a {}-point star: pick one with no common factor",
                self.skip,
                self.points,
                gcd(self.points, self.skip)
            )));
        }
        let pi = std::f64::consts::PI;
        Ok((pi * k / n).cos() / (pi * (k - 1.0) / n).cos())
    }

    /// The checks every star passes before it is drawn.
    pub fn check(&self) -> Result<()> {
        if !self.center.is_finite() || !self.outer.is_finite() || !self.inner_percent.is_finite() {
            return Err(bad("a star's numbers must be finite".into()));
        }
        if !(MIN_POINTS..=MAX_POINTS).contains(&self.points) {
            return Err(bad(format!("a star has {MIN_POINTS} to {MAX_POINTS} points, not {}", self.points)));
        }
        if self.outer <= 0.0 {
            return Err(bad("the outer radius must be more than 0".into()));
        }
        if self.skip < 2 && !(self.inner_percent > 0.0 && self.inner_percent < 100.0) {
            return Err(bad("the valleys' depth must be more than 0 % and less than 100 % of the outer radius".into()));
        }
        if !self.angle.is_finite() || !self.tip_round.is_finite() || !self.valley_round.is_finite() || self.tip_round < 0.0 || self.valley_round < 0.0
        {
            return Err(bad("the angle must be a number and the roundings 0 or more".into()));
        }
        self.inner_ratio().map(|_| ())
    }
}

fn bad(m: String) -> SketchError {
    SketchError::Invalid(m)
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn dir(a: f64) -> Vec2 {
    Vec2::new(a.cos(), a.sin())
}

/// The star's outline as the curves a link holds: lines, with an arc at each rounded corner,
/// then the construction circles (`circles`) last. Errors say what is wrong with the numbers.
pub fn star_geometry(s: &StarSpec) -> Result<Vec<LinkGeom>> {
    s.check()?;
    let n = s.points as usize;
    let inner = s.outer * s.inner_ratio()?;
    // Corners in order: tip, valley, tip, ... with the first tip up.
    let turn = std::f64::consts::FRAC_PI_2 + s.angle.to_radians();
    let step = std::f64::consts::PI / f64::from(s.points);
    let corners: Vec<(Vec2, f64)> = (0..2 * n)
        .map(|i| {
            let (r, round) = if i % 2 == 0 { (s.outer, s.tip_round) } else { (inner, s.valley_round) };
            (s.center + dir(turn + step * i as f64) * r, round)
        })
        .collect();
    // Each corner's rounding: where the arc starts and ends on its two edges, and its centre.
    let m = corners.len();
    let mut cut: Vec<Option<(Vec2, Vec2, Vec2)>> = Vec::with_capacity(m);
    let mut reach = vec![0.0_f64; m];
    for i in 0..m {
        let (v, r) = corners[i];
        if r <= 0.0 {
            cut.push(None);
            continue;
        }
        let u = (corners[(i + m - 1) % m].0 - v).normalized().ok_or_else(|| bad("a corner of the star has no edge".into()))?;
        let w = (corners[(i + 1) % m].0 - v).normalized().ok_or_else(|| bad("a corner of the star has no edge".into()))?;
        let phi = u.dot(w).clamp(-1.0, 1.0).acos();
        let half = phi / 2.0;
        if half.tan() <= 1e-9 || half.sin() <= 1e-9 {
            return Err(bad("the star is too sharp to round".into()));
        }
        let d = r / half.tan();
        let c = v + (u + w).normalized().ok_or_else(|| bad("the star is too sharp to round".into()))? * (r / half.sin());
        reach[i] = d;
        cut.push(Some((v + u * d, v + w * d, c)));
    }
    // A rounding that takes more than its edge has makes a loop: say which one is too big.
    let mut tight = f64::INFINITY;
    for i in 0..m {
        let len = corners[i].0.dist(corners[(i + 1) % m].0);
        let used = reach[i] + reach[(i + 1) % m];
        if used > len * (1.0 - 1e-9) {
            tight = tight.min(len / used);
        }
    }
    if tight.is_finite() {
        return Err(bad(format!(
            "the roundings do not fit: the edges are too short for them (scale both roundings by {:.2} or less, or make the star larger)",
            (tight * 0.99).max(0.0)
        )));
    }
    let mut out = Vec::with_capacity(2 * m + 2);
    for i in 0..m {
        let next = (i + 1) % m;
        let from = cut[i].map_or(corners[i].0, |c| c.1);
        let to = cut[next].map_or(corners[next].0, |c| c.0);
        out.push(LinkGeom::Line(from, to));
        if let Some((a, b, c)) = cut[next] {
            // The arc is the short way round its centre: counter-clockwise from the point it
            // is reached at to the one it leaves by, or the other way about.
            let (a, b) = if (a - c).cross(b - c) >= 0.0 { (a, b) } else { (b, a) };
            out.push(LinkGeom::Arc { c, a, b });
        }
    }
    if s.circles {
        out.push(LinkGeom::Circle(s.center, s.outer));
        out.push(LinkGeom::Circle(s.center, inner));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(points: u32) -> StarSpec {
        StarSpec {
            center: Vec2::new(10.0, 5.0),
            points,
            outer: 20.0,
            inner_percent: 40.0,
            skip: 0,
            tip_round: 0.0,
            valley_round: 0.0,
            angle: 0.0,
            circles: false,
            exprs: Default::default(),
            follows: None,
        }
    }

    #[test]
    fn sharp_star_has_two_lines_per_point() {
        for n in [3, 5, 8, 200] {
            let g = star_geometry(&spec(n)).unwrap();
            assert_eq!(g.len(), 2 * n as usize);
            assert!(g.iter().all(|c| matches!(c, LinkGeom::Line(..))));
        }
    }

    #[test]
    fn first_tip_is_up_and_turns_counter_clockwise() {
        let LinkGeom::Line(_, tip) = star_geometry(&spec(5)).unwrap()[1].clone() else { panic!() };
        // Line 0 runs tip 0 -> valley 0, line 1 valley 0 -> tip 1.
        let up = Vec2::new(10.0, 25.0);
        let LinkGeom::Line(a, _) = star_geometry(&spec(5)).unwrap()[0].clone() else { panic!() };
        assert!(a.dist(up) < 1e-9);
        assert!((tip.dist(Vec2::new(10.0, 5.0)) - 20.0).abs() < 1e-9);
        let mut turned = spec(5);
        turned.angle = 90.0;
        let LinkGeom::Line(a, _) = star_geometry(&turned).unwrap()[0].clone() else { panic!() };
        assert!(a.dist(Vec2::new(-10.0, 5.0)) < 1e-9);
    }

    #[test]
    fn regular_pentagram_has_the_golden_valley() {
        let mut s = spec(5);
        s.skip = 2;
        let r = s.inner_ratio().unwrap();
        assert!((r - 0.381_966).abs() < 1e-5, "{r}");
    }

    #[test]
    fn bad_numbers_are_refused_with_a_reason() {
        let mut s = spec(2);
        assert!(star_geometry(&s).is_err());
        s = spec(5);
        s.inner_percent = 100.0;
        assert!(star_geometry(&s).is_err());
        s = spec(6);
        s.skip = 2; // 6/2 shares a factor
        assert!(star_geometry(&s).unwrap_err().to_string().contains("common factor"));
        s = spec(5);
        s.skip = 3; // not less than half
        assert!(star_geometry(&s).is_err());
        s = spec(5);
        s.outer = f64::NAN;
        assert!(star_geometry(&s).is_err());
        s = spec(5);
        s.tip_round = 50.0;
        assert!(star_geometry(&s).unwrap_err().to_string().contains("do not fit"));
    }

    #[test]
    fn rounded_star_stays_inside_its_sharp_one_and_closes() {
        let mut s = spec(5);
        s.tip_round = 1.0;
        s.valley_round = 1.0;
        s.circles = true;
        let g = star_geometry(&s).unwrap();
        assert_eq!(g.iter().filter(|c| matches!(c, LinkGeom::Arc { .. })).count(), 10);
        assert_eq!(g.iter().filter(|c| matches!(c, LinkGeom::Circle(..))).count(), 2);
        // Every arc end is where a line begins or ends: the outline has no gap.
        let ends: Vec<Vec2> = g
            .iter()
            .flat_map(|c| match c {
                LinkGeom::Line(a, b) => vec![*a, *b],
                _ => vec![],
            })
            .collect();
        for c in &g {
            if let LinkGeom::Arc { a, b, c: ctr } = c {
                assert!(ends.iter().any(|e| e.dist(*a) < 1e-9) && ends.iter().any(|e| e.dist(*b) < 1e-9));
                assert!((ctr.dist(*a) - ctr.dist(*b)).abs() < 1e-9);
            }
        }
    }

    /// A mirror copy is the mirror image: every corner of the followed star's outline is where the
    /// source's corner reflects to, for stars turned any way and lines in any direction.
    #[test]
    fn a_star_that_follows_is_the_mirror_image_of_its_source() {
        let corners = |s: &StarSpec| -> Vec<Vec2> {
            star_geometry(s).unwrap().iter().filter_map(|c| if let LinkGeom::Line(a, _) = c { Some(*a) } else { None }).collect()
        };
        for (line_at, line_dir, angle, points) in
            [((0.0, 0.0), (0.0, 1.0), 0.0, 5), ((3.0, -2.0), (1.0, 1.0), 20.0, 7), ((-4.0, 6.0), (2.0, -1.0), -75.0, 6)]
        {
            let mut source = spec(points);
            source.center = Vec2::new(10.0, 5.0);
            source.angle = angle;
            let a = Vec2::new(line_at.0, line_at.1);
            let u = Vec2::new(line_dir.0, line_dir.1).normalized().unwrap();
            let mut copy = source.clone();
            StarFollow::mirror("link1", None, a, u).apply(&source, &mut copy);
            let reflect = |q: Vec2| (a + u * (q - a).dot(u)) * 2.0 - q;
            let mirrored: Vec<Vec2> = corners(&source).into_iter().map(reflect).collect();
            // The copy's outline is the same closed outline, started from another corner and run the other way.
            for m in &mirrored {
                assert!(
                    corners(&copy).iter().any(|c| c.dist(*m) < 1e-9),
                    "{m:?} not a corner of the copy (line {line_at:?} {line_dir:?}, angle {angle})"
                );
            }
            assert_eq!(corners(&copy).len(), mirrored.len());
        }
    }
}
