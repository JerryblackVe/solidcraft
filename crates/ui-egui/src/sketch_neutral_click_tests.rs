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
    let outline = super::star_outline(tool, Vec2::new(0.0, 10.0), 5.0).unwrap();
    assert_eq!(outline.len(), 11, "5 tips and 5 valleys, closed");
    assert!(outline[0].dist(Vec2::new(0.0, 10.0)) < 1e-9 && (outline[1].dist(Vec2::ZERO) - 4.0).abs() < 1e-9, "{outline:?}");
    assert!(super::star_outline(tool, Vec2::ZERO, 5.0).is_none());
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

/// The Star tool's Points box sets the number of points before the second click: the preview and
/// the drawn star follow it, and a count outside 3 to 200 draws nothing.
#[test]
fn a_typed_points_count_sets_the_stars_points() {
    use solidcore_record::Vec2;
    let mut h = Harness::new();
    h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
    h.app.cam.distance = 50.0;
    h.step(&json!({"start":"sketch.star"})).unwrap();
    h.step(&json!({"click":{"world":[0, 0, 0]}})).unwrap();
    h.frames(2);
    let lock = |h: &mut Harness, text: &str| {
        let tool = h.app.tool.as_mut().unwrap();
        let points = tool.dims.iter_mut().find(|b| b.label == "Points").expect("a Points box next to Radius");
        points.value = text.into();
        points.locked = true;
    };
    lock(&mut h, "7");
    h.frames(2);
    let tool = h.app.tool.as_ref().unwrap();
    assert_eq!(super::star_outline(tool, Vec2::new(0.0, 10.0), 7.0).unwrap().len(), 15, "7 tips and 7 valleys, closed");
    assert!(super::star_outline(tool, Vec2::new(0.0, 10.0), 2.0).is_none() && super::star_outline(tool, Vec2::new(0.0, 10.0), 4.5).is_none());
    h.step(&json!({"click":{"world":[0, 10, 0]}})).unwrap();
    let sketch = &h.app.session.world_state().unwrap().sketches[0].sketch;
    let Some(solidcraft_engine::sketch::LinkSource::Star(star)) = sketch.links.first().map(|l| l.source.clone()) else {
        panic!("a star was drawn: {:?}", h.app.status)
    };
    assert_eq!(star.points, 7, "{star:?}");
    // 2 points is no star: nothing more is drawn and the status says why.
    h.step(&json!({"start":"sketch.star"})).unwrap();
    h.step(&json!({"click":{"world":[30, 0, 0]}})).unwrap();
    h.frames(2);
    lock(&mut h, "2");
    h.step(&json!({"click":{"world":[30, 10, 0]}})).unwrap();
    assert_eq!(h.app.session.world_state().unwrap().sketches[0].sketch.links.len(), 1, "no second star");
    assert!(format!("{:?}", h.app.status).contains("Points"), "{:?}", h.app.status);
}

/// MEJ-12, MEJ-22: with a star selected the pattern commands and the mirror are available.
#[test]
fn a_pattern_and_a_mirror_are_available_for_a_selected_star() {
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
    for id in ["sketch.pattern.circular", "sketch.pattern.rectangular", "sketch.mirror"] {
        let i = info(&h, id);
        assert!(i.enabled, "{id}: {:?}", i.disabled_reason);
    }
}

/// An untouched Points box shows the count the star is drawn with, not the polygon's 6.
#[test]
fn the_untouched_points_box_shows_the_points_drawn() {
    use solidcore_record::Vec2;
    let mut h = Harness::new();
    h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
    h.app.cam.distance = 50.0;
    h.step(&json!({"start":"sketch.star"})).unwrap();
    h.step(&json!({"click":{"world":[0, 0, 0]}})).unwrap();
    h.frames(2);
    let tool = h.app.tool.as_ref().unwrap();
    let shown = crate::sketch_dims::live(&h.app, tool, Vec2::new(0.0, 10.0), 0.1);
    let points = tool.dims.iter().position(|b| b.label == "Points").expect("a Points box next to Radius");
    assert_eq!(shown[points].0, crate::sketch_dims::DEFAULT_STAR_POINTS);
    h.step(&json!({"click":{"world":[0, 10, 0]}})).unwrap();
    let Some(solidcraft_engine::sketch::LinkSource::Star(star)) =
        h.app.session.world_state().unwrap().sketches[0].sketch.links.first().map(|l| l.source.clone())
    else {
        panic!("a star was drawn: {:?}", h.app.status)
    };
    assert_eq!(f64::from(star.points), shown[points].0, "{star:?}");
}

/// A double-click inside a star (its fill, not its outline) opens Edit Star, and so does a
/// double-click on a star while the Star tool is still drawing: it edits that star, it does not
/// start another one.
#[test]
fn a_double_click_on_a_stars_fill_or_with_the_star_tool_edits_the_star() {
    let editing = |h: &Harness| {
        matches!(h.app.dialog.as_ref().map(|d| &d.kind), Some(crate::dialogs::Kind::SketchModify(crate::sketch_modify_ui::Mod::Star { .. })))
    };
    let mut h = Harness::new();
    h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
    h.app.run("sketch.star", json!({"center":[0, 0], "outer_radius":10, "points":5})).unwrap();
    h.app.cam.distance = 50.0;
    h.frames(2);
    h.step(&json!({"click":{"world":[1.5, -0.5, 0]},"double":true})).unwrap();
    h.frames(2);
    assert!(editing(&h), "the fill: {:?} {:?}", h.app.session.selection, h.app.status);
    h.app.dialog = None;
    h.app.session.selection.clear();
    h.step(&json!({"start":"sketch.star"})).unwrap();
    let before = h.app.session.doc.to_json();
    h.step(&json!({"click":{"world":[1.5, -0.5, 0]},"double":true})).unwrap();
    h.frames(2);
    assert!(editing(&h) && h.app.tool.is_none(), "with the tool: {:?} {:?}", h.app.tool.as_ref().map(|t| t.cmd), h.app.status);
    assert_eq!(h.app.session.doc.to_json(), before, "nothing was drawn");
}

/// A star sized by typing in its boxes is drawn with the typed radius, with or without a typed
/// Points count: the Radius box is the star's own number, not a dimension.
#[test]
fn a_star_typed_in_its_boxes_is_drawn() {
    for points in [None, Some("10")] {
        let mut h = Harness::new();
        h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
        h.app.cam.distance = 50.0;
        h.step(&json!({"start":"sketch.star"})).unwrap();
        h.step(&json!({"click":{"world":[0, 0, 0]}})).unwrap();
        h.frames(2);
        let tool = h.app.tool.as_mut().unwrap();
        for (label, text) in [("Radius", Some("26")), ("Points", points)] {
            if let Some(text) = text {
                let b = tool.dims.iter_mut().find(|b| b.label == label).expect(label);
                b.value = text.into();
                b.locked = true;
            }
        }
        h.step(&json!({"click":{"world":[3, 8, 0]}})).unwrap();
        let sketch = &h.app.session.world_state().unwrap().sketches[0].sketch;
        let Some(solidcraft_engine::sketch::LinkSource::Star(star)) = sketch.links.first().map(|l| l.source.clone()) else {
            panic!("{points:?}: a star was drawn: {:?}", h.app.status)
        };
        assert!((star.outer - 26.0).abs() < 1e-9 && star.points == if points.is_some() { 10 } else { 5 }, "{star:?}");
    }
}
