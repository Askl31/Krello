//! Application state and provider selection.

use std::sync::Arc;

use krello_core::ai::provider::{AiProvider, AppleIntelligenceProvider, UnavailableProvider};
use krello_core::service::Service;

pub struct AppState {
    pub service: Arc<Service>,
    pub ai: Arc<dyn AiProvider>,
    /// Optional bearer token (--token). None = open localhost access.
    pub token: Option<String>,
    /// Pre-built `Bearer <token>` compared in constant time — avoids
    /// re-allocating the header string on every /api/v1 request.
    pub bearer: Option<String>,
}
pub type SharedState = Arc<AppState>;

pub fn build_ai_provider() -> Arc<dyn AiProvider> {
    if krello_ai_helper::apple_intelligence_supported_os() {
        Arc::new(AppleIntelligenceProvider::new())
    } else {
        Arc::new(UnavailableProvider {
            reason: if cfg!(target_os = "macos") {
                "helper_not_compiled".into()
            } else {
                "unsupported_platform".into()
            },
        })
    }
}
