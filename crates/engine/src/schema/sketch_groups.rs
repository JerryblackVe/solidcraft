//! Contract and result type for `sketch.groups` (`cmd/sketch_groups.rs`): the curves of a sketch
//! that are one object (a text) and the members inside each (its letters), with the bodies made
//! from them.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::sketch_ends::SketchTarget;

/// Which sketch, and optionally the curve whose group is wanted.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct SketchGroupsParams {
    #[serde(default)]
    pub sketch: Option<SketchTarget>,
    /// A curve id: only the group it belongs to, with `member` set to its member (none when the
    /// curve is in no group).
    #[serde(default)]
    pub curve: Option<String>,
    /// A body name, given alone: the group and member whose curves made it, from whichever
    /// sketch (none when no group made it).
    #[serde(default)]
    pub body: Option<String>,
}

/// What `sketch.groups` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SketchGroups {
    pub groups: Vec<GroupReport>,
    /// With `curve` or `body`: the index in `members` of the member it belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<usize>,
    /// In the warn parameter-check mode: what the call's parameters got wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}

/// Curves of a sketch picked and moved as one object.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GroupReport {
    /// The sketch's id.
    pub sketch: u64,
    /// The id of what owns the curves (a text's link id, as `sketch.edit_text` takes it).
    pub id: String,
    /// What made the group: `text`.
    pub kind: String,
    /// Its name as shown: a text's words.
    pub label: String,
    /// Every curve of the group.
    pub curves: Vec<String>,
    /// The bodies made from its curves (extruded, revolved, swept…), by name: the faces a
    /// feature built from this sketch named after these curves, wherever the body has moved since.
    pub bodies: Vec<String>,
    /// Its members in order (a text's letters; a space draws nothing and is left out). Empty when
    /// they cannot be told apart: the group is then picked whole.
    pub members: Vec<MemberReport>,
}

/// A member of a group: a text's letter, with its outlines (an accent and the dot of an i
/// included) and the bodies made from them.
#[derive(Debug, Serialize, JsonSchema)]
pub struct MemberReport {
    /// Its place in `members`.
    pub index: usize,
    /// The characters it sets.
    pub label: String,
    pub curves: Vec<String>,
    pub bodies: Vec<String>,
}

#[cfg(test)]
#[path = "sketch_groups_tests.rs"]
mod tests;
