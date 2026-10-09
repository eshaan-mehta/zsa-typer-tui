//! Fetches a layout from Oryx, ZSA's online configurator.
//!
//! This is Oryx's own (undocumented) GraphQL backend, so everything here is written to
//! fail loudly rather than misread data if the response shape changes.

use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

use crate::layout::{Action, Key, Layer, Layout};

const ENDPOINT: &str = "https://oryx.zsa.io/graphql";
const QUERY: &str = "query getLayout($hashId: String!, $revisionId: String!, $geometry: String) { \
    layout(hashId: $hashId, geometry: $geometry, revisionId: $revisionId) { \
        hashId title geometry revision { hashId layers { title position keys } } } }";

/// Split the firmware's serial number (`<layoutId>/<revisionId>`) into its parts.
pub fn parse_firmware_id(serial: &str) -> Option<(String, String)> {
    let (layout, revision) = serial.trim().split_once('/')?;
    let ok = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric());
    (ok(layout) && ok(revision)).then(|| (layout.to_string(), revision.to_string()))
}

pub fn fetch(layout_id: &str, revision_id: &str) -> Result<Layout> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(8)))
        .build()
        .into();
    let body = json!({
        "query": QUERY,
        "variables": { "hashId": layout_id, "revisionId": revision_id, "geometry": "voyager" },
    });
    let response: Value = agent
        .post(ENDPOINT)
        .send_json(&body)
        .context("couldn't reach Oryx")?
        .body_mut()
        .read_json()
        .context("Oryx sent a response that isn't JSON")?;
    parse(&response, layout_id, revision_id)
}

pub fn parse(response: &Value, layout_id: &str, revision_id: &str) -> Result<Layout> {
    if let Some(errors) = response.get("errors").and_then(Value::as_array) {
        let msg = errors.iter().filter_map(|e| e["message"].as_str()).collect::<Vec<_>>().join("; ");
        bail!("Oryx returned an error: {msg}");
    }
    let layout = &response["data"]["layout"];
    if layout.is_null() {
        bail!("Oryx has no layout {layout_id}/{revision_id} (deleted, private, or not made in Oryx?)");
    }
    let geometry = layout["geometry"].as_str().unwrap_or("");
    if geometry != "voyager" {
        bail!("layout is for \"{geometry}\", not the Voyager");
    }
    let mut raw_layers: Vec<&Value> = layout["revision"]["layers"]
        .as_array()
        .ok_or_else(|| anyhow!("unexpected Oryx response: no layers"))?
        .iter()
        .collect();
    raw_layers.sort_by_key(|l| l["position"].as_i64().unwrap_or(i64::MAX));

    let layers = raw_layers
        .iter()
        .map(|l| {
            let keys = l["keys"]
                .as_array()
                .ok_or_else(|| anyhow!("unexpected Oryx response: layer without keys"))?
                .iter()
                .map(parse_key)
                .collect();
            Ok(Layer { title: l["title"].as_str().unwrap_or("").to_string(), keys })
        })
        .collect::<Result<Vec<_>>>()?;

    let parsed = Layout {
        title: layout["title"].as_str().unwrap_or("Untitled").to_string(),
        layout_id: layout_id.to_string(),
        revision_id: revision_id.to_string(),
        layers,
    };
    parsed.validate().map_err(|e| anyhow!("unexpected Oryx response: {e}"))?;
    Ok(parsed)
}

fn parse_key(k: &Value) -> Key {
    Key {
        tap: parse_action(&k["tap"]),
        hold: parse_action(&k["hold"]),
        custom_label: k["customLabel"].as_str().filter(|s| !s.is_empty()).map(String::from),
    }
}

fn parse_action(a: &Value) -> Option<Action> {
    let code = a.get("code")?.as_str()?.to_string();
    let layer = a["layer"].as_u64().and_then(|l| u8::try_from(l).ok());
    let mut mods = Vec::new();
    if let Some(m) = a["modifiers"].as_object() {
        let on = |names: [&str; 2]| names.iter().any(|n| m.get(*n).and_then(Value::as_bool) == Some(true));
        for (name, keys) in [
            ("ctrl", ["leftCtrl", "rightCtrl"]),
            ("shift", ["leftShift", "rightShift"]),
            ("alt", ["leftAlt", "rightAlt"]),
            ("gui", ["leftGui", "rightGui"]),
        ] {
            if on(keys) {
                mods.push(name.to_string());
            }
        }
    }
    Some(Action { code, layer, mods })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firmware_ids() {
        assert_eq!(parse_firmware_id("nvKL0/yoAjor"), Some(("nvKL0".into(), "yoAjor".into())));
        assert_eq!(parse_firmware_id("nvKL0"), None);
        assert_eq!(parse_firmware_id("/x"), None);
        assert_eq!(parse_firmware_id("a b/c"), None);
    }

    #[test]
    fn parses_real_response() {
        let v: Value = serde_json::from_str(include_str!("../tests/fixtures/oryx_layout.json")).unwrap();
        let l = parse(&v, "nvKL0", "yoAjor").unwrap();
        assert_eq!(l.layers.len(), 4);
        assert_eq!(l.layers[0].title, "Main");
        assert_eq!(l.layers[0].keys[7].label(), "B");
        assert_eq!(l.layers[0].keys[51].hold.as_ref().and_then(Action::target_layer), Some(2));
        assert_eq!(l.find_char('e').map(|h| h.key), Some(41));
    }

    #[test]
    fn rejects_bad_responses() {
        assert!(parse(&json!({"errors": [{"message": "nope"}]}), "a", "b").is_err());
        assert!(parse(&json!({"data": {"layout": null}}), "a", "b").is_err());
        let moonlander = json!({"data": {"layout": {"geometry": "moonlander", "revision": {"layers": []}}}});
        assert!(parse(&moonlander, "a", "b").is_err());
    }
}
