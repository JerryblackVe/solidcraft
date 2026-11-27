//! Transform in the viewport: the picked sketch curves and points get a box with a handle at each
//! corner to scale (the opposite corner stays put, the aspect is kept), the box itself to move,
//! and a knob above it to turn (Shift snaps to 15 degrees). The handles edit the same Position,
//! Scale and Angle the window shows, and the entities are drawn where OK would put them. Scale and
//! Angle work about the window's Center point when one is picked (#11718), else the box middle.

use egui::{Pos2, Sense, Stroke, Vec2 as Screen, vec2};
use solidcore_record::{Plane, Vec2};
use solidcraft_engine::geom::prelude::*;

use crate::SolveApp;
use crate::dialogs::Kind;
use crate::sketch_modify_ui::Mod;
use crate::viewport::Proj;

/// Half the side of a corner handle, in screen points.
const HANDLE: f32 = 6.0;
/// How far above the box the turning knob sits, in screen points.
const KNOB_GAP: f32 = 28.0;
const MIN_FACTOR: f64 = 0.0001;
const MAX_FACTOR: f64 = 1e5;

/// A number as the window's fields show it: up to three decimals, none trailing.
pub(crate) fn plain(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// What a drag took hold of.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Part {
    Move,
    /// A corner, by its signs in the box's own frame.
    Corner(f64, f64),
    /// The middle of a side, by its axis (0: a left or right side, so the width; 1: a top or bottom
    /// side, so the height) and its sign: only that size changes, the opposite side stays put.
    Side(u8, f64),
    /// The pivot: where the scale and the turn keep fixed. Moving it moves nothing else.
    Pivot,
    Turn,
}

/// A drag in progress: the part, where the pointer began on the sketch, and the placement then.
#[derive(Clone, Debug)]
pub struct Drag {
    part: Part,
    from: Vec2,
    at: Frame,
}

/// The box as placed: where its pivot is (the point it scales and turns about), how much it is
/// scaled (`scale` wide, `scale * aspect` high), how far turned, its own size before scaling, and
/// where the pivot and the box middle were before the move.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Frame {
    x: f64,
    y: f64,
    scale: f64,
    /// The height's scale over the width's: 1 keeps the shape.
    aspect: f64,
    /// The total turn, `base` and the window's Angle together.
    angle: f64,
    /// The box's own turn before this transform: a turned group's (its width runs this way).
    base: f64,
    natural: (f64, f64),
    pivot: Vec2,
    middle: Vec2,
}

/// `v` as the scale makes it: wide by `scale`, high by `scale * aspect`, before the turn.
fn stretch(f: &Frame, v: Vec2) -> Vec2 {
    Vec2::new(v.x * f.scale, v.y * f.scale * f.aspect)
}

/// `v` from the pivot, stretched in the box's own frame (before the total turn).
fn in_box(f: &Frame, v: Vec2) -> Vec2 {
    stretch(f, turn(v, -f.base))
}

/// Where the box's middle is placed.
fn center(f: &Frame) -> Vec2 {
    Vec2::new(f.x, f.y) + turn(in_box(f, f.middle - f.pivot), f.angle)
}

fn turn(v: Vec2, degrees: f64) -> Vec2 {
    let (s, c) = degrees.to_radians().sin_cos();
    Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

/// A spot of the box in its own frame (`u`, `v` in half sizes) on the sketch.
fn spot(f: &Frame, u: f64, v: f64) -> Vec2 {
    center(f) + turn(stretch(f, Vec2::new(u * f.natural.0 / 2.0, v * f.natural.1 / 2.0)), f.angle)
}

fn on_screen(plane: &Plane, proj: &Proj, q: Vec2) -> Option<Pos2> {
    proj.to_screen(plane.to_world(q))
}

fn pointer_on_sketch(plane: &Plane, proj: &Proj, at: Pos2) -> Option<Vec2> {
    let (o, d) = proj.ray(at);
    plane.intersect_ray(o, d).map(|w| plane.to_local(w))
}

/// The picked curves by index, with an ellipse's axes (they go with it when OK applies,
/// `sketch.transform`), and the picked points by index.
fn picked(app: &SolveApp, d: &crate::dialogs::Dialog) -> Option<(Vec<usize>, Vec<usize>)> {
    let state = crate::open_meshing::session_world(&app.session);
    let sk = &state.sketch(app.session.active_sketch?)?.sketch;
    let mut points: Vec<usize> = Vec::new();
    let mut drawn: Vec<usize> = Vec::new();
    for id in crate::sketch_modify_ui::entity_ids(&app.session, &d.inputs, 0) {
        if let Some(i) = sk.curve_index(&id) {
            let (major, minor) = sk.ellipse_axis_lines(i).unwrap_or((None, None));
            for c in std::iter::once(i).chain(major.map(|(l, _)| l)).chain(minor.map(|(l, ..)| l)) {
                if !drawn.contains(&c) {
                    drawn.push(c);
                }
            }
        } else if let Some(i) = sk.resolve_point(&id) {
            points.push(i);
        }
    }
    Some((drawn, points))
}

/// The open sketch's curves and points Transform draws where OK would put them, by index (a
/// curve's own points with it). They are not drawn where they are while its window is open: the
/// shape moves with the handles, as one copy.
pub(crate) fn moving(app: &SolveApp) -> (Vec<usize>, Vec<usize>) {
    let Some(d) = app.dialog.as_ref().filter(|d| matches!(d.kind, Kind::SketchModify(Mod::Transform { .. }))) else { return Default::default() };
    let state = crate::open_meshing::session_world(&app.session);
    let (Some(sk), Some(_), Some((curves, mut points))) =
        (app.session.active_sketch.and_then(|sid| state.sketch(sid)).map(|ss| &ss.sketch), picked_box(app, d), picked(app, d))
    else {
        return Default::default();
    };
    for c in curves.iter().filter_map(|&c| sk.curves.get(c)) {
        points.extend(c.kind.point_ids());
    }
    (curves, points)
}

/// The picked curves (as polylines on the sketch) and the box around them and the picked points:
/// its middle, its size and its turn (a turned group's, else 0: along the sketch's axes). None
/// when nothing usable is picked or the box has no extent.
fn picked_box(app: &SolveApp, d: &crate::dialogs::Dialog) -> Option<(Vec<Vec<Vec2>>, [f64; 2], [f64; 2], f64)> {
    let state = crate::open_meshing::session_world(&app.session);
    let sk = &state.sketch(app.session.active_sketch?)?.sketch;
    let (drawn, points) = picked(app, d)?;
    let points = points.into_iter().filter_map(|i| sk.point(i));
    let lines: Vec<Vec<Vec2>> = drawn.iter().map(|c| sk.polyline(*c)).collect();
    let base = sk.box_angle(&drawn);
    // Measured in the box's own frame, then its middle back on the sketch.
    let all: Vec<Vec2> = lines.iter().flatten().copied().chain(points).map(|q| turn(q, -base)).collect();
    let first = *all.first()?;
    let (lo, hi) = all.iter().fold((first, first), |(lo, hi), p| (Vec2::new(lo.x.min(p.x), lo.y.min(p.y)), Vec2::new(hi.x.max(p.x), hi.y.max(p.y))));
    let size = [hi.x - lo.x, hi.y - lo.y];
    let middle = turn(Vec2::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0), base);
    // A single point or a straight line along an axis has no area to take hold of.
    (size[0] > 1e-9 && size[1] > 1e-9).then_some((lines, [middle.x, middle.y], size, base))
}

/// Keep the Transform window's box up to date with what is picked.
pub fn refresh(app: &mut SolveApp) {
    let Some(d) = app.dialog.as_ref().filter(|d| matches!(d.kind, Kind::SketchModify(Mod::Transform { .. }))) else { return };
    let boxed = picked_box(app, d).map(|(_, middle, size, base)| (middle, size, base));
    // The pivot a group saves comes into the window when the group does (not while a handle is
    // dragged: the window's pivot is the newer one then).
    let group = picked_group(app, d);
    let seen = d.extra.get("pivot_group").and_then(|v| v.as_str()).map(str::to_owned);
    let load = group.as_ref().map(|g| g.0.clone()) != seen && app.viewport.transform_drag.is_none();
    let at = frame_of(app, d).map(|f| center(&f)).map(|c| [c.x, c.y]);
    if let Some(Kind::SketchModify(Mod::Transform { frame, pivot, placed, .. })) = app.dialog.as_mut().map(|d| &mut d.kind) {
        *frame = boxed;
        *placed = at;
        if load && let Some((_, saved)) = &group {
            *pivot = saved.map(|p| [p.x, p.y]);
        }
    }
    if load && let Some(d) = app.dialog.as_mut() {
        match group {
            Some((id, _)) => d.extra.insert("pivot_group".into(), serde_json::json!(id)),
            None => d.extra.remove("pivot_group"),
        };
    }
}

/// The group all the picked curves are in (its id) and the pivot it saves; None when a point or a
/// curve outside a group is picked, or the curves are of several groups.
fn picked_group(app: &SolveApp, d: &crate::dialogs::Dialog) -> Option<(String, Option<Vec2>)> {
    let state = crate::open_meshing::session_world(&app.session);
    let sk = &state.sketch(app.session.active_sketch?)?.sketch;
    let ids = crate::sketch_modify_ui::entity_ids(&app.session, &d.inputs, 0);
    let g = sk.object_group_of(ids.first()?)?;
    // One set lookup per curve: a drawing may hold thousands, and this runs every frame.
    let members: std::collections::HashSet<&str> = g.curves.iter().map(String::as_str).collect();
    ids.iter().all(|id| members.contains(id.as_str())).then(|| (g.id.clone(), g.pivot))
}

/// Save the window's pivot with the group the picked curves are (`sketch.group_pivot`), or the
/// group's middle again with None.
fn save_pivot(app: &mut SolveApp, pivot: Option<Vec2>) {
    let Some((group, saved)) = app.dialog.as_ref().and_then(|d| picked_group(app, d)) else { return };
    if saved == pivot {
        return;
    }
    let params = match pivot {
        Some(p) => serde_json::json!({"group": group, "pivot": [p.x, p.y]}),
        None => serde_json::json!({"group": group}),
    };
    if let Err(e) = app.run("sketch.group_pivot", params) {
        app.set_status(e, true);
    }
}

/// Under the Transform window's fields: Center Pivot, when the pivot was moved, puts it back at the
/// middle of the box (and in a group, in the design too).
pub(crate) fn pivot_row(app: &mut SolveApp, d: &mut crate::dialogs::Dialog, ui: &mut egui::Ui) {
    let Kind::SketchModify(Mod::Transform { pivot: Some(_), .. }) = &d.kind else { return };
    if crate::ui_kit::button(ui, solidcraft_ui_text::tr("ui.transform_center_pivot"), crate::ui_kit::Button::Small).clicked() {
        if let Kind::SketchModify(Mod::Transform { pivot, .. }) = &mut d.kind {
            *pivot = None;
        }
        // `d` is out of the app while the window is drawn, so the group is found through it.
        if let Some((group, Some(_))) = picked_group(app, d)
            && let Err(e) = app.run("sketch.group_pivot", serde_json::json!({"group": group}))
        {
            app.set_status(e, true);
        }
    }
}

/// The window's Center point on the sketch, when one is picked.
fn picked_pivot(app: &SolveApp, d: &crate::dialogs::Dialog) -> Option<Vec2> {
    let state = crate::open_meshing::session_world(&app.session);
    let sk = &state.sketch(app.session.active_sketch?)?.sketch;
    let sel = d.inputs.get(1)?.items.first()?;
    crate::sketch_modify_ui::point_id(&app.session, sel).and_then(|id| sk.resolve_point(id)).and_then(|i| sk.point(i))
}

/// The placement the window's fields describe, as a frame.
fn frame_of(app: &SolveApp, d: &crate::dialogs::Dialog) -> Option<Frame> {
    let Kind::SketchModify(Mod::Transform { x, y, factor, angle, frame, aspect, pivot: moved_pivot, .. }) = &d.kind else { return None };
    let (middle, size, base) = (*frame)?;
    let middle = Vec2::new(middle[0], middle[1]);
    // The pivot the box was given by hand wins over a picked Center, which wins over the middle.
    let pivot = moved_pivot.map(|q| Vec2::new(q[0], q[1])).or_else(|| picked_pivot(app, d)).unwrap_or(middle);
    let aspect = crate::ui_kit::parse_number(aspect.trim()).filter(|a| a.is_finite() && *a > 0.0).unwrap_or(1.0);
    let (dx, dy) =
        (crate::sketch_modify_ui::value(app, x, ValueKind::Length, "X").ok()?, crate::sketch_modify_ui::value(app, y, ValueKind::Length, "Y").ok()?);
    let scale = crate::sketch_modify_ui::factor(app, factor).ok()?;
    let turned = crate::sketch_modify_ui::value(app, angle, ValueKind::Angle, "Angle").ok()?.to_degrees();
    Some(Frame { x: pivot.x + dx, y: pivot.y + dy, scale, aspect, angle: base + turned, base, natural: (size[0], size[1]), pivot, middle })
}

use solidcraft_engine::doc::expr::Kind as ValueKind;

/// Write a frame back into the window's fields.
fn store(k: &mut Mod, f: &Frame, pivot_moved: bool) {
    if let Mod::Transform { x, y, factor, angle, aspect, pivot, .. } = k {
        *x = format!("{} mm", plain(f.x - f.pivot.x));
        *y = format!("{} mm", plain(f.y - f.pivot.y));
        *factor = format!("{}", (f.scale * 1e6).round() / 1e6);
        *aspect = format!("{}", (f.aspect * 1e6).round() / 1e6);
        *angle = format!("{} deg", plain(f.angle - f.base));
        if pivot_moved {
            *pivot = Some([f.pivot.x, f.pivot.y]);
        }
    }
}

/// The frame after the pointer has moved to `p`, from where `drag` began.
fn apply(drag: &Drag, p: Vec2, snap: bool) -> Frame {
    let start = drag.at;
    let mut now = start;
    match drag.part {
        Part::Move => {
            now.x = start.x + (p.x - drag.from.x);
            now.y = start.y + (p.y - drag.from.y);
        }
        Part::Turn => {
            // Turned about the pivot by as much as the pointer has turned about it since the drag
            // began; with the box middle as the pivot that keeps the knob under the pointer.
            let pivot = Vec2::new(start.x, start.y);
            let (to, from) = (p - pivot, drag.from - pivot);
            if to.len() > 1e-9 && from.len() > 1e-9 {
                // The window's Angle, from the box's own turn, is what snaps.
                let a = start.angle - start.base + (to.angle() - from.angle()).to_degrees();
                let a = if snap { (a / 15.0).round() * 15.0 } else { a };
                now.angle = start.base + (a + 180.0).rem_euclid(360.0) - 180.0;
            }
        }
        Part::Side(axis, sign) => {
            // One size changes, from the opposite side, which stays: the pointer in the box's own
            // frame says how big that size is now.
            let (u, v) = if axis == 0 { (sign, 0.0) } else { (0.0, sign) };
            let fixed = spot(&start, -u, -v);
            let local = turn(p - fixed, -start.angle);
            let high = start.scale * start.aspect;
            let reach = if axis == 0 {
                let wide = (local.x.abs() / start.natural.0).clamp(MIN_FACTOR, MAX_FACTOR);
                now.scale = wide;
                now.aspect = (high / wide).clamp(MIN_FACTOR, MAX_FACTOR);
                Vec2::new(sign * wide * start.natural.0 / 2.0, 0.0)
            } else {
                let tall = (local.y.abs() / start.natural.1).clamp(MIN_FACTOR, MAX_FACTOR);
                now.aspect = (tall / start.scale).clamp(MIN_FACTOR, MAX_FACTOR);
                Vec2::new(0.0, sign * tall * start.natural.1 / 2.0)
            };
            let middle = fixed + turn(reach, start.angle);
            let pivot = middle - turn(in_box(&now, start.middle - start.pivot), start.angle);
            if pivot.x.is_finite() && pivot.y.is_finite() {
                now.x = pivot.x;
                now.y = pivot.y;
            }
        }
        Part::Pivot => {
            // The pivot goes under the pointer; the entities do not move, so what the pivot is
            // placed at is where the box has put that spot.
            now.pivot = p;
            let at = moved(&start, p);
            now.x = at.x;
            now.y = at.y;
        }
        Part::Corner(u, v) => {
            let fixed = spot(&start, -u, -v);
            // The pointer in the box's own frame, measured from the corner that stays.
            let local = turn(p - fixed, -start.angle);
            let high = start.natural.1 * start.aspect;
            let k = (local.x.abs() / start.natural.0).max(local.y.abs() / high).clamp(MIN_FACTOR, MAX_FACTOR);
            if k.is_finite() {
                let reach = turn(Vec2::new(u * k * start.natural.0 / 2.0, v * k * high / 2.0), start.angle);
                now.scale = k;
                // The pivot goes where it keeps the box middle at `fixed + reach`.
                let pivot = fixed + reach - turn(in_box(&now, start.middle - start.pivot), start.angle);
                now.x = pivot.x;
                now.y = pivot.y;
            }
        }
    }
    now
}

/// The balloon beside the pointer while the knob turns: the total turn, in (-180, 180].
fn turn_text(f: &Frame) -> String {
    let a = (f.angle + 180.0).rem_euclid(360.0) - 180.0;
    format!("R: {}\u{b0}", plain(if a <= -180.0 { 180.0 } else { a }))
}

/// The frame with the box turned back to 0 degrees in all (the window's Angle undoes `base`).
fn upright(f: &Frame) -> Frame {
    Frame { angle: 0.0, ..*f }
}

/// Where a point of the original box goes: scaled and turned about the pivot, then moved.
fn moved(f: &Frame, q: Vec2) -> Vec2 {
    Vec2::new(f.x, f.y) + turn(in_box(f, q - f.pivot), f.angle)
}

/// The Transform window's picks with their handles; a drag on them edits the window's values.
pub fn show(app: &mut SolveApp, ui: &mut egui::Ui, proj: &Proj) {
    let Some(d) = app.dialog.as_ref().filter(|d| matches!(d.kind, Kind::SketchModify(Mod::Transform { .. }))) else {
        app.viewport.transform_drag = None;
        return;
    };
    let Some((lines, ..)) = picked_box(app, d) else { return };
    let Some(frame) = frame_of(app, d) else { return };
    let Some(plane) = app.session.active_sketch.and_then(|id| crate::canvas::profile_plane(app, id)) else { return };
    let t = solidcraft_theme::Tokens::get();
    let painter = ui.painter().clone();
    let stroke = Stroke::new(1.5, t.accent);
    // The picks where OK would leave them.
    for line in &lines {
        let pts: Vec<Pos2> = line.iter().filter_map(|q| on_screen(&plane, proj, moved(&frame, *q))).collect();
        if pts.len() > 1 {
            painter.add(egui::Shape::line(pts, Stroke::new(2.5, t.preview_sketch)));
        }
    }
    let signs = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    let Some(corners) = signs.iter().map(|(u, v)| on_screen(&plane, proj, spot(&frame, *u, *v))).collect::<Option<Vec<_>>>() else { return };
    let (Some(top), Some(heart)) = (on_screen(&plane, proj, spot(&frame, 0.0, 1.0)), on_screen(&plane, proj, spot(&frame, 0.0, 0.0))) else { return };
    let away = (top - heart).normalized();
    let knob = top + if away.length() > 0.0 { away * KNOB_GAP } else { vec2(0.0, -KNOB_GAP) };
    painter.add(egui::Shape::closed_line(corners.clone(), stroke));
    painter.line_segment([top, knob], stroke);
    painter.circle(knob, HANDLE, t.overlay, stroke);
    // The middle of each side takes only that size; the pivot is the dot where the scale and the turn hold.
    let sides: Vec<(Pos2, Part)> = [(0u8, 1.0), (0, -1.0), (1, 1.0), (1, -1.0)]
        .into_iter()
        .filter_map(|(axis, sign)| {
            let (u, v) = if axis == 0 { (sign, 0.0) } else { (0.0, sign) };
            on_screen(&plane, proj, spot(&frame, u, v)).map(|at| (at, Part::Side(axis, sign)))
        })
        .collect();
    for (at, _) in &sides {
        painter.rect(egui::Rect::from_center_size(*at, Screen::splat(HANDLE * 1.4)), 1.0, t.overlay, stroke, egui::StrokeKind::Inside);
    }
    let pivot_at = on_screen(&plane, proj, Vec2::new(frame.x, frame.y));
    if let Some(at) = pivot_at {
        painter.circle(at, HANDLE * 0.8, t.overlay, stroke);
        painter.circle_filled(at, 2.0, t.accent);
    }

    // The whole box first, so the corners and the knob above it win where they overlap.
    let mut parts: Vec<(egui::Rect, Part, egui::CursorIcon)> = vec![(egui::Rect::from_points(&corners), Part::Move, egui::CursorIcon::Grab)];
    for (c, (u, v)) in corners.iter().zip(signs) {
        painter.rect(egui::Rect::from_center_size(*c, Screen::splat(HANDLE * 2.0)), 1.0, t.overlay, stroke, egui::StrokeKind::Inside);
        parts.push((egui::Rect::from_center_size(*c, Screen::splat(HANDLE * 3.0)), Part::Corner(u, v), egui::CursorIcon::ResizeNwSe));
    }
    parts.push((egui::Rect::from_center_size(knob, Screen::splat(HANDLE * 3.0)), Part::Turn, egui::CursorIcon::Crosshair));
    for (at, part) in sides {
        let cursor = if matches!(part, Part::Side(0, _)) { egui::CursorIcon::ResizeHorizontal } else { egui::CursorIcon::ResizeVertical };
        parts.push((egui::Rect::from_center_size(at, Screen::splat(HANDLE * 2.4)), part, cursor));
    }
    // Last, so the pivot wins over the corner or side it may sit on.
    if let Some(at) = pivot_at {
        parts.push((egui::Rect::from_center_size(at, Screen::splat(HANDLE * 2.4)), Part::Pivot, egui::CursorIcon::Move));
    }
    // While Center waits for its pick, a click goes to the sketch: a rectangle's corner, the
    // point most often picked there, is under a corner handle (#11718). Once it has one, the
    // handles work again; Clear (×) empties it to pick another point there.
    if d.active == 1 && d.inputs.get(1).is_some_and(|i| i.items.is_empty()) && app.viewport.transform_drag.is_none() {
        return;
    }
    for (i, (rect, part, cursor)) in parts.into_iter().enumerate() {
        // A double click on the knob turns the box back to 0 degrees.
        let sense = if part == Part::Turn { Sense::click_and_drag() } else { Sense::drag() };
        let response = ui.interact(rect, egui::Id::unique(("sc_transform_handle", i)), sense).on_hover_cursor(cursor);
        if part == Part::Turn && response.double_clicked() {
            if let Some(Kind::SketchModify(k)) = app.dialog.as_mut().map(|d| &mut d.kind) {
                store(k, &upright(&frame), false);
            }
            app.viewport.transform_drag = None;
            continue;
        }
        crate::scenario::publish_handle(&format!("transform:handle:{i}"), rect.center());
        if response.drag_started()
            && let Some(from) = response.interact_pointer_pos().and_then(|p| pointer_on_sketch(&plane, proj, p))
        {
            app.viewport.transform_drag = Some(Drag { part, from, at: frame });
        }
        // The release frame carries the last move too: it counts, or a quick drag stops short.
        if (response.dragged() || response.drag_stopped())
            && let Some(drag) = app.viewport.transform_drag.clone()
            && drag.part == part
            && let Some(p) =
                response.interact_pointer_pos().or_else(|| ui.input(|i| i.pointer.latest_pos())).and_then(|p| pointer_on_sketch(&plane, proj, p))
        {
            let now = apply(&drag, p, ui.input(|i| i.modifiers.shift));
            if let Some(Kind::SketchModify(k)) = app.dialog.as_mut().map(|d| &mut d.kind) {
                store(k, &now, drag.part == Part::Pivot);
            }
            if drag.part == Part::Turn && response.dragged() {
                let galley = painter.layout_no_wrap(turn_text(&now), egui::FontId::proportional(t.font_body), t.text);
                let at = ui.input(|i| i.pointer.latest_pos()).unwrap_or(knob) + vec2(14.0, -26.0);
                let back = egui::Rect::from_min_size(at, galley.size()).expand(4.0);
                painter.rect(back, 3.0, t.overlay, stroke, egui::StrokeKind::Inside);
                painter.galley(at, galley, t.text);
            }
        }
        if response.drag_stopped()
            && let Some(drag) = app.viewport.transform_drag.take()
            && drag.part == Part::Pivot
            && let Some(Kind::SketchModify(Mod::Transform { pivot: Some(q), .. })) = app.dialog.as_ref().map(|d| &d.kind)
        {
            let q = Vec2::new(q[0], q[1]);
            save_pivot(app, Some(q));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> Frame {
        let middle = Vec2::new(10.0, 5.0);
        Frame { x: 10.0, y: 5.0, scale: 1.0, aspect: 1.0, angle: 0.0, base: 0.0, natural: (20.0, 10.0), pivot: middle, middle }
    }

    /// The box of `frame()` with its lower left corner (0, 0) as the pivot.
    fn cornered() -> Frame {
        Frame { x: 0.0, y: 0.0, pivot: Vec2::ZERO, ..frame() }
    }

    /// #11718: with a picked Center, the knob turns and a corner scales about it, and the moved
    /// entities and the box agree.
    #[test]
    fn a_picked_center_stays_put_while_the_box_turns_and_scales() {
        let f = cornered();
        assert!(center(&f).dist(Vec2::new(10.0, 5.0)) < 1e-9 && spot(&f, -1.0, -1.0).dist(Vec2::ZERO) < 1e-9, "unmoved, the box is where it was");
        // The pointer goes a quarter turn round the pivot: from (10, 0) up to (0, 10).
        let turned = apply(&drag(Part::Turn, Vec2::new(10.0, 0.0), f), Vec2::new(0.0, 10.0), false);
        assert!((turned.angle - 90.0).abs() < 1e-9, "{}", turned.angle);
        assert!(moved(&turned, Vec2::ZERO).dist(Vec2::ZERO) < 1e-9, "the pivot stays");
        assert!(moved(&turned, Vec2::new(20.0, 0.0)).dist(Vec2::new(0.0, 20.0)) < 1e-9, "the far corner turns about it");
        assert!(spot(&turned, 1.0, -1.0).dist(Vec2::new(0.0, 20.0)) < 1e-9, "the box turns with the entities");
        // A corner drag still keeps the opposite corner, the pivot following the box.
        let fixed = spot(&turned, -1.0, -1.0);
        let pulled = spot(&turned, 1.0, 1.0) + turn(Vec2::new(20.0, 10.0), 90.0);
        let scaled = apply(&drag(Part::Corner(1.0, 1.0), spot(&turned, 1.0, 1.0), turned), pulled, false);
        assert!((scaled.scale - 2.0).abs() < 1e-9 && spot(&scaled, -1.0, -1.0).dist(fixed) < 1e-9, "{scaled:?}");
        for (u, v, q) in [(-1.0, -1.0, Vec2::ZERO), (1.0, 1.0, Vec2::new(20.0, 10.0))] {
            assert!(moved(&scaled, q).dist(spot(&scaled, u, v)) < 1e-9, "a corner of the entities is a corner of the box");
        }
    }

    fn drag(part: Part, from: Vec2, at: Frame) -> Drag {
        Drag { part, from, at }
    }

    #[test]
    fn a_corner_scales_about_the_opposite_corner_turned_or_not() {
        for angle in [0.0, 30.0] {
            let f = Frame { angle, ..frame() };
            let fixed = spot(&f, -1.0, -1.0);
            let pulled = spot(&f, 1.0, 1.0) + turn(Vec2::new(20.0, 10.0), angle);
            let now = apply(&drag(Part::Corner(1.0, 1.0), spot(&f, 1.0, 1.0), f), pulled, false);
            assert!((now.scale - 2.0).abs() < 1e-9, "{}", now.scale);
            assert!(spot(&now, -1.0, -1.0).dist(fixed) < 1e-9, "the opposite corner stays");
        }
    }

    /// A side's handle changes that size alone and the opposite side stays, turned or not.
    #[test]
    fn a_side_changes_one_size_and_keeps_the_opposite_side() {
        for angle in [0.0, 30.0] {
            let f = Frame { angle, ..frame() };
            let wide_fixed = spot(&f, -1.0, 0.0);
            let pulled = spot(&f, 1.0, 0.0) + turn(Vec2::new(10.0, 0.0), angle);
            let now = apply(&drag(Part::Side(0, 1.0), spot(&f, 1.0, 0.0), f), pulled, false);
            assert!((now.scale - 1.5).abs() < 1e-9, "{}", now.scale);
            assert!(spot(&now, -1.0, 0.0).dist(wide_fixed) < 1e-9, "the left side stays");
            assert!((spot(&now, 0.0, 1.0).dist(spot(&now, 0.0, -1.0)) - 10.0).abs() < 1e-9, "the height is as it was");
            assert!((spot(&now, 1.0, 0.0).dist(spot(&now, -1.0, 0.0)) - 30.0).abs() < 1e-9);
            // The height: pulled down from the top side, the bottom stays.
            let tall_fixed = spot(&now, 0.0, -1.0);
            let up = spot(&now, 0.0, 1.0) + turn(Vec2::new(0.0, 5.0), angle);
            let taller = apply(&drag(Part::Side(1, 1.0), spot(&now, 0.0, 1.0), now), up, false);
            assert!(spot(&taller, 0.0, -1.0).dist(tall_fixed) < 1e-9, "the bottom stays");
            assert!((spot(&taller, 0.0, 1.0).dist(spot(&taller, 0.0, -1.0)) - 15.0).abs() < 1e-9);
            assert!((spot(&taller, 1.0, 0.0).dist(spot(&taller, -1.0, 0.0)) - 30.0).abs() < 1e-9, "the width is as it was");
            for q in [Vec2::ZERO, Vec2::new(20.0, 10.0)] {
                let (u, v) = (if q.x > 0.0 { 1.0 } else { -1.0 }, if q.y > 0.0 { 1.0 } else { -1.0 });
                assert!(moved(&taller, q).dist(spot(&taller, u, v)) < 1e-9, "a corner of the entities is a corner of the box");
            }
        }
    }

    /// The pivot handle moves where the scale and the turn hold, and nothing else.
    #[test]
    fn moving_the_pivot_moves_no_entity() {
        let f = Frame { angle: 25.0, scale: 2.0, aspect: 0.5, x: 3.0, y: -2.0, ..frame() };
        let now = apply(&drag(Part::Pivot, center(&f), f), Vec2::new(-7.0, 4.0), false);
        assert_eq!(now.pivot, Vec2::new(-7.0, 4.0));
        for q in [Vec2::ZERO, Vec2::new(20.0, 10.0), Vec2::new(3.0, 8.0)] {
            assert!(moved(&now, q).dist(moved(&f, q)) < 1e-9, "{q:?}");
        }
    }

    #[test]
    fn the_box_moves_and_the_knob_turns() {
        let f = frame();
        let now = apply(&drag(Part::Move, Vec2::new(10.0, 5.0), f), Vec2::new(14.0, 1.0), false);
        assert_eq!((now.x, now.y), (14.0, 1.0));
        // The knob is straight above the middle: to the left of it is a quarter turn.
        let turned = apply(&drag(Part::Turn, Vec2::new(14.0, 10.0), now), Vec2::new(4.0, 1.0), false);
        assert!((turned.angle - 90.0).abs() < 1e-9, "{}", turned.angle);
        let snapped = apply(&drag(Part::Turn, Vec2::new(14.0, 10.0), now), Vec2::new(4.0, 3.0), true);
        assert!((snapped.angle % 15.0).abs() < 1e-9, "{}", snapped.angle);
    }

    #[test]
    fn the_balloon_shows_the_total_turn_and_a_double_click_makes_it_zero() {
        let mut f = frame();
        f.base = 30.0;
        f.angle = 75.0;
        assert_eq!(turn_text(&f), "R: 75\u{b0}");
        f.angle = 190.0;
        assert_eq!(turn_text(&f), "R: -170\u{b0}");
        let up = upright(&f);
        assert_eq!(turn_text(&up), "R: 0\u{b0}");
        let mut k = Mod::Transform {
            x: String::new(),
            y: String::new(),
            factor: String::new(),
            angle: String::new(),
            frame: None,
            aspect: String::new(),
            pivot: None,
            placed: None,
            shown: Default::default(),
        };
        store(&mut k, &up, false);
        assert!(matches!(&k, Mod::Transform { angle, .. } if angle == "-30 deg"), "the window's Angle is relative to the box's own turn");
    }

    #[test]
    fn a_typed_place_moves_the_distance_by_the_difference_in_the_design_unit() {
        let mut doc = solidcraft_engine::doc::Document::default();
        let mut x = "2 mm".to_string();
        let mut at = 25.0;
        crate::sketch_modify_ui::move_to(&doc, &mut x, &mut at, "25 mm");
        assert_eq!((x.as_str(), at), ("2 mm", 25.0), "a row left as shown changes nothing");
        crate::sketch_modify_ui::move_to(&doc, &mut x, &mut at, "10 in");
        assert_eq!((x.as_str(), at), ("231 mm", 254.0), "10 in is 254 mm: 2 + 254 - 25");
        let mut shown = String::new();
        crate::sketch_modify_ui::sync_place(&doc, &mut shown, 25.4);
        assert_eq!(shown, "25.4 mm");
        doc.units = "in".into();
        let mut x = "0 in".to_string();
        let mut at = 25.4;
        crate::sketch_modify_ui::move_to(&doc, &mut x, &mut at, "2");
        assert_eq!(x, "1 in", "a bare 2 is 2 in: 0 + 50.8 - 25.4 mm");
        let mut typed = "1 in".to_string();
        crate::sketch_modify_ui::sync_place(&doc, &mut typed, 25.4);
        assert_eq!(typed, "1 in", "a text that gives the place stays as typed");
        crate::sketch_modify_ui::sync_place(&doc, &mut typed, 50.8);
        assert_eq!(typed, "2 in");
    }

    #[test]
    fn plain_numbers_keep_three_decimals_at_most() {
        assert_eq!(plain(12.0), "12");
        assert_eq!(plain(1.23456), "1.235");
        assert_eq!(plain(-0.0001), "0");
    }

    /// An ellipse picked alone is drawn where OK would leave it with its axes, as OK moves,
    /// turns and scales them with it.
    #[test]
    fn an_ellipse_picked_alone_is_previewed_with_its_axes() {
        let mut h = crate::scenario::Harness::new();
        h.app.run("sketch.create", serde_json::json!({"plane": "XY"})).unwrap();
        h.app.run("sketch.ellipse", serde_json::json!({"center": [0, 0], "major": [20, 0], "minor_radius": 8})).unwrap();
        h.call("ui.view", serde_json::json!({"view": "top"}));
        h.settle();
        h.step(&serde_json::json!({"click": {"sketch": [10.0, 6.928]}})).unwrap();
        h.step(&serde_json::json!({"start": "sketch.transform"})).unwrap();
        h.frames(3);
        let d = h.app.dialog.as_ref().expect("the Transform window");
        let (lines, middle, size, _) = picked_box(&h.app, d).expect("a box round the ellipse");
        assert_eq!(lines.len(), 3, "the ellipse and its two axes");
        assert!(
            middle[0].abs() < 1e-6 && middle[1].abs() < 1e-6 && (size[0] - 40.0).abs() < 1e-3 && (size[1] - 16.0).abs() < 1e-3,
            "{middle:?} {size:?}"
        );
        // With its minor axis deleted, the major one that is left.
        h.step(&serde_json::json!({"key": "Escape"})).unwrap();
        let sk = h.app.session.doc.sketch(h.app.session.active_sketch.unwrap()).unwrap();
        let minor = sk.curves.iter().rev().find(|c| c.construction).unwrap().id.clone();
        h.app.run("sketch.delete", serde_json::json!({"entities": [minor]})).unwrap();
        h.settle();
        h.step(&serde_json::json!({"click": {"sketch": [10.0, 6.928]}})).unwrap();
        h.step(&serde_json::json!({"start": "sketch.transform"})).unwrap();
        h.frames(3);
        let d = h.app.dialog.as_ref().expect("the Transform window");
        let (lines, _, size, _) = picked_box(&h.app, d).expect("a box round the ellipse");
        assert_eq!(lines.len(), 2, "the ellipse and its major axis");
        assert!((size[0] - 40.0).abs() < 1e-3, "{size:?}");
    }

    /// While Transform's window is open the picked curves are drawn once, where the handles put
    /// them: the sketch scene and the selection highlight leave them out where they are.
    #[test]
    fn the_picked_curves_are_not_drawn_twice_while_transform_is_open() {
        let mut h = crate::scenario::Harness::new();
        h.app.run("sketch.create", serde_json::json!({"plane": "XY"})).unwrap();
        h.app.run("sketch.ellipse", serde_json::json!({"center": [0, 0], "major": [20, 0], "minor_radius": 8})).unwrap();
        h.app.run("sketch.line", serde_json::json!({"points": [[-30, 20], [30, 20]]})).unwrap();
        h.call("ui.view", serde_json::json!({"view": "top"}));
        h.settle();
        h.step(&serde_json::json!({"click": {"sketch": [10.0, 6.928]}})).unwrap();
        h.frames(2);
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let drawn = |app: &SolveApp| {
            let scene = crate::viewport::build_sketch_scene(app, &crate::viewport::projection(app, rect));
            let lit = crate::viewport::build_highlight(app).merged();
            (scene.lines.len() + scene.overlay.len(), lit.lines.len() + lit.overlay.len())
        };
        let (scene, lit) = drawn(&h.app);
        assert!(moving(&h.app).0.is_empty(), "no Transform window");
        h.step(&serde_json::json!({"start": "sketch.transform"})).unwrap();
        h.frames(3);
        assert_eq!(moving(&h.app).0.len(), 3, "the ellipse and its two axes");
        assert!(!moving(&h.app).1.is_empty(), "with their points");
        let (moving_scene, moving_lit) = drawn(&h.app);
        assert!(moving_scene < scene && moving_scene > 0, "the line stays: {moving_scene} of {scene}");
        assert!(moving_lit < lit, "{moving_lit} of {lit}");
        h.step(&serde_json::json!({"key": "Escape"})).unwrap();
        h.frames(2);
        assert_eq!(moving(&h.app), Default::default());
        assert_eq!(drawn(&h.app).0, scene, "drawn where they are again");
    }

    /// The constraint glyphs of what Transform moves are not drawn where that geometry was.
    #[test]
    fn the_picked_curves_constraint_glyphs_hide_while_transform_is_open() {
        let mut h = crate::scenario::Harness::new();
        h.app.run("sketch.create", serde_json::json!({"plane": "XY"})).unwrap();
        h.app.run("sketch.rectangle.two_point", serde_json::json!({"p0": [-20, -10], "p1": [10, 10]})).unwrap();
        h.call("ui.view", serde_json::json!({"view": "top"}));
        h.settle();
        let glyphs = || crate::sketch_tools::glyph_positions().len();
        let all = glyphs();
        assert!(all > 0, "the rectangle shows its constraints");
        // The top and right sides (one line alone has no box to move).
        let sk = h.app.session.doc.sketch(h.app.session.active_sketch.unwrap()).unwrap().clone();
        let side = |on: &dyn Fn(Vec2) -> bool| {
            let i = (0..sk.curves.len()).find(|&i| sk.polyline(i).iter().all(|p| on(*p))).unwrap();
            solidcraft_engine::Sel::SketchCurve { id: sk.curves[i].id.clone(), sketch: None }
        };
        h.app.session.selection = vec![side(&|p| (p.y - 10.0).abs() < 1e-9), side(&|p| (p.x - 10.0).abs() < 1e-9)];
        h.step(&serde_json::json!({"start": "sketch.transform"})).unwrap();
        h.frames(3);
        assert!(!moving(&h.app).0.is_empty(), "the top and right sides are picked");
        let moving_glyphs = glyphs();
        assert!(moving_glyphs < all && moving_glyphs > 0, "the other sides keep theirs: {moving_glyphs} of {all}");
        h.step(&serde_json::json!({"key": "Escape"})).unwrap();
        h.frames(3);
        assert_eq!(glyphs(), all, "back where they are");
    }
}
