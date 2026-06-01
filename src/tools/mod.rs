pub mod file_tools;
pub mod cmd_tool;

use std::future::Future;
use std::pin::Pin;
use serde_json::Value;

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;
    fn execute(&self, arguments: &str) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;
}
