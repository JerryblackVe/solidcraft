use serde_json::{Value, json};

use super::{SketchGroups, SketchGroupsParams};
use crate::Session;
use crate::schema::tests::{assert_keys, conforms};
use crate::schema::{ParamCheck, input_schema_for, output_schema_for};

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

/// A sketch being edited with a text and a lone line.
fn text_sketch(s: &mut Session) {
    run(s, "sketch.create", json!({"plane": "XY", "name": "Words"}));
    run(s, "sketch.text", json!({"text": "Hi", "at": [0, 0], "height": 10}));
    run(s, "sketch.line", json!({"points": [[0, -20], [10, -20]]}));
}

#[test]
fn schemas_and_the_key_check_agree() {
    assert_keys(input_schema_for::<SketchGroupsParams>(), &["body", "curve", "sketch"]);
}

/// A call that fails today gets the same message in strict.
#[test]
fn strict_keeps_todays_errors() {
    for p in [
        json!({"sketch": true}),
        json!({"sketch": -1}),
        json!({"sketch": "Nope"}),
        json!({"curve": 3}),
        json!({"curve": "nope"}),
        json!({"body": 2}),
        json!({"body": "Nope"}),
        json!({"body": "Nope", "sketch": "Words"}),
    ] {
        let error = |level: ParamCheck| {
            let mut s = Session { param_check: level, ..Session::default() };
            text_sketch(&mut s);
            s.execute("sketch.groups", &p).err().map(|e| e.to_string())
        };
        let off = error(ParamCheck::Off);
        assert!(off.is_some(), "{p} should fail today");
        assert_eq!(error(ParamCheck::Strict), off, "{p}");
    }
}

/// The reply is as its output schema says, in Warn without warnings for good calls and with them
/// for a misspelt key.
#[test]
fn sketch_groups_replies_with_its_schema() {
    let schema = output_schema_for::<SketchGroups>();
    let schema = schema.as_value();
    let mut s = Session { param_check: ParamCheck::Warn, ..Session::default() };
    text_sketch(&mut s);
    let r = run(&mut s, "sketch.groups", json!({}));
    assert!(conforms(schema, schema, &r) && r.get("param_warnings").is_none(), "{r}");
    let curve = r["groups"][0]["members"][1]["curves"][0].clone();
    let r = run(&mut s, "sketch.groups", json!({"sketch": "Words", "curve": curve}));
    assert!(conforms(schema, schema, &r) && r["member"] == json!(1), "{r}");
    let r = run(&mut s, "sketch.groups", json!({"skech": "Words"}));
    assert!(conforms(schema, schema, &r) && r.get("param_warnings").is_some(), "{r}");
}
