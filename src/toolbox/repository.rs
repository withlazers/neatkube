use super::tool::ToolDefinition;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Repository {
    tools: Vec<ToolDefinition>,
}
impl Repository {
    pub fn tools<'a>(&'a self) -> &'a [ToolDefinition] {
        &self.tools
    }
}
