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
