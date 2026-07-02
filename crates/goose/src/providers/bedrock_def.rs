use std::collections::HashMap;

use crate::providers::base::ProviderDef;
use anyhow::Result;
use futures::future::BoxFuture;
use goose_providers::base::{ProviderDescriptor, ProviderMetadata};
use goose_providers::bedrock::{
    BedrockProvider, BedrockProviderConfig, BEDROCK_DEFAULT_BACKOFF_MULTIPLIER,
    BEDROCK_DEFAULT_INITIAL_RETRY_INTERVAL_MS, BEDROCK_DEFAULT_MAX_RETRIES,
    BEDROCK_DEFAULT_MAX_RETRY_INTERVAL_MS,
};
use goose_providers::retry::RetryConfig;
use serde_json::Value;

use crate::config::ExtensionConfig;
use crate::providers::api_client::TlsConfig;

pub struct BedrockProviderDef;

impl ProviderDescriptor for BedrockProviderDef {
    fn metadata() -> ProviderMetadata {
        BedrockProvider::metadata()
    }
}

impl ProviderDef for BedrockProviderDef {
    type Provider = BedrockProvider;

    fn from_env(
        _extensions: Vec<ExtensionConfig>,
        _tls_config: Option<TlsConfig>,
    ) -> BoxFuture<'static, Result<Self::Provider>> {
        Box::pin(from_env())
    }
}

async fn from_env() -> Result<BedrockProvider> {
    let config = crate::config::Config::global();

    let set_aws_env_vars = |res: Result<HashMap<String, Value>, _>| {
        if let Ok(map) = res {
            map.into_iter()
                .filter(|(key, _)| key.starts_with("AWS_"))
                .filter_map(|(key, value)| value.as_str().map(|s| (key, s.to_string())))
                .for_each(|(key, s)| std::env::set_var(key, s));
        }
    };

    let filtered_secrets = config.all_secrets().map(|map| {
        map.into_iter()
            .filter(|(key, _)| key != "AWS_BEARER_TOKEN_BEDROCK")
            .collect()
    });

    set_aws_env_vars(config.all_values());
    set_aws_env_vars(filtered_secrets);

    let bearer_token = config
        .get_secret::<String>("AWS_BEARER_TOKEN_BEDROCK")
        .ok()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty());

    let region = config
        .get_param::<String>("AWS_REGION")
        .ok()
        .filter(|region| !region.is_empty());

    let profile_name = config
        .get_param::<String>("AWS_PROFILE")
        .ok()
        .filter(|profile| !profile.is_empty());

    let retry_config = RetryConfig::new(
        config
            .get_param::<usize>("BEDROCK_MAX_RETRIES")
            .unwrap_or(BEDROCK_DEFAULT_MAX_RETRIES),
        config
            .get_param::<u64>("BEDROCK_INITIAL_RETRY_INTERVAL_MS")
            .unwrap_or(BEDROCK_DEFAULT_INITIAL_RETRY_INTERVAL_MS),
        config
            .get_param::<f64>("BEDROCK_BACKOFF_MULTIPLIER")
            .unwrap_or(BEDROCK_DEFAULT_BACKOFF_MULTIPLIER),
        config
            .get_param::<u64>("BEDROCK_MAX_RETRY_INTERVAL_MS")
            .unwrap_or(BEDROCK_DEFAULT_MAX_RETRY_INTERVAL_MS),
    );

    BedrockProvider::from_config(BedrockProviderConfig {
        profile_name,
        region,
        bearer_token,
        retry_config,
        enable_caching: config
            .get_param::<bool>("BEDROCK_ENABLE_CACHING")
            .unwrap_or(false),
        disable_streaming: config
            .get_param::<bool>("BEDROCK_DISABLE_STREAMING")
            .unwrap_or(false),
        session_id_provider: Some(std::sync::Arc::new(|| {
            crate::session_context::current_session_id()
        })),
    })
    .await
}
