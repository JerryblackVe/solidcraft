use crate::scenario::Harness;
use serde_json::json;

#[test]
fn repeated_first_point_is_neutral_and_drawing_can_continue() {
    for (command, rest) in [
        ("sketch.rectangle.two_point", vec![[10, 5, 0]]),
        ("sketch.rectangle.center", vec![[10, 5, 0]]),
        ("sketch.rectangle.three_point", vec![[10, 0, 0], [10, 5, 0]]),
        ("sketch.circle.center", vec![[5, 0, 0]]),
        ("sketch.circle.three_point", vec![[10, 0, 0], [5, 5, 0]]),
        ("sketch.line", vec![[10, 5, 0]]),
        ("sketch.polygon.inscribed", vec![[10, 5, 0]]),
        ("sketch.star", vec![[10, 5, 0]]),
        ("sketch.slot.center_to_center", vec![[10, 0, 0], [10, 3, 0]]),
    ] {
        let mut h = Harness::new();
        h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
        h.app.cam.distance = 50.0;
        h.step(&json!({"start":command})).unwrap();
        let before = h.app.session.doc.to_json();
        let undo = h.app.session.undo.len();
        let p = h.call("ui.at", json!({"sketch":[0,0]}))["result"].clone();
        h.call("ui.click", json!({"x":p[0],"y":p[1],"double":true}));
        assert!(!h.app.status.as_ref().is_some_and(|s| s.2), "{command}: {:?}", h.app.status);
        assert_eq!(h.app.tool.as_ref().unwrap().pts.len(), 1, "{command}");
        assert_eq!(h.app.session.doc.to_json(), before);
        assert_eq!(h.app.session.undo.len(), undo);
        for point in rest {
            h.step(&json!({"move":{"world":point}})).unwrap();
            if command == "sketch.slot.center_to_center" {
                // The live width editor is placed at the cursor; Enter accepts it.
                h.step(&json!({"key":"Enter"})).unwrap();
            } else {
                h.step(&json!({"click":{"world":point}})).unwrap();
            }
        }
        assert!(!h.app.status.as_ref().is_some_and(|s| s.2), "{command}: {:?}", h.app.status);
        let state = h.app.session.world_state().unwrap();
        assert!(!state.sketches[0].sketch.curves.is_empty(), "{command}: {:?}; {:?}", h.app.tool, h.app.status);
        assert_eq!(h.app.session.undo.len(), undo + 1, "{command}");
        h.app.run("edit.undo", json!({})).unwrap();
        assert_eq!(h.app.session.doc.to_json(), before, "{command}");
    }
}

/// The square chord on a rectangle's second corner: a corner the square moves loses its snap, a
/// corner that is already square to the first point keeps the place and the snap it was clicked
/// with.
#[test]
fn a_squared_corner_keeps_its_snap_only_where_it_was_clicked() {
    use solidcore_record::Vec2;
    for command in ["sketch.rectangle.two_point", "sketch.rectangle.center"] {
        let mut tool = super::Tool::for_command(command).unwrap();
        tool.pts.push((Vec2::new(20.0, 0.0), None));
        let snap = Some("on:side:50,30".to_string());
        crate::tool_modifiers::set_square_rectangle(true);
        let kept = super::squared(&tool, (Vec2::new(50.0, 30.0), snap.clone()));
        let moved = super::squared(&tool, (Vec2::new(50.0, 25.0), Some("on:side:50,25".to_string())));
        crate::tool_modifiers::set_square_rectangle(false);
        let free = super::squared(&tool, (Vec2::new(50.0, 25.0), snap.clone()));
        assert_eq!(kept, (Vec2::new(50.0, 30.0), snap.clone()), "{command}: already square");
        assert_eq!(moved, (Vec2::new(50.0, 30.0), None), "{command}: moved to the square");
        assert_eq!(free, (Vec2::new(50.0, 25.0), snap), "{command}: without the chord");
    }
}

/// The Star tool draws with the mouse: a click is the centre, the second click (or the release of a
/// drag) the size and the turn, with a preview before it. Double-clicking a curve of the star
/// opens Edit Star on it.
#[test]
fn a_star_is_drawn_from_its_centre_and_a_double_click_edits_it() {
    use solidcore_record::Vec2;
    use solidcraft_engine::geom::prelude::*;
    let mut h = Harness::new();
    h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
    h.app.cam.distance = 50.0;
    h.step(&json!({"start":"sketch.star"})).unwrap();
    assert!(h.app.dialog.is_none() && h.app.tool.is_some(), "Star starts as a drawing tool");
    h.step(&json!({"click":{"world":[0, 0, 0]}})).unwrap();
    let tool = h.app.tool.as_ref().unwrap();
    let outline = super::star_outline(tool, Vec2::new(0.0, 10.0)).unwrap();
    assert_eq!(outline.len(), 11, "5 tips and 5 valleys, closed");
    assert!(outline[0].dist(Vec2::new(0.0, 10.0)) < 1e-9 && (outline[1].dist(Vec2::ZERO) - 4.0).abs() < 1e-9, "{outline:?}");
    assert!(super::star_outline(tool, Vec2::ZERO).is_none());
    h.step(&json!({"click":{"world":[0, 10, 0]}})).unwrap();
    let state = h.app.session.world_state().unwrap();
    let sketch = &state.sketches[0].sketch;
    let Some(solidcraft_engine::sketch::LinkSource::Star(star)) = sketch.links.first().map(|l| l.source.clone()) else {
        panic!("a star was drawn: {:?} {:?}", h.app.status, sketch.links)
    };
    assert!((star.outer - 10.0).abs() < 1e-9 && star.center.dist(Vec2::ZERO) < 1e-9 && star.angle.abs() < 1e-9, "{star:?}");
    // A quarter of the way along the edge from the top tip to the next valley (its middle is a snap point).
    h.app.tool = None;
    h.app.session.selection.clear();
    h.frames(2);
    h.step(&json!({"click":{"world":[-0.588, 8.309, 0]},"double":true})).unwrap();
    h.frames(2);
    assert!(
        matches!(h.app.dialog.as_ref().map(|d| &d.kind), Some(crate::dialogs::Kind::SketchModify(crate::sketch_modify_ui::Mod::Star { .. }))),
        "the double-click opened Edit Star: {:?} {:?}",
        h.app.session.selection,
        h.app.status
    );
}
