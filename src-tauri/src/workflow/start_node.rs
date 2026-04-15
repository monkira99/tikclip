use crate::db::models::FlowNodeDefinition;

use super::EngineNodeResult;

pub fn run(def: &FlowNodeDefinition, input_json: Option<&str>) -> Result<EngineNodeResult, String> {
    let _ = def.published_config_json.as_str();
    let _ = input_json;
    Ok(EngineNodeResult {
        status: "completed".to_string(),
        output_json: input_json.map(|x| x.to_string()),
        error: None,
        next_node: Some("record".to_string()),
    })
}
