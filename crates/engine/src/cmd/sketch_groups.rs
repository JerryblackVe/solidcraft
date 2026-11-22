//! The curves of a sketch that are one object (a text) and the members inside each (its
//! letters), with the bodies made from them: what picking a whole text or one letter selects, in
//! the sketch and after an extrude.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use solidcraft_doc::ModelState;
use solidcraft_sketch::{CurveGroup, GroupKind, Sketch};

use super::CommandSpec;
use super::sketch_store;
use crate::params::bad;
use crate::schema::sketch_groups::{GroupReport, MemberReport, SketchGroups, SketchGroupsParams};
use crate::schema::{TypedContract, output_schema_for};
use crate::{EngineError, Result, Session};

const CMD: &str = "sketch.groups";
const CONTRACT: TypedContract<SketchGroupsParams> = TypedContract::new();

pub static COMMANDS: &[CommandSpec] = &[CommandSpec::new(CMD, "Sketch Groups", crate::checked!("sketch.groups", groups_cmd))
    .icon("sketch_text")
    .noundo()
    .contract(&CONTRACT)
    .output(output_schema_for::<SketchGroups>)
    .params(
        "sketch?: id|name (default active); curve?: a curve id (only its group, and `member`: its member's index); or body?: a body name (no sketch needed: the group and member whose curves made it, from any sketch). Returns groups: [{sketch, id, kind: text, label, curves, bodies, members: [{index, label, curves, bodies}]}]: the curves picked as one object (a text, its link id as id) and the members inside it (its letters, an accent or the dot of an i with its letter; a space is left out), each with the bodies made from its curves (the faces a feature on that sketch named after them, wherever the body has moved since). Select the whole or a member with select.set (sketch_curve items, or body items for its bodies)",
    )];

/// The face name `F<feature>:side:<curve>` (with any `@…` copy suffix) as its feature and curve.
fn swept_curve(name: &str) -> Option<(u64, &str)> {
    let (feature, role) = name.strip_prefix('F')?.split_once(':')?;
    let curve = role.strip_prefix("side:")?;
    Some((feature.parse().ok()?, curve.split('@').next()?))
}

/// A text argument (`curve`, `body`): absent, or a string.
fn text_arg<'a>(p: &'a Value, key: &str) -> Result<Option<&'a str>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(c)) => Ok(Some(c.as_str())),
        Some(_) => Err(bad(CMD, format!("`{key}` must be text"))),
    }
}

/// The groups of sketch `sid`'s evaluated `sk`, as reported, with the bodies made from them.
fn reports(s: &Session, st: &ModelState, sid: u64, sk: &Sketch, groups: &[CurveGroup]) -> Vec<GroupReport> {
    // The bodies whose faces were swept from each curve of this sketch.
    let features: BTreeSet<u64> = s.doc.features.iter().filter(|f| f.kind.input_sketches().contains(&sid)).map(|f| f.id).collect();
    let mut bodies_of: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    if !features.is_empty() {
        for b in &st.bodies {
            let names = solidcraft_doc::naming::face_names(b);
            for (feature, curve) in names.iter().filter_map(|n| swept_curve(n)) {
                if features.contains(&feature)
                    && let Some(c) = sk.curve_index(curve).and_then(|i| sk.curves.get(i))
                {
                    bodies_of.entry(c.id.as_str()).or_default().insert(b.name.as_str());
                }
            }
        }
    }
    let bodies = |curves: &[String]| -> Vec<String> {
        let set: BTreeSet<&str> = curves.iter().filter_map(|c| bodies_of.get(c.as_str())).flatten().copied().collect();
        set.into_iter().map(String::from).collect()
    };
    groups
        .iter()
        .map(|g| GroupReport {
            sketch: sid,
            id: g.id.clone(),
            kind: match g.kind {
                GroupKind::Text => "text".into(),
                GroupKind::Object => "object".into(),
            },
            label: g.label.clone(),
            curves: g.curves.clone(),
            bodies: bodies(&g.curves),
            members: g
                .members
                .iter()
                .enumerate()
                .map(|(index, m)| MemberReport { index, label: m.label.clone(), curves: m.curves.clone(), bodies: bodies(&m.curves) })
                .collect(),
        })
        .collect()
}

fn reply(groups: Vec<GroupReport>, member: Option<usize>) -> Result<Value> {
    serde_json::to_value(SketchGroups { groups, member, param_warnings: Vec::new() }).map_err(|e| bad(CMD, e.to_string()))
}

/// The group and member whose curves made body `name`: the first curve its faces were swept from
/// that is in a group of the sketch the sweeping feature was built on. None for a body no group
/// made; an error for a body that is not there.
fn body_group(s: &Session, name: &str) -> Result<Option<(GroupReport, Option<usize>)>> {
    let st = s.model.state();
    let b = st.body(name).ok_or_else(|| bad(CMD, format!("no body `{name}`")))?;
    let names = solidcraft_doc::naming::face_names(b);
    for (feature, curve) in names.iter().filter_map(|n| swept_curve(n)) {
        let Some(f) = s.doc.feature(feature) else { continue };
        for sid in f.kind.input_sketches() {
            let Some(sk) = st.sketch(sid).map(|ss| &ss.sketch) else { continue };
            if let Some((group, member)) = sk.curve_group_of(curve) {
                return Ok(reports(s, &st, sid, sk, &[group]).pop().map(|g| (g, member)));
            }
        }
    }
    Ok(None)
}

/// The bodies made from the same text as body `name`, and those made from its letter: what
/// picking the whole text or that letter selects in the model. None when no text made it.
pub fn text_bodies(s: &Session, name: &str) -> Option<(Vec<String>, Vec<String>)> {
    let (group, member) = body_group(s, name).ok()??;
    let letter = member.and_then(|m| group.members.get(m)).map(|m| m.bodies.clone()).unwrap_or_default();
    Some((group.bodies, letter))
}

fn groups_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    let (curve, body) = (text_arg(p, "curve")?, text_arg(p, "body")?);
    if let Some(body) = body {
        if curve.is_some() || p.get("sketch").is_some_and(|v| !v.is_null()) {
            return Err(bad(CMD, "`body` finds its sketch: give it alone"));
        }
        return match body_group(s, body)? {
            Some((group, member)) => reply(vec![group], member),
            None => reply(Vec::new(), None),
        };
    }
    let sid = sketch_store::target(s, p, CMD)?;
    let st = s.model.state();
    let sk = &st.sketch(sid).ok_or_else(|| EngineError::Other("the sketch has not been evaluated".into()))?.sketch;
    let (groups, member) = match curve {
        None => (sk.curve_groups(), None),
        Some(curve) => {
            if sk.curve_index(curve).is_none() {
                return Err(bad(CMD, format!("no curve `{curve}` in this sketch")));
            }
            match sk.curve_group_of(curve) {
                Some((group, member)) => (vec![group], member),
                None => (Vec::new(), None),
            }
        }
    };
    reply(reports(s, &st, sid, sk, &groups), member)
}

#[cfg(test)]
#[path = "sketch_groups_tests.rs"]
mod tests;
