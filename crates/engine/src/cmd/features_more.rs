//! Rib, Web, Emboss, Replace Face, Align, Place on Plane and Remove.

use serde_json::Value;
use solidcore_record::Vec3;
use solidcraft_doc::expr::Kind;
use solidcraft_doc::{AlignFrame, FeatureKind};
use solidcraft_doc::{BodyName, SketchId};
use solidcraft_doc::{Dir, FacePick, Position};
use solidcraft_geom::prelude::*;

use super::CommandSpec;
use super::features::{
    add_feature, axis_dir_param, check_expr, feature_sketch, length_operands, plane_param, point_param, profiles, typed_length_unit,
};
use crate::params::{bad, bool_, expr, req_expr, str_, string_list, vec3};
use crate::schema::align::AlignParams;
use crate::schema::features::FeatureResult;
use crate::schema::features_more::{BoundaryFillParams, CoilParams, EmbossParams, PlaceOnPlaneParams, RemoveParams, ReplaceFaceParams, RibParams};
use crate::schema::{TypedContract, output_schema_for};
use crate::{Result, Session};

/// Rib and Web take the same keys.
const RIB_CONTRACT: TypedContract<RibParams> = TypedContract::new();
const ALIGN_CONTRACT: TypedContract<AlignParams> = TypedContract::new();
const REMOVE_CONTRACT: TypedContract<RemoveParams> = TypedContract::new();
const PLACE_ON_PLANE_CONTRACT: TypedContract<PlaceOnPlaneParams> = TypedContract::new();
const BOUNDARY_FILL_CONTRACT: TypedContract<BoundaryFillParams> = TypedContract::new();
const COIL_CONTRACT: TypedContract<CoilParams> = TypedContract::new();
const EMBOSS_CONTRACT: TypedContract<EmbossParams> = TypedContract::new();
const REPLACE_FACE_CONTRACT: TypedContract<ReplaceFaceParams> = TypedContract::new();

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("solid.boundary_fill", "Boundary Fill", crate::checked!("solid.boundary_fill", boundary_fill))
        .at("SOLID", "CREATE")
        .icon("boundary_fill")
        .contract(&BOUNDARY_FILL_CONTRACT)
        .output(output_schema_for::<FeatureResult>)
        .params(
            "tools: [bodies]; cells: [[x,y,z] a point inside each cell to fill (a cell: inside some tools, outside the rest)]; operation?: new|join|cut|intersect; remove_tools?: bool",
        ),
    CommandSpec::new("solid.coil", "Coil", crate::checked!("solid.coil", coil)).at("SOLID", "CREATE").icon("coil").contract(&COIL_CONTRACT).output(output_schema_for::<FeatureResult>).params(
        "diameter; two of revolutions (or turns), height, pitch; section_size; section?: circular|square; section_position?: inside|center|outside; \
         base?: [x,y,z] (numbers in mm or length expressions; default: the origin); axis?: X|Y|Z|[x,y,z] (default Z); start_angle?; clockwise?: bool; \
         sketch? (a sketch; its plane is the section plane, so its placement sets the radius and start phase), profiles? (default: all; one closed profile without holes); \
         operation?, targets?, name?, body_name?",
    ),
    CommandSpec::new("solid.rib", "Rib", crate::checked!("solid.rib", rib))
        .at("SOLID", "CREATE")
        .icon("rib")
        .contract(&RIB_CONTRACT)
        .output(output_schema_for::<FeatureResult>)
        .params("sketch, curves: [open curve ids, in order]; thickness (across the sketch plane, or across the curves when normal); depth? (default: to the next face); flip?: bool (fill to the other side); direction?: in_plane|normal (grow across the sketch plane, the default, or along its normal); extend?: bool (run the curves' open ends on to the body; default false)"),
    CommandSpec::new("solid.web", "Web", crate::checked!("solid.web", web))
        .at("SOLID", "CREATE")
        .icon("web")
        .contract(&RIB_CONTRACT)
        .output(output_schema_for::<FeatureResult>)
        .params("sketch, curves: [open curve ids] (one wall each); thickness; depth? (default: to the next face); flip?: bool; direction?: in_plane|normal; extend?: bool (default false)"),
    CommandSpec::new("solid.emboss", "Emboss", crate::checked!("solid.emboss", emboss))
        .at("SOLID", "CREATE")
        .icon("emboss")
        .contract(&EMBOSS_CONTRACT)
        .output(output_schema_for::<FeatureResult>)
        .params("sketch, profiles?; depth; mode?: emboss|deboss; targets?; face?: world point [x,y,z] on the face to emboss: planar, a cylinder wall or another curved face (inferred when exactly one supported tangent cylinder matches the selected profiles); one sketch tangent to a complete cylinder wall wraps round it, anything else is projected, and the reply warns when profiles are projected onto a curved face; faces?: several such points, each profile going onto the face it lies over; text?: a text's group id (sketch.groups) in place of profiles, letters?: some of its letters by index; more_profiles?: [{sketch, profiles?|text, letters?}] more profiles from the same or other sketches; dx?, dy?: shift along the sketch's x and y; angle?: turn about the profiles' centre; flip?: raise or sink toward the sketch's back (no face); distribute?: for each profile in order (the first sketch's, then more_profiles'), the index in `faces` of the face it goes on, centred on that point (six letters on six faces); without it each profile goes onto the face it lies over"),
    CommandSpec::new("solid.replace_face", "Replace Face", crate::checked!("solid.replace_face", replace_face))
        .contract(&REPLACE_FACE_CONTRACT)
        .output(output_schema_for::<FeatureResult>)
        .at("SOLID", "MODIFY")
        .icon("replace_face")
        .params("faces: [[x,y,z] points on planar faces, or face names from model.faces]; target: plane (XY|XZ|YZ|construction plane|{origin, normal}) | target_face: [x,y,z] on a parallel planar face; body?"),
    CommandSpec::new("solid.align", "Align", crate::checked!("solid.align", align))
        .at("SOLID", "MODIFY")
        .icon("align")
        .contract(&ALIGN_CONTRACT)
        .output(output_schema_for::<FeatureResult>)
        .params("bodies: [names]; from: [x,y,z] | \"origin\" | {snap: vertex|edge_mid|face_center|circle_center, at: [x,y,z]} | from_face: [x,y,z] | from_plane: XY|XZ|YZ|construction or offset plane name or id|{origin, normal}; to: (the same) | to_face: [x,y,z] | to_plane: (a plane as from_plane; the point or face goes onto it along its normal, and the plane may move later: the bodies follow); from_frame / to_frame: a frame found again whenever the design changes, so the bodies follow it, in place of that side's point, face or plane: a joint origin as joint.create takes one ({face|circle|edge|point: [x,y,z]}, a face's name, a named joint origin) or {sketch, point|curve} (a sketch point, or a circle's, arc's or ellipse's centre, z along the sketch's normal), on the bodies' own component; faces, planes and frames also turn to meet; flip?: bool; axes?: [x, y, z] (point-to-point only: which world axes the offset moves along; default all three, at least one true); angle?: expr (a turn about the target's direction after the move)"),
    CommandSpec::new("solid.place_on_plane", "Place on Plane", crate::checked!("solid.place_on_plane", place_on_plane))
        .at("SOLID", "MODIFY")
        .icon("align")
        .contract(&PLACE_ON_PLANE_CONTRACT)
        .output(output_schema_for::<FeatureResult>)
        .params("bodies: [names]; plane?: XY|XZ|YZ (default XY)|construction plane name or id|{origin, normal}|{face: [x,y,z]} (a planar face of a body that does not move, followed as it changes); offset?: length expr (the gap left between the bodies and the plane, default 0); each?: bool (default false: the bodies move together as one set, keeping where they are relative to each other; true: each rests on the plane by itself). The bodies move along the plane's normal only, never turning, until the point nearest the plane (of the exact geometry) touches it from the side the normal points to"),
    CommandSpec::new("solid.remove", "Remove", crate::checked!("solid.remove", remove))
        .at("SOLID", "MODIFY")
        .icon("delete")
        .contract(&REMOVE_CONTRACT)
        .output(output_schema_for::<FeatureResult>)
        .params("bodies: [names] (removed from here on in the timeline)"),
];

fn coil(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "solid.coil";
    let given = |k: &str| p.get(k).is_some_and(|v| !v.is_null());
    let profile_mode = given("sketch");
    if profile_mode {
        if let Some(k) = ["diameter", "section_size", "section", "section_position", "start_angle"].into_iter().find(|k| given(k)) {
            return Err(bad(cmd, format!("`{k}` does not apply with `sketch`: the section comes from the sketch")));
        }
    } else if given("profiles") {
        return Err(bad(cmd, "`profiles` needs `sketch`"));
    }
    let sketch = if profile_mode { Some(solidcraft_doc::SketchId(super::features::sketch_id(s, p.get("sketch"), cmd, "sketch")?)) } else { None };
    let profiles = if profile_mode { super::features::profiles(p, cmd)? } else { solidcraft_doc::ProfileSel::All };
    let base = point_param(s, p, "base", cmd)?.unwrap_or_else(|| solidcraft_doc::point_expr(Vec3::ZERO));
    let axis = axis_dir_param(p, cmd)?;
    let diameter = if profile_mode {
        None
    } else {
        let d = req_expr(cmd, p, "diameter")?;
        check_expr(s, &d, Kind::Length, cmd, "diameter")?;
        Some(d)
    };
    let turns = expr(p, "revolutions").or_else(|| expr(p, "turns"));
    let (height, pitch) = (expr(p, "height"), expr(p, "pitch"));
    // The pitch or the revolutions from the height keep the units the lengths were given in.
    let unit = typed_length_unit(s);
    let (turns, pitch) = match (turns, height, pitch) {
        (Some(n), _, Some(pt)) => (n, pt),
        (Some(n), Some(h), None) => {
            let [h] = length_operands(s, [(&h, &unit)]);
            (n.clone(), format!("({h}) / ({n})").into())
        }
        (None, Some(h), Some(pt)) => {
            let [h, by] = length_operands(s, [(&h, &unit), (&pt, &unit)]);
            (format!("({h}) / ({by})").into(), pt)
        }
        _ => return Err(bad(cmd, "give two of `revolutions`, `height` and `pitch`")),
    };
    check_expr(s, &turns, Kind::Unitless, cmd, "revolutions")?;
    check_expr(s, &pitch, Kind::Length, cmd, "pitch")?;
    let section_size = if profile_mode {
        None
    } else {
        let size = req_expr(cmd, p, "section_size")?;
        check_expr(s, &size, Kind::Length, cmd, "section_size")?;
        Some(size)
    };
    let section = match str_(p, "section").map(str::to_ascii_lowercase).as_deref() {
        None | Some("circular" | "circle") => solidcraft_doc::CoilSection::Circular,
        Some("square") => solidcraft_doc::CoilSection::Square,
        Some(o) => return Err(bad(cmd, format!("unknown section `{o}` (circular or square)"))),
    };
    let position = match str_(p, "section_position").map(str::to_ascii_lowercase).as_deref() {
        None | Some("center" | "on" | "on_center") => 0,
        Some("inside") => -1,
        Some("outside") => 1,
        Some(o) => return Err(bad(cmd, format!("unknown section position `{o}` (inside, center or outside)"))),
    };
    let start_angle = expr(p, "start_angle");
    if let Some(a) = &start_angle {
        check_expr(s, a, Kind::Angle, cmd, "start_angle")?;
    }
    let clockwise = bool_(p, "clockwise").unwrap_or(false);
    let operation = super::features::operation(p, cmd)?;
    add_feature(
        s,
        p,
        FeatureKind::Coil {
            base,
            axis: Dir(axis),
            sketch,
            profiles,
            diameter,
            pitch,
            turns,
            section_size,
            section,
            position,
            start_angle,
            clockwise,
            operation,
            targets: string_list(cmd, p, "targets")?.into_iter().map(BodyName).collect(),
        },
    )
}

fn curves_param(p: &Value, cmd: &str) -> Result<Vec<String>> {
    let c = string_list(cmd, p, "curves")?;
    if c.is_empty() || c.len() > 1000 {
        return Err(bad(cmd, "`curves` must list 1…1000 sketch curve ids"));
    }
    Ok(c)
}

fn rib_like(s: &mut Session, p: &Value, cmd: &str, web: bool) -> Result<Value> {
    let sketch = feature_sketch(s, p, cmd)?;
    let curves = curves_param(p, cmd)?;
    {
        let st = s.model.state();
        let ss = st.sketch(sketch).ok_or_else(|| bad(cmd, "the sketch is not evaluated"))?;
        if let Some(c) = curves.iter().find(|c| ss.sketch.curve_index(c).is_none()) {
            return Err(bad(cmd, format!("no curve `{c}` in the sketch")));
        }
    }
    let thickness = req_expr(cmd, p, "thickness")?;
    check_expr(s, &thickness, Kind::Length, cmd, "thickness")?;
    let depth = expr(p, "depth");
    if let Some(d) = &depth {
        check_expr(s, d, Kind::Length, cmd, "depth")?;
    }
    let flip = bool_(p, "flip").unwrap_or(false);
    let normal = match str_(p, "direction").map(str::to_ascii_lowercase).as_deref() {
        None | Some("in_plane" | "plane") => false,
        Some("normal") => true,
        Some(o) => return Err(bad(cmd, format!("unknown direction `{o}` (in_plane or normal)"))),
    };
    // Opt-in (#13019): without `extend` a rib ends where its curves end, as before.
    let extend = bool_(p, "extend").unwrap_or(false);
    add_feature(s, p, FeatureKind::Rib { sketch: SketchId(sketch), curves, thickness, depth, flip, web, normal, extend })
}

fn rib(s: &mut Session, p: &Value) -> Result<Value> {
    rib_like(s, p, "solid.rib", false)
}

fn web(s: &mut Session, p: &Value) -> Result<Value> {
    rib_like(s, p, "solid.web", true)
}

fn emboss(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "solid.emboss";
    let sketch = feature_sketch(s, p, cmd)?;
    let profiles = emboss_profiles(s, p, sketch)?;
    let depth = req_expr(cmd, p, "depth")?;
    check_expr(s, &depth, Kind::Length, cmd, "depth")?;
    let deboss = match str_(p, "mode").map(str::to_ascii_lowercase).as_deref() {
        None | Some("emboss") => false,
        Some("deboss") => true,
        Some(m) => return Err(bad(cmd, format!("unknown mode `{m}` (emboss or deboss)"))),
    };
    let more_profiles = emboss_more_profiles(s, p)?;
    let mut sketches = vec![sketch];
    sketches.extend(more_profiles.iter().map(|m| m.sketch.0));
    let mut shift = [None, None, None];
    for (slot, (key, kind)) in shift.iter_mut().zip([("dx", Kind::Length), ("dy", Kind::Length), ("angle", Kind::Angle)]) {
        *slot = expr(p, key);
        if let Some(e) = slot.as_deref() {
            check_expr(s, e, kind, cmd, key)?;
        } else if p.get(key).is_some_and(|v| !v.is_null()) {
            return Err(bad(cmd, format!("`{key}` must be a number or an expression")));
        }
    }
    let [dx, dy, angle] = shift;
    let flip = bool_(p, "flip").unwrap_or(false);
    let mut targets = string_list(cmd, p, "targets")?;
    let (key, points): (&str, Vec<&Value>) = match (p.get("face"), p.get("faces")) {
        (Some(_), Some(_)) => return Err(bad(cmd, "give `face` or `faces`, not both")),
        (Some(one), None) => ("face", vec![one]),
        (None, Some(Value::Array(many))) if (1..=64).contains(&many.len()) => ("faces", many.iter().collect()),
        (None, Some(_)) => return Err(bad(cmd, "`faces` must list 1 to 64 points [x,y,z]")),
        (None, None) => ("face", Vec::new()),
    };
    if flip && !points.is_empty() {
        return Err(bad(cmd, "an emboss on a selected face can't be flipped: the face's outside sets its direction (Deboss sinks it)"));
    }
    if points.len() == 1 && targets.len() > 1 {
        return Err(bad(cmd, "an emboss on a selected face needs one target body"));
    }
    // Distribute: the face (index into the points) each profile goes on, in profile order.
    let distribute: Vec<u32> = match p.get("distribute") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(list)) if (1..=4096).contains(&list.len()) => list
            .iter()
            .map(|v| v.as_u64().and_then(|n| u32::try_from(n).ok()))
            .collect::<Option<Vec<u32>>>()
            .ok_or_else(|| bad(cmd, "`distribute` must list face indices: 0 for the first face in `faces`"))?,
        Some(_) => return Err(bad(cmd, "`distribute` must list, for each profile, the index of the face it goes on (1 to 4096 entries)")),
    };
    if let Some(far) = distribute.iter().find(|n| usize::try_from(**n).map_or(true, |n| n >= points.len())) {
        return Err(bad(
            cmd,
            match points.len() {
                0 => "`distribute` puts the profiles on selected faces: give the faces in `faces`".to_owned(),
                n => format!("`distribute` names face {far}, but `{key}` gives {n} (0 to {})", n - 1),
            },
        ));
    }
    let mut faces = Vec::with_capacity(points.len());
    let mut bodies: Vec<String> = Vec::new();
    for value in points {
        let (face, body) = emboss_face_param(s, key, value, &targets, &sketches)?;
        if faces.iter().any(|f: &solidcraft_doc::EmbossFace| f.body == face.body && f.path == face.path && f.at.dist(face.at.0) < 1e-9) {
            return Err(bad(cmd, "a face is picked twice: pick each face once"));
        }
        if !bodies.contains(&body) {
            bodies.push(body);
        }
        faces.push(face);
    }
    let face = if faces.is_empty() {
        // Only one sketch's profiles, unflipped, are wrapped onto a cylinder found for them.
        let inferred = if more_profiles.is_empty() && !flip { infer_emboss_selection(s, sketch, &profiles, &depth, deboss, &targets)? } else { None };
        if let Some(face) = &inferred {
            targets = vec![face.body.0.clone()];
        }
        inferred
    } else {
        targets = bodies;
        Some(faces.remove(0))
    };
    add_feature(
        s,
        p,
        FeatureKind::Emboss {
            sketch: SketchId(sketch),
            profiles,
            depth,
            deboss,
            targets: targets.into_iter().map(BodyName).collect(),
            face,
            more_profiles,
            more_faces: faces,
            dx,
            dy,
            angle,
            flip,
            distribute,
        },
    )
}

/// An Emboss source's profiles: `profiles`, or a whole text of the sketch (`text`: its group id,
/// as `sketch.groups` lists it) or some of its letters (`letters`: member indices). A text's
/// profiles are kept by their outline curves, so they follow the text when it changes; a letter's
/// counter (the inside of an O) is its hole, never a profile of its own.
fn emboss_profiles(s: &Session, p: &Value, sketch: u64) -> Result<solidcraft_doc::ProfileSel> {
    let cmd = "solid.emboss";
    let Some(text) = p.get("text").filter(|v| !v.is_null()) else {
        if p.get("letters").is_some_and(|v| !v.is_null()) {
            return Err(bad(cmd, "`letters` needs the `text` they are in"));
        }
        return profiles(p, cmd);
    };
    if p.get("profiles").is_some_and(|v| !v.is_null()) {
        return Err(bad(cmd, "give `profiles` or `text`, not both"));
    }
    let text = text.as_str().ok_or_else(|| bad(cmd, "`text` must be a text's group id (sketch.groups)"))?;
    let state = s.model.state();
    let ss = state.sketch(sketch).ok_or_else(|| bad(cmd, "the sketch is not evaluated"))?;
    let groups = ss.sketch.curve_groups();
    let group =
        groups.iter().find(|g| g.id == text).ok_or_else(|| bad(cmd, format!("the sketch has no text `{text}` (sketch.groups lists them)")))?;
    let curves: std::collections::BTreeSet<&str> = match p.get("letters").filter(|v| !v.is_null()) {
        None => group.curves.iter().map(String::as_str).collect(),
        Some(Value::Array(letters)) if !letters.is_empty() && letters.len() <= 10_000 => {
            let mut picked = std::collections::BTreeSet::new();
            for letter in letters {
                let member = letter.as_u64().and_then(|k| group.members.get(usize::try_from(k).ok()?)).ok_or_else(|| {
                    bad(cmd, format!("`letters` must be indices 0 to {} of the text's letters", group.members.len().saturating_sub(1)))
                })?;
                picked.extend(member.curves.iter().map(String::as_str));
            }
            picked
        }
        Some(_) => return Err(bad(cmd, "`letters` must list one or more letter indices")),
    };
    let candidates: Vec<&solidcraft_sketch::Profile> =
        ss.profiles.iter().filter(|pr| !pr.outer_curves.is_empty() && pr.outer_curves.iter().all(|c| curves.contains(c.as_str()))).collect();
    // A candidate bounded by another candidate's hole is that letter's counter.
    let holes: Vec<std::collections::BTreeSet<&str>> =
        candidates.iter().flat_map(|pr| pr.hole_curves.iter().map(|h| h.iter().map(String::as_str).collect())).collect();
    let mut loops: Vec<Vec<String>> = candidates
        .into_iter()
        .filter(|pr| !holes.contains(&pr.outer_curves.iter().map(String::as_str).collect()))
        .map(|pr| pr.outer_curves.clone())
        .collect();
    // In reading order, so the first letter goes on a distribution's first face.
    loops.sort_by_key(|l| group.members.iter().position(|m| l.first().is_some_and(|c| m.curves.contains(c))).unwrap_or(usize::MAX));
    if loops.is_empty() {
        return Err(bad(cmd, format!("the text `{text}` makes no closed profile to emboss")));
    }
    Ok(solidcraft_doc::ProfileSel::Curves { loops })
}

/// `more_profiles`: more sketches' (or the same sketch's) profiles, `[{sketch, profiles?}]`.
fn emboss_more_profiles(s: &Session, p: &Value) -> Result<Vec<solidcraft_doc::EmbossProfiles>> {
    let cmd = "solid.emboss";
    let list = match p.get("more_profiles") {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(list)) if list.len() <= 64 => list,
        Some(_) => return Err(bad(cmd, "`more_profiles` must list at most 64 entries {sketch, profiles?}")),
    };
    list.iter()
        .map(|entry| {
            if !entry.is_object() || entry.get("sketch").is_none() {
                return Err(bad(cmd, "each `more_profiles` entry must be {sketch, profiles?} or {sketch, text, letters?}"));
            }
            let sketch = feature_sketch(s, entry, cmd)?;
            let profiles = emboss_profiles(s, entry, sketch)?;
            let state = s.model.state();
            let ss = state.sketch(sketch).ok_or_else(|| bad(cmd, "the sketch is not evaluated"))?;
            solidcraft_doc::select_profiles(ss, &profiles).map_err(|e| bad(cmd, e.to_string()))?;
            Ok(solidcraft_doc::EmbossProfiles { sketch: SketchId(sketch), profiles })
        })
        .collect()
}

/// A picked face of an Emboss (`face`, or an item of `faces`): a world point on one face of the
/// target bodies, certified on the exact face. Returns the selection and its body's name.
fn emboss_face_param(s: &Session, key: &str, value: &Value, targets: &[String], sketches: &[u64]) -> Result<(solidcraft_doc::EmbossFace, String)> {
    let cmd = "solid.emboss";
    let local = vec3(value).ok_or_else(|| bad(cmd, format!("`{key}` must be a finite point [x,y,z]")))?;
    let at = solidcraft_doc::apply_point(&s.active_component_frame(), local);
    let state = s.world_state()?;
    let mut candidates = Vec::new();
    // The nearest face off the point: (distance, body), for the error when none holds it.
    let mut nearest: Option<(f64, &str)> = None;
    for body in &state.bodies {
        let definition = body.definition_name();
        if !targets.is_empty() && !targets.iter().any(|n| n == definition || n == &body.name) {
            continue;
        }
        let mesh = body.mesh();
        let closest = mesh
            .triangles
            .iter()
            .zip(&mesh.tri_face)
            .filter_map(|(tri, face)| {
                let [a, b, c] = mesh.tri(tri)?;
                let n = (b - a).cross(c - a).normalized()?;
                let q = at - n * (at - a).dot(n);
                let inside = [(a, b), (b, c), (c, a)].iter().all(|(u, v)| (*v - *u).cross(q - *u).dot(n) >= -1e-9);
                inside.then_some(((at - a).dot(n).abs(), *face as usize))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        match closest {
            Some((distance, index)) if distance < (body.body.size() * 1e-3).max(1e-3) => candidates.push((body, index)),
            Some((distance, _)) if nearest.is_none_or(|(d, _)| distance < d) => nearest = Some((distance, body.name.as_str())),
            _ => {}
        }
    }
    // #392: say what is wrong with the point, not only that it picks no single face.
    let point = format!("[{}, {}, {}]", local.x, local.y, local.z);
    match (candidates.as_slice(), nearest) {
        ([], Some((d, body))) => {
            return Err(bad(cmd, format!("`{key}` {point} lies on no face: the nearest, on `{body}`, is {d:.3} mm away; give a point on the face")));
        }
        ([], None) => return Err(bad(cmd, format!("`{key}` {point} lies on no face of the target bodies"))),
        ([_], _) => {}
        (more, _) => {
            let names: Vec<&str> = more.iter().map(|(b, _)| b.name.as_str()).collect();
            return Err(bad(cmd, format!("`{key}` {point} lies on faces of {} bodies ({}): name one in `targets`", names.len(), names.join(", "))));
        }
    }
    let (selected, index) = candidates.first().copied().ok_or_else(|| bad(cmd, "no target face"))?;
    // Viewport picks lie on tessellated chords: certify their distance and store the point
    // on the exact surface for later replay.
    let at = solidcraft_kernel::emboss_anchor(&selected.body, index, at).map_err(|e| bad(cmd, e.to_string()))?;
    let face = emboss_selection(s, selected, at)?;
    for sketch in sketches {
        check_emboss_positions(s, *sketch, selected, &face)?;
    }
    Ok((face, selected.definition_name().to_owned()))
}

fn emboss_selection(s: &Session, selected: &solidcraft_doc::ModelBody, at: Vec3) -> Result<solidcraft_doc::EmbossFace> {
    let cmd = "solid.emboss";
    let name = selected.definition_name();
    let original = s.model.state();
    let owner = s.doc.model_body_component(selected);
    let paths: Vec<_> = s
        .doc
        .placements()
        .into_iter()
        .filter(|(comp, path, _)| *comp == owner && s.doc.world_body_name(&original, name, owner, path) == selected.name)
        .map(|(_, path, _)| path)
        .collect();
    if paths.len() != 1 {
        return Err(bad(cmd, "the selected body's placement is ambiguous"));
    }
    let path = paths.into_iter().next().ok_or_else(|| bad(cmd, "selected placement"))?;
    let author_path = s.active_occurrence_path.clone().unwrap_or_else(|| s.doc.first_path(s.active_component));
    if path.len() > 64 || author_path.len() > 64 {
        return Err(bad(cmd, "the selected placement is too deep"));
    }
    let inverse = solidcraft_doc::mat_inverse(&s.active_component_frame()).ok_or_else(|| bad(cmd, "invalid active frame"))?;
    Ok(solidcraft_doc::EmbossFace { body: name.into(), at: FacePick(solidcraft_doc::apply_point(&inverse, at)), path, author_path })
}

fn check_emboss_positions(s: &Session, sketch: u64, body: &solidcraft_doc::ModelBody, face: &solidcraft_doc::EmbossFace) -> Result<()> {
    let cmd = "solid.emboss";
    let owner = s.doc.model_body_component(body);
    let sketch_owner = s.doc.feature(sketch).map(|f| f.component).ok_or_else(|| bad(cmd, "profile sketch no longer exists"))?;
    let sketch_path = if sketch_owner == owner {
        face.path.clone()
    } else if sketch_owner == s.active_component {
        face.author_path.clone()
    } else {
        s.doc.first_path(sketch_owner)
    };
    if face.path.iter().chain(&face.author_path).chain(&sketch_path).any(|id| s.pending_moves.contains_key(id)) {
        return Err(bad(cmd, "capture pending source, author and profile positions with Capture Position before an emboss on a face"));
    }
    Ok(())
}

fn infer_emboss_selection(
    s: &Session,
    sketch: u64,
    profiles: &solidcraft_doc::ProfileSel,
    depth: &str,
    deboss: bool,
    targets: &[String],
) -> Result<Option<solidcraft_doc::EmbossFace>> {
    let cmd = "solid.emboss";
    let original = s.model.state();
    let ss = original.sketch(sketch).ok_or_else(|| bad(cmd, "the sketch is not evaluated"))?;
    let regions = solidcraft_doc::select_profiles(ss, profiles).map_err(|e| bad(cmd, e.to_string()))?;
    let sketch_owner = s.doc.feature(sketch).map(|f| f.component).ok_or_else(|| bad(cmd, "profile sketch no longer exists"))?;
    let d = s.doc.eval(&s.doc.with_design_unit(depth, Kind::Length), Kind::Length).map_err(|e| bad(cmd, e.to_string()))?;
    let world = s.world_state()?;
    let placed = s.placed_document();
    let mut chosen = None;
    let mut faces = 0usize;
    for selected in &world.bodies {
        let name = selected.definition_name();
        if !targets.is_empty() && !targets.iter().any(|n| n == name || n == &selected.name) {
            continue;
        }
        let raw = original.body(name).ok_or_else(|| bad(cmd, "target definition no longer exists"))?;
        faces = faces.saturating_add(raw.body.face_count());
        if faces > 50_000 {
            return Err(bad(cmd, "cylinder target search exceeds face work limit"));
        }
        if !solidcraft_kernel::has_cylinder_emboss_wall(&raw.body).map_err(|e| bad(cmd, e.to_string()))? {
            continue;
        }
        let mut face = emboss_selection(s, selected, Vec3::ZERO)?;
        let body_frame = face.body_frame(&placed, raw).map_err(|e| bad(cmd, e.to_string()))?;
        let inverse = solidcraft_doc::mat_inverse(&body_frame).ok_or_else(|| bad(cmd, "invalid cylinder body frame"))?;
        let displayed = world.sketch(sketch).ok_or_else(|| bad(cmd, "the profile sketch is not displayed"))?;
        let plane = solidcore_record::Plane::new(
            solidcraft_doc::apply_point(&inverse, displayed.plane.origin),
            solidcraft_doc::apply_vector(&inverse, displayed.plane.x),
            solidcraft_doc::apply_vector(&inverse, displayed.plane.y),
        )
        .ok_or_else(|| bad(cmd, "invalid cylinder sketch frame"))?;
        if let Some((_, at)) =
            solidcraft_kernel::infer_cylinder_emboss(&raw.body, &plane, &regions, if deboss { -d } else { d }).map_err(|e| bad(cmd, e.to_string()))?
        {
            face.sketch_frame(&placed, s.active_component, sketch_owner, raw).map_err(|e| bad(cmd, e.to_string()))?;
            check_emboss_positions(s, sketch, selected, &face)?;
            let author_inverse = solidcraft_doc::mat_inverse(&s.active_component_frame()).ok_or_else(|| bad(cmd, "invalid author frame"))?;
            face.at = FacePick(solidcraft_doc::apply_point(&solidcraft_doc::mat_mul(&author_inverse, &body_frame), at));
            if chosen.is_some() {
                return Err(bad(cmd, "multiple tangent cylinders match the selected profiles; select a target face or body"));
            }
            chosen = Some(face);
        }
    }
    Ok(chosen)
}
fn face_points(s: &Session, p: &Value, k: &str, cmd: &str) -> Result<Vec<Vec3>> {
    let a = p.get(k).and_then(Value::as_array).ok_or_else(|| bad(cmd, format!("`{k}` must list points [x, y, z] or face names")))?;
    if a.is_empty() || a.len() > 1000 {
        return Err(bad(cmd, format!("`{k}` must list 1…1000 points")));
    }
    a.iter().map(|v| super::face::face_ref(s, v, str_(p, "body"), cmd)).collect()
}

fn replace_face(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "solid.replace_face";
    let faces = face_points(s, p, "faces", cmd)?;
    let refs = p.get("faces").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    // The body first: a face flush with another body's is looked up on that body alone.
    let picked = super::face::faces_body(s, p, refs, &faces);
    for f in &faces {
        if super::face::face_normal_in(s, *f, picked.lookup()).is_none() {
            return Err(bad(cmd, "not supported yet: replacing a face that is not planar"));
        }
    }
    let target = match p.get("target_face").and_then(vec3) {
        Some(tp) => {
            let n = super::face::face_normal(s, tp).ok_or_else(|| bad(cmd, "the target face must be planar"))?;
            let plane = solidcore_record::Plane::from_normal(tp, n).ok_or_else(|| bad(cmd, "the target face's plane"))?;
            solidcraft_doc::PlaneRef::Custom { plane }
        }
        None => plane_param(s, p.get("target"), cmd)?,
    };
    let body = picked.body.clone().map(BodyName);
    super::features::add_face_feature(&picked, s, p, FeatureKind::ReplaceFace { faces: faces.into_iter().map(FacePick).collect(), target, body })
}

/// What `solid.align` says when `axes` is not three booleans, or when none is true.
pub(crate) const AXES_SHAPE: &str = "`axes` must be [x, y, z] booleans";
pub(crate) const AXES_NONE: &str = "`axes` must enable at least one of X, Y and Z";

/// `axes` as `solid.align` reads it: three booleans, one per world axis, at least one of them
/// true; the message is the handler's.
pub(crate) fn axes_mask(v: &Value) -> std::result::Result<[bool; 3], &'static str> {
    let a = v.as_array().filter(|a| a.len() == 3).ok_or(AXES_SHAPE)?;
    let mut mask = [false; 3];
    for (slot, v) in mask.iter_mut().zip(a) {
        *slot = v.as_bool().ok_or(AXES_SHAPE)?;
    }
    if !mask.iter().any(|on| *on) {
        return Err(AXES_NONE);
    }
    Ok(mask)
}

/// `axes`: three booleans, one per world axis, for a point-to-point Align.
/// `None` when the call does not restrict the move.
fn axes_param(p: &Value, cmd: &str) -> Result<Option<[bool; 3]>> {
    let Some(v) = p.get("axes").filter(|v| !v.is_null()) else { return Ok(None) };
    let mask = axes_mask(v).map_err(|why| bad(cmd, why))?;
    Ok((mask != [true; 3]).then_some(mask))
}

/// An Align side given as a point (`from` or `to`), as the handler reads it.
pub(crate) enum SidePoint {
    /// The text `origin` (any case): the world origin.
    Origin,
    /// `{snap?, at}`: the nearest `snap` (vertex unless the object says another) to `at`.
    Snap { at: Vec3, kind: String },
    /// `[x, y, z]`.
    Point(Vec3),
}

/// The side `pk` (`from` or `to`) as a point, with the handler's message when it is none of the
/// forms; `fk` is the face key beside it (`from_face`).
pub(crate) fn side_point(v: Option<&Value>, pk: &str, fk: &str) -> std::result::Result<SidePoint, String> {
    // The origin point.
    if v.and_then(Value::as_str).is_some_and(|o| o.eq_ignore_ascii_case("origin")) {
        return Ok(SidePoint::Origin);
    }
    // A snap point: {snap: vertex|edge_mid|face_center|circle_center, at: [x,y,z]}.
    if let Some(o) = v.filter(|v| v.is_object()) {
        let at = o.get("at").and_then(vec3).ok_or_else(|| format!("`{pk}.at` must be [x, y, z]"))?;
        let kind = o.get("snap").and_then(Value::as_str).unwrap_or("vertex").to_string();
        return Ok(SidePoint::Snap { at, kind });
    }
    v.and_then(vec3).map(SidePoint::Point).ok_or_else(|| format!("give `{pk}`, `{fk}` or `{pk}_plane` ([x, y, z], \"origin\", or a plane)"))
}

fn align(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "solid.align";
    let bodies = string_list(cmd, p, "bodies")?;
    if bodies.is_empty() {
        return Err(bad(cmd, "`bodies` must list the bodies to move"));
    }
    {
        let st = s.model.state();
        if let Some(b) = bodies.iter().find(|b| st.body(b).is_none()) {
            return Err(bad(cmd, format!("no body `{b}`")));
        }
    }
    // Picks and snaps are where the view shows them (the world); the feature moves the bodies
    // in their component's frame, so both ends go into the first moved body's frame. A body
    // picked in another placed component (aligning two imported parts) is found where it
    // is shown, not where its own frame has it.
    let frame = s
        .placed_document()
        .body_frames_with_source(&s.model.state())
        .into_iter()
        .find(|(world, own, ..)| *world == bodies[0] || *own == bodies[0])
        .map(|(.., m)| m);
    let inverse = frame.and_then(|m| solidcraft_doc::mat_inverse(&m)).unwrap_or(solidcraft_doc::IDENTITY);
    // A side is a plane (`from_plane`/`to_plane`: XY|XZ|YZ, a construction or offset plane, or
    // {origin, normal}), kept as a plane so the bodies follow it when it moves; or a point and, for
    // a face, its normal.
    let plane = |s: &Session, key: &str| -> Result<Option<solidcraft_doc::PlaneRef>> {
        p.get(key).map(|_| super::features::plane_param(s, p.get(key), cmd)).transpose()
    };
    let (from_plane, to_plane) = (plane(s, "from_plane")?, plane(s, "to_plane")?);
    // Or a frame (#6257), found again when the feature is evaluated.
    let (from_frame, to_frame) = (align_frame(s, p, "from", &bodies)?, align_frame(s, p, "to", &bodies)?);
    // A target on the bodies that move would move away with them: it must be found the same
    // without them.
    if let Some(fr @ AlignFrame::Origin { .. }) = &to_frame {
        let st = s.model.state();
        let mut rest = (*st).clone();
        rest.bodies.retain(|b| !bodies.contains(&b.name));
        let same = |(a, n): (Vec3, Vec3), (b, m): (Vec3, Vec3)| a.dist(b) <= 1e-9 * (1.0 + a.len()) && n.dot(m) > 1.0 - 1e-12;
        if !matches!((fr.resolve(&s.doc, &st), fr.resolve(&s.doc, &rest)), (Ok(a), Ok(b)) if same(a, b)) {
            return Err(bad(cmd, "`to_frame` is on a body that moves: pick the target on another body or on a sketch"));
        }
    }
    // Worked out from the plane or the frame when the feature is evaluated.
    let held = |plane: &Option<solidcraft_doc::PlaneRef>, frame: &Option<AlignFrame>| plane.is_some() || frame.is_some();
    let world_pick = |s: &Session, pk: &str, fk: &str, held: bool| -> Result<(Vec3, Option<Vec3>)> {
        if held {
            if p.get(pk).is_some() || p.get(fk).is_some() {
                return Err(bad(cmd, format!("give `{pk}_plane` or `{pk}`/`{fk}`, not both")));
            }
            return Ok((Vec3::ZERO, None));
        }
        if let Some(fp) = p.get(fk).and_then(vec3) {
            let n = super::face::face_normal_world(s, fp).ok_or_else(|| bad(cmd, format!("`{fk}` must be on a planar face")))?;
            return Ok((fp, Some(n)));
        }
        match side_point(p.get(pk), pk, fk).map_err(|why| bad(cmd, why))? {
            SidePoint::Origin => Ok((Vec3::ZERO, None)),
            SidePoint::Snap { at, kind } => {
                snap_point(&*s.world_state()?, &kind, at).map(|q| (q, None)).ok_or_else(|| bad(cmd, format!("no {kind} to snap to near `{pk}`")))
            }
            SidePoint::Point(v) => Ok((v, None)),
        }
    };
    let pick = |s: &Session, pk: &str, fk: &str, held: bool| -> Result<(Vec3, Option<Vec3>)> {
        let (at, n) = world_pick(s, pk, fk, held)?;
        if held {
            return Ok((at, n));
        }
        Ok((solidcraft_doc::apply_point(&inverse, at), n.and_then(|n| solidcraft_doc::apply_vector(&inverse, n).normalized())))
    };
    let (from, from_normal) = pick(s, "from", "from_face", held(&from_plane, &from_frame))?;
    let (to, to_normal) = pick(s, "to", "to_face", held(&to_plane, &to_frame))?;
    // A turn about the target's direction after the move.
    let angle = expr(p, "angle");
    if let Some(a) = &angle {
        check_expr(s, a, Kind::Angle, cmd, "angle")?;
        if to_normal.is_none() && !held(&to_plane, &to_frame) {
            return Err(bad(cmd, "`angle` turns about the target's direction: give `to_frame`, `to_face` or `to_plane`"));
        }
    }
    // Faces turn only to meet another face, a plane or a frame; a plane's normal and a frame's z
    // are read when evaluated. A target face's normal is also the axis of an `angle`.
    let (from_normal, to_normal) = match (from_normal, to_normal) {
        (Some(a), Some(b)) => (Some(a), Some(b)),
        (a, b) if held(&from_plane, &from_frame) || held(&to_plane, &to_frame) || angle.is_some() => (a, b),
        _ => (None, None),
    };
    let axes = axes_param(p, cmd)?;
    if axes.is_some() && (held(&from_plane, &from_frame) || held(&to_plane, &to_frame) || from_normal.is_some() || to_normal.is_some()) {
        return Err(bad(
            cmd,
            "`axes` applies to point-to-point alignment: leave out `from_face`, `to_face`, `from_plane`, `to_plane`, `from_frame` and `to_frame`",
        ));
    }
    // The directions are stored in the same frame as `from`/`to` (the first moved body's frame).
    let axes = axes.map(|mask| {
        mask.into_iter()
            .zip([Vec3::X, Vec3::Y, Vec3::Z])
            .filter(|&(on, _)| on)
            .map(|(_, world)| Dir(solidcraft_doc::apply_vector(&inverse, world).normalized().unwrap_or(world)))
            .collect::<Vec<_>>()
    });
    let flip = bool_(p, "flip").unwrap_or(false);
    add_feature(
        s,
        p,
        FeatureKind::Align {
            bodies: bodies.into_iter().map(BodyName).collect(),
            from: Position(from),
            to: Position(to),
            from_normal: from_normal.map(Dir),
            to_normal: to_normal.map(Dir),
            flip,
            from_plane,
            to_plane,
            axes,
            from_frame,
            to_frame,
            angle,
        },
    )
}

/// The longest sketch point or curve id an Align frame takes.
pub(crate) const MAX_SKETCH_ID: usize = 256;

/// The point or curve of an Align `{sketch, point|curve}` frame: an id of 1 to
/// [`MAX_SKETCH_ID`] characters. `None` when the frame gives both, neither or an id that is not
/// one.
pub(crate) enum SketchFrameId {
    Point(String),
    Curve(String),
}

pub(crate) fn sketch_frame_id(v: &Value) -> Option<SketchFrameId> {
    let id = |k: &str| str_(v, k).filter(|n| !n.is_empty() && n.chars().count() <= MAX_SKETCH_ID).map(str::to_string);
    match (id("point"), id("curve")) {
        (Some(point), None) => Some(SketchFrameId::Point(point)),
        (None, Some(curve)) => Some(SketchFrameId::Curve(curve)),
        _ => None,
    }
}

/// An Align side's frame (#6257), `<key>_frame`: a joint origin as `joint.create` takes one, or a
/// sketch point or the centre of a sketch circle, arc or ellipse (`{sketch, point|curve}`), on the
/// moved bodies' component (all of them on one, placed once). It takes the place of `<key>`,
/// `<key>_face` and `<key>_plane`, and is checked to resolve now.
fn align_frame(s: &Session, p: &Value, key: &str, bodies: &[String]) -> Result<Option<AlignFrame>> {
    let cmd = "solid.align";
    let fk = format!("{key}_frame");
    let Some(v) = p.get(&fk).filter(|v| !v.is_null()) else { return Ok(None) };
    if let Some(other) = [key.to_string(), format!("{key}_face"), format!("{key}_plane")].into_iter().find(|k| p.get(k).is_some()) {
        return Err(bad(cmd, format!("give `{fk}` or `{other}`, not both")));
    }
    let st = s.model.state();
    let (frame, on) = match v.get("sketch") {
        Some(sk) => {
            let sketch = super::features::sketch_id(s, Some(sk), cmd, &fk)?;
            let frame = match sketch_frame_id(v) {
                Some(SketchFrameId::Point(point)) => AlignFrame::SketchPoint { sketch: SketchId(sketch), point },
                Some(SketchFrameId::Curve(curve)) => AlignFrame::SketchCurve { sketch: SketchId(sketch), curve },
                None => return Err(bad(cmd, format!("`{fk}`: a sketch frame takes `point` or `curve` (an id of 1 to {MAX_SKETCH_ID} characters)"))),
            };
            (frame, s.doc.feature(sketch).map_or(0, |f| f.component))
        }
        None => {
            // On its part, in the part's own coordinates: the same in every copy of a
            // sub-assembly the part is in, wherever it was picked.
            let picked = super::joints::origin_param(s, Some(v), cmd)?;
            let origin = solidcraft_doc::joints::JointOrigin::new(picked.part(), picked.snap);
            let on = super::joints::component_of(s, origin.occurrence);
            (AlignFrame::Origin { origin: Box::new(origin) }, on)
        }
    };
    // The move is worked out in that component's coordinates: every body must be in it, and a
    // component placed twice would not say which placement the picks were made in.
    if bodies.iter().any(|b| st.body(b).map_or(0, |b| s.doc.model_body_component(b)) != on) {
        return Err(bad(cmd, format!("not supported yet: `{fk}` on another component than the bodies")));
    }
    if on != 0 && s.doc.occurrences.iter().filter(|o| o.component == on).count() != 1 {
        return Err(bad(cmd, format!("not supported yet: `{fk}` on a component placed more than once")));
    }
    frame.resolve(&s.doc, &st).map_err(|e| bad(cmd, format!("`{fk}`: {e}")))?;
    Ok(Some(frame))
}

fn place_on_plane(s: &mut Session, p: &Value) -> Result<Value> {
    use solidcraft_doc::PlaneRef;
    let cmd = "solid.place_on_plane";
    let bodies = string_list(cmd, p, "bodies")?;
    if bodies.is_empty() {
        return Err(bad(cmd, "`bodies` must list bodies"));
    }
    {
        let st = s.model.state();
        if let Some(b) = bodies.iter().find(|b| st.body(b).is_none()) {
            return Err(bad(cmd, format!("no body `{b}`")));
        }
    }
    let offset = expr(p, "offset");
    if let Some(e) = &offset {
        check_expr(s, e, Kind::Length, cmd, "offset")?;
    }
    let each = bool_(p, "each").unwrap_or(false);
    // A planar face is attached exactly, as a sketch on it is, so the bodies follow it.
    let face = p.get("plane").filter(|v| v.get("face").is_some());
    let plane = match (face, p.get("plane")) {
        (Some(face), _) => {
            let plane = super::sketch::face::capture(s, face, cmd)?;
            super::sketch::face::capture_positions(s, &plane)?;
            plane
        }
        (None, None | Some(Value::Null)) => PlaneRef::Origin { name: "XY".into() },
        (None, v) => plane_param(s, v, cmd)?,
    };
    // The plane must be found the same without the bodies that move: on one of them it would
    // move away with them.
    {
        let st = s.model.state();
        let mut rest = (*st).clone();
        rest.bodies.retain(|b| !bodies.contains(&b.name));
        let (vals, _) = s.doc.param_values();
        let here = s.doc.resolve_plane_in(&vals, &plane, 0, s.active_component, Some(&st)).map_err(|e| bad(cmd, format!("`plane`: {e}")))?;
        let same = s
            .doc
            .resolve_plane_in(&vals, &plane, 0, s.active_component, Some(&rest))
            .is_ok_and(|q| q.origin.dist(here.origin) <= 1e-9 * (1.0 + here.origin.len()) && q.normal().dot(here.normal()) > 1.0 - 1e-12);
        let on_moved = super::sketch::face::anchor(&plane).is_some_and(|a| bodies.contains(&a.target.body));
        if !same || on_moved {
            return Err(bad(cmd, "the plane is on a body that moves: pick a face of another body, or a plane"));
        }
    }
    let kind = FeatureKind::PlaceOnPlane { bodies: bodies.into_iter().map(BodyName).collect(), plane, offset, each };
    // A captured face is in its body's frame already.
    if face.is_some() { super::features::add_feature_in(s, p, kind, false) } else { add_feature(s, p, kind) }
}

fn remove(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "solid.remove";
    let bodies = string_list(cmd, p, "bodies")?;
    if bodies.is_empty() {
        return Err(bad(cmd, "`bodies` must list bodies"));
    }
    let st = s.model.state();
    if let Some(b) = bodies.iter().find(|b| st.body(b).is_none()) {
        return Err(bad(cmd, format!("no body `{b}`")));
    }
    add_feature(s, p, FeatureKind::Remove { bodies: bodies.into_iter().map(BodyName).collect() })
}

#[cfg(test)]
#[path = "features_more_tests.rs"]
mod tests;

/// A snap point near `at` on the model: the nearest vertex, edge midpoint, planar face's centre
/// or circular edge's centre (the end of a cylindrical face) of the placed model `st`.
pub(crate) fn snap_point(st: &solidcraft_doc::ModelState, kind: &str, at: Vec3) -> Option<Vec3> {
    let near = |pts: Vec<Vec3>| pts.into_iter().min_by(|a, b| a.dist(at).total_cmp(&b.dist(at)));
    match kind {
        "vertex" => near(
            st.bodies
                .iter()
                .flat_map(|b| b.mesh().edges.iter().flat_map(|e| [e.first().copied(), e.last().copied()]).flatten().collect::<Vec<_>>())
                .collect(),
        ),
        "edge_mid" => {
            let d = |e: &solidcraft_doc::kernel::EdgeInfo| e.points.windows(2).map(|w| at.dist_to_segment(w[0], w[1])).fold(f64::MAX, f64::min);
            st.bodies
                .iter()
                .filter_map(|b| b.body.edges((b.body.size() * 2e-3).max(1e-3)).ok())
                .flatten()
                .min_by(|a, b| d(a).total_cmp(&d(b)))
                .map(|e| e.mid)
        }
        "face_center" => st.bodies.iter().find_map(|b| {
            let i = solidcraft_doc::appearance::face_index_at(b, at)?;
            b.body.faces((b.body.size() * 2e-3).max(1e-3)).ok()?.into_iter().find(|f| f.index == i).map(|f| f.centroid)
        }),
        "circle_center" => st.bodies.iter().find_map(|b| {
            let c = solidcraft_doc::kernel::cylinder_face_at(&b.body, at)?;
            let along = (at - c.axis_point).dot(c.axis);
            let end = if (along - c.start).abs() <= (c.end - along).abs() { c.start } else { c.end };
            Some(c.axis_point + c.axis * end)
        }),
        _ => None,
    }
}

fn boundary_fill(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "solid.boundary_fill";
    let tools = string_list(cmd, p, "tools")?;
    let st = s.model.state();
    if tools.is_empty() || tools.len() > 64 || tools.iter().any(|t| st.body(t).is_none()) {
        return Err(bad(cmd, "`tools` must list 1…64 bodies"));
    }
    let cells: Vec<Vec3> = p.get("cells").and_then(Value::as_array).map(|a| a.iter().filter_map(vec3).collect()).unwrap_or_default();
    if cells.is_empty() || cells.len() > 1000 {
        return Err(bad(cmd, "`cells` must list 1…1000 points [x, y, z], one inside each cell"));
    }
    let operation = super::features::operation(p, cmd)?;
    add_feature(
        s,
        p,
        FeatureKind::BoundaryFill {
            tools: tools.into_iter().map(BodyName).collect(),
            cells: cells.into_iter().map(Position).collect(),
            operation,
            remove_tools: bool_(p, "remove_tools").unwrap_or(false),
        },
    )
}
