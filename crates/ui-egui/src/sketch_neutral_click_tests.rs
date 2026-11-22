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

/// MEJ-12: with a star selected the pattern commands are greyed out and say why; a mirror stays on.
#[test]
fn a_pattern_is_greyed_out_for_a_selected_star_and_says_why() {
    let mut h = Harness::new();
    h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
    let made = h.app.run("sketch.star", json!({"center": [0, 0], "outer_radius": 10})).unwrap();
    let curve = made["curves"][0].as_str().unwrap().to_owned();
    let info = |h: &Harness, id: &str| crate::render_workspace::interactive_command_info(&h.app, solidcraft_engine::find_command(id).unwrap());
    h.app.session.selection.clear();
    h.frames(2);
    assert!(info(&h, "sketch.pattern.circular").enabled, "nothing selected: a pattern can start");
    h.app.session.selection = vec![solidcraft_engine::Sel::SketchCurve { id: curve, sketch: h.app.session.active_sketch }];
    h.frames(2);
    for id in ["sketch.pattern.circular", "sketch.pattern.rectangular"] {
        let i = info(&h, id);
        assert!(!i.enabled && i.disabled_reason.as_deref().is_some_and(|r| r.starts_with("Not available")), "{id}: {:?}", i.disabled_reason);
    }
    assert!(info(&h, "sketch.mirror").enabled, "a mirror of a star works");
}
