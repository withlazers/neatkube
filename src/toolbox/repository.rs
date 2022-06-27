use super::tool::ToolDefinition;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DefaultTools {
    with_subcommand: String,
    no_subcommand: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Repository {
    default: DefaultTools,
    tools: Vec<ToolDefinition>,
}
impl Repository {
    pub fn tools(&self) -> &[ToolDefinition] {
        &self.tools
    }
    pub fn default_with_subcommand(&self) -> &str {
        &self.default.with_subcommand
    }
    pub fn default_no_subcommand(&self) -> &str {
        &self.default.no_subcommand
    }
}
