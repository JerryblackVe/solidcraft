//! Contracts and results of `sketch.star` and `sketch.edit_star` (`cmd/sketch_star.rs`).
//!
//! The numbers are checked by the handler (`StarSpec::check`), which says what is wrong with them;
//! the schema only names the keys and their types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::types::Point2;

/// Draw a star in the sketch being edited: outlines kept linked to their numbers, so
/// `sketch.edit_star` can change the star later (points, size, depth, rounding, turn) without
/// drawing it again. One undo step.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct StarParams {
    /// The centre, in mm: `[x, y]`.
    pub center: Point2,
    /// Distance from the centre to a tip, in mm, above 0.
    pub outer_radius: f64,
    /// How many tips, 3 to 200. Default: 5.
    #[serde(default)]
    pub points: Option<u32>,
    /// How deep the valleys go, as a share of the outer radius: the valley radius, above 0 and
    /// under 100. Default: 40. Ignored with a `skip` of 2 or more.
    #[serde(default)]
    pub inner_percent: Option<f64>,
    /// 0 or 1: valleys by `inner_percent`. 2 or more: the regular star `{points/skip}` (a skip
    /// of 2 on 5 points is the pentagram); it must be under half the points and share no factor
    /// with them. Default: 0.
    #[serde(default)]
    pub skip: Option<u32>,
    /// Radius in mm of the rounding of each tip. Default: 0 (sharp).
    #[serde(default)]
    pub tip_round: Option<f64>,
    /// Radius in mm of the rounding of each valley. Default: 0 (sharp).
    #[serde(default)]
    pub valley_round: Option<f64>,
    /// Turn in degrees, counter-clockwise; 0 has a tip straight up. Default: 0.
    #[serde(default)]
    pub angle: Option<f64>,
    /// `true` also draws the construction circles through the tips and the valleys (never a
    /// profile). Default: false.
    #[serde(default)]
    pub circles: Option<bool>,
}

/// Change a star already drawn: any of its numbers, the rest kept. The star's outlines are
/// rebuilt, and what depends on them (an extrude of its profile) follows when the result is
/// valid. The star is `link` (its id), or `entity` (one of its curves).
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct EditStarParams {
    /// The star's link id, as `sketch.star` returned it.
    #[serde(default)]
    pub link: Option<String>,
    /// One of the star's curves, by id.
    #[serde(default)]
    pub entity: Option<String>,
    /// The new centre `[x, y]` in mm.
    #[serde(default)]
    pub center: Option<Point2>,
    /// The new outer radius in mm.
    #[serde(default)]
    pub outer_radius: Option<f64>,
    /// The new number of tips, 3 to 200.
    #[serde(default)]
    pub points: Option<u32>,
    /// The new valley depth, a share of the outer radius (above 0, under 100).
    #[serde(default)]
    pub inner_percent: Option<f64>,
    /// The new skip (0 or 1: none).
    #[serde(default)]
    pub skip: Option<u32>,
    /// The new tip rounding radius in mm.
    #[serde(default)]
    pub tip_round: Option<f64>,
    /// The new valley rounding radius in mm.
    #[serde(default)]
    pub valley_round: Option<f64>,
    /// The new turn in degrees.
    #[serde(default)]
    pub angle: Option<f64>,
    /// Draw (or stop drawing) the construction circles.
    #[serde(default)]
    pub circles: Option<bool>,
}

/// What `sketch.star` and `sketch.edit_star` return.
#[derive(Debug, Serialize, JsonSchema)]
pub struct StarResult {
    /// The star's link id: what `sketch.edit_star` takes.
    pub link: String,
    /// Its curves' ids (outline, then construction circles).
    pub curves: Vec<String>,
    /// The star's numbers now: `{center, points, outer_radius, inner_percent, skip, tip_round,
    /// valley_round, angle, circles}`.
    pub star: Value,
    /// In the warn parameter-check mode: what the call's parameters got wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}
