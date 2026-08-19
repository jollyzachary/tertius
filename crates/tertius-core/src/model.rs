use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    pub activation_mode: ActivationMode,
    pub model_id: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            activation_mode: ActivationMode::Hold,
            model_id: "parakeet-field".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ActivationMode {
    #[default]
    Hold,
    Toggle,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub id: Uuid,
    pub created_at_ms: u64,
    pub duration_ms: u64,
    pub text: String,
    pub app_name: Option<String>,
    pub words: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UserData {
    pub settings: AppSettings,
    pub history: Vec<Transcript>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDescriptor {
    pub id: String,
    pub label: String,
    pub file_name: String,
    pub url: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub languages: String,
    pub recommended: bool,
}

pub fn model_catalog() -> Vec<ModelDescriptor> {
    vec![ModelDescriptor {
        id: "parakeet-field".into(),
        label: "Field".into(),
        file_name: "parakeet-tdt-0.6b-v3-int8".into(),
        url: "https://blob.handy.computer/parakeet-v3-int8.tar.gz".into(),
        size_bytes: 478_517_071,
        sha256: "43d37191602727524a7d8c6da0eef11c4ba24320f5b4730f1a2497befc2efa77".into(),
        languages: "25 European languages / auto".into(),
        recommended: true,
    }]
}
