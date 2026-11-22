//! Groups of a sketch's curves that are picked and moved as one object, and the members inside
//! each (a text and its letters). Worked out from what the sketch already keeps (a text's link
//! and its source), so nothing new is saved in a design.

use crate::link::{LinkKind, LinkSource};
use crate::model::Sketch;

/// What made a group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupKind {
    /// A text: its members are its letters.
    Text,
}

/// A member of a group (a letter of a text) and its curves.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupMember {
    /// The characters it sets.
    pub label: String,
    pub curves: Vec<String>,
}

/// Curves of a sketch that are one object.
#[derive(Clone, Debug, PartialEq)]
pub struct CurveGroup {
    /// The id of what owns the curves (a text's link id).
    pub id: String,
    pub kind: GroupKind,
    /// What the group shows as its name (a text's words).
    pub label: String,
    /// Every curve of the group, in the sketch's order.
    pub curves: Vec<String>,
    /// Its members in order. Empty when they cannot be told apart (a text whose font cannot be
    /// read here): the group is then picked whole only.
    pub members: Vec<GroupMember>,
}

impl Sketch {
    /// The groups of this sketch's curves: each text, with its letters.
    pub fn curve_groups(&self) -> Vec<CurveGroup> {
        self.links.iter().filter_map(|l| self.curve_group(&l.id)).collect()
    }

    /// The group owned by link `link`: a text that is not lost and has curves.
    fn curve_group(&self, link: &str) -> Option<CurveGroup> {
        let l = self.link(link).filter(|l| l.kind == LinkKind::Text && !l.lost)?;
        let LinkSource::Text { text, .. } = &l.source else { return None };
        let curves: Vec<String> = self.link_curves(&l.id).iter().filter_map(|&i| self.curves.get(i)).map(|c| c.id.clone()).collect();
        if curves.is_empty() {
            return None;
        }
        let members = self.text_letters(&l.id).unwrap_or_default();
        Some(CurveGroup { id: l.id.clone(), kind: GroupKind::Text, label: text.clone(), curves, members })
    }

    /// The group curve `id` belongs to and, when the group's members are known, the index of
    /// its member. Only that group is worked out.
    pub fn curve_group_of(&self, id: &str) -> Option<(CurveGroup, Option<usize>)> {
        let link = self.curve_index(id).and_then(|i| self.curves.get(i))?.link.clone()?;
        let group = self.curve_group(&link)?;
        let member = group.members.iter().position(|m| m.curves.iter().any(|c| c == id));
        Some((group, member))
    }
}
