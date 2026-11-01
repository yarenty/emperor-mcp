//! `schema::compact` on real `tools/list` answers (fixtures dumped from four servers built on
//! emperor-mcp: a folder, a PDF, a mail and a task server, every tool switched on), and the
//! one-line opt-in on a `#[tool_router]` server.

use emperor_mcp::schema::{CompactSchemas, compact_list};
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::ListToolsResult;
use rmcp::{schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::Value;

const SERVERS: [&str; 4] = ["folder", "pdf", "mail", "vikunja"];

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/tools/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn wire_len(value: &impl serde::Serialize) -> usize {
    serde_json::to_string(value).unwrap().len()
}

#[test]
fn real_tool_lists_compact_to_the_expected_fixtures() {
    for server in SERVERS {
        let mut list: ListToolsResult = serde_json::from_str(&fixture(&format!("{server}.json")))
            .unwrap_or_else(|e| panic!("{server}: {e}"));
        let before = wire_len(&list);
        compact_list(&mut list);
        let after = wire_len(&list);
        let expected: Value =
            serde_json::from_str(&fixture(&format!("{server}.compact.json"))).unwrap();
        assert_eq!(
            serde_json::to_value(&list).unwrap(),
            expected,
            "{server}: compacted list differs from the fixture"
        );
        assert!(after < before, "{server}: {before} -> {after} bytes");
        let wire = serde_json::to_string(&list).unwrap();
        for boilerplate in [
            "$schema",
            "\"null\"",
            "\"default\":null",
            "\"format\":\"uint",
        ] {
            assert!(!wire.contains(boilerplate), "{server}: {boilerplate} left");
        }
        // Compacting again changes nothing.
        let once = serde_json::to_value(&list).unwrap();
        compact_list(&mut list);
        assert_eq!(serde_json::to_value(&list).unwrap(), once, "{server}");
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ReadArgs {
    /// File to read.
    path: String,
    /// First line to return, 1-based.
    #[serde(default)]
    start_line: Option<u32>,
    /// How to read it.
    #[serde(default)]
    mode: Option<Mode>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Mode {
    /// As text.
    Text,
    /// As bytes.
    Raw,
}

struct ReadServer {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl ReadServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router().compact_schemas(),
        }
    }

    /// Read a file.
    #[tool(annotations(read_only_hint = true))]
    async fn read(&self, Parameters(args): Parameters<ReadArgs>) -> String {
        args.path
    }
}

#[test]
fn router_opts_in_with_one_line() {
    let raw = serde_json::to_string(&ReadServer::tool_router().list_all()).unwrap();
    assert!(raw.contains("$schema") && raw.contains("\"null\""), "{raw}");

    let tools = ReadServer::new().tool_router.list_all();
    let schema = serde_json::to_value(&*tools[0].input_schema).unwrap();
    assert!(schema.get("$schema").is_none(), "{schema}");
    assert!(schema.get("$defs").is_none(), "{schema}");
    assert_eq!(schema["properties"]["start_line"]["type"], "integer");
    assert!(schema["properties"]["start_line"].get("format").is_none());
    let mode = &schema["properties"]["mode"];
    assert!(
        mode.get("oneOf").is_some() || mode.get("enum").is_some(),
        "mode keeps its choices: {mode}"
    );
    assert!(!mode.to_string().contains("null"), "{mode}");
    assert_eq!(schema["required"], serde_json::json!(["path"]));

    // What the compact schema still allows, the arguments accept: optional means absent.
    let args: ReadArgs = serde_json::from_value(serde_json::json!({ "path": "a" })).unwrap();
    assert!(args.start_line.is_none() && args.mode.is_none());

    #[cfg(feature = "testkit")]
    {
        emperor_mcp::testkit::assert_compact_schemas(&tools);
        assert!(
            emperor_mcp::testkit::check_compact_schemas(&ReadServer::tool_router().list_all())
                .is_err()
        );
    }
}
