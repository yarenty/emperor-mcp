//! Conformance checks a server runs in its own tests (feature `testkit`).
//!
//! The checks hold a server's tool list to the authoring rules (`MCP_REQUIREMENTS.md`):
//!
//! - between 1 and [`MAX_TOOLS`] tools;
//! - every tool has a non-empty description;
//! - every tool has annotations with `readOnlyHint` set explicitly, and a tool that is not
//!   read-only also sets `destructiveHint` explicitly;
//! - every input schema is a JSON object schema (`"type": "object"`).
//!
//! Tool descriptions longer than [`MAX_DESCRIPTION_CHARS`] are a warning, not a failure:
//! [`warnings`] / [`warnings_json`] list them, and the `assert_tools*` forms print them to stderr.
//!
//! Two more checks a server opts into:
//!
//! - [`check_read_only_default`]: the tool list of a server started without any switch offers
//!   only read-only tools (`readOnlyHint: true`); writing is switched on by the operator;
//! - [`check_compact_schemas`]: input schemas carry no `$schema` and no `null` alternative on
//!   optional properties, i.e. they went through [`crate::schema::compact`].
//!
//! Two entry points, one rule set: [`check_tools`] for the `Vec<Tool>` of an rmcp tool router,
//! [`check_tools_json`] for the JSON a server answers to `tools/list` (any server, including the
//! hand-rolled [`crate::McpHandler`] path). The `assert_*` forms panic with every problem found.
//!
//! ```ignore
//! // In the server's tests/conformance.rs, with emperor-mcp's `testkit` feature in
//! // [dev-dependencies]:
//! #[test]
//! fn tools_conform() {
//!     emperor_mcp::testkit::assert_tools(&MyServer::tool_router().list_all());
//! }
//!
//! #[test]
//! fn default_server_is_read_only_and_lean() {
//!     let tools = MyServer::new(Config::default()).tool_router.list_all();
//!     emperor_mcp::testkit::assert_read_only_default(&tools);
//!     emperor_mcp::testkit::assert_compact_schemas(&tools);
//! }
//! ```

use rmcp::model::Tool;
use serde_json::Value;

use crate::doctor::MAX_TOOLS;

/// Soft limit on a tool description, in characters: say what the tool does and when to use it
/// rather than another tool. Longer descriptions are a warning ([`warnings`]), not a failure.
pub const MAX_DESCRIPTION_CHARS: usize = 300;

/// The tools array of a `tools/list` answer in any of the accepted shapes.
fn tools_array(value: &Value) -> Result<&Vec<Value>, Vec<String>> {
    value
        .pointer("/result/tools")
        .or_else(|| value.get("tools"))
        .unwrap_or(value)
        .as_array()
        .ok_or_else(|| vec!["no tools array found".to_string()])
}

/// `tool `name`` (or `tool #i` without a name) for messages.
fn label(i: usize, tool: &Value) -> String {
    tool.get("name")
        .and_then(Value::as_str)
        .map_or_else(|| format!("tool #{i}"), |n| format!("tool `{n}`"))
}

fn to_json(tools: &[Tool]) -> Result<Value, Vec<String>> {
    serde_json::to_value(tools).map_err(|e| vec![format!("tools do not serialize to JSON: {e}")])
}

fn verdict(problems: Vec<String>) -> Result<(), Vec<String>> {
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

#[track_caller]
fn fail_on(result: Result<(), Vec<String>>, what: &str) {
    if let Err(problems) = result {
        panic!("{what}:\n  - {}", problems.join("\n  - "));
    }
}

fn print_warnings(warnings: &[String]) {
    for w in warnings {
        eprintln!("emperor-mcp testkit warning: {w}");
    }
}

/// Check rmcp [`Tool`]s (e.g. `ToolRouter::list_all()`). `Err` lists every problem found.
pub fn check_tools(tools: &[Tool]) -> Result<(), Vec<String>> {
    check_tools_json(&to_json(tools)?)
}

/// Check a `tools/list` answer: the `result` object (`{"tools": [...]}`), the whole JSON-RPC
/// reply (`{"result": {"tools": [...]}}`), or the bare tools array. `Err` lists every problem.
pub fn check_tools_json(value: &Value) -> Result<(), Vec<String>> {
    let tools = tools_array(value)?;
    let mut problems = Vec::new();
    if tools.is_empty() || tools.len() > MAX_TOOLS {
        problems.push(format!(
            "{} tools; a server exposes 1 to {MAX_TOOLS}",
            tools.len()
        ));
    }
    for (i, tool) in tools.iter().enumerate() {
        let name = label(i, tool);
        if tool
            .get("name")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            problems.push(format!("{name}: missing name"));
        }
        if tool
            .get("description")
            .and_then(Value::as_str)
            .is_none_or(|d| d.trim().is_empty())
        {
            problems.push(format!("{name}: missing or empty description"));
        }
        match tool.get("annotations") {
            Some(a) if a.is_object() => match a.get("readOnlyHint").and_then(Value::as_bool) {
                None => problems.push(format!("{name}: annotations.readOnlyHint is not set")),
                Some(false) if a.get("destructiveHint").and_then(Value::as_bool).is_none() => {
                    problems.push(format!(
                        "{name}: not read-only, so annotations.destructiveHint must be set"
                    ))
                }
                Some(_) => {}
            },
            _ => problems.push(format!("{name}: no annotations")),
        }
        if tool.pointer("/inputSchema/type").and_then(Value::as_str) != Some("object") {
            problems.push(format!("{name}: inputSchema is not an object schema"));
        }
    }
    verdict(problems)
}

/// [`check_tools`], panicking with every problem found. Prints [`warnings`] to stderr.
#[track_caller]
pub fn assert_tools(tools: &[Tool]) {
    print_warnings(&warnings(tools));
    fail_on(check_tools(tools), "tool list does not conform");
}

/// [`check_tools_json`], panicking with every problem found. Prints [`warnings_json`] to stderr.
#[track_caller]
pub fn assert_tools_json(value: &Value) {
    print_warnings(&warnings_json(value));
    fail_on(check_tools_json(value), "tool list does not conform");
}

/// Soft findings that do not fail a check: tool descriptions longer than
/// [`MAX_DESCRIPTION_CHARS`].
pub fn warnings(tools: &[Tool]) -> Vec<String> {
    to_json(tools).map_or_else(|e| e, |json| warnings_json(&json))
}

/// [`warnings`] for a `tools/list` answer (any shape [`check_tools_json`] takes).
pub fn warnings_json(value: &Value) -> Vec<String> {
    let Ok(tools) = tools_array(value) else {
        return Vec::new();
    };
    tools
        .iter()
        .enumerate()
        .filter_map(|(i, tool)| {
            let chars = tool.get("description")?.as_str()?.chars().count();
            (chars > MAX_DESCRIPTION_CHARS).then(|| {
                format!(
                    "{}: description is {chars} characters; keep it under {MAX_DESCRIPTION_CHARS}: what the tool does and when to use it",
                    label(i, tool)
                )
            })
        })
        .collect()
}

/// Check the tool list of a server started with no switch: every tool is read-only
/// (`readOnlyHint: true`). A server offers a tool that changes anything only when the operator
/// switched writing on at start-up (a flag plus an environment variable).
pub fn check_read_only_default(tools: &[Tool]) -> Result<(), Vec<String>> {
    check_read_only_default_json(&to_json(tools)?)
}

/// [`check_read_only_default`] for a `tools/list` answer (any shape [`check_tools_json`] takes).
pub fn check_read_only_default_json(value: &Value) -> Result<(), Vec<String>> {
    let tools = tools_array(value)?;
    let problems = tools
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            t.pointer("/annotations/readOnlyHint")
                .and_then(Value::as_bool)
                != Some(true)
        })
        .map(|(i, t)| {
            format!(
                "{}: offered without a write switch but not read-only (readOnlyHint is not true)",
                label(i, t)
            )
        })
        .collect();
    verdict(problems)
}

/// [`check_read_only_default`], panicking with every problem found.
#[track_caller]
pub fn assert_read_only_default(tools: &[Tool]) {
    fail_on(check_read_only_default(tools), "default tool list writes");
}

/// [`check_read_only_default_json`], panicking with every problem found.
#[track_caller]
pub fn assert_read_only_default_json(value: &Value) {
    fail_on(
        check_read_only_default_json(value),
        "default tool list writes",
    );
}

/// Check that input schemas are compact ([`crate::schema::compact`]): no `$schema`, and no
/// `null` alternative (`"type": [T, "null"]`, a `{"type": "null"}` branch) on an optional
/// property, at any depth.
pub fn check_compact_schemas(tools: &[Tool]) -> Result<(), Vec<String>> {
    check_compact_schemas_json(&to_json(tools)?)
}

/// [`check_compact_schemas`] for a `tools/list` answer (any shape [`check_tools_json`] takes).
pub fn check_compact_schemas_json(value: &Value) -> Result<(), Vec<String>> {
    let tools = tools_array(value)?;
    let mut problems = Vec::new();
    for (i, tool) in tools.iter().enumerate() {
        let name = label(i, tool);
        let Some(schema) = tool.get("inputSchema").and_then(Value::as_object) else {
            continue;
        };
        if schema.contains_key("$schema") {
            problems.push(format!("{name}: inputSchema carries $schema"));
        }
        nullable_optionals(schema, "", &mut |path| {
            problems.push(format!("{name}: optional `{path}` also allows null"));
        });
    }
    verdict(problems)
}

/// [`check_compact_schemas`], panicking with every problem found.
#[track_caller]
pub fn assert_compact_schemas(tools: &[Tool]) {
    fail_on(
        check_compact_schemas(tools),
        "input schemas are not compact",
    );
}

/// [`check_compact_schemas_json`], panicking with every problem found.
#[track_caller]
pub fn assert_compact_schemas_json(value: &Value) {
    fail_on(
        check_compact_schemas_json(value),
        "input schemas are not compact",
    );
}

/// Report every optional property (dotted path) whose schema also allows `null`.
fn nullable_optionals(
    schema: &serde_json::Map<String, Value>,
    prefix: &str,
    report: &mut dyn FnMut(String),
) {
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        for (key, prop) in props {
            let Some(prop) = prop.as_object() else {
                continue;
            };
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            if !required.contains(&key.as_str()) && allows_null(prop) {
                report(path.clone());
            }
            nullable_optionals(prop, &path, report);
        }
    }
    if let Some(items) = schema.get("items").and_then(Value::as_object) {
        nullable_optionals(items, &format!("{prefix}[]"), report);
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        for sub in schema
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
        {
            nullable_optionals(sub, prefix, report);
        }
    }
}

fn allows_null(prop: &serde_json::Map<String, Value>) -> bool {
    let null_type = |s: &serde_json::Map<String, Value>| match s.get("type") {
        Some(Value::String(t)) => t == "null",
        Some(Value::Array(ts)) => ts.iter().any(|t| t == "null"),
        _ => false,
    };
    null_type(prop)
        || ["anyOf", "oneOf"].iter().any(|k| {
            prop.get(*k)
                .and_then(Value::as_array)
                .is_some_and(|bs| bs.iter().filter_map(Value::as_object).any(null_type))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn good(name: &str) -> Value {
        json!({
            "name": name,
            "description": "Does one thing.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true }
        })
    }

    #[test]
    fn accepts_conforming_lists_in_every_shape() {
        let tools = json!([good("a"), good("b")]);
        assert_eq!(check_tools_json(&tools), Ok(()));
        assert_eq!(check_tools_json(&json!({ "tools": tools })), Ok(()));
        assert_eq!(
            check_tools_json(&json!({ "jsonrpc": "2.0", "id": 1, "result": { "tools": tools } })),
            Ok(())
        );
    }

    #[test]
    fn writing_tool_needs_destructive_hint() {
        let mut t = good("send");
        t["annotations"] = json!({ "readOnlyHint": false });
        let err = check_tools_json(&json!([t.clone()])).unwrap_err();
        assert!(err[0].contains("destructiveHint"), "{err:?}");
        t["annotations"]["destructiveHint"] = json!(false);
        assert_eq!(check_tools_json(&json!([t])), Ok(()));
    }

    #[test]
    fn reports_every_problem() {
        let bad = json!({ "name": "x", "description": " ", "inputSchema": { "type": "string" } });
        let err = check_tools_json(&json!([bad])).unwrap_err();
        assert_eq!(err.len(), 3, "{err:?}");
        let empty = check_tools_json(&json!([])).unwrap_err();
        assert!(empty[0].contains("0 tools"), "{empty:?}");
        let many: Vec<Value> = (0..=MAX_TOOLS).map(|i| good(&format!("t{i}"))).collect();
        assert!(check_tools_json(&Value::Array(many)).is_err());
        assert!(check_tools_json(&json!({ "nothing": 1 })).is_err());
        let no_ro = json!([{ "name": "y", "description": "d",
            "inputSchema": { "type": "object" }, "annotations": { "title": "Y" } }]);
        assert!(check_tools_json(&no_ro).unwrap_err()[0].contains("readOnlyHint"));
    }

    #[test]
    fn default_list_must_be_read_only() {
        assert_eq!(check_read_only_default_json(&json!([good("a")])), Ok(()));
        let mut w = good("write");
        w["annotations"] = json!({ "readOnlyHint": false, "destructiveHint": true });
        let mut unset = good("unset");
        unset["annotations"] = json!({});
        let err =
            check_read_only_default_json(&json!({ "tools": [good("a"), w, unset] })).unwrap_err();
        assert_eq!(err.len(), 2, "{err:?}");
        assert!(
            err[0].contains("`write`") && err[1].contains("`unset`"),
            "{err:?}"
        );
    }

    #[test]
    #[should_panic(expected = "default tool list writes")]
    fn read_only_default_assert_panics() {
        let mut w = good("send");
        w["annotations"] = json!({ "readOnlyHint": false, "destructiveHint": false });
        assert_read_only_default_json(&json!([w]));
    }

    #[test]
    fn compact_schema_check() {
        let mut lean = good("lean");
        lean["inputSchema"] = json!({ "type": "object",
            "properties": { "n": { "type": "integer" }, "r": { "type": ["string", "null"] } },
            "required": ["r"] });
        assert_eq!(check_compact_schemas_json(&json!([lean])), Ok(()));

        let mut fat = good("fat");
        fat["inputSchema"] = json!({ "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object", "properties": {
                "n": { "type": ["integer", "null"] },
                "p": { "anyOf": [ { "type": "string" }, { "type": "null" } ] },
                "inputs": { "type": "array", "items": { "type": "object",
                    "properties": { "pages": { "type": ["string", "null"] } } } } } });
        let err = check_compact_schemas_json(&json!([fat.clone()])).unwrap_err();
        assert_eq!(err.len(), 4, "{err:?}");
        assert!(err.iter().any(|e| e.contains("inputs[].pages")), "{err:?}");

        let mut tool: Tool = serde_json::from_value(fat).unwrap();
        crate::schema::compact_tool(&mut tool);
        assert_compact_schemas(&[tool]);
    }

    #[test]
    fn long_descriptions_warn_without_failing() {
        let mut long = good("long");
        long["description"] = json!("x".repeat(MAX_DESCRIPTION_CHARS + 1));
        let list = json!([good("short"), long]);
        assert_eq!(check_tools_json(&list), Ok(()));
        let w = warnings_json(&list);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].contains("`long`") && w[0].contains("301"), "{w:?}");
        assert_tools_json(&list);
        assert!(warnings_json(&json!([good("é")])).is_empty());
    }

    #[test]
    #[should_panic(expected = "no annotations")]
    fn assert_panics_with_the_problem() {
        assert_tools_json(&json!([{ "name": "z", "description": "d",
            "inputSchema": { "type": "object" } }]));
    }
}
