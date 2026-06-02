use regex::Regex;
use serde::de::{DeserializeOwned, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::OnceLock;
use thiserror::Error;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingEffort {
    Off,
    Low,
    Medium,
    High,
    Max,
}

impl FromStr for ThinkingEffort {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "off" | "disabled" | "none" => Ok(Self::Off),
            "low" => Ok(Self::Low),
            "medium" | "med" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "max" | "xhigh" => Ok(Self::Max),
            other => Err(format!("unknown thinking effort: '{other}'")),
        }
    }
}

impl fmt::Display for ThinkingEffort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => write!(f, "off"),
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Max => write!(f, "max"),
        }
    }
}

#[derive(Error, Debug)]
pub enum ConfigParamError {
    #[error("Configuration value not found: {0}")]
    NotFound(String),
    #[error("Failed to deserialize value: {0}")]
    DeserializeError(String),
    #[error("Failed to read configuration value: {0}")]
    ReadError(String),
}

pub trait Config {
    fn get_param<T: DeserializeOwned>(&self, key: &str) -> Result<T, ConfigParamError>;
}

pub struct EnvConfig;

impl Config for EnvConfig {
    fn get_param<T: DeserializeOwned>(&self, key: &str) -> Result<T, ConfigParamError> {
        let env_key = key.to_uppercase();
        let val =
            std::env::var(&env_key).map_err(|_| ConfigParamError::NotFound(key.to_string()))?;
        let value = parse_env_value(&val)?;
        serde_json::from_value(value).map_err(|e| ConfigParamError::DeserializeError(e.to_string()))
    }
}

fn parse_env_value(val: &str) -> Result<Value, ConfigParamError> {
    if let Ok(json_value) = serde_json::from_str(val) {
        return Ok(json_value);
    }

    let trimmed = val.trim();

    match trimmed.to_lowercase().as_str() {
        "true" => return Ok(Value::Bool(true)),
        "false" => return Ok(Value::Bool(false)),
        _ => {}
    }

    if let Ok(int_val) = trimmed.parse::<i64>() {
        return Ok(Value::Number(int_val.into()));
    }

    if let Ok(float_val) = trimmed.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(float_val) {
            return Ok(Value::Number(num));
        }
    }

    Ok(Value::String(val.to_string()))
}

pub const DEFAULT_CONTEXT_LIMIT: usize = 128_000;

#[derive(Debug, Clone, Deserialize)]
struct PredefinedModel {
    name: String,
    #[serde(default)]
    context_limit: Option<usize>,
    #[serde(default)]
    request_params: Option<HashMap<String, Value>>,
}

fn get_predefined_models<C: Config>(config: &C) -> Vec<PredefinedModel> {
    match config.get_param::<Vec<PredefinedModel>>("GOOSE_PREDEFINED_MODELS") {
        Ok(models) => models,
        Err(ConfigParamError::NotFound(_)) => Vec::new(),
        Err(e) => {
            tracing::warn!("Failed to parse GOOSE_PREDEFINED_MODELS: {}", e);
            Vec::new()
        }
    }
}

fn find_predefined_model<C: Config>(model_name: &str, config: &C) -> Option<PredefinedModel> {
    get_predefined_models(config)
        .into_iter()
        .find(|m| m.name == model_name)
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Environment variable '{0}' not found")]
    EnvVarMissing(String),
    #[error("Invalid value for '{0}': '{1}' - {2}")]
    InvalidValue(String, String, String),
    #[error("Value for '{0}' is out of valid range: {1}")]
    InvalidRange(String, String),
}

#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct ModelConfig {
    pub model_name: String,
    pub context_limit: Option<usize>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub toolshim: bool,
    pub toolshim_model: Option<String>,
    #[serde(skip)]
    pub fast_model_config: Option<Box<ModelConfig>>,
    /// Provider-specific request parameters (e.g., anthropic_beta headers)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_params: Option<HashMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,
}

impl<'de> Deserialize<'de> for ModelConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawModelConfig {
            model_name: String,
            context_limit: Option<usize>,
            temperature: Option<f32>,
            max_tokens: Option<i32>,
            toolshim: bool,
            toolshim_model: Option<String>,
            #[serde(default)]
            fast_model_config: Option<Box<ModelConfig>>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            request_params: Option<HashMap<String, Value>>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            reasoning: Option<bool>,
        }

        let raw = RawModelConfig::deserialize(deserializer)?;
        let mut config = Self {
            model_name: raw.model_name,
            context_limit: raw.context_limit,
            temperature: raw.temperature,
            max_tokens: raw.max_tokens,
            toolshim: raw.toolshim,
            toolshim_model: raw.toolshim_model,
            fast_model_config: raw.fast_model_config,
            request_params: raw.request_params,
            reasoning: raw.reasoning,
        };
        config.normalize_effort_suffix();
        Ok(config)
    }
}

impl ModelConfig {
    pub fn new_with_config<C: Config>(model_name: &str, config: &C) -> Result<Self, ConfigError> {
        Self::new_base(model_name.to_string(), None, config)
    }

    pub fn new_with_context_env_and_config<C: Config>(
        model_name: String,
        provider_name: &str,
        context_env_var: Option<&str>,
        config: &C,
    ) -> Result<Self, ConfigError> {
        let config = Self::new_base(model_name, context_env_var, config)?;
        Ok(config.with_canonical_limits_config(provider_name, config))
    }

    fn new_base<C: Config>(
        model_name: String,
        context_env_var: Option<&str>,
        config: &C,
    ) -> Result<Self, ConfigError> {
        // Check a provider-specific context limit first when provided, otherwise
        // fall back to GOOSE_CONTEXT_LIMIT through the supplied configuration source.
        let context_limit_key = context_env_var.unwrap_or("GOOSE_CONTEXT_LIMIT");
        let context_limit = match config.get_param::<usize>(context_limit_key) {
            Ok(limit) => Some(Self::validate_context_limit(limit, context_limit_key)?),
            Err(ConfigParamError::NotFound(_)) => None,
            Err(e) => {
                return Err(ConfigError::InvalidValue(
                    context_limit_key.to_string(),
                    String::new(),
                    e.to_string(),
                ))
            }
        };

        let max_tokens = Self::parse_max_tokens(config)?;
        let temperature = Self::parse_temperature(config)?;
        let toolshim = Self::parse_toolshim(config)?;
        let toolshim_model = Self::parse_toolshim_model(config)?;

        // Pick up predefined model settings before legacy suffix normalization.
        let predefined = find_predefined_model(&model_name, config);
        let predefined_context_limit = predefined.as_ref().and_then(|pm| pm.context_limit);
        let request_params = predefined.and_then(|pm| pm.request_params);

        let mut config = Self {
            model_name,
            context_limit: context_limit.or(predefined_context_limit),
            temperature,
            max_tokens,
            toolshim,
            toolshim_model,
            fast_model_config: None,
            request_params,
            reasoning: None,
        };
        config.normalize_effort_suffix();
        Ok(config)
    }

    pub fn with_canonical_limits_config<C: Config>(
        mut self,
        provider_name: &str,
        config: &C,
    ) -> Self {
        if let Some(pm) = find_predefined_model(&self.model_name, config) {
            if self.context_limit.is_none() {
                self.context_limit = pm.context_limit;
            }
        }

        // Try canonical lookup with the full model name first, then fall back
        // to the name with reasoning-effort suffixes stripped (e.g.
        // "databricks-gpt-5.4-high" → "databricks-gpt-5.4").
        let canonical = goose_models_db::maybe_get_canonical_model(provider_name, &self.model_name)
            .or_else(|| {
                let (base, _effort) = extract_reasoning_effort(&self.model_name);
                if base != self.model_name {
                    goose_models_db::maybe_get_canonical_model(provider_name, &base)
                } else {
                    None
                }
            });

        if let Some(canonical) = canonical {
            if self.context_limit.is_none() {
                self.context_limit = Some(canonical.limit.context);
            }
            if self.max_tokens.is_none() {
                self.max_tokens = canonical
                    .limit
                    .output
                    .filter(|&output| output < canonical.limit.context)
                    .map(|output| output as i32);
            }
            if self.reasoning.is_none() {
                self.reasoning = canonical.reasoning;
            }
        }

        self
    }

    fn validate_context_limit(limit: usize, key: &str) -> Result<usize, ConfigError> {
        if limit == 0 {
            return Err(ConfigError::InvalidRange(
                key.to_string(),
                "must be greater than 0".to_string(),
            ));
        }

        if limit < 4 * 1024 {
            return Err(ConfigError::InvalidRange(
                key.to_string(),
                "must be greater than 4K".to_string(),
            ));
        }

        Ok(limit)
    }

    fn parse_temperature<C: Config>(config: &C) -> Result<Option<f32>, ConfigError> {
        match config.get_param::<f32>("GOOSE_TEMPERATURE") {
            Ok(temp) => {
                if temp < 0.0 {
                    return Err(ConfigError::InvalidRange(
                        "GOOSE_TEMPERATURE".to_string(),
                        temp.to_string(),
                    ));
                }
                Ok(Some(temp))
            }
            Err(ConfigParamError::NotFound(_)) => Ok(None),
            Err(e) => Err(ConfigError::InvalidValue(
                "GOOSE_TEMPERATURE".to_string(),
                String::new(),
                e.to_string(),
            )),
        }
    }

    fn parse_max_tokens<C: Config>(config: &C) -> Result<Option<i32>, ConfigError> {
        match config.get_param::<i32>("GOOSE_MAX_TOKENS") {
            Ok(tokens) => {
                if tokens <= 0 {
                    return Err(ConfigError::InvalidRange(
                        "goose_max_tokens".to_string(),
                        "must be greater than 0".to_string(),
                    ));
                }
                Ok(Some(tokens))
            }
            Err(ConfigParamError::NotFound(_)) => Ok(None),
            Err(e) => Err(ConfigError::InvalidValue(
                "goose_max_tokens".to_string(),
                String::new(),
                e.to_string(),
            )),
        }
    }

    fn parse_toolshim<C: Config>(config: &C) -> Result<bool, ConfigError> {
        if let Ok(enabled) = config.get_param::<bool>("GOOSE_TOOLSHIM") {
            return Ok(enabled);
        }

        match config.get_param::<String>("GOOSE_TOOLSHIM") {
            Ok(val) => match val.to_lowercase().as_str() {
                "1" | "true" | "yes" | "on" => Ok(true),
                "0" | "false" | "no" | "off" => Ok(false),
                _ => Err(ConfigError::InvalidValue(
                    "GOOSE_TOOLSHIM".to_string(),
                    val,
                    "must be one of: 1, true, yes, on, 0, false, no, off".to_string(),
                )),
            },
            Err(ConfigParamError::NotFound(_)) => Ok(false),
            Err(e) => Err(ConfigError::InvalidValue(
                "GOOSE_TOOLSHIM".to_string(),
                String::new(),
                e.to_string(),
            )),
        }
    }

    fn parse_toolshim_model<C: Config>(config: &C) -> Result<Option<String>, ConfigError> {
        match config.get_param::<String>("GOOSE_TOOLSHIM_OLLAMA_MODEL") {
            Ok(val) if val.trim().is_empty() => Err(ConfigError::InvalidValue(
                "GOOSE_TOOLSHIM_OLLAMA_MODEL".to_string(),
                val,
                "cannot be empty if set".to_string(),
            )),
            Ok(val) => Ok(Some(val)),
            Err(ConfigParamError::NotFound(_)) => Ok(None),
            Err(e) => Err(ConfigError::InvalidValue(
                "GOOSE_TOOLSHIM_OLLAMA_MODEL".to_string(),
                String::new(),
                e.to_string(),
            )),
        }
    }

    pub fn with_context_limit(mut self, limit: Option<usize>) -> Self {
        if limit.is_some() {
            self.context_limit = limit;
        }
        self
    }

    pub fn with_temperature(mut self, temp: Option<f32>) -> Self {
        self.temperature = temp;
        self
    }

    pub fn with_max_tokens(mut self, tokens: Option<i32>) -> Self {
        self.max_tokens = tokens;
        self
    }

    pub fn with_toolshim(mut self, toolshim: bool) -> Self {
        self.toolshim = toolshim;
        self
    }

    pub fn with_toolshim_model(mut self, model: Option<String>) -> Self {
        self.toolshim_model = model;
        self
    }

    pub fn with_fast_config<C: Config>(
        mut self,
        fast_model_name: &str,
        provider_name: &str,
        config: &C,
    ) -> Result<Self, ConfigError> {
        let name = config
            .get_param::<String>("GOOSE_FAST_MODEL")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| fast_model_name.to_string());
        let fast_config = ModelConfig::new_with_config(&name, config)?
            .with_canonical_limits_config(provider_name, config);
        self.fast_model_config = Some(Box::new(fast_config));
        Ok(self)
    }

    pub fn with_merged_request_params(mut self, params: HashMap<String, Value>) -> Self {
        match self.request_params.as_mut() {
            Some(existing) => {
                for (k, v) in params {
                    existing.insert(k, v);
                }
            }
            None => {
                self.request_params = Some(params);
            }
        }
        self
    }

    pub fn use_fast_model(&self) -> Self {
        if let Some(fast_config) = &self.fast_model_config {
            *fast_config.clone()
        } else {
            self.clone()
        }
    }

    pub fn context_limit(&self) -> usize {
        self.context_limit.unwrap_or(DEFAULT_CONTEXT_LIMIT)
    }

    pub fn is_openai_reasoning_model(&self) -> bool {
        is_openai_responses_model(&self.model_name)
    }

    pub fn is_reasoning_model(&self) -> bool {
        if let Some(reasoning) = self.reasoning {
            return reasoning;
        }

        self.is_openai_reasoning_model()
            || self.model_name.to_lowercase().contains("claude")
            || Self::is_gemini3_reasoning_model_name(&self.model_name)
    }

    fn is_gemini3_reasoning_model_name(model_name: &str) -> bool {
        let lower = model_name.to_lowercase();
        lower.starts_with("gemini-3") || lower.contains("/gemini-3") || lower.contains("-gemini-3")
    }

    pub fn max_output_tokens(&self) -> i32 {
        if let Some(tokens) = self.max_tokens {
            return tokens;
        }

        4_096
    }

    pub fn normalize_effort_suffix(&mut self) {
        if !self.is_openai_reasoning_model() {
            return;
        }
        let parts: Vec<&str> = self.model_name.split('-').collect();
        let last = match parts.last() {
            Some(l) => *l,
            None => return,
        };
        let effort = match last {
            "none" => ThinkingEffort::Off,
            "low" => ThinkingEffort::Low,
            "medium" => ThinkingEffort::Medium,
            "high" => ThinkingEffort::High,
            "xhigh" => ThinkingEffort::Max,
            _ => return,
        };
        self.model_name = parts[..parts.len() - 1].join("-");
        let has_explicit_effort = self
            .request_params
            .as_ref()
            .and_then(|p| p.get("thinking_effort"))
            .is_some();
        if !has_explicit_effort {
            let params = self.request_params.get_or_insert_with(HashMap::new);
            params.insert(
                "thinking_effort".to_string(),
                serde_json::json!(effort.to_string()),
            );
        }
    }

    pub fn thinking_effort_with_config<C: Config>(&self, config: &C) -> Option<ThinkingEffort> {
        self.get_config_param_with_config::<String, C>(
            "thinking_effort",
            "GOOSE_THINKING_EFFORT",
            config,
        )
        .and_then(|s| s.parse::<ThinkingEffort>().ok())
        .or_else(|| Self::legacy_thinking_effort(config))
    }

    fn legacy_thinking_effort<C: Config>(config: &C) -> Option<ThinkingEffort> {
        if let Ok(value) = config.get_param::<String>("CLAUDE_THINKING_TYPE") {
            if let Some(effort) = match value.to_lowercase().as_str() {
                "adaptive" | "enabled" => Some(ThinkingEffort::High),
                "disabled" => Some(ThinkingEffort::Off),
                _ => None,
            } {
                return Some(effort);
            }
        }

        if let Ok(enabled) = config.get_param::<bool>("CLAUDE_THINKING_ENABLED") {
            return Some(if enabled {
                ThinkingEffort::High
            } else {
                ThinkingEffort::Off
            });
        }

        if let Ok(value) = config.get_param::<String>("GEMINI3_THINKING_LEVEL") {
            if let Some(effort) = Self::legacy_gemini3_thinking_effort(&value) {
                return Some(effort);
            }
        }

        None
    }

    fn legacy_gemini3_thinking_effort(value: &str) -> Option<ThinkingEffort> {
        match value.to_lowercase().as_str() {
            "low" => Some(ThinkingEffort::Low),
            "high" => Some(ThinkingEffort::High),
            _ => None,
        }
    }

    pub fn get_config_param_with_config<T: DeserializeOwned, C: Config>(
        &self,
        request_key: &str,
        config_key: &str,
        config: &C,
    ) -> Option<T> {
        self.request_params
            .as_ref()
            .and_then(|params| params.get(request_key))
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .or_else(|| config.get_param::<T>(config_key).ok())
    }

    pub fn new_or_fail_with_config<C: Config>(model_name: &str, config: &C) -> ModelConfig {
        ModelConfig::new_with_config(model_name, config)
            .unwrap_or_else(|_| panic!("Failed to create model config for {}", model_name))
    }
}

pub fn is_openai_responses_model(model_name: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| Regex::new(r"(?:^|[-/])(?:o[0-9]+(?:$|-)|gpt-5(?:$|[-.]))").unwrap());
    re.is_match(&model_name.to_lowercase())
}

pub fn extract_reasoning_effort(model_name: &str) -> (String, Option<String>) {
    if !is_openai_responses_model(model_name) {
        return (model_name.to_string(), None);
    }

    let lower = model_name.to_lowercase();
    for effort in ["none", "low", "medium", "high", "xhigh"] {
        let suffix = format!("-{effort}");
        if lower.ends_with(&suffix) {
            let base = model_name[..model_name.len() - suffix.len()].to_string();
            return (base, Some(effort.to_string()));
        }
    }

    (model_name.to_string(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_aliases() {
        assert_eq!("off".parse::<ThinkingEffort>(), Ok(ThinkingEffort::Off));
        assert_eq!(
            "disabled".parse::<ThinkingEffort>(),
            Ok(ThinkingEffort::Off)
        );
        assert_eq!("none".parse::<ThinkingEffort>(), Ok(ThinkingEffort::Off));
        assert_eq!("med".parse::<ThinkingEffort>(), Ok(ThinkingEffort::Medium));
        assert_eq!("max".parse::<ThinkingEffort>(), Ok(ThinkingEffort::Max));
        assert_eq!("xhigh".parse::<ThinkingEffort>(), Ok(ThinkingEffort::Max));
        assert!("invalid".parse::<ThinkingEffort>().is_err());
    }

    #[test]
    fn test_parse_max_tokens_valid() {
        let _guard = env_lock::lock_env([("GOOSE_MAX_TOKENS", Some("4096"))]);
        let result = ModelConfig::parse_max_tokens(&EnvConfig).unwrap();
        assert_eq!(result, Some(4096));
    }

    #[test]
    fn test_parse_max_tokens_not_set() {
        let _guard = env_lock::lock_env([("GOOSE_MAX_TOKENS", None::<&str>)]);
        let result = ModelConfig::parse_max_tokens(&EnvConfig).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_max_tokens_invalid_string() {
        let _guard = env_lock::lock_env([("GOOSE_MAX_TOKENS", Some("not_a_number"))]);
        let result = ModelConfig::parse_max_tokens(&EnvConfig);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::InvalidValue(..)));
    }

    #[test]
    fn test_parse_max_tokens_zero() {
        let _guard = env_lock::lock_env([("GOOSE_MAX_TOKENS", Some("0"))]);
        let result = ModelConfig::parse_max_tokens(&EnvConfig);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::InvalidRange(..)));
    }

    #[test]
    fn test_parse_max_tokens_negative() {
        let _guard = env_lock::lock_env([("GOOSE_MAX_TOKENS", Some("-100"))]);
        let result = ModelConfig::parse_max_tokens(&EnvConfig);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::InvalidRange(..)));
    }

    #[test]
    fn test_model_config_with_max_tokens_env() {
        let _guard = env_lock::lock_env([
            ("GOOSE_MAX_TOKENS", Some("8192")),
            ("GOOSE_TEMPERATURE", None::<&str>),
            ("GOOSE_CONTEXT_LIMIT", None::<&str>),
            ("GOOSE_TOOLSHIM", None::<&str>),
            ("GOOSE_TOOLSHIM_OLLAMA_MODEL", None::<&str>),
        ]);
        let config = ModelConfig::new_with_config("test-model", &EnvConfig).unwrap();
        assert_eq!(config.max_tokens, Some(8192));
    }

    #[test]
    fn test_model_config_without_max_tokens_env() {
        let _guard = env_lock::lock_env([
            ("GOOSE_MAX_TOKENS", None::<&str>),
            ("GOOSE_TEMPERATURE", None::<&str>),
            ("GOOSE_CONTEXT_LIMIT", None::<&str>),
            ("GOOSE_TOOLSHIM", None::<&str>),
            ("GOOSE_TOOLSHIM_OLLAMA_MODEL", None::<&str>),
        ]);
        let config = ModelConfig::new_with_config("test-model", &EnvConfig).unwrap();
        assert_eq!(config.max_tokens, None);
    }

    #[test]
    fn test_get_config_param() {
        let _guard = env_lock::lock_env([("GOOSE_THINKING_EFFORT", Some("high"))]);

        let mut params = HashMap::new();
        params.insert("thinking_effort".to_string(), serde_json::json!("low"));

        let config_with_params = ModelConfig {
            model_name: "test".to_string(),
            request_params: Some(params),
            ..Default::default()
        };

        let config_without_params = ModelConfig {
            request_params: None,
            ..config_with_params.clone()
        };

        assert_eq!(
            config_with_params.get_config_param_with_config::<String, _>(
                "thinking_effort",
                "GOOSE_THINKING_EFFORT",
                &EnvConfig
            ),
            Some("low".to_string())
        );
        assert_eq!(
            config_without_params.get_config_param_with_config::<String, _>(
                "thinking_effort",
                "GOOSE_THINKING_EFFORT",
                &EnvConfig
            ),
            Some("high".to_string())
        );
        assert_eq!(
            config_without_params.get_config_param_with_config::<String, _>(
                "nonexistent",
                "NONEXISTENT_CONFIG_KEY",
                &EnvConfig
            ),
            None
        );
    }

    #[test]
    fn test_deserialize_preserves_fast_model_config() {
        let config: ModelConfig = serde_json::from_value(serde_json::json!({
            "model_name": "primary-model",
            "context_limit": null,
            "temperature": null,
            "max_tokens": null,
            "toolshim": false,
            "toolshim_model": null,
            "fast_model_config": {
                "model_name": "fast-model",
                "context_limit": 4096,
                "temperature": null,
                "max_tokens": 1024,
                "toolshim": false,
                "toolshim_model": null
            }
        }))
        .unwrap();

        let fast_config = config.fast_model_config.as_ref().unwrap();
        assert_eq!(fast_config.model_name, "fast-model");
        assert_eq!(fast_config.context_limit, Some(4096));
        assert_eq!(fast_config.max_tokens, Some(1024));
        assert_eq!(config.use_fast_model().model_name, "fast-model");
    }

    mod thinking_effort_tests {
        use super::*;

        #[test]
        fn from_request_params() {
            let _guard = env_lock::lock_env([("GOOSE_THINKING_EFFORT", None::<&str>)]);
            let mut params = HashMap::new();
            params.insert("thinking_effort".to_string(), serde_json::json!("medium"));
            let config = ModelConfig {
                model_name: "test".to_string(),
                request_params: Some(params),
                ..Default::default()
            };
            assert_eq!(
                config.thinking_effort_with_config(&EnvConfig),
                Some(ThinkingEffort::Medium)
            );
        }

        #[test]
        fn from_env_var() {
            let _guard = env_lock::lock_env([("GOOSE_THINKING_EFFORT", Some("high"))]);
            let config = ModelConfig {
                model_name: "test".to_string(),
                ..Default::default()
            };
            assert_eq!(
                config.thinking_effort_with_config(&EnvConfig),
                Some(ThinkingEffort::High)
            );
        }

        #[test]
        fn request_params_override_env() {
            let _guard = env_lock::lock_env([("GOOSE_THINKING_EFFORT", Some("high"))]);
            let mut params = HashMap::new();
            params.insert("thinking_effort".to_string(), serde_json::json!("low"));
            let config = ModelConfig {
                model_name: "test".to_string(),
                request_params: Some(params),
                ..Default::default()
            };
            assert_eq!(
                config.thinking_effort_with_config(&EnvConfig),
                Some(ThinkingEffort::Low)
            );
        }

        #[test]
        fn legacy_claude_thinking_type_fallback() {
            for value in ["enabled", "adaptive"] {
                let _guard = env_lock::lock_env([
                    ("GOOSE_THINKING_EFFORT", None::<&str>),
                    ("CLAUDE_THINKING_TYPE", Some(value)),
                    ("CLAUDE_THINKING_ENABLED", None::<&str>),
                    ("GEMINI3_THINKING_LEVEL", None::<&str>),
                    ("ANTHROPIC_THINKING_BUDGET", None::<&str>),
                    ("CLAUDE_THINKING_BUDGET", None::<&str>),
                    ("GEMINI25_THINKING_BUDGET", None::<&str>),
                ]);
                let config = ModelConfig {
                    model_name: "test".to_string(),
                    ..Default::default()
                };
                assert_eq!(
                    config.thinking_effort_with_config(&EnvConfig),
                    Some(ThinkingEffort::High)
                );
            }
        }

        #[test]
        fn legacy_gemini3_thinking_level_mapping() {
            assert_eq!(
                ModelConfig::legacy_gemini3_thinking_effort("low"),
                Some(ThinkingEffort::Low)
            );
            assert_eq!(
                ModelConfig::legacy_gemini3_thinking_effort("high"),
                Some(ThinkingEffort::High)
            );
            assert_eq!(ModelConfig::legacy_gemini3_thinking_effort("auto"), None);
        }

        #[test]
        fn legacy_gemini3_thinking_level_fallback() {
            let temp_dir = tempfile::tempdir().unwrap();
            let temp_root = temp_dir.path().to_string_lossy().to_string();
            let _guard = env_lock::lock_env([
                ("GOOSE_PATH_ROOT", Some(temp_root.as_str())),
                ("GOOSE_THINKING_EFFORT", None::<&str>),
                ("CLAUDE_THINKING_TYPE", None::<&str>),
                ("CLAUDE_THINKING_ENABLED", None::<&str>),
                ("GEMINI3_THINKING_LEVEL", Some("high")),
                ("ANTHROPIC_THINKING_BUDGET", None::<&str>),
                ("CLAUDE_THINKING_BUDGET", None::<&str>),
                ("GEMINI25_THINKING_BUDGET", None::<&str>),
            ]);
            let config = ModelConfig {
                model_name: "gemini-3-pro".to_string(),
                ..Default::default()
            };
            assert_eq!(
                config.thinking_effort_with_config(&EnvConfig),
                Some(ThinkingEffort::High)
            );
        }

        #[test]
        fn effort_suffix_stripped_from_model_name() {
            let _guard = env_lock::lock_env([
                ("GOOSE_THINKING_EFFORT", None::<&str>),
                ("GOOSE_MAX_TOKENS", None::<&str>),
                ("GOOSE_TEMPERATURE", None::<&str>),
                ("GOOSE_CONTEXT_LIMIT", None::<&str>),
                ("GOOSE_TOOLSHIM", None::<&str>),
                ("GOOSE_TOOLSHIM_OLLAMA_MODEL", None::<&str>),
            ]);
            let config = ModelConfig::new_with_config("o3-mini-high", &EnvConfig).unwrap();
            assert_eq!(config.model_name, "o3-mini");
            assert_eq!(
                config.thinking_effort_with_config(&EnvConfig),
                Some(ThinkingEffort::High)
            );
        }

        #[test]
        fn none_suffix_stripped_from_model_name() {
            let _guard = env_lock::lock_env([
                ("GOOSE_THINKING_EFFORT", Some("high")),
                ("GOOSE_MAX_TOKENS", None::<&str>),
                ("GOOSE_TEMPERATURE", None::<&str>),
                ("GOOSE_CONTEXT_LIMIT", None::<&str>),
                ("GOOSE_TOOLSHIM", None::<&str>),
                ("GOOSE_TOOLSHIM_OLLAMA_MODEL", None::<&str>),
            ]);
            let config = ModelConfig::new_with_config("o3-mini-none", &EnvConfig).unwrap();
            assert_eq!(config.model_name, "o3-mini");
            assert_eq!(
                config.thinking_effort_with_config(&EnvConfig),
                Some(ThinkingEffort::Off)
            );
        }

        #[test]
        fn xhigh_suffix_stripped_from_model_name() {
            let _guard = env_lock::lock_env([
                ("GOOSE_THINKING_EFFORT", Some("low")),
                ("GOOSE_MAX_TOKENS", None::<&str>),
                ("GOOSE_TEMPERATURE", None::<&str>),
                ("GOOSE_CONTEXT_LIMIT", None::<&str>),
                ("GOOSE_TOOLSHIM", None::<&str>),
                ("GOOSE_TOOLSHIM_OLLAMA_MODEL", None::<&str>),
            ]);
            let config = ModelConfig::new_with_config("gpt-5.4-xhigh", &EnvConfig).unwrap();
            assert_eq!(config.model_name, "gpt-5.4");
            assert_eq!(
                config.thinking_effort_with_config(&EnvConfig),
                Some(ThinkingEffort::Max)
            );
        }

        #[test]
        fn effort_suffix_not_stripped_when_thinking_effort_set() {
            let _guard = env_lock::lock_env([
                ("GOOSE_THINKING_EFFORT", None::<&str>),
                ("GOOSE_MAX_TOKENS", None::<&str>),
                ("GOOSE_TEMPERATURE", None::<&str>),
                ("GOOSE_CONTEXT_LIMIT", None::<&str>),
                ("GOOSE_TOOLSHIM", None::<&str>),
                ("GOOSE_TOOLSHIM_OLLAMA_MODEL", None::<&str>),
            ]);
            let mut params = HashMap::new();
            params.insert("thinking_effort".to_string(), serde_json::json!("low"));
            let mut config = ModelConfig::new_with_config("o3-mini-high", &EnvConfig).unwrap();
            // Suffix was already normalized during new(), but if request_params
            // were set before construction, the suffix would not be stripped.
            // Verify the normalized state:
            assert_eq!(config.model_name, "o3-mini");

            // Now simulate setting explicit effort after construction
            config.request_params = Some(params);
            assert_eq!(
                config.thinking_effort_with_config(&EnvConfig),
                Some(ThinkingEffort::Low)
            );
        }

        #[test]
        fn no_suffix_no_change() {
            let _guard = env_lock::lock_env([
                ("GOOSE_THINKING_EFFORT", None::<&str>),
                ("GOOSE_MAX_TOKENS", None::<&str>),
                ("GOOSE_TEMPERATURE", None::<&str>),
                ("GOOSE_CONTEXT_LIMIT", None::<&str>),
                ("GOOSE_TOOLSHIM", None::<&str>),
                ("GOOSE_TOOLSHIM_OLLAMA_MODEL", None::<&str>),
            ]);
            let config = ModelConfig::new_with_config("o3-mini", &EnvConfig).unwrap();
            assert_eq!(config.model_name, "o3-mini");
        }

        #[test]
        fn non_reasoning_model_suffix_not_stripped() {
            let _guard = env_lock::lock_env([
                ("GOOSE_THINKING_EFFORT", None::<&str>),
                ("GOOSE_MAX_TOKENS", None::<&str>),
                ("GOOSE_TEMPERATURE", None::<&str>),
                ("GOOSE_CONTEXT_LIMIT", None::<&str>),
                ("GOOSE_TOOLSHIM", None::<&str>),
                ("GOOSE_TOOLSHIM_OLLAMA_MODEL", None::<&str>),
            ]);
            let config = ModelConfig::new_with_config("claude-sonnet-4-high", &EnvConfig).unwrap();
            assert_eq!(config.model_name, "claude-sonnet-4-high");
        }
    }

    mod with_canonical_limits {
        use super::*;

        #[test]
        fn sets_limits_from_canonical_model() {
            let _guard = env_lock::lock_env([
                ("GOOSE_MAX_TOKENS", None::<&str>),
                ("GOOSE_CONTEXT_LIMIT", None::<&str>),
            ]);
            let config = ModelConfig::new_or_fail_with_config("gpt-4o", &EnvConfig)
                .with_canonical_limits_config("openai", &EnvConfig);

            assert_eq!(config.context_limit, Some(128_000));
            assert_eq!(config.max_tokens, Some(4_096));
            assert_eq!(config.reasoning, Some(false));
        }

        #[test]
        fn does_not_override_existing_context_limit() {
            let _guard = env_lock::lock_env([
                ("GOOSE_MAX_TOKENS", None::<&str>),
                ("GOOSE_CONTEXT_LIMIT", None::<&str>),
            ]);
            let mut config = ModelConfig::new_or_fail_with_config("gpt-4o", &EnvConfig);
            config.context_limit = Some(64_000);
            let config = config.with_canonical_limits_config("openai", &EnvConfig);

            assert_eq!(config.context_limit, Some(64_000));
        }

        #[test]
        fn does_not_override_existing_max_tokens() {
            let _guard = env_lock::lock_env([
                ("GOOSE_MAX_TOKENS", None::<&str>),
                ("GOOSE_CONTEXT_LIMIT", None::<&str>),
            ]);
            let mut config = ModelConfig::new_or_fail_with_config("gpt-4o", &EnvConfig);
            config.max_tokens = Some(1_000);
            let config = config.with_canonical_limits_config("openai", &EnvConfig);

            assert_eq!(config.max_tokens, Some(1_000));
        }

        #[test]
        fn skips_canonical_output_limit_when_it_equals_context_limit() {
            let _guard = env_lock::lock_env([
                ("GOOSE_MAX_TOKENS", None::<&str>),
                ("GOOSE_CONTEXT_LIMIT", None::<&str>),
            ]);
            let config = ModelConfig::new_or_fail_with_config("moonshotai/kimi-k2.6", &EnvConfig)
                .with_canonical_limits_config("nvidia", &EnvConfig);

            assert_eq!(config.context_limit, Some(262_144));
            assert_eq!(config.max_tokens, None);
            assert_eq!(config.max_output_tokens(), 4_096);
        }

        #[test]
        fn unknown_model_leaves_fields_none() {
            let _guard = env_lock::lock_env([
                ("GOOSE_MAX_TOKENS", None::<&str>),
                ("GOOSE_CONTEXT_LIMIT", None::<&str>),
            ]);
            let config = ModelConfig::new_or_fail_with_config("totally-unknown-model", &EnvConfig)
                .with_canonical_limits_config("openai", &EnvConfig);

            assert_eq!(config.context_limit, None);
            assert_eq!(config.max_tokens, None);
            assert_eq!(config.reasoning, None);
        }

        #[test]
        fn resolves_after_stripping_reasoning_effort_suffix() {
            let _guard = env_lock::lock_env([
                ("GOOSE_MAX_TOKENS", None::<&str>),
                ("GOOSE_CONTEXT_LIMIT", None::<&str>),
            ]);

            // "databricks-gpt-5.4-high" should resolve via "databricks-gpt-5.4"
            let config =
                ModelConfig::new_or_fail_with_config("databricks-gpt-5.4-high", &EnvConfig)
                    .with_canonical_limits_config("databricks", &EnvConfig);
            assert_eq!(config.context_limit, Some(1_050_000));

            // "gpt-5.4-xhigh" should resolve via "gpt-5.4"
            let config = ModelConfig::new_or_fail_with_config("gpt-5.4-xhigh", &EnvConfig)
                .with_canonical_limits_config("openai", &EnvConfig);
            assert_eq!(config.context_limit, Some(1_050_000));

            // "gpt-5.4-nano-low" should resolve via "gpt-5.4-nano"
            let config = ModelConfig::new_or_fail_with_config("gpt-5.4-nano-low", &EnvConfig)
                .with_canonical_limits_config("openai", &EnvConfig);
            assert_eq!(config.context_limit, Some(400_000));
        }
    }

    mod is_openai_reasoning_model {
        use super::*;

        const ENV_LOCK_KEYS: [(&str, Option<&str>); 5] = [
            ("GOOSE_MAX_TOKENS", None),
            ("GOOSE_TEMPERATURE", None),
            ("GOOSE_CONTEXT_LIMIT", None),
            ("GOOSE_TOOLSHIM", None),
            ("GOOSE_TOOLSHIM_OLLAMA_MODEL", None),
        ];

        #[test]
        fn bare_reasoning_models() {
            let _guard = env_lock::lock_env(ENV_LOCK_KEYS);
            assert!(
                ModelConfig::new_or_fail_with_config("o1", &EnvConfig).is_openai_reasoning_model()
            );
            assert!(
                ModelConfig::new_or_fail_with_config("o1-preview", &EnvConfig)
                    .is_openai_reasoning_model()
            );
            assert!(
                ModelConfig::new_or_fail_with_config("o3", &EnvConfig).is_openai_reasoning_model()
            );
            assert!(ModelConfig::new_or_fail_with_config("o3-mini", &EnvConfig)
                .is_openai_reasoning_model());
            assert!(ModelConfig::new_or_fail_with_config("o4-mini", &EnvConfig)
                .is_openai_reasoning_model());
            assert!(ModelConfig::new_or_fail_with_config("gpt-5", &EnvConfig)
                .is_openai_reasoning_model());
            assert!(
                ModelConfig::new_or_fail_with_config("gpt-5-3-codex", &EnvConfig)
                    .is_openai_reasoning_model()
            );
        }

        #[test]
        fn goose_prefixed_reasoning_models() {
            let _guard = env_lock::lock_env(ENV_LOCK_KEYS);
            assert!(
                ModelConfig::new_or_fail_with_config("goose-o3-mini", &EnvConfig)
                    .is_openai_reasoning_model()
            );
            assert!(
                ModelConfig::new_or_fail_with_config("goose-o4-mini", &EnvConfig)
                    .is_openai_reasoning_model()
            );
            assert!(
                ModelConfig::new_or_fail_with_config("goose-gpt-5", &EnvConfig)
                    .is_openai_reasoning_model()
            );
        }

        #[test]
        fn databricks_prefixed_reasoning_models() {
            let _guard = env_lock::lock_env(ENV_LOCK_KEYS);
            assert!(
                ModelConfig::new_or_fail_with_config("databricks-o3-mini", &EnvConfig)
                    .is_openai_reasoning_model()
            );
            assert!(
                ModelConfig::new_or_fail_with_config("databricks-o4-mini", &EnvConfig)
                    .is_openai_reasoning_model()
            );
            assert!(
                ModelConfig::new_or_fail_with_config("databricks-gpt-5", &EnvConfig)
                    .is_openai_reasoning_model()
            );
        }

        #[test]
        fn non_reasoning_models() {
            let _guard = env_lock::lock_env(ENV_LOCK_KEYS);
            assert!(
                !ModelConfig::new_or_fail_with_config("claude-sonnet-4", &EnvConfig)
                    .is_openai_reasoning_model()
            );
            assert!(!ModelConfig::new_or_fail_with_config("gpt-4o", &EnvConfig)
                .is_openai_reasoning_model());
            assert!(!ModelConfig::new_or_fail_with_config(
                "databricks-claude-sonnet-4",
                &EnvConfig
            )
            .is_openai_reasoning_model());
            assert!(
                !ModelConfig::new_or_fail_with_config("goose-claude-sonnet-4", &EnvConfig)
                    .is_openai_reasoning_model()
            );
            assert!(
                !ModelConfig::new_or_fail_with_config("llama-3-70b", &EnvConfig)
                    .is_openai_reasoning_model()
            );
        }
    }

    mod is_reasoning_model {
        use super::*;

        const ENV_LOCK_KEYS: [(&str, Option<&str>); 5] = [
            ("GOOSE_MAX_TOKENS", None),
            ("GOOSE_TEMPERATURE", None),
            ("GOOSE_CONTEXT_LIMIT", None),
            ("GOOSE_TOOLSHIM", None),
            ("GOOSE_TOOLSHIM_OLLAMA_MODEL", None),
        ];

        #[test]
        fn includes_reasoning_model_families() {
            let _guard = env_lock::lock_env(ENV_LOCK_KEYS);
            assert!(
                ModelConfig::new_or_fail_with_config("o3-mini", &EnvConfig).is_reasoning_model()
            );
            assert!(
                ModelConfig::new_or_fail_with_config("claude-sonnet-4", &EnvConfig)
                    .is_reasoning_model()
            );
            assert!(
                ModelConfig::new_or_fail_with_config("gemini-3-pro", &EnvConfig)
                    .is_reasoning_model()
            );
        }

        #[test]
        fn uses_explicit_metadata_first() {
            let _guard = env_lock::lock_env(ENV_LOCK_KEYS);
            let mut config = ModelConfig::new_or_fail_with_config("provider-alias", &EnvConfig);
            config.reasoning = Some(true);
            assert!(config.is_reasoning_model());

            let mut config = ModelConfig::new_or_fail_with_config("claude-sonnet-4", &EnvConfig);
            config.reasoning = Some(false);
            assert!(!config.is_reasoning_model());
        }
    }
}
