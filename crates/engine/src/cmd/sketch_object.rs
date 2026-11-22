//! `sketch.group` and `sketch.ungroup`: curves made one object, and taken apart again.

use serde_json::{Value, json};
use solidcraft_sketch::Sketch;

use super::sketch_store;
use super::{CommandSpec, in_sketch};
use crate::params::{bad, str_, string_list};
use crate::schema::sketch_object::{GroupParams, GroupResult, UngroupParams};
use crate::schema::{TypedContract, output_schema_for};
use crate::{Result, Session};

const GROUP_CONTRACT: TypedContract<GroupParams> = TypedContract::new();
const UNGROUP_CONTRACT: TypedContract<UngroupParams> = TypedContract::new();

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("sketch.group", "Group", crate::checked!("sketch.group", group))
        .in_sketch_part()
        .icon("feature_group")
        .enabled(in_sketch)
        .contract(&GROUP_CONTRACT)
        .output(output_schema_for::<GroupResult>)
        .params("curves: [ids] (a curve is in one group at most); name?: text. The curves become one object: sketch.move and sketch.transform on one of them take them all. An imported SVG or DXF is a group already"),
    CommandSpec::new("sketch.ungroup", "Ungroup", crate::checked!("sketch.ungroup", ungroup))
        .in_sketch_part()
        .icon("explode")
        .enabled(in_sketch)
        .contract(&UNGROUP_CONTRACT)
        .output(output_schema_for::<GroupResult>)
        .params("group: the group's id, or entity: one of its curves. The curves stay and are separate again"),
];

fn reply(group: &str, curves: Vec<String>) -> Result<Value> {
    serde_json::to_value(GroupResult { group: group.to_owned(), curves, param_warnings: Vec::new() }).map_err(|e| bad("sketch.group", e.to_string()))
}

fn group(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "sketch.group";
    let curves = string_list(cmd, p, "curves")?;
    let name = str_(p, "name").map(str::trim).filter(|n| !n.is_empty()).unwrap_or("Group").to_string();
    let id = sketch_store::active(s).ok_or_else(|| bad(cmd, "no sketch is being edited"))?;
    sketch_store::update(s, id, |_, _, sk| {
        let g = sk.add_object_group(&name, &curves).map_err(|e| bad(cmd, e.to_string()))?;
        reply(&g, curves)
    })
}

fn ungroup(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "sketch.ungroup";
    let id = sketch_store::active(s).ok_or_else(|| bad(cmd, "no sketch is being edited"))?;
    sketch_store::update(s, id, |_, _, sk| {
        let g = match (str_(p, "group"), str_(p, "entity")) {
            (Some(g), _) => g.to_string(),
            (None, Some(e)) => sk.object_group_of(e).map(|g| g.id.clone()).ok_or_else(|| bad(cmd, format!("`{e}` is not in a group")))?,
            _ => return Err(bad(cmd, "give `group` or `entity`")),
        };
        let curves =
            sk.object_groups.iter().find(|x| x.id == g).map(|x| x.curves.clone()).ok_or_else(|| bad(cmd, format!("`{g}` is not a group")))?;
        sk.remove_object_group(&g).map_err(|e| bad(cmd, e.to_string()))?;
        reply(&g, curves)
    })
}

/// `ids` with every curve of each group one of them is in. Picking one curve of a group moves,
/// turns or scales the whole object.
pub(super) fn group_ids(sk: &Sketch, ids: &[String]) -> Vec<String> {
    let mut all: Vec<String> = Vec::with_capacity(ids.len());
    for id in ids {
        let members = sk.object_group_of(id).map_or_else(|| std::slice::from_ref(id), |g| g.curves.as_slice());
        for c in members {
            if !all.contains(c) {
                all.push(c.clone());
            }
        }
    }
    all
}

/// `p` with its `entities` widened by [`group_ids`].
pub(super) fn with_group_members(sk: &Sketch, p: &Value, cmd: &str) -> Result<Value> {
    // `out["entities"] = …` below panics on anything but an object.
    if !(p.is_object() || p.is_null()) {
        return Err(bad(cmd, "params must be an object"));
    }
    let mut out = if p.is_null() { json!({}) } else { p.clone() };
    out["entities"] = json!(group_ids(sk, &string_list(cmd, p, "entities")?));
    Ok(out)
}
