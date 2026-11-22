//! A group of curves the user (or an import) made one object: a drawing brought in from an SVG or
//! DXF, or curves grouped by hand. It names its curves; a curve of it moves, turns and scales with
//! the rest (see `sketch.transform`) until the group is taken apart.

use serde::{Deserialize, Serialize};

use solidcore_record::Vec2;
use solidcraft_geom::Vec2Geom;

use crate::model::{Result, Sketch, SketchError};

/// Most curves one group holds (an imported drawing may have thousands).
pub const MAX_GROUP_CURVES: usize = crate::link::MAX_LINK_ENTITIES;

/// What may change in a group's curves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupLock {
    /// Anything: the whole object moves, turns and stretches, and its nodes are edited.
    #[default]
    Free,
    /// The whole object moves, turns and scales; no node, curve or end is changed on its own.
    Protect,
    /// Nothing changes until it is unlocked.
    Total,
}

impl GroupLock {
    pub fn name(self) -> &'static str {
        match self {
            GroupLock::Free => "free",
            GroupLock::Protect => "protect geometry",
            GroupLock::Total => "total lock",
        }
    }
}

fn is_free(l: &GroupLock) -> bool {
    *l == GroupLock::Free
}

/// A locked group as an edit found it: its curves and where each of their points was.
#[derive(Clone, Debug, Default)]
pub struct LockSnapshot(Vec<LockedGroup>);

#[derive(Clone, Debug)]
struct LockedGroup {
    name: String,
    lock: GroupLock,
    curves: Vec<String>,
    points: Vec<(String, Vec2)>,
}

/// Curves that are one object.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObjectGroup {
    pub id: String,
    /// What the group shows as its name (the drawing's file name).
    pub name: String,
    /// The ids of its curves. A curve that is deleted leaves the group; a group with no curves
    /// is gone.
    pub curves: Vec<String>,
    /// What an edit may change in it. A design saved before locks has none: free.
    #[serde(default, skip_serializing_if = "is_free")]
    pub lock: GroupLock,
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
        self.object_groups.push(ObjectGroup { id: id.clone(), name: name.trim().to_string(), curves: curves.to_vec(), lock: GroupLock::Free });
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

    /// Set group `id`'s lock.
    pub fn set_group_lock(&mut self, id: &str, lock: GroupLock) -> Result<()> {
        let g = self.object_groups.iter_mut().find(|g| g.id == id).ok_or_else(|| SketchError::Unknown(id.to_string()))?;
        g.lock = lock;
        Ok(())
    }

    /// The locked groups as they are now, for [`Sketch::check_locks`] after an edit.
    pub fn lock_snapshot(&self) -> LockSnapshot {
        LockSnapshot(
            self.object_groups
                .iter()
                .filter(|g| g.lock != GroupLock::Free)
                .map(|g| {
                    let mut ids = std::collections::BTreeSet::new();
                    for c in g.curves.iter().filter_map(|c| self.curve_index(c)).filter_map(|i| self.curves.get(i)) {
                        ids.extend(c.kind.point_ids());
                    }
                    let points = ids.into_iter().filter_map(|i| self.points.get(i)).map(|p| (p.id.clone(), p.pos)).collect();
                    LockedGroup { name: g.name.clone(), lock: g.lock, curves: g.curves.clone(), points }
                })
                .collect(),
        )
    }

    /// Whether an edit left every locked group in `before` as its lock allows: a curve or point
    /// gone, or any point moved under Total, or the points no longer one affine copy of the old ones
    /// under Protect (a move, turn or scale of the whole object keeps them one), is refused.
    pub fn check_locks(&self, before: &LockSnapshot) -> Result<()> {
        for g in &before.0 {
            let refuse = |what: &str| {
                Err(SketchError::Invalid(format!("group `{}` is locked ({}): {what}. Unlock it with sketch.group_lock first", g.name, g.lock.name())))
            };
            if g.curves.iter().any(|c| self.curve_index(c).is_none()) {
                return refuse("its curves cannot be removed");
            }
            let mut now = Vec::with_capacity(g.points.len());
            for (id, _) in &g.points {
                match self.resolve_point(id).and_then(|i| self.points.get(i)) {
                    Some(p) => now.push(p.pos),
                    None => return refuse("its points cannot be removed"),
                }
            }
            let was: Vec<Vec2> = g.points.iter().map(|(_, p)| *p).collect();
            let reach = was.iter().fold(1.0_f64, |m, p| m.max(p.x.abs()).max(p.y.abs()));
            let tol = 1e-6 * reach;
            match g.lock {
                GroupLock::Free => {}
                GroupLock::Total => {
                    if was.iter().zip(&now).any(|(a, b)| a.dist(*b) > tol) {
                        return refuse("nothing can be moved, turned or scaled");
                    }
                }
                GroupLock::Protect => {
                    if !is_affine_copy(&was, &now, tol) {
                        return refuse("it can only be moved, turned or scaled as a whole, not edited point by point");
                    }
                }
            }
        }
        Ok(())
    }

    /// Curves deleted from the sketch leave their groups; a group left empty goes.
    pub(crate) fn prune_object_groups(&mut self, removed: &std::collections::BTreeSet<String>) {
        for g in &mut self.object_groups {
            g.curves.retain(|c| !removed.contains(c));
        }
        self.object_groups.retain(|g| !g.curves.is_empty());
    }
}

/// Whether `now` is `was` under one affine map (a move, turn, scale or stretch of the whole).
/// shortcut: points all on one line pass, since no map is fixed by them; check with the ends.
fn is_affine_copy(was: &[Vec2], now: &[Vec2], tol: f64) -> bool {
    let Some(&p0) = was.first() else { return true };
    let far = |from: Vec2| was.iter().enumerate().fold((0, 0.0), |m, (i, p)| if p.dist(from) > m.1 { (i, p.dist(from)) } else { m });
    let (i1, d1) = far(p0);
    if d1 <= 1e-12 {
        return was.iter().zip(now).all(|(a, b)| (*b - now[0] - (*a - p0)).len() <= tol);
    }
    let e1 = was[i1] - p0;
    let (i2, area) = was.iter().enumerate().fold((0, 0.0), |m, (i, p)| {
        let c = e1.cross(*p - p0).abs();
        if c > m.1 { (i, c) } else { m }
    });
    if area <= 1e-9 * d1 * d1 {
        return true;
    }
    let (e2, f1, f2) = (was[i2] - p0, now[i1] - now[0], now[i2] - now[0]);
    // The map sending e1, e2 to f1, f2: M = [f1 f2] * [e1 e2]^-1.
    let det = e1.x * e2.y - e1.y * e2.x;
    let m = |v: Vec2| {
        let a = (v.x * e2.y - v.y * e2.x) / det;
        let b = (e1.x * v.y - e1.y * v.x) / det;
        f1 * a + f2 * b
    };
    was.iter().zip(now).all(|(p, q)| (now[0] + m(*p - p0) - *q).len() <= tol)
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
}
