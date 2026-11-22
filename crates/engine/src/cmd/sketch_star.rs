//! `sketch.star` and `sketch.edit_star`: a star kept as its numbers, so it can be edited after it
//! is drawn (Sketch › Create › Polygon › Star, and Edit Star).

use serde_json::{Value, json};
use solidcore_record::Vec2;
use solidcraft_doc::Document;
use solidcraft_doc::expr::{self, Kind};
use solidcraft_sketch::{LinkKind, LinkSource, Sketch, StarExprs, StarSpec, star_geometry};

use super::sketch::ids_of;
use super::sketch_store;
use super::{CommandSpec, in_sketch};
use crate::params::{bad, bool_, req_vec2};
use crate::schema::sketch_star::{EditStarParams, StarParams, StarResult};
use crate::schema::{TypedContract, output_schema_for};
use crate::{Result, Session};

const STAR_CONTRACT: TypedContract<StarParams> = TypedContract::new();
const EDIT_STAR_CONTRACT: TypedContract<EditStarParams> = TypedContract::new();

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("sketch.star", "Star", crate::checked!("sketch.star", star))
        .in_sketch_part()
        .at("SKETCH", "CREATE")
        .icon("sketch_star")
        .enabled(in_sketch)
        .contract(&STAR_CONTRACT)
        .output(output_schema_for::<StarResult>)
        .params("center: [x,y] mm; outer_radius: mm (> 0); points?: 3..200 (default 5); inner_percent?: valley radius as % of outer (0..100 exclusive, default 40); skip?: 2+ for the regular star {points/skip} (valleys where the lines cross; under half the points, no common factor); tip_round?, valley_round?: mm radius of the rounding (default 0); angle?: degrees counter-clockwise, 0 = a tip straight up; circles?: bool, construction circles through tips and valleys (never profiles). Outlines stay linked to these numbers: sketch.edit_star changes them later and an extrude of the star follows"),
    CommandSpec::new("sketch.edit_star", "Edit Star", crate::checked!("sketch.edit_star", edit_star))
        .in_sketch_part()
        .icon("edit")
        .enabled(in_sketch)
        .contract(&EDIT_STAR_CONTRACT)
        .output(output_schema_for::<StarResult>)
        .params("link: the star's id (from sketch.star), or entity: one of its curves; then any of center, outer_radius, points, inner_percent, skip, tip_round, valley_round, angle, circles: the rest is kept. A star of 5 points becomes one of 8 without being deleted; a change that makes no valid star (roundings too big, bad skip) is refused and nothing changes"),
];

/// A star number in a call: a number (lengths in mm, angles in degrees, counts as they are), or
/// an expression over the design's parameters (`"R / 2"`, `"n"`). Returns the value (an angle in
/// degrees) and the expression to keep, when it names a parameter. None when the key is left out.
fn star_number(doc: &Document, p: &Value, cmd: &str, key: &str, kind: Kind) -> Result<Option<(f64, Option<String>)>> {
    let must = match kind {
        Kind::Length => format!("`{key}` must be a length in mm, or an expression"),
        Kind::Angle => format!("`{key}` must be an angle in degrees, or an expression"),
        Kind::Unitless => format!("`{key}` must be a number, or an expression"),
    };
    match p.get(key).filter(|v| !v.is_null()) {
        None => Ok(None),
        Some(Value::Number(n)) => n.as_f64().and_then(crate::params::finite).map(|x| Some((x, None))).ok_or_else(|| bad(cmd, must)),
        Some(Value::String(e)) if !e.trim().is_empty() && e.len() <= 4096 => {
            let e = if kind == Kind::Unitless { e.trim().to_string() } else { doc.with_design_unit(e.trim(), kind) };
            let x = doc.eval(&e, kind).map_err(|error| bad(cmd, format!("`{key}`: {error}")))?;
            let x = crate::params::finite(x).ok_or_else(|| bad(cmd, must))?;
            let x = if kind == Kind::Angle { x.to_degrees() } else { x };
            Ok(Some((x, (!expr::references(&e).is_empty()).then_some(e))))
        }
        Some(_) => Err(bad(cmd, must)),
    }
}

/// [`star_number`] for a count (points, skip): a whole number, 0 or more.
fn star_count(doc: &Document, p: &Value, cmd: &str, key: &str) -> Result<Option<(u32, Option<String>)>> {
    let Some((x, e)) = star_number(doc, p, cmd, key, Kind::Unitless)? else { return Ok(None) };
    let n = x.round();
    if (x - n).abs() > 1e-9 || !(0.0..=f64::from(u32::MAX)).contains(&n) {
        return Err(bad(cmd, format!("`{key}` must be a whole number, 0 or more")));
    }
    Ok(Some((n as u32, e)))
}

/// The star numbers in a call, over `base` where a key is left out. A number replaces the
/// expression a key had; an expression naming a parameter is kept, so the star follows it.
fn spec_from(doc: &Document, p: &Value, cmd: &str, base: Option<&StarSpec>) -> Result<StarSpec> {
    let center = match (p.get("center").filter(|v| !v.is_null()), base) {
        (Some(Value::Array(a)), _) if a.len() == 2 && a.iter().any(Value::is_string) => {
            let [x, y] = a.as_slice() else { return Err(bad(cmd, "`center` must be [x, y]")) };
            let xy = json!({"x": x, "y": y});
            let x = star_number(doc, &xy, cmd, "x", Kind::Length)?;
            let y = star_number(doc, &xy, cmd, "y", Kind::Length)?;
            match (x, y) {
                (Some(x), Some(y)) => Some((x, y)),
                _ => return Err(bad(cmd, "`center` must be [x, y]: numbers in mm or expressions")),
            }
        }
        (Some(_), _) | (None, None) => {
            let c = req_vec2(cmd, p, "center")?;
            Some(((c.x, None), (c.y, None)))
        }
        (None, Some(_)) => None,
    };
    let def = StarSpec {
        center: Vec2::ZERO,
        points: 5,
        outer: 0.0,
        inner_percent: 40.0,
        skip: 0,
        tip_round: 0.0,
        valley_round: 0.0,
        angle: 0.0,
        circles: false,
        exprs: StarExprs::default(),
        follows: None,
    };
    let mut spec = base.cloned().unwrap_or(def);
    if let Some(((x, ex), (y, ey))) = center {
        spec.center = Vec2::new(x, y);
        (spec.exprs.x, spec.exprs.y) = (ex, ey);
    }
    if let Some((v, e)) = star_count(doc, p, cmd, "points")? {
        (spec.points, spec.exprs.points) = (v, e);
    }
    if let Some((v, e)) = star_number(doc, p, cmd, "outer_radius", Kind::Length)? {
        (spec.outer, spec.exprs.outer) = (v, e);
    }
    if let Some((v, e)) = star_number(doc, p, cmd, "inner_percent", Kind::Unitless)? {
        (spec.inner_percent, spec.exprs.inner_percent) = (v, e);
    }
    if let Some((v, e)) = star_count(doc, p, cmd, "skip")? {
        (spec.skip, spec.exprs.skip) = (v, e);
    }
    if let Some((v, e)) = star_number(doc, p, cmd, "tip_round", Kind::Length)? {
        (spec.tip_round, spec.exprs.tip_round) = (v, e);
    }
    if let Some((v, e)) = star_number(doc, p, cmd, "valley_round", Kind::Length)? {
        (spec.valley_round, spec.exprs.valley_round) = (v, e);
    }
    if let Some((v, e)) = star_number(doc, p, cmd, "angle", Kind::Angle)? {
        (spec.angle, spec.exprs.angle) = (v, e);
    }
    if p.get("circles").is_some_and(|v| !v.is_null()) {
        spec.circles = bool_(p, "circles").ok_or_else(|| bad(cmd, "`circles` must be true or false"))?;
    }
    spec.check().map_err(|e| bad(cmd, e.to_string()))?;
    Ok(spec)
}

fn numbers(s: &StarSpec) -> Value {
    json!({
        "center": [s.center.x, s.center.y], "points": s.points, "outer_radius": s.outer, "inner_percent": s.inner_percent,
        "skip": s.skip, "tip_round": s.tip_round, "valley_round": s.valley_round, "angle": s.angle, "circles": s.circles,
        "exprs": (!s.exprs.is_empty()).then(|| json!(s.exprs)),
    })
}

fn reply(sk: &Sketch, link: &str, spec: &StarSpec) -> Result<Value> {
    let out = StarResult { link: link.to_owned(), curves: ids_of(sk, &sk.link_curves(link)), star: numbers(spec), param_warnings: Vec::new() };
    serde_json::to_value(out).map_err(|e| bad("sketch.star", e.to_string()))
}

fn star(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "sketch.star";
    let spec = spec_from(&s.doc, p, cmd, None)?;
    let id = sketch_store::active(s).ok_or_else(|| bad(cmd, "no sketch is being edited"))?;
    sketch_store::update(s, id, |_, _, sk| {
        let geom = star_geometry(&spec).map_err(|e| bad(cmd, e.to_string()))?;
        let link = sk.add_link(LinkKind::Star, LinkSource::Star(spec.clone()), &geom)?;
        reply(sk, &link, &spec)
    })
}

/// The star link a call names: `link`, or the link of curve `entity`.
fn star_link(sk: &Sketch, p: &Value, cmd: &str) -> Result<String> {
    let id = match (p.get("link").and_then(Value::as_str), p.get("entity").and_then(Value::as_str)) {
        (Some(l), _) => l.to_owned(),
        (None, Some(e)) => sk
            .curve_index(e)
            .and_then(|i| sk.curves.get(i))
            .and_then(|c| c.link.clone())
            .ok_or_else(|| bad(cmd, format!("`{e}` is not part of a star")))?,
        _ => return Err(bad(cmd, "give `link` or `entity`")),
    };
    match sk.link(&id) {
        Some(l) if l.kind == LinkKind::Star => Ok(id),
        _ => Err(bad(cmd, format!("`{id}` is not a star"))),
    }
}

/// Rebuild the outlines of the stars that follow another, from the numbers their sources have now.
pub(in crate::cmd) fn refresh_followers(sk: &mut Sketch, cmd: &str) -> Result<()> {
    for link in sk.resolve_star_follows() {
        let Some(LinkSource::Star(spec)) = sk.link(&link).map(|l| l.source.clone()) else { continue };
        let geom = star_geometry(&spec).map_err(|e| bad(cmd, e.to_string()))?;
        sk.update_link(&link, &geom)?;
    }
    Ok(())
}

fn edit_star(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "sketch.edit_star";
    let id = sketch_store::active(s).ok_or_else(|| bad(cmd, "no sketch is being edited"))?;
    sketch_store::update(s, id, |_, doc, sk| {
        let link = star_link(sk, p, cmd)?;
        // The numbers its expressions give now (a parameter may have changed since the sketch was
        // stored); one that cannot be evaluated is left as it was, to be replaced by this edit.
        let _ = doc.apply_text_values(&doc.param_values().0, sk);
        let Some(LinkSource::Star(old)) = sk.link(&link).map(|l| l.source.clone()) else { return Err(bad(cmd, "not a star")) };
        if let Some(follow) = &old.follows {
            return Err(bad(
                cmd,
                format!(
                    "this star is a mirror copy of `{}`: edit that star and the copy follows; move the copy to make it a star of its own",
                    follow.source
                ),
            ));
        }
        let spec = spec_from(doc, p, cmd, Some(&old))?;
        let geom = star_geometry(&spec).map_err(|e| bad(cmd, e.to_string()))?;
        sk.set_link_source(&link, LinkSource::Star(spec.clone()));
        sk.update_link(&link, &geom)?;
        refresh_followers(sk, cmd)?;
        reply(sk, &link, &spec)
    })
}
