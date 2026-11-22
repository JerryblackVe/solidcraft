//! Contracts and results of `sketch.group` and `sketch.ungroup` (`cmd/sketch_object.rs`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Make curves of the sketch being edited one object: picking one takes them all, and moving,
/// turning or scaling one (`sketch.move`, `sketch.transform`) takes them all. A curve is in one
/// group at most. One undo step.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct GroupParams {
    /// The curves' ids (at least one).
    pub curves: Vec<String>,
    /// The group's name. Default: "Group".
    #[serde(default)]
    pub name: Option<String>,
}

/// Take a group apart: its curves stay, each on its own again. Name the group, or one of its
/// curves. One undo step.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct UngroupParams {
    /// The group's id, as `sketch.group` or an import returned it.
    #[serde(default)]
    pub group: Option<String>,
    /// One of the group's curves, by id.
    #[serde(default)]
    pub entity: Option<String>,
}

/// What `sketch.group` and `sketch.ungroup` return.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GroupResult {
    /// The group's id.
    pub group: String,
    /// Its curves.
    pub curves: Vec<String>,
    /// In the warn parameter-check mode: what the call's parameters got wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}

/// Set what may change in a group: `free`, `protect` (the whole object moves, turns and scales but
/// its points are not edited one by one) or `total` (nothing changes). A locked group refuses the
/// edit that would break its lock, and nothing changes. Not an object lock on the design: the
/// other curves of the sketch are free. One undo step.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct GroupLockParams {
    /// The group's id, as `sketch.group` or an import returned it.
    #[serde(default)]
    pub group: Option<String>,
    /// One of the group's curves, by id.
    #[serde(default)]
    pub entity: Option<String>,
    /// `free`, `protect` or `total`.
    pub lock: String,
}

/// Set where the Transform box scales and turns a group about, when it is not the middle of the
/// group's box. The pivot is saved with the group and moves, turns and scales with it. One undo
/// step.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct GroupPivotParams {
    /// The group's id, as `sketch.group` or an import returned it.
    #[serde(default)]
    pub group: Option<String>,
    /// One of the group's curves, by id.
    #[serde(default)]
    pub entity: Option<String>,
    /// The pivot `[x, y]` on the sketch in mm; left out or null, the middle of the group's box.
    #[serde(default)]
    pub pivot: Option<[f64; 2]>,
}

/// Switch between object mode (a group is picked and edited as one object, its points are neither
/// drawn nor picked) and nodes mode (the points of grouped curves show and can be picked). A view
/// setting of the window: not saved with the design, not an undo step.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct NodesModeParams {
    /// True for nodes mode, false for object mode; left out, the mode switches.
    #[serde(default)]
    pub nodes: Option<bool>,
}

/// What `sketch.nodes_mode` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct NodesModeResult {
    /// True when nodes mode is on now, false in object mode.
    pub nodes: bool,
    /// In the warn parameter-check mode: what the call's parameters got wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}
