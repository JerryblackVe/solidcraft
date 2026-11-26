//! Contracts and results of `sketch.star` and `sketch.edit_star` (`cmd/sketch_star.rs`).
//!
//! The numbers are checked by the handler (`StarSpec::check`), which says what is wrong with them;
//! the schema only names the keys and their types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::types::{AngleExpr, LengthExpr, NumberExpr};

/// Draw a star in the sketch being edited: outlines kept linked to their numbers, so
/// `sketch.edit_star` can change the star later (points, size, depth, rounding, turn) without
/// drawing it again. One undo step.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct StarParams {
    /// The centre: `[x, y]`, each a number in mm or an expression over the design's parameters.
    #[schemars(with = "[LengthExpr; 2]")]
    pub center: Value,
    /// Distance from the centre to a tip, in mm, above 0 (a number or an expression).
    pub outer_radius: LengthExpr,
    /// How many tips, 3 to 200. Default: 5.
    #[serde(default)]
    pub points: Option<NumberExpr>,
    /// How deep the valleys go, as a share of the outer radius: the valley radius, above 0 and
    /// under 100. Default: 40. Ignored with a `skip` of 2 or more.
    #[serde(default)]
    pub inner_percent: Option<NumberExpr>,
    /// 0 or 1: valleys by `inner_percent`. 2 or more: the regular star `{points/skip}` (a skip
    /// of 2 on 5 points is the pentagram); it must be under half the points and share no factor
    /// with them. Default: 0.
    #[serde(default)]
    pub skip: Option<NumberExpr>,
    /// Radius in mm of the rounding of each tip. Default: 0 (sharp).
    #[serde(default)]
    pub tip_round: Option<LengthExpr>,
    /// Radius in mm of the rounding of each valley. Default: 0 (sharp).
    #[serde(default)]
    pub valley_round: Option<LengthExpr>,
    /// Turn in degrees, counter-clockwise; 0 has a tip straight up. Default: 0.
    #[serde(default)]
    pub angle: Option<AngleExpr>,
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
    /// The new centre `[x, y]`: numbers in mm or expressions.
    #[serde(default)]
    #[schemars(with = "Option<[LengthExpr; 2]>")]
    pub center: Option<Value>,
    /// The new outer radius in mm (a number or an expression).
    #[serde(default)]
    pub outer_radius: Option<LengthExpr>,
    /// The new number of tips, 3 to 200.
    #[serde(default)]
    pub points: Option<NumberExpr>,
    /// The new valley depth, a share of the outer radius (above 0, under 100).
    #[serde(default)]
    pub inner_percent: Option<NumberExpr>,
    /// The new skip (0 or 1: none).
    #[serde(default)]
    pub skip: Option<NumberExpr>,
    /// The new tip rounding radius in mm.
    #[serde(default)]
    pub tip_round: Option<LengthExpr>,
    /// The new valley rounding radius in mm.
    #[serde(default)]
    pub valley_round: Option<LengthExpr>,
    /// The new turn in degrees.
    #[serde(default)]
    pub angle: Option<AngleExpr>,
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
    /// valley_round, angle, circles}`, and `exprs`: the expressions behind those typed over the
    /// design's parameters (`{x, y, points, outer, inner_percent, skip, tip_round, valley_round,
    /// angle}`), absent when none was.
    pub star: Value,
    /// In the warn parameter-check mode: what the call's parameters got wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}
