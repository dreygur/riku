/// Agent types: scope, response, and error structs.
use serde_json::json;

/// Agent permissions scope
#[derive(Debug, Clone, PartialEq)]
pub enum AgentScope {
    Readonly,
    Staging,
    Production,
}

impl AgentScope {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "readonly" => AgentScope::Readonly,
            "staging" => AgentScope::Staging,
            "production" => AgentScope::Production,
            _ => AgentScope::Readonly,
        }
    }

    /// Check if scope allows a specific action
    pub fn allows(&self, action: &str) -> bool {
        match action {
            "apps" | "logs" | "ps" | "config:get" | "config:show" | "stats" => true,
            "deploy" | "restart" | "run" | "config:set" | "config:unset" => {
                matches!(self, AgentScope::Staging | AgentScope::Production)
            }
            "destroy" | "stop" => matches!(self, AgentScope::Production),
            _ => false,
        }
    }

    /// Get rate limit (commands per minute)
    pub fn rate_limit(&self) -> u32 {
        match self {
            AgentScope::Readonly => 60,
            AgentScope::Staging => 30,
            AgentScope::Production => 20,
        }
    }
}

/// Agent response structure
#[derive(Debug)]
pub struct AgentResponse {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<AgentError>,
    pub message: Option<String>,
    pub confirmation_required: bool,
    pub confirm_token: Option<String>,
    pub job_id: Option<String>,
}

impl AgentResponse {
    pub fn success(data: serde_json::Value) -> Self {
        AgentResponse {
            success: true,
            data: Some(data),
            error: None,
            message: None,
            confirmation_required: false,
            confirm_token: None,
            job_id: None,
        }
    }

    pub fn error(code: &str, message: &str) -> Self {
        AgentResponse {
            success: false,
            data: None,
            error: Some(AgentError {
                code: code.to_string(),
                message: message.to_string(),
            }),
            message: None,
            confirmation_required: false,
            confirm_token: None,
            job_id: None,
        }
    }

    pub fn confirmation_required(action: &str, _app: &str, token: &str) -> Self {
        AgentResponse {
            success: false,
            data: None,
            error: None,
            message: Some(format!("Human confirmation required for {}", action)),
            confirmation_required: true,
            confirm_token: Some(token.to_string()),
            job_id: None,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        let mut result = json!({
            "success": self.success,
        });

        if let Some(ref data) = self.data {
            result["data"] = data.clone();
        }

        if let Some(ref error) = self.error {
            result["error"] = json!({
                "code": error.code,
                "message": error.message,
            });
        }

        if let Some(ref message) = self.message {
            result["message"] = json!(message);
        }

        result["confirmation_required"] = json!(self.confirmation_required);

        if let Some(ref token) = self.confirm_token {
            result["confirm_token"] = json!(token);
        }

        if let Some(ref job_id) = self.job_id {
            result["job_id"] = json!(job_id);
        }

        result
    }
}

#[derive(Debug)]
pub struct AgentError {
    pub code: String,
    pub message: String,
}
