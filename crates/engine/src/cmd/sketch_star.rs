//! `sketch.star` and `sketch.edit_star`: a star kept as its numbers, so it can be edited after it
//! is drawn (Sketch › Create › Polygon › Star, and Edit Star).

use serde_json::{Value, json};
use solidcraft_sketch::{LinkKind, LinkSource, Sketch, StarSpec, star_geometry};

use super::sketch::ids_of;
use super::sketch_store;
use super::{CommandSpec, in_sketch};
use crate::params::{bad, bool_, num, req_vec2};
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

/// The star numbers in a call, over `base` where a key is left out.
fn spec_from(p: &Value, cmd: &str, base: Option<&StarSpec>) -> Result<StarSpec> {
    let int = |k: &str| -> Result<Option<u32>> {
        match p.get(k).filter(|v| !v.is_null()) {
            None => Ok(None),
            Some(v) => {
                v.as_u64().and_then(|n| u32::try_from(n).ok()).map(Some).ok_or_else(|| bad(cmd, format!("`{k}` must be a whole number, 0 or more")))
            }
        }
    };
    let float = |k: &str| -> Result<Option<f64>> {
        match p.get(k).filter(|v| !v.is_null()) {
            None => Ok(None),
            Some(_) => num(p, k).map(Some).ok_or_else(|| bad(cmd, format!("`{k}` must be a finite number"))),
        }
    };
    let center = match (p.get("center").filter(|v| !v.is_null()), base) {
        (Some(_), _) | (None, None) => req_vec2(cmd, p, "center")?,
        (None, Some(b)) => b.center,
    };
    let def = StarSpec { center, points: 5, outer: 0.0, inner_percent: 40.0, skip: 0, tip_round: 0.0, valley_round: 0.0, angle: 0.0, circles: false };
    let b = base.cloned().unwrap_or(def);
    let spec = StarSpec {
        center,
        points: int("points")?.unwrap_or(b.points),
        outer: float("outer_radius")?.unwrap_or(b.outer),
        inner_percent: float("inner_percent")?.unwrap_or(b.inner_percent),
        skip: int("skip")?.unwrap_or(b.skip),
        tip_round: float("tip_round")?.unwrap_or(b.tip_round),
        valley_round: float("valley_round")?.unwrap_or(b.valley_round),
        angle: float("angle")?.unwrap_or(b.angle),
        circles: match p.get("circles").filter(|v| !v.is_null()) {
            None => b.circles,
            Some(_) => bool_(p, "circles").ok_or_else(|| bad(cmd, "`circles` must be true or false"))?,
        },
    };
    spec.check().map_err(|e| bad(cmd, e.to_string()))?;
    Ok(spec)
}

fn numbers(s: &StarSpec) -> Value {
    json!({
        "center": [s.center.x, s.center.y], "points": s.points, "outer_radius": s.outer, "inner_percent": s.inner_percent,
        "skip": s.skip, "tip_round": s.tip_round, "valley_round": s.valley_round, "angle": s.angle, "circles": s.circles,
    })
}

fn reply(sk: &Sketch, link: &str, spec: &StarSpec) -> Result<Value> {
    let out = StarResult { link: link.to_owned(), curves: ids_of(sk, &sk.link_curves(link)), star: numbers(spec), param_warnings: Vec::new() };
    serde_json::to_value(out).map_err(|e| bad("sketch.star", e.to_string()))
}

fn star(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "sketch.star";
    let spec = spec_from(p, cmd, None)?;
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

fn edit_star(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "sketch.edit_star";
    let id = sketch_store::active(s).ok_or_else(|| bad(cmd, "no sketch is being edited"))?;
    sketch_store::update(s, id, |_, _, sk| {
        let link = star_link(sk, p, cmd)?;
        let Some(LinkSource::Star(old)) = sk.link(&link).map(|l| l.source.clone()) else { return Err(bad(cmd, "not a star")) };
        let spec = spec_from(p, cmd, Some(&old))?;
        let geom = star_geometry(&spec).map_err(|e| bad(cmd, e.to_string()))?;
        sk.set_link_source(&link, LinkSource::Star(spec.clone()));
        sk.update_link(&link, &geom)?;
        reply(sk, &link, &spec)
    })
}
