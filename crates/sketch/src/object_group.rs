//! A group of curves the user (or an import) made one object: a drawing brought in from an SVG or
//! DXF, or curves grouped by hand. It names its curves; a curve of it moves, turns and scales with
//! the rest (see `sketch.transform`) until the group is taken apart.

use serde::{Deserialize, Serialize};

use crate::model::{Result, Sketch, SketchError};

/// Most curves one group holds (an imported drawing may have thousands).
pub const MAX_GROUP_CURVES: usize = crate::link::MAX_LINK_ENTITIES;

/// Curves that are one object.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObjectGroup {
    pub id: String,
    /// What the group shows as its name (the drawing's file name).
    pub name: String,
    /// The ids of its curves. A curve that is deleted leaves the group; a group with no curves
    /// is gone.
    pub curves: Vec<String>,
    /// How far (degrees) the group has been turned since it was made: its box's width runs this
    /// way, so a later stretch goes along the group's own width and height (0 when absent).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub angle: f64,
}

fn is_zero(a: &f64) -> bool {
    *a == 0.0
}

impl Sketch {
    /// Make `curves` (ids) one group. A curve belongs to one group at most.
    pub fn add_object_group(&mut self, name: &str, curves: &[String]) -> Result<String> {
        if curves.is_empty() {
            return Err(SketchError::Invalid("a group needs at least one curve".into()));
        }
        if curves.len() > MAX_GROUP_CURVES {
            return Err(SketchError::TooLarge);
        }
        let mut seen = std::collections::BTreeSet::new();
        for c in curves {
            if self.curve_index(c).is_none() {
                return Err(SketchError::Unknown(c.clone()));
            }
            if !seen.insert(c.as_str()) {
                return Err(SketchError::Invalid(format!("curve `{c}` is listed twice")));
            }
            if let Some(g) = self.object_group_of(c) {
                return Err(SketchError::Invalid(format!("curve `{c}` is already in group `{}`: take that group apart first", g.id)));
            }
        }
        let id = self.fresh("g")?;
        self.object_groups.push(ObjectGroup { id: id.clone(), name: name.trim().to_string(), curves: curves.to_vec(), angle: 0.0 });
        Ok(id)
    }

    /// Take group `id` apart. Its curves stay.
    pub fn remove_object_group(&mut self, id: &str) -> Result<()> {
        let before = self.object_groups.len();
        self.object_groups.retain(|g| g.id != id);
        if self.object_groups.len() == before {
            return Err(SketchError::Unknown(id.to_string()));
        }
        Ok(())
    }

    /// The group that holds curve `id`.
    pub fn object_group_of(&self, id: &str) -> Option<&ObjectGroup> {
        self.object_groups.iter().find(|g| g.curves.iter().any(|c| c == id))
    }

    /// The direction (degrees) of the width of the box around `curves` (indices): the turn of the
    /// groups that hold them when those are turned alike, else 0, the sketch's own axes.
    pub fn box_angle(&self, curves: &[usize]) -> f64 {
        let ids: std::collections::BTreeSet<&str> = curves.iter().filter_map(|&i| self.curves.get(i)).map(|c| c.id.as_str()).collect();
        let mut angles = self.object_groups.iter().filter(|g| g.curves.iter().any(|c| ids.contains(c.as_str()))).map(|g| g.angle);
        match angles.next() {
            Some(a) if angles.all(|b| (b - a).abs() < 1e-9) => a,
            _ => 0.0,
        }
    }

    /// The groups whose every curve is in `curves` (indices) have been turned by `degrees`.
    pub fn turn_object_groups(&mut self, curves: &[usize], degrees: f64) {
        let ids: std::collections::BTreeSet<String> = curves.iter().filter_map(|&i| self.curves.get(i)).map(|c| c.id.clone()).collect();
        for g in &mut self.object_groups {
            if g.curves.iter().all(|c| ids.contains(c)) {
                g.angle = (g.angle + degrees + 180.0).rem_euclid(360.0) - 180.0;
            }
        }
    }

    /// Curves deleted from the sketch leave their groups; a group left empty goes.
    pub(crate) fn prune_object_groups(&mut self, removed: &std::collections::BTreeSet<String>) {
        for g in &mut self.object_groups {
            g.curves.retain(|c| !removed.contains(c));
        }
        self.object_groups.retain(|g| !g.curves.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidcore_record::Vec2;

    fn two_lines() -> (Sketch, Vec<String>) {
        let mut sk = Sketch::new();
        let a = sk.add_line(Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), None, None, None).unwrap();
        let b = sk.add_line(Vec2::new(0.0, 5.0), Vec2::new(1.0, 5.0), None, None, None).unwrap();
        let ids = [a, b].iter().map(|&i| sk.curves[i].id.clone()).collect();
        (sk, ids)
    }

    #[test]
    fn a_curve_is_in_one_group_and_leaves_it_when_deleted() {
        let (mut sk, ids) = two_lines();
        let g = sk.add_object_group("crown.svg", &ids).unwrap();
        assert_eq!(sk.object_group_of(&ids[1]).unwrap().id, g);
        assert!(sk.add_object_group("again", &ids[..1]).is_err());
        assert!(sk.add_object_group("", &[]).is_err());
        assert!(sk.add_object_group("x", &["nope".to_string()]).is_err());
        sk.prune_object_groups(&std::iter::once(ids[0].clone()).collect());
        assert_eq!(sk.object_groups[0].curves, vec![ids[1].clone()]);
        sk.prune_object_groups(&std::iter::once(ids[1].clone()).collect());
        assert!(sk.object_groups.is_empty());
    }

    #[test]
    fn ungrouping_keeps_the_curves() {
        let (mut sk, ids) = two_lines();
        let g = sk.add_object_group("g", &ids).unwrap();
        sk.remove_object_group(&g).unwrap();
        assert!(sk.object_group_of(&ids[0]).is_none() && sk.curves.len() == 2);
        assert!(sk.remove_object_group(&g).is_err());
    }

    #[test]
    fn a_turned_group_gives_its_box_its_angle() {
        let (mut sk, ids) = two_lines();
        sk.add_object_group("g", &ids).unwrap();
        assert_eq!(sk.box_angle(&[0, 1]), 0.0);
        sk.turn_object_groups(&[0], 30.0);
        assert_eq!(sk.box_angle(&[0, 1]), 0.0, "half a group is not turned");
        sk.turn_object_groups(&[0, 1], 30.0);
        sk.turn_object_groups(&[0, 1], 170.0);
        assert!((sk.box_angle(&[0]) + 160.0).abs() < 1e-9, "{}", sk.box_angle(&[0]));
        assert_eq!(sk.box_angle(&[]), 0.0);
    }
}
