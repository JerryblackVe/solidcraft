//! `sketch.groups`: a text is one group whose members are its letters, and after an extrude each
//! letter knows its bodies, so a whole text or one letter is picked in one step.

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    match s.execute(id, &p) {
        Ok(v) => v,
        Err(e) => panic!("{id} {p}: {e}"),
    }
}

fn names(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()
}

fn reopened(s: &Session) -> Session {
    Session::new(solidcraft_doc::Document::from_json(&s.doc.to_json()).unwrap())
}

/// `MARTINA` extruded 3 mm: seven letters, one body each.
fn martina(s: &mut Session) -> String {
    run(s, "sketch.create", json!({"plane": "XY", "name": "S"}));
    let link = run(s, "sketch.text", json!({"text": "MARTINA", "at": [0, 0], "height": 10}))["link"].as_str().unwrap().to_string();
    run(s, "sketch.finish", json!({}));
    run(s, "solid.extrude", json!({"sketch": "S", "distance": "3 mm"}));
    link
}

#[test]
fn an_extruded_text_knows_the_body_of_each_letter() {
    let mut s = Session::default();
    let link = martina(&mut s);
    let r = run(&mut s, "sketch.groups", json!({"sketch": "S"}));
    let groups = r["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 1, "{r}");
    let g = &groups[0];
    assert_eq!((g["id"].as_str(), g["kind"].as_str(), g["label"].as_str()), (Some(link.as_str()), Some("text"), Some("MARTINA")));
    let letters: Vec<&str> = g["members"].as_array().unwrap().iter().map(|m| m["label"].as_str().unwrap()).collect();
    assert_eq!(letters, ["M", "A", "R", "T", "I", "N", "A"]);
    // One body per letter, all different, and the group has the seven.
    let per_letter: Vec<Vec<String>> = g["members"].as_array().unwrap().iter().map(|m| names(&m["bodies"])).collect();
    assert!(per_letter.iter().all(|b| b.len() == 1), "{per_letter:?}");
    let mut all: Vec<String> = per_letter.concat();
    all.sort();
    all.dedup();
    assert_eq!(all.len(), 7);
    assert_eq!(names(&g["bodies"]), all);
    // The whole text's bodies are selected in one step, and move together.
    let items: Vec<Value> = all.iter().map(|b| json!({"type": "body", "name": b})).collect();
    assert_eq!(run(&mut s, "select.set", json!({"items": items}))["selected"], json!(7));
    run(&mut s, "solid.move", json!({"bodies": all, "translate": [0, 0, 5]}));
    // Moved, they are still the letters' bodies; and the same after saving and opening again.
    let moved = run(&mut s, "sketch.groups", json!({"sketch": "S"}));
    assert_eq!(moved["groups"][0]["members"], g["members"]);
    let mut again = reopened(&s);
    assert_eq!(run(&mut again, "sketch.groups", json!({"sketch": "S"}))["groups"][0]["members"], g["members"]);
}

#[test]
fn a_letter_with_an_accent_is_one_member_with_both_bodies() {
    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": "XY", "name": "S"}));
    run(&mut s, "sketch.text", json!({"text": "MÁ", "at": [0, 0], "height": 10}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.extrude", json!({"sketch": "S", "distance": "2 mm"}));
    let r = run(&mut s, "sketch.groups", json!({"sketch": "S"}));
    let members = r["groups"][0]["members"].as_array().unwrap();
    assert_eq!(members.len(), 2, "{r}");
    assert_eq!((members[0]["label"].as_str(), names(&members[0]["bodies"]).len()), (Some("M"), 1));
    // The A and its accent are two bodies (they do not touch) of one letter.
    assert_eq!((members[1]["label"].as_str(), names(&members[1]["bodies"]).len()), (Some("Á"), 2));
}

#[test]
fn a_curve_finds_its_group_and_letter() {
    let mut s = Session::default();
    martina(&mut s);
    let all = run(&mut s, "sketch.groups", json!({"sketch": "S"}));
    let r_curve = all["groups"][0]["members"][2]["curves"][0].clone();
    let r = run(&mut s, "sketch.groups", json!({"sketch": "S", "curve": r_curve}));
    assert_eq!((r["groups"].as_array().unwrap().len(), r["member"].as_u64()), (1, Some(2)), "{r}");
    // A body finds the text and the letter that made it, without naming the sketch.
    let r_body = names(&all["groups"][0]["members"][2]["bodies"]).remove(0);
    let by_body = run(&mut s, "sketch.groups", json!({"body": r_body}));
    assert_eq!(
        (by_body["member"].as_u64(), &by_body["groups"][0]["members"], &by_body["groups"][0]["sketch"]),
        (Some(2), &all["groups"][0]["members"], &all["groups"][0]["sketch"])
    );
    assert!(s.execute("sketch.groups", &json!({"body": "Nope"})).is_err());
    // A body no text made has no group.
    run(&mut s, "solid.box", json!({"length": 5, "width": 5, "height": 5, "corner": [100, 100, 0], "body_name": "Block"}));
    let block = run(&mut s, "sketch.groups", json!({"body": "Block"}));
    assert!(block["groups"].as_array().unwrap().is_empty() && block.get("member").is_none(), "{block}");
    assert!(crate::cmd::sketch_groups::text_bodies(&s, "Block").is_none());
    assert_eq!(crate::cmd::sketch_groups::text_bodies(&s, &r_body).map(|(all, letter)| (all.len(), letter.len())), Some((7, 1)));
    assert!(s.execute("sketch.groups", &json!({"body": r_body, "curve": "l1"})).is_err());
    // A curve that is in no group: no groups; an unknown one or a bad sketch: refused.
    run(&mut s, "sketch.edit", json!({"sketch": "S"}));
    let line = run(&mut s, "sketch.line", json!({"points": [[0, -20], [10, -20]]}))["curves"][0].clone();
    let lone = run(&mut s, "sketch.groups", json!({"curve": line}));
    assert!(lone["groups"].as_array().unwrap().is_empty() && lone.get("member").is_none(), "{lone}");
    assert!(s.execute("sketch.groups", &json!({"curve": "nope"})).is_err());
    assert!(s.execute("sketch.groups", &json!({"sketch": "missing"})).is_err());
    assert!(s.execute("sketch.groups", &json!({"curve": 3})).is_err());
    // Reading the groups changes nothing to undo.
    let undo = s.undo.len();
    run(&mut s, "sketch.groups", json!({}));
    assert_eq!(s.undo.len(), undo);
}
