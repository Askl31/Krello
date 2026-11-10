mod common;

use common::test_service;
use krello_core::ai::provider::{AiProvider, UnavailableProvider};
use krello_core::ai;

#[test]
fn unavailable_provider_reports_status() {
    let p = UnavailableProvider { reason: "test".into() };
    let st = p.status();
    assert!(!st.available);
    assert_eq!(st.reason.as_deref(), Some("test"));
    assert!(p.generate(None, "ping", None).is_err());
}

#[cfg(target_os = "macos")]
#[test]
fn apple_intelligence_status_matches_runtime() {
    let p = krello_core::ai::provider::AppleIntelligenceProvider::new();
    let st = p.status();
    // On eligible hardware → available ; on CI/VM → device_not_eligible etc.
    // We only assert the contract, not the machine's capability.
    assert_eq!(st.backend, "apple_intelligence");
    if st.available {
        let text = p
            .generate(Some("Réponds en un mot."), "Dis bonjour.", Some(64))
            .expect("available provider must generate");
        assert!(!text.is_empty());
    } else {
        assert!(st.reason.is_some());
        assert!(p.generate(None, "ping", None).is_err());
    }
}

#[test]
fn parse_task_with_mock_provider() {
    let mock = MockFixed {
        first: r#"Voici la carte :
```json
{"title": "Rédiger le rapport", "description": "synthèse trimestrielle", "due": "2026-11-13", "priority": "high", "labels": ["rapport"], "checklist": ["collecte des chiffres", "rédaction"]}
```
"#.into(),
    };
    struct MockFixed { first: String }
    impl AiProvider for MockFixed {
        fn status(&self) -> krello_core::ai::provider::AiStatus {
            krello_core::ai::provider::AiStatus { available: true, reason: None, backend: "mock".into() }
        }
        fn generate(&self, _s: Option<&str>, _p: &str, _m: Option<u32>) -> krello_core::error::Result<String> {
            Ok(self.first.clone())
        }
    }
    let parsed = ai::parse_task(&mock, "écrire le rapport pour vendredi 13 novembre, priorité haute").unwrap();
    assert_eq!(parsed.title, "Rédiger le rapport");
    assert_eq!(parsed.due.as_deref(), Some("2026-11-13"));
    assert_eq!(parsed.priority.as_deref(), Some("high"));
    assert_eq!(parsed.checklist.len(), 2);
}

#[test]
fn parse_task_rejects_invalid_schema() {
    struct MockBad;
    impl AiProvider for MockBad {
        fn status(&self) -> krello_core::ai::provider::AiStatus {
            krello_core::ai::provider::AiStatus { available: true, reason: None, backend: "mock".into() }
        }
        fn generate(&self, _s: Option<&str>, _p: &str, _m: Option<u32>) -> krello_core::error::Result<String> {
            Ok(r#"{"title": ""}"#.into())
        }
    }
    assert!(ai::parse_task(&MockBad, "x").is_err());
}

#[test]
fn plan_project_parses_tasks_and_dependencies() {
    struct MockPlan;
    impl AiProvider for MockPlan {
        fn status(&self) -> krello_core::ai::provider::AiStatus {
            krello_core::ai::provider::AiStatus { available: true, reason: None, backend: "mock".into() }
        }
        fn generate(&self, _s: Option<&str>, _p: &str, _m: Option<u32>) -> krello_core::error::Result<String> {
            Ok(r#"{"tasks": [
                {"title": "Cadrage", "duration_days": 2, "milestone": false, "depends_on": []},
                {"title": "Dev", "duration_days": 5, "milestone": false, "depends_on": ["Cadrage"]},
                {"title": "Lancement", "duration_days": 0, "milestone": true, "depends_on": ["Dev"]}
            ]}"#.into())
        }
    }
    let plan = ai::plan_project(&MockPlan, "lancer un produit").unwrap();
    assert_eq!(plan.len(), 3);
    assert!(plan[2].milestone);
    assert_eq!(plan[1].depends_on, vec!["Cadrage".to_string()]);
}

#[test]
fn checklist_generation_validated() {
    struct MockItems;
    impl AiProvider for MockItems {
        fn status(&self) -> krello_core::ai::provider::AiStatus {
            krello_core::ai::provider::AiStatus { available: true, reason: None, backend: "mock".into() }
        }
        fn generate(&self, _s: Option<&str>, _p: &str, _m: Option<u32>) -> krello_core::error::Result<String> {
            Ok(r#"{"items": ["écrire le plan", "rédiger", "relire"]}"#.into())
        }
    }
    let items = ai::generate_checklist(&MockItems, "Article blog", "sujet: IA locale").unwrap();
    assert_eq!(items.len(), 3);
    let _ = test_service();
}
