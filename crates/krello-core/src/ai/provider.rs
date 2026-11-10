//! AI provider abstraction: Apple Intelligence (macOS) or explicit unavailability.

use crate::error::{Error, Result};

use super::helper::HelperSession;

#[derive(Debug, Clone, serde::Serialize)]
pub struct AiStatus {
    pub available: bool,
    /// machine-readable reason when unavailable
    pub reason: Option<String>,
    pub backend: String,
}

pub trait AiProvider: Send + Sync {
    fn status(&self) -> AiStatus;
    fn generate(&self, system: Option<&str>, prompt: &str, max_tokens: Option<u32>) -> Result<String>;
}

/// Apple Intelligence via the on-device Foundation Models framework.
/// The actual model runs in the embedded Swift helper process.
pub struct AppleIntelligenceProvider {
    helper: Option<std::path::PathBuf>,
    session: HelperSession,
    cached_status: std::sync::Mutex<Option<AiStatus>>,
}

impl AppleIntelligenceProvider {
    pub fn new() -> Self {
        let helper = super::helper::materialize_helper().ok().flatten();
        let session = HelperSession::new().expect("helper session");
        Self {
            helper,
            session,
            cached_status: std::sync::Mutex::new(None),
        }
    }

    fn status_uncached(&self) -> AiStatus {
        match &self.helper {
            Some(path) => {
                let (available, reason) = self.session.status(path);
                AiStatus {
                    available,
                    reason,
                    backend: "apple_intelligence".into(),
                }
            }
            None => AiStatus {
                available: false,
                reason: Some("helper_not_compiled".into()),
                backend: "apple_intelligence".into(),
            },
        }
    }
}

impl Default for AppleIntelligenceProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AiProvider for AppleIntelligenceProvider {
    fn status(&self) -> AiStatus {
        // Cache after first success to avoid spawning a process per poll;
        // re-check while unavailable (user may enable Apple Intelligence).
        let mut cache = self.cached_status.lock().unwrap_or_else(|e| e.into_inner());
        match &*cache {
            Some(s) if s.available => s.clone(),
            _ => {
                let s = self.status_uncached();
                *cache = Some(s.clone());
                s
            }
        }
    }

    fn generate(&self, system: Option<&str>, prompt: &str, max_tokens: Option<u32>) -> Result<String> {
        let Some(path) = &self.helper else {
            return Err(Error::Ai("unavailable:helper_not_compiled".into()));
        };
        self.session.generate(path, system, prompt, max_tokens)
    }
}

/// Always unavailable (non-macOS, tests, explicit opt-out).
pub struct UnavailableProvider {
    pub reason: String,
}

impl AiProvider for UnavailableProvider {
    fn status(&self) -> AiStatus {
        AiStatus {
            available: false,
            reason: Some(self.reason.clone()),
            backend: "none".into(),
        }
    }

    fn generate(&self, _system: Option<&str>, _prompt: &str, _max_tokens: Option<u32>) -> Result<String> {
        Err(Error::Ai(format!("unavailable:{}", self.reason)))
    }
}
