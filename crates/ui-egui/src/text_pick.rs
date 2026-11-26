//! Picking a text as one object. A click on a text's outline in the sketch, or on a body made from
//! a text, takes the whole text; a click on it again takes that letter (an accent and the dot of
//! an i with it); once more, the curve or the face alone, as any other click. Shift or Ctrl and a
//! click adds or takes away one letter. The groups and their letters come from `sketch.groups`.

use std::cell::RefCell;
use std::hash::{Hash, Hasher};

use serde_json::json;
use solidcraft_engine::Sel;

use crate::SolveApp;
use crate::selection::same_item;

type Text = Option<(Vec<Sel>, Vec<Sel>)>;

thread_local! {
    /// The last item looked up and its text: the hover asks for the item under the pointer every
    /// frame, and laying a text out again each time would be wasted work.
    static LAST: RefCell<Option<(u64, Text)>> = const { RefCell::new(None) };
}

/// The whole text and the letter that `sel` is part of, as selection items: curves of the sketch
/// being edited, or the bodies made from them when no sketch is.
fn text_of(app: &SolveApp, sel: &Sel) -> Text {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    // A face or edge stands for its body: moving over the faces of one letter looks it up once.
    let item = match sel {
        Sel::Face { body, .. } | Sel::Edge { body, .. } => format!("body {body}"),
        _ => format!("{sel:?}"),
    };
    (app.session.uid, app.session.revision, app.session.active_sketch, item).hash(&mut h);
    let key = h.finish();
    if let Some(found) = LAST.with_borrow(|last| last.as_ref().filter(|(k, _)| *k == key).map(|(_, found)| found.clone())) {
        return found;
    }
    let found = look_up(app, sel);
    LAST.with_borrow_mut(|last| *last = Some((key, found.clone())));
    found
}

fn look_up(app: &SolveApp, sel: &Sel) -> Text {
    match sel {
        Sel::SketchCurve { id, .. } => {
            let state = app.session.model.state();
            let sk = &state.sketch(app.session.active_sketch?)?.sketch;
            let (group, member) = sk.curve_group_of(id)?;
            let curves = |ids: &[String]| ids.iter().map(|id| Sel::SketchCurve { id: id.clone(), sketch: None }).collect::<Vec<_>>();
            let letter = member.and_then(|m| group.members.get(m)).map(|m| curves(&m.curves)).unwrap_or_default();
            Some((curves(&group.curves), letter))
        }
        Sel::Body { name: body } | Sel::Face { body, .. } | Sel::Edge { body, .. } if app.session.active_sketch.is_none() => {
            let (whole, letter) = solidcraft_engine::cmd::sketch_groups::text_bodies(&app.session, body)?;
            let bodies = |names: Vec<String>| names.into_iter().map(|name| Sel::Body { name }).collect::<Vec<_>>();
            // A text made into one body is not picked as a group.
            (whole.len() > 1).then(|| (bodies(whole), bodies(letter)))
        }
        _ => None,
    }
}

/// What a click on `sel` (`add`: with Shift or Ctrl) takes, and the selection it leaves. None when
/// the click is an ordinary one (not on a text, or the third click, which takes the curve or face
/// alone).
fn step(app: &SolveApp, sel: &Sel, add: bool) -> Option<(Vec<Sel>, Vec<Sel>)> {
    let (whole, letter) = text_of(app, sel)?;
    let selected = &app.session.selection;
    let has = |x: &Sel| selected.iter().any(|y| same_item(x, y));
    let exactly = |items: &[Sel]| items.len() == selected.len() && items.iter().all(has);
    // The letter is a step of its own only when it is less than the whole text.
    let letter = if letter.len() < whole.len() { letter } else { Vec::new() };
    if add {
        let part = if letter.is_empty() { whole } else { letter };
        let items = if part.iter().all(has) {
            selected.iter().filter(|y| !part.iter().any(|x| same_item(x, y))).cloned().collect()
        } else {
            selected.iter().cloned().chain(part.iter().filter(|x| !has(x)).cloned()).collect()
        };
        Some((part, items))
    } else if exactly(&whole) && !letter.is_empty() {
        Some((letter.clone(), letter))
    } else if exactly(&whole) || exactly(&letter) {
        None
    } else {
        Some((whole.clone(), whole))
    }
}

/// A click on `sel` (`add`: with Shift or Ctrl). True when it picked a text or a letter; false
/// when the click is an ordinary one.
pub(crate) fn click(app: &mut SolveApp, sel: &Sel, add: bool) -> bool {
    let Some((_, items)) = step(app, sel, add) else { return false };
    let _ = app.run("select.set", json!({ "items": items }));
    true
}

/// What the pointer resting on `sel` lights: what a click there would take (the whole text, then
/// the letter), so the hover never shows one curve or body where the click takes a whole word.
/// None when the click would take `sel` alone.
// shortcut: shows what a plain click takes, also while Shift or Ctrl is held (where the click adds
// or takes away the letter); pass the held keys here if that is ever asked for.
pub(crate) fn hover(app: &SolveApp, sel: &Sel) -> Option<Vec<Sel>> {
    if app.tool.is_some() || app.dialog.as_ref().is_some_and(|d| d.wants_picks()) {
        return None;
    }
    step(app, sel, false).map(|(taken, _)| taken)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::Harness;

    fn selected_curves(app: &SolveApp) -> usize {
        app.session.selection.iter().filter(|s| matches!(s, Sel::SketchCurve { .. })).count()
    }

    /// MARTINA: a click takes the text, a second its letter, a third the curve alone; Shift adds
    /// and takes away letters. After an extrude the same goes for its seven bodies. Before each
    /// click the hover lights what it will take.
    #[test]
    fn a_click_takes_the_text_then_the_letter_then_the_curve_and_the_hover_shows_each_first() {
        let mut h = Harness::new();
        h.app.run("sketch.create", json!({"plane": "XY", "name": "S"})).unwrap();
        h.app.run("sketch.text", json!({"text": "MARTINA", "at": [0, 0], "height": 10})).unwrap();
        let groups = h.app.run("sketch.groups", json!({})).unwrap();
        let g = &groups["groups"][0];
        let (all, r): (usize, Vec<String>) = (
            g["curves"].as_array().unwrap().len(),
            g["members"][2]["curves"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect(),
        );
        let a_curve = |id: &str| Sel::SketchCurve { id: id.into(), sketch: None };
        // The pointer resting on the R lights what the click takes, step by step.
        let lit = |app: &SolveApp, sel: &Sel| hover(app, sel).map(|items| items.len());
        assert_eq!(lit(&h.app, &a_curve(&r[0])), Some(all));
        assert!(click(&mut h.app, &a_curve(&r[0]), false));
        assert_eq!(selected_curves(&h.app), all);
        assert_eq!(lit(&h.app, &a_curve(&r[0])), Some(r.len()));
        assert!(click(&mut h.app, &a_curve(&r[0]), false));
        assert_eq!(selected_curves(&h.app), r.len());
        // The third click is an ordinary one: the caller takes the curve alone, and so the hover.
        assert_eq!(lit(&h.app, &a_curve(&r[0])), None);
        assert!(!click(&mut h.app, &a_curve(&r[0]), false));
        // Shift adds the M to the R, and takes it away again.
        let m = g["members"][0]["curves"][0].as_str().unwrap();
        let m_len = g["members"][0]["curves"].as_array().unwrap().len();
        assert!(click(&mut h.app, &a_curve(m), true));
        assert_eq!(selected_curves(&h.app), r.len() + m_len);
        assert!(click(&mut h.app, &a_curve(m), true));
        assert_eq!(selected_curves(&h.app), r.len());
        // A curve that is not a text's is an ordinary click.
        let line = h.app.run("sketch.line", json!({"points": [[0, -20], [10, -20]]})).unwrap()["curves"][0].as_str().unwrap().to_string();
        assert!(!click(&mut h.app, &a_curve(&line), false));

        h.app.run("sketch.finish", json!({})).unwrap();
        h.app.run("solid.extrude", json!({"sketch": "S", "distance": "3 mm"})).unwrap();
        let r_body =
            h.app.run("sketch.groups", json!({"sketch": "S"})).unwrap()["groups"][0]["members"][2]["bodies"][0].as_str().unwrap().to_string();
        let body = Sel::Body { name: r_body.clone() };
        let bodies = |app: &SolveApp| app.session.selection.iter().filter(|s| matches!(s, Sel::Body { .. })).count();
        assert_eq!(lit(&h.app, &body), Some(7));
        assert!(click(&mut h.app, &body, false));
        assert_eq!(bodies(&h.app), 7);
        assert_eq!(hover(&h.app, &body), Some(vec![body.clone()]));
        assert!(click(&mut h.app, &body, false));
        assert_eq!(h.app.session.selection, vec![Sel::Body { name: r_body }]);
        assert_eq!(lit(&h.app, &body), None);
        assert!(!click(&mut h.app, &body, false));
    }
}
