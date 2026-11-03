//! The Host page's flow (owner 3 Nov 2026: "Private / Public, templates, a panel of sliders, Claude in the server
//! creation, a full guide; way more streamlined"): Private or Public, a template, the settings panel, Ask Claude, Host.
//! The page is ui/hostflow.js; players read about it in docs/leyline/host-a-realm.md.
//!
//! The settings and templates are one list, `ui/hostflow-catalog.js`, read here too (`CATALOG`), so the page and the
//! core never drift. Every setting maps to something the realm really does:
//! * `world`: the ruleset (realm/profiles/<name>.env), picked with `host_choose` as before;
//! * `env`: world server settings written to `state/mode.json` as `resolved` (the kit's setup.py `read_mode` puts them
//!   in `state/realm.env`, read last by the world server). Only the names a mode may set (`modes::env_name_ok`);
//! * `option`: `state/options.json` (realm type, roaming bots, player limit, who can make an account), read once by the
//!   kit's setup.py `init` with the same checks as the website's options (`check_options`) and then removed;
//! * `rule`: a line of the house rules (`state/rules.txt`, as Realm tools > House rules);
//! * `mods`: the realm's suggested addons and game modes (catalog ids; the realm list's mods);
//! * `coming`: shown on the page, never applied.
//!
//! Ask Claude: the player's own assistant (agents.rs, `host_plan_request` / `host_plan_propose`) or any AI chat (the page's
//! copy-paste prompt and paste-back box) answers with a JSON block of known keys. `answer_from_value` checks it: unknown
//! keys are left out and said, numbers are clamped to their range and snapped to their step, and an answer can never
//! choose who can join (Private or Public, who can make an account): the host does that.

use super::modes;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The page's list, as a JS file: `window.LeylineHostCatalog = { ...JSON... };`.
pub const CATALOG_JS: &str = include_str!("../../../ui/hostflow-catalog.js");
/// What the last Host press chose (template, settings, visibility), for "Edit settings" on the running realm.
pub const PLAN_FILE: &str = "host-plan.json";
/// Read once by the kit's setup.py init (then removed).
pub const OPTIONS_FILE: &str = "options.json";
/// Settings an answer from an assistant may never set: who can join is the host's own choice (agents.rs NEVER).
pub const NOT_FROM_AI: &[&str] = &["signup"];
/// Lines of house rules (setup.py read_rules): 10 of 120 characters.
pub const RULES_MAX: usize = 10;
pub const RULE_LEN: usize = 120;
/// One "why" line from an assistant.
pub const WHY_LEN: usize = 160;
/// Mods a realm suggests (modes.rs: 8 at most).
pub const MODS_MAX: usize = 8;
const TYPES: &[&str] = &["normal", "pvp", "rp", "rppvp"];
const BRANDS: &[&str] = &["world of warcraft", "warcraft", "lich king", "lichking", "blizzard"];

/// The catalog, parsed once. A broken catalog is a build bug: the tests parse it.
pub fn catalog() -> &'static Value {
    static C: OnceLock<Value> = OnceLock::new();
    C.get_or_init(|| parse_catalog(CATALOG_JS).unwrap_or(Value::Null))
}

pub fn parse_catalog(js: &str) -> Result<Value, String> {
    let body = js.split_once("window.LeylineHostCatalog =").map(|(_, b)| b).ok_or("no catalog")?;
    let (start, end) = (body.find('{').ok_or("no {")?, body.rfind('}').ok_or("no }")?);
    serde_json::from_str(&body[start..=end]).map_err(|e| e.to_string())
}

fn arr<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k).and_then(Value::as_array).map(|a| a.as_slice()).unwrap_or(&[])
}

pub fn settings() -> &'static [Value] {
    arr(catalog(), "settings")
}

pub fn setting(key: &str) -> Option<&'static Value> {
    settings().iter().find(|s| s["key"] == key)
}

pub fn template(id: &str) -> Option<&'static Value> {
    arr(catalog(), "templates").iter().find(|t| t["id"] == id)
}

fn world(id: &str) -> Option<&'static Value> {
    arr(catalog(), "worlds").iter().find(|w| w["id"] == id)
}

/// A template's full settings: the base, then the template's own.
pub fn template_values(id: &str) -> Result<Map<String, Value>, String> {
    let t = template(id).ok_or("That template isn't here.")?;
    let mut v = catalog()["base"].as_object().cloned().unwrap_or_default();
    for (k, x) in t["set"].as_object().into_iter().flatten() {
        v.insert(k.clone(), x.clone());
    }
    Ok(v)
}

fn plain(s: &str) -> bool {
    let low = s.to_ascii_lowercase();
    !BRANDS.iter().any(|b| low.contains(b)) && !low.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| w == "wow")
}

/// A number as given (a number, or a number in a string like "1.5x"). None: not a number.
fn raw_number(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str().and_then(|t| t.trim().trim_end_matches('x').trim().parse().ok())).filter(|n: &f64| n.is_finite())
}

/// A number in a setting's range, snapped to its step (the nearest one). None: not a number.
fn number(s: &Value, v: &Value) -> Option<f64> {
    let n = raw_number(v)?;
    let (lo, hi, step) = (s["min"].as_f64()?, s["max"].as_f64()?, s["step"].as_f64().unwrap_or(1.0));
    let snapped = lo + ((n.clamp(lo, hi) - lo) / step).round() * step;
    Some((snapped.min(hi) * 100.0).round() / 100.0)
}

fn toggle(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::Number(n) => n.as_i64().filter(|n| *n == 0 || *n == 1).map(|n| n == 1),
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "on" | "yes" | "true" | "1" => Some(true),
            "off" | "no" | "false" | "0" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// A value as JSON: whole numbers as integers.
fn num_json(n: f64) -> Value {
    if n.fract() == 0.0 { json!(n as i64) } else { json!(n) }
}

/// Settings checked against the catalog: `given` over `start`. Returns the clean settings, a line for each value that
/// was changed to fit, and each key left out (unknown, coming, or not a value it takes).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Checked {
    pub values: Map<String, Value>,
    pub changed: Vec<String>,
    pub left_out: Vec<String>,
}

pub fn check_values(start: &Map<String, Value>, given: &Map<String, Value>, from_ai: bool) -> Checked {
    let mut out = Checked { values: start.clone(), ..Default::default() };
    for (k, v) in given {
        let short: String = k.chars().take(32).collect();
        let Some(s) = setting(k) else {
            out.left_out.push(format!("{short}: not a setting"));
            continue;
        };
        if from_ai && NOT_FROM_AI.contains(&k.as_str()) {
            out.left_out.push(format!("{short}: you choose who can join yourself"));
            continue;
        }
        let label = s["label"].as_str().unwrap_or(k);
        match s["kind"].as_str() {
            Some("range") => match number(s, v) {
                Some(n) => {
                    if raw_number(v).map_or(true, |x| (x - n).abs() > 1e-9) {
                        out.changed.push(format!("{label}: {} to {} (its range is {} to {})", show(v), fmt(n), s["min"], s["max"]));
                    }
                    out.values.insert(k.clone(), num_json(n));
                }
                None => out.left_out.push(format!("{label}: not a number")),
            },
            Some("toggle") => match toggle(v) {
                Some(b) => {
                    out.values.insert(k.clone(), json!(b));
                }
                None => out.left_out.push(format!("{label}: on or off")),
            },
            Some("choice") => {
                let want = v.as_str().unwrap_or("");
                if arr(s, "options").iter().any(|o| o[0] == want) {
                    out.values.insert(k.clone(), json!(want));
                } else {
                    out.left_out.push(format!("{label}: not one of its choices"));
                }
            }
            _ => out.left_out.push(format!("{label}: coming, not yet")),
        }
    }
    out
}

fn show(v: &Value) -> String {
    match v {
        Value::String(s) => s.chars().take(12).collect(),
        other => other.to_string().chars().take(12).collect(),
    }
}

/// A number as the realm's settings want it: "3", "1.5".
pub fn fmt(n: f64) -> String {
    if n.fract() == 0.0 { format!("{}", n as i64) } else { format!("{n}") }
}

/// What a setting's value is on this world: false when it does nothing there (its `only` or `needs`).
pub fn applies(s: &Value, values: &Map<String, Value>) -> bool {
    let w = values.get("world").and_then(Value::as_str).unwrap_or("whole");
    let only = arr(s, "only");
    if !only.is_empty() && !only.iter().any(|x| x == w) {
        return false;
    }
    match s["needs"].as_str() {
        Some(n) => values.get(n).and_then(Value::as_bool).unwrap_or(false),
        None => true,
    }
}

/// What the realm gets.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Plan {
    pub profile: String,
    pub visibility: String,
    pub env: BTreeMap<String, String>,
    pub options: Map<String, Value>,
    pub mods: Vec<String>,
    pub rules: Vec<String>,
}

/// The plan for these settings (already checked) and the host's own house rules.
pub fn resolve(values: &Map<String, Value>, visibility: &str, own_rules: &[String]) -> Result<Plan, String> {
    let wid = values.get("world").and_then(Value::as_str).unwrap_or("whole");
    let w = world(wid).ok_or("Pick a world.")?;
    let private_only = w["private_only"].as_bool().unwrap_or(false);
    let visibility = match visibility {
        "public" if private_only => return Err(format!("{} is private only: pick Private, or another world.", w["label"].as_str().unwrap_or(wid))),
        "public" | "private" => visibility.to_string(),
        _ => return Err("Pick Private or Public.".into()),
    };
    let mut p = Plan { profile: w["ruleset"].as_str().unwrap_or("main").to_string(), visibility, ..Default::default() };
    let mut rule_lines = Vec::new();
    for s in settings() {
        let k = s["key"].as_str().unwrap_or_default();
        let Some(v) = values.get(k) else { continue };
        if !applies(s, values) {
            continue;
        }
        match s["maps"].as_str() {
            Some("env") => {
                let text = match (s["kind"].as_str(), v) {
                    (Some("toggle"), Value::Bool(b)) => if *b { "1".into() } else { "0".into() },
                    (Some("range"), n) => {
                        let n = n.as_f64().ok_or(format!("{k}: a number"))? * s["scale"].as_f64().unwrap_or(1.0);
                        fmt(n)
                    }
                    (_, Value::String(x)) => x.clone(),
                    _ => return Err(format!("{k}: not a value it takes")),
                };
                for name in arr(s, "env").iter().filter_map(Value::as_str) {
                    if !modes::env_name_ok(name) || !modes::env_value_ok(&text) {
                        return Err(format!("{name} is not a setting the realm takes from here."));
                    }
                    p.env.insert(name.to_string(), text.clone());
                }
            }
            Some("option") => {
                let o = s["option"].as_str().unwrap_or(k);
                let ok = match o {
                    "type" => v.as_str().is_some_and(|t| TYPES.contains(&t)),
                    "bots" => v.as_i64().is_some_and(|n| (0..=200).contains(&n)),
                    "max" => v.as_i64().is_some_and(|n| (0..=10000).contains(&n)),
                    "signup" => v.as_str().is_some_and(|t| ["invite", "open", "off"].contains(&t)),
                    _ => false,
                };
                if !ok {
                    return Err(format!("{k}: not a value it takes"));
                }
                p.options.insert(o.to_string(), v.clone());
            }
            Some("rule") if v.as_bool() == Some(true) => rule_lines.push(s["rule"].as_str().unwrap_or_default().to_string()),
            Some("mods") => {
                let items: Vec<String> = match (k, v) {
                    ("minigolf", Value::Bool(true)) => arr(catalog(), "minigolf_mods").iter().filter_map(Value::as_str).map(String::from).collect(),
                    ("loadout", Value::String(id)) => arr(&catalog()["loadouts"], id).iter().filter_map(Value::as_str).map(String::from).collect(),
                    _ => vec![],
                };
                for m in items {
                    if modes::mod_ok(&m) && !p.mods.contains(&m) && p.mods.len() < MODS_MAX {
                        p.mods.push(m);
                    }
                }
            }
            _ => {}
        }
    }
    for l in own_rules.iter().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()) {
        if l.chars().count() > RULE_LEN || l.starts_with('#') || !plain(&l) {
            return Err("House rules: up to 120 characters each, no game names, not starting with #.".into());
        }
        if !rule_lines.contains(&l) {
            rule_lines.push(l);
        }
    }
    if rule_lines.len() > RULES_MAX {
        return Err("House rules: 10 lines at most (the rule switches take one each).".into());
    }
    p.rules = rule_lines;
    Ok(p)
}

/// An answer from an assistant (agents.rs) or pasted back from any AI chat.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Answer {
    pub template: String,
    pub values: Map<String, Value>,
    pub why: BTreeMap<String, String>,
    pub name: Option<String>,
    pub changed: Vec<String>,
    pub left_out: Vec<String>,
}

/// The first JSON object in a chat answer: a ```json block, or the outermost braces.
pub fn json_block(text: &str) -> Option<Value> {
    if text.len() > 64 * 1024 {
        return None;
    }
    let inner = text.split("```").enumerate().filter(|(i, _)| i % 2 == 1).map(|(_, b)| b.trim_start_matches("json").trim()).find(|b| b.starts_with('{'));
    let cand = match inner {
        Some(b) => b.to_string(),
        None => {
            let (s, e) = (text.find('{')?, text.rfind('}')?);
            if e <= s {
                return None;
            }
            text[s..=e].to_string()
        }
    };
    serde_json::from_str::<Value>(&cand).ok().filter(Value::is_object)
}

pub fn parse_answer(text: &str) -> Result<Answer, String> {
    let v = json_block(text).ok_or("No settings block in that answer: paste the whole reply, with its { } block.")?;
    answer_from_value(&v)
}

pub fn answer_from_value(v: &Value) -> Result<Answer, String> {
    let given = v.get("settings").and_then(Value::as_object).ok_or("The answer has no \"settings\" in it.")?;
    if given.len() > 64 {
        return Err("Too many settings in that answer.".into());
    }
    let tid = v.get("template").and_then(Value::as_str).filter(|t| template(t).is_some()).unwrap_or("classic");
    let mut a = Answer { template: tid.to_string(), ..Default::default() };
    if let Some(t) = v.get("template").and_then(Value::as_str).filter(|t| template(t).is_none()) {
        a.left_out.push(format!("template {}: not one of ours, so Classic levelling", t.chars().take(24).collect::<String>()));
    }
    let c = check_values(&template_values(tid)?, given, true);
    a.values = c.values;
    a.changed = c.changed;
    a.left_out.extend(c.left_out);
    for (k, w) in v.get("why").and_then(Value::as_object).into_iter().flatten().take(64) {
        let Some(line) = w.as_str().map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ")) else { continue };
        if setting(k).is_some() && !line.is_empty() && line.chars().count() <= WHY_LEN && plain(&line) && !line.chars().any(char::is_control) {
            a.why.insert(k.clone(), line);
        }
    }
    a.name = v.get("name").and_then(Value::as_str).map(str::trim).filter(|n| super::name_ok(n)).map(String::from);
    Ok(a)
}

/// The settings list as the assistant reads it (agents.rs host_plan_request): each key, what it takes, what it does.
pub fn spec() -> Value {
    let keys: Vec<Value> = settings().iter().filter(|s| s["kind"] != "coming" && !NOT_FROM_AI.contains(&s["key"].as_str().unwrap_or(""))).map(|s| {
        let mut o = json!({ "key": s["key"], "label": s["label"], "kind": s["kind"] });
        for f in ["min", "max", "step", "unit", "only", "needs", "help"] {
            if !s[f].is_null() {
                o[f] = s[f].clone();
            }
        }
        if let Some(opts) = s["options"].as_array() {
            o["options"] = Value::Array(opts.iter().map(|x| x[0].clone()).collect());
        }
        o
    }).collect();
    let templates: Vec<Value> = arr(catalog(), "templates").iter().map(|t| json!({ "id": t["id"], "name": t["name"], "pitch": t["pitch"] })).collect();
    let coming: Vec<Value> = settings().iter().filter(|s| s["kind"] == "coming").map(|s| s["label"].clone()).collect();
    json!({
        "settings": keys, "templates": templates, "not_yet": coming,
        "answer": { "template": "a template id", "settings": { "key": "value" }, "why": { "key": "one short line" }, "name": "optional realm name" },
        "rules": "Only the keys listed. Numbers inside their range. Who can join (Private or Public, sign-up) is the player's own choice: leave it out.",
    })
}

// ---------------------------------------------------------------------------------------------- apply it

fn state(kit: &Path) -> PathBuf {
    kit.join("state")
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Applied {
    /// The ruleset to start (host_choose).
    pub profile: String,
    pub visibility: String,
    /// What the realm list shows as its mode, or None (Blank: no mode).
    pub mode_name: Option<String>,
    /// Plain lines: what will happen.
    pub lines: Vec<String>,
}

/// Host this plan: the settings checked again, then state/mode.json (the world server's settings), state/options.json
/// (read once at the next start), the house rules and state/host-plan.json (for Edit settings). The page then picks the
/// returned ruleset and starts the realm (host_choose, host_start_quiet), as for a mode.
pub fn apply(kit: &Path, tid: &str, given: &Map<String, Value>, visibility: &str, own_rules: &[String]) -> Result<Applied, String> {
    let t = template(tid).ok_or("Pick a template.")?;
    let start = template_values(tid)?;
    let c = check_values(&start, given, false);
    if let Some(bad) = c.left_out.first() {
        return Err(format!("{bad}."));
    }
    let plan = resolve(&c.values, visibility, own_rules)?;
    let custom = c.values != start;
    let tname = t["name"].as_str().unwrap_or(tid);
    fs::create_dir_all(state(kit)).map_err(|e| e.to_string())?;
    let blank = t["blank"].as_bool().unwrap_or(false) && !custom;
    let mode_name = if blank {
        modes::clear(kit);
        None
    } else {
        let name: String = if custom { format!("Custom (from {tname})") } else { tname.to_string() };
        let name = name.chars().take(32).collect::<String>().trim_end().to_string();
        let id = format!("realm-{}", if custom { "custom" } else { tid });
        let doc = json!({
            "id": id, "name": name, "line": t["pitch"].as_str().unwrap_or("").chars().take(120).collect::<String>(), "author": "Host",
            "version": "1", "profile": plan.profile, "scripts": [], "env": {}, "mods": plan.mods, "tags": [], "settings": [],
            "resolved": plan.env, "host_page": { "template": tid, "custom": custom },
        });
        let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
        crate::settings::write_atomic(&modes::mode_file(kit), text.as_bytes()).map_err(|e| format!("Couldn't save the settings: {e}"))?;
        Some(name)
    };
    let opts = serde_json::to_string_pretty(&Value::Object(plan.options.clone())).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&state(kit).join(OPTIONS_FILE), opts.as_bytes()).map_err(|e| format!("Couldn't save the settings: {e}"))?;
    if !plan.rules.is_empty() {
        super::console::write_rules(kit, &plan.rules)?;
    }
    let saved = json!({ "template": tid, "values": c.values, "visibility": plan.visibility, "rules": own_rules });
    let text = serde_json::to_string_pretty(&saved).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&state(kit).join(PLAN_FILE), text.as_bytes()).map_err(|e| e.to_string())?;
    let mut lines = vec![format!("Ruleset: {}.", plan.profile)];
    if !plan.env.is_empty() {
        lines.push(format!("{} world server settings.", plan.env.len()));
    }
    if !plan.rules.is_empty() {
        lines.push(format!("{} house rule{}.", plan.rules.len(), if plan.rules.len() == 1 { "" } else { "s" }));
    }
    Ok(Applied { profile: plan.profile, visibility: plan.visibility, mode_name, lines })
}

/// The last plan hosted from this page (Edit settings), or None.
pub fn current(kit: &Path) -> Option<Value> {
    let t = fs::read_to_string(state(kit).join(PLAN_FILE)).ok()?;
    let v: Value = serde_json::from_str(t.trim_start_matches('\u{feff}')).ok()?;
    let tid = v.get("template").and_then(Value::as_str).filter(|t| template(t).is_some())?;
    let given = v.get("values").and_then(Value::as_object).cloned().unwrap_or_default();
    let c = check_values(&template_values(tid).ok()?, &given, false);
    Some(json!({ "template": tid, "values": c.values, "visibility": v.get("visibility"), "rules": v.get("rules") }))
}

// ---------------------------------------------------------------------------------------------- the assistant's side
// The page writes the host's words (`ask`), the player's assistant reads them (agents.rs host_plan_request) and, after
// the player approves its card, its answer lands here (`propose`); the page takes it (`take`) into the panel to review.

pub const ASK_FILE: &str = "host-plan-ask.json";
pub const ANSWER_FILE: &str = "host-plan-answer.json";
pub const ASK_MAX: usize = 600;

pub fn ask(agents_dir: &Path, text: &str) -> Result<(), String> {
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() || t.chars().count() > ASK_MAX {
        return Err("Say what you want in up to 600 letters.".into());
    }
    fs::create_dir_all(agents_dir).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(agents_dir.join(ANSWER_FILE));
    let doc = json!({ "text": t, "at": crate::agents::now() });
    crate::settings::write_atomic(&agents_dir.join(ASK_FILE), doc.to_string().as_bytes()).map_err(|e| e.to_string())
}

pub fn request(agents_dir: &Path) -> Value {
    let asked = fs::read_to_string(agents_dir.join(ASK_FILE)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
    json!({ "request": asked.as_ref().and_then(|a| a.get("text")).cloned().unwrap_or(Value::Null), "catalog": spec() })
}

pub fn propose(agents_dir: &Path, v: &Value) -> Result<Answer, String> {
    let a = answer_from_value(v)?;
    let text = serde_json::to_string(&a).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&agents_dir.join(ANSWER_FILE), text.as_bytes()).map_err(|e| e.to_string())?;
    Ok(a)
}

/// The assistant's answer, once (checked again), or None.
pub fn take(agents_dir: &Path) -> Option<Value> {
    let p = agents_dir.join(ANSWER_FILE);
    let t = fs::read_to_string(&p).ok()?;
    let _ = fs::remove_file(&p);
    let v: Value = serde_json::from_str(&t).ok()?;
    // the file holds an Answer; check its settings again as an answer would be
    let a = answer_from_value(&json!({ "template": v.get("template"), "settings": v.get("values"), "why": v.get("why"), "name": v.get("name") })).ok()?;
    serde_json::to_value(a).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(v: Value) -> Map<String, Value> {
        v.as_object().cloned().unwrap()
    }

    #[test]
    fn the_catalog_parses_and_every_template_is_whole() {
        let c = catalog();
        assert!(c.is_object(), "ui/hostflow-catalog.js parses");
        let ids: Vec<&str> = arr(c, "templates").iter().filter_map(|t| t["id"].as_str()).collect();
        assert_eq!(ids, ["pure-vanilla", "vanilla-plus", "original", "classic", "fun", "rp", "hardcore", "solo-bots", "pvp", "event-night", "blank"]);
        let base = c["base"].as_object().unwrap();
        for s in settings() {
            let k = s["key"].as_str().unwrap();
            assert_eq!(s["kind"] == "coming", !base.contains_key(k), "{k}: the base sets every real setting, never a coming one");
            for name in arr(s, "env").iter().filter_map(Value::as_str) {
                assert!(modes::env_name_ok(name), "{name}: a name a mode may set (setup.py MODE_ENV_RE)");
            }
        }
        for id in ids {
            let v = template_values(id).unwrap();
            let c = check_values(&v, &v, false);
            assert!(c.changed.is_empty() && c.left_out.is_empty(), "{id}: its own values are in range: {c:?}");
            let t = template(id).unwrap();
            let vis = if v["world"] == "solo" { "private" } else { "public" };
            resolve(&v, vis, &[]).unwrap_or_else(|e| panic!("{id}: {e}"));
            for f in ["name", "pitch", "best_for"] {
                assert!(t[f].as_str().is_some_and(|s| !s.is_empty() && plain(s)), "{id}.{f}");
            }
        }
        // the loadouts match the website's (stream/loadouts.json)
        let site: Value = serde_json::from_str(include_str!("../../../../stream/loadouts.json")).unwrap();
        for (id, items) in c["loadouts"].as_object().unwrap() {
            if id == "none" {
                continue;
            }
            let t = arr(&site, "templates").iter().find(|t| t["id"] == id.as_str()).unwrap_or_else(|| panic!("{id} on the site"));
            assert_eq!(&t["items"], items, "{id}");
        }
    }

    #[test]
    fn templates_map_to_the_realm_settings() {
        // Pure vanilla: the first era's ruleset, 1x, trainers, no bots, no scaling
        let p = resolve(&template_values("pure-vanilla").unwrap(), "public", &[]).unwrap();
        assert_eq!(p.profile, "vanillaplus");
        for k in ["AC_RATE_XP_KILL", "AC_RATE_XP_QUEST", "AC_RATE_XP_QUEST_DF", "AC_RATE_XP_EXPLORE", "AC_RATE_DROP_MONEY", "AC_RATE_HONOR"] {
            assert_eq!(p.env[k], "1", "{k}");
        }
        assert_eq!((p.env["OV_AUTOLEARN"].as_str(), p.env["AC_AUTO_BALANCE_ENABLE_GLOBAL"].as_str()), ("0", "0"));
        assert!(!p.env.contains_key("OV_MAIN") && !p.env.contains_key("OV_START_MOUNT"), "no boost on the first era");
        assert_eq!(p.options["bots"], 0);
        assert_eq!(p.options["type"], "normal");
        // Vanilla+: 1.5x, Quester suggested
        let p = resolve(&template_values("vanilla-plus").unwrap(), "private", &[]).unwrap();
        assert_eq!((p.profile.as_str(), p.env["AC_RATE_XP_KILL"].as_str(), p.env["AC_RATE_REPUTATION_GAIN"].as_str()), ("vanillaplus", "1.5", "1.5"));
        assert_eq!(p.mods, ["up:questie", "up:gathermate", "up:idtip", "up:ratingbuster"]);
        // Fun: 10x, the boost, the mount, 100 gold to start (in copper)
        let p = resolve(&template_values("fun").unwrap(), "public", &[]).unwrap();
        assert_eq!((p.profile.as_str(), p.env["AC_RATE_XP_KILL"].as_str(), p.env["OV_MAIN"].as_str(), p.env["OV_START_MOUNT"].as_str()), ("main", "10", "1", "1"));
        assert_eq!(p.env["AC_START_PLAYER_MONEY"], "1000000");
        assert_eq!(p.options["bots"], 60);
        // RP: realm type rp and its naming rule in the house rules
        let p = resolve(&template_values("rp").unwrap(), "public", &["Be kind.".into()]).unwrap();
        assert_eq!(p.options["type"], "rp");
        assert_eq!(p.rules.len(), 2);
        assert!(p.rules[0].starts_with("Role-play realm") && p.rules[1] == "Be kind.");
        // Solo with bots: private only, helper bots on the solo ruleset
        let v = template_values("solo-bots").unwrap();
        assert!(resolve(&v, "public", &[]).unwrap_err().contains("private only"));
        let p = resolve(&v, "private", &[]).unwrap();
        assert_eq!((p.profile.as_str(), p.env["OV_COMPANIONS_PER_PLAYER"].as_str()), ("solo", "4"));
        // PvP: everyone at 80, PvP type, the ladder
        let p = resolve(&template_values("pvp").unwrap(), "public", &[]).unwrap();
        assert_eq!((p.profile.as_str(), p.options["type"].as_str(), p.env["OV_DUELS"].as_str()), ("brawl", Some("pvp"), "1"));
        assert!(!p.env.contains_key("OV_COMPANIONS_PER_PLAYER"), "helpers only on the solo world");
        // Event night: the hangout (no bots there: none written) and the game modes
        let p = resolve(&template_values("event-night").unwrap(), "public", &[]).unwrap();
        assert_eq!(p.profile, "dalaran");
        assert!(!p.options.contains_key("bots"), "the hangout runs no bots (setup.py NO_BOTS)");
        assert_eq!(p.mods, ["mode:minigolf", "mode:bumper-balls", "mode:coin-dash"]);
        // a setting that needs another: the mount only with the boost
        let mut v = template_values("classic").unwrap();
        v.insert("boost".into(), json!(false));
        let p = resolve(&v, "public", &[]).unwrap();
        assert_eq!(p.env["OV_MAIN"], "0");
        assert!(!p.env.contains_key("OV_START_MOUNT"));
        // every env name and value is one setup.py takes (MODE_ENV_RE, MODE_VALUE_RE), and at most 48 of them
        for id in ["pure-vanilla", "vanilla-plus", "original", "classic", "fun", "rp", "hardcore", "pvp", "event-night", "blank"] {
            let p = resolve(&template_values(id).unwrap(), "public", &[]).unwrap();
            assert!(p.env.len() <= 48, "{id}");
            assert!(p.env.iter().all(|(k, v)| modes::env_name_ok(k) && modes::env_value_ok(v)), "{id}");
        }
    }

    #[test]
    fn ai_answers_are_checked_clamped_and_never_choose_who_joins() {
        let text = "Here you go!\n```json\n{\"template\": \"rp\", \"settings\": {\"xp\": 50, \"rep\": \"2\", \"weather_magic\": 3, \"signup\": \"open\", \
            \"realm_type\": \"rp\", \"bots\": 7, \"boost\": \"off\", \"start_level\": 20, \"realm_type2\": 1}, \"why\": {\"xp\": \"Fast, as you asked.\", \
            \"rep\": \"Warcraft style\", \"nope\": \"x\"}, \"name\": \"Cozy Hearth\"}\n```\nEnjoy.";
        let a = parse_answer(text).unwrap();
        assert_eq!(a.template, "rp");
        assert_eq!(a.values["xp"], 10, "clamped to its range");
        assert_eq!(a.values["rep"], 2, "a number in a string");
        assert_eq!(a.values["bots"], 5, "snapped to its step of 5");
        assert_eq!(a.values["boost"], false);
        assert_eq!(a.values["signup"], template_values("rp").unwrap()["signup"], "never from an assistant");
        assert!(a.changed.iter().any(|c| c.starts_with("Experience: 50 to 10")), "{:?}", a.changed);
        for k in ["weather_magic", "signup", "Any start level", "realm_type2"] {
            assert!(a.left_out.iter().any(|l| l.starts_with(k)), "{k}: {:?}", a.left_out);
        }
        assert_eq!(a.why.keys().collect::<Vec<_>>(), ["xp"], "known keys, no game names");
        assert_eq!(a.name.as_deref(), Some("Cozy Hearth"));
        // braces without a fence; an unknown template falls back
        let a = parse_answer("ok {\"template\": \"mmo-x\", \"settings\": {\"world\": \"first\"}}").unwrap();
        assert_eq!((a.template.as_str(), a.values["world"].as_str()), ("classic", Some("first")));
        assert!(a.left_out[0].starts_with("template mmo-x"));
        assert!(parse_answer("no json here").is_err());
        assert!(parse_answer("{\"x\": 1}").unwrap_err().contains("settings"));
        assert!(parse_answer("{\"settings\": {\"world\": \"mars\"}}").unwrap().left_out[0].contains("not one of its choices"));
        assert!(answer_from_value(&json!({ "settings": { "name": "Warcraft Land" } })).unwrap().name.is_none());
    }

    #[test]
    fn apply_writes_the_kits_files_and_blank_writes_no_mode() {
        let k = tempfile::tempdir().unwrap();
        let a = apply(k.path(), "fun", &obj(json!({ "xp": 7 })), "public", &["Have fun.".into()]).unwrap();
        assert_eq!((a.profile.as_str(), a.mode_name.as_deref()), ("main", Some("Custom (from Fun realm)")));
        let m: Value = serde_json::from_str(&fs::read_to_string(modes::mode_file(k.path())).unwrap()).unwrap();
        assert_eq!((m["id"].as_str(), m["profile"].as_str(), m["resolved"]["AC_RATE_XP_KILL"].as_str()), (Some("realm-custom"), Some("main"), Some("7")));
        let o: Value = serde_json::from_str(&fs::read_to_string(k.path().join("state").join(OPTIONS_FILE)).unwrap()).unwrap();
        assert_eq!(o, json!({ "bots": 60, "max": 0, "signup": "invite", "type": "normal" }));
        assert_eq!(super::super::console::rules(k.path()), ["Have fun."]);
        let cur = current(k.path()).unwrap();
        assert_eq!((cur["template"].as_str(), cur["values"]["xp"].as_i64()), (Some("fun"), Some(7)));
        // Blank, unchanged: no mode at all (the ruleset as it ships)
        let a = apply(k.path(), "blank", &Map::new(), "private", &[]).unwrap();
        assert_eq!(a.mode_name, None);
        assert!(!modes::mode_file(k.path()).exists());
        // a bad value never writes
        assert!(apply(k.path(), "fun", &obj(json!({ "lasers": true })), "public", &[]).is_err());
        assert!(apply(k.path(), "solo-bots", &Map::new(), "public", &[]).is_err());
        assert!(apply(k.path(), "fun", &Map::new(), "public", &["Visit Warcraft".into()]).is_err());
    }

    #[test]
    fn the_assistant_round_trip() {
        let d = tempfile::tempdir().unwrap();
        assert!(ask(d.path(), "  ").is_err());
        ask(d.path(), "a cozy RP realm for 10 friends,   slow levelling, no PvP").unwrap();
        let r = request(d.path());
        assert_eq!(r["request"], "a cozy RP realm for 10 friends, slow levelling, no PvP");
        let keys: Vec<&str> = arr(&r["catalog"], "settings").iter().filter_map(|s| s["key"].as_str()).collect();
        assert!(keys.contains(&"xp") && !keys.contains(&"signup") && !keys.contains(&"weather"));
        propose(d.path(), &json!({ "template": "rp", "settings": { "xp": 1, "max_players": 10 }, "why": { "xp": "Slow levelling." } })).unwrap();
        let t = take(d.path()).unwrap();
        assert_eq!((t["template"].as_str(), t["values"]["max_players"].as_i64(), t["why"]["xp"].as_str()), (Some("rp"), Some(10), Some("Slow levelling.")));
        assert!(take(d.path()).is_none(), "taken once");
    }
}
