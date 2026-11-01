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
//! ```

use rmcp::model::Tool;
use serde_json::Value;

use crate::doctor::MAX_TOOLS;

/// Check rmcp [`Tool`]s (e.g. `ToolRouter::list_all()`). `Err` lists every problem found.
pub fn check_tools(tools: &[Tool]) -> Result<(), Vec<String>> {
    let json = serde_json::to_value(tools)
        .map_err(|e| vec![format!("tools do not serialize to JSON: {e}")])?;
    check_tools_json(&json)
}

/// Check a `tools/list` answer: the `result` object (`{"tools": [...]}`), the whole JSON-RPC
/// reply (`{"result": {"tools": [...]}}`), or the bare tools array. `Err` lists every problem.
pub fn check_tools_json(value: &Value) -> Result<(), Vec<String>> {
    let tools = value
        .pointer("/result/tools")
        .or_else(|| value.get("tools"))
        .unwrap_or(value);
    let Some(tools) = tools.as_array() else {
        return Err(vec!["no tools array found".into()]);
    };

    let mut problems = Vec::new();
    if tools.is_empty() || tools.len() > MAX_TOOLS {
        problems.push(format!(
            "{} tools; a server exposes 1 to {MAX_TOOLS}",
            tools.len()
        ));
    }
    for (i, tool) in tools.iter().enumerate() {
        let name = tool
            .get("name")
            .and_then(Value::as_str)
            .map_or_else(|| format!("tool #{i}"), |n| format!("tool `{n}`"));
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
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

/// [`check_tools`], panicking with every problem found.
#[track_caller]
pub fn assert_tools(tools: &[Tool]) {
    if let Err(problems) = check_tools(tools) {
        panic!(
            "tool list does not conform:\n  - {}",
            problems.join("\n  - ")
        );
    }
}

/// [`check_tools_json`], panicking with every problem found.
#[track_caller]
pub fn assert_tools_json(value: &Value) {
    if let Err(problems) = check_tools_json(value) {
        panic!(
            "tool list does not conform:\n  - {}",
            problems.join("\n  - ")
        );
    }
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
    #[should_panic(expected = "no annotations")]
    fn assert_panics_with_the_problem() {
        assert_tools_json(&json!([{ "name": "z", "description": "d",
            "inputSchema": { "type": "object" } }]));
    }
}
