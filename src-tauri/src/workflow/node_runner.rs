use crate::db::models::FlowNodeDefinition;

use super::{caption_node, clip_node, record_node, start_node, EngineNodeResult};

pub fn next_node_key(current: &str) -> Option<&'static str> {
    match current {
        "start" => Some("record"),
        "record" => Some("clip"),
        "clip" => Some("caption"),
        "caption" => Some("upload"),
        _ => None,
    }
}

pub fn run_node(
    definition: &FlowNodeDefinition,
    input_json: Option<&str>,
) -> Result<EngineNodeResult, String> {
    match definition.node_key.as_str() {
        "start" => start_node::run(definition, input_json),
        "record" => record_node::run(definition, input_json),
        "clip" => clip_node::run(definition, input_json),
        "caption" => caption_node::run(definition, input_json),
        "upload" => Ok(EngineNodeResult {
            status: "skipped".to_string(),
            output_json: input_json.map(|x| x.to_string()),
            error: None,
            next_node: None,
        }),
        other => Err(format!("unsupported node_key: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(node_key: &str) -> FlowNodeDefinition {
        let published_config_json = match node_key {
            "start" => r#"{"username":"shop_abc"}"#,
            "record" => r#"{"max_duration_minutes":5}"#,
            _ => "{}",
        };

        FlowNodeDefinition {
            id: 1,
            flow_id: 1,
            node_key: node_key.to_string(),
            position: 1,
            draft_config_json: "{}".to_string(),
            published_config_json: published_config_json.to_string(),
            draft_updated_at: "t".to_string(),
            published_at: "t".to_string(),
        }
    }

    #[test]
    fn next_node_order_matches_fixed_pipeline() {
        assert_eq!(next_node_key("start"), Some("record"));
        assert_eq!(next_node_key("record"), Some("clip"));
        assert_eq!(next_node_key("clip"), Some("caption"));
        assert_eq!(next_node_key("caption"), Some("upload"));
        assert_eq!(next_node_key("upload"), None);
    }

    #[test]
    fn run_node_stubs_return_expected_next() {
        let r = run_node(&def("start"), None).unwrap();
        assert_eq!(r.next_node.as_deref(), Some("record"));
        let r = run_node(&def("record"), Some(r#"{"x":1}"#)).unwrap();
        assert_eq!(r.next_node.as_deref(), Some("clip"));
    }
}
