use std::{collections::HashMap, time::Duration};

use serde::Deserialize;
use svc_agent::{mqtt::AgentConfig, AccountId};
use svc_authn::jose::Algorithm;
use svc_authz::ConfigMap as Authz;
use svc_error::extension::sentry::Config as SentryConfig;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub id: AccountId,
    pub id_token: JwtConfig,
    pub agent_label: String,
    pub broker_id: AccountId,
    pub authz: Authz,
    pub mqtt: AgentConfig,
    pub sentry: Option<SentryConfig>,
    pub backend: BackendConfig,
    pub upload: UploadConfigs,
    #[serde(default)]
    pub telemetry: TelemetryConfig,
    #[serde(default)]
    pub kruonis: KruonisConfig,
    pub metrics: MetricsConfig,
    pub max_room_duration: Option<i64>,
    pub janus_group: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct JwtConfig {
    #[serde(deserialize_with = "svc_authn::serde::algorithm")]
    pub algorithm: Algorithm,
    #[serde(deserialize_with = "svc_authn::serde::file")]
    pub key: Vec<u8>,
}

pub fn load() -> Result<Config, config::ConfigError> {
    let mut parser = config::Config::default();
    parser.merge(config::File::with_name("App"))?;
    parser.merge(config::Environment::with_prefix("APP").separator("__"))?;
    parser.try_into::<Config>()
}

#[derive(Clone, Debug, Deserialize)]
pub struct BackendConfig {
    pub id: AccountId,
    pub default_timeout: u64,
    pub stream_upload_timeout: u64,
    pub transaction_watchdog_check_period: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UploadConfigs {
    pub shared: UploadConfigMap,
    pub owned: UploadConfigMap,
}

pub type UploadConfigMap = HashMap<String, UploadConfig>;

#[derive(Clone, Debug, Deserialize)]
pub struct UploadConfig {
    pub backend: String,
    pub bucket: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct TelemetryConfig {
    pub id: Option<AccountId>,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct KruonisConfig {
    pub id: Option<AccountId>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MetricsConfig {
    pub http: MetricsHttpConfig,
    #[serde(with = "humantime_serde")]
    pub janus_metrics_collect_interval: Duration,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MetricsHttpConfig {
    pub bind_address: std::net::SocketAddr,
}
