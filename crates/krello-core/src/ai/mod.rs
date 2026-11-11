//! AI use cases: natural language → structured Krello data, all JSON-validated.

pub mod helper;
pub mod provider;

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::models::{CreateCard, PRIORITIES};
use crate::service::Service;
use crate::time::{parse_date, today};

use provider::AiProvider;

pub const JSON_ONLY_SUFFIX: &str =
    "\n\nRéponds UNIQUEMENT avec du JSON valide, sans texte avant ni après, sans balises de code.";

fn extract_json(text: &str) -> Result<serde_json::Value> {
    // Tolerate fences or prose around the JSON object.
    let start = text.find('{').ok_or_else(|| Error::Ai("pas de JSON dans la réponse".into()))?;
    let end = text.rfind('}').ok_or_else(|| Error::Ai("JSON incomplet".into()))?;
    if end < start {
        return Err(Error::Ai("JSON malformé".into()));
    }
    serde_json::from_str(&text[start..=end]).map_err(|e| Error::Ai(format!("JSON invalide: {e}")))
}

fn ask_json(provider: &dyn AiProvider, system: &str, prompt: &str) -> Result<serde_json::Value> {
    let text = provider.generate(Some(system), &format!("{prompt}{JSON_ONLY_SUFFIX}"), Some(2048))?;
    match extract_json(&text) {
        Ok(v) => Ok(v),
        Err(_) => {
            // one corrective retry
            let retry = provider.generate(
                Some(system),
                &format!(
                    "{prompt}{JSON_ONLY_SUFFIX}\n\nTa réponse précédente n'était pas du JSON valide : « {} ». Réponds cette fois UNIQUEMENT avec le JSON demandé.",
                    text.chars().take(200).collect::<String>()
                ),
                Some(2048),
            )?;
            extract_json(&retry)
        }
    }
}

// ---------- use cases ----------

#[derive(Debug, Clone, Deserialize, Default, serde::Serialize)]
pub struct ParsedTask {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub start: Option<String>,
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub checklist: Vec<String>,
}

/// Natural language → card fields (dates resolved against today).
pub fn parse_task(provider: &dyn AiProvider, input: &str) -> Result<ParsedTask> {
    let system = "Tu aides à créer des cartes de tâches Kanban à partir de texte libre.";
    let prompt = format!(
        "Convertis cette demande en tâche. Aujourd'hui : {}. \
Règles : dates au format YYYY-MM-DD ; priority ∈ [\"none\",\"low\",\"medium\",\"high\",\"urgent\"] (omise sinon évidente) ; \
labels = mots-clés courts ; checklist = étapes si l'énoncé en contient, sinon []. \
Demande : « {} » \
JSON attendu : {{\"title\": str, \"description\": str, \"start\": date|null, \"due\": date|null, \"priority\": str, \"labels\": [str], \"checklist\": [str]}}",
        today(),
        input
    );
    let v = ask_json(provider, system, &prompt)?;
    let parsed: ParsedTask = serde_json::from_value(v).map_err(|e| Error::Ai(format!("schéma tâche invalide: {e}")))?;
    for d in [&parsed.start, &parsed.due].into_iter().flatten() {
        if parse_date(d).is_none() {
            return Err(Error::Ai(format!("date générée invalide: {d}")));
        }
    }
    if let Some(p) = &parsed.priority {
        if !PRIORITIES.contains(&p.as_str()) {
            return Err(Error::Ai(format!("priorité invalide: {p}")));
        }
    }
    if parsed.title.trim().is_empty() {
        return Err(Error::Ai("titre manquant".into()));
    }
    Ok(parsed)
}

pub fn parsed_task_to_create_card(parsed: &ParsedTask, list_id: &str) -> CreateCard {
    CreateCard {
        title: parsed.title.clone(),
        description: parsed.description.clone(),
        list_id: list_id.to_string(),
        start: parsed.start.clone(),
        due: parsed.due.clone(),
        priority: parsed.priority.clone(),
        ..Default::default()
    }
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct PlannedTask {
    pub title: String,
    #[serde(default)]
    pub duration_days: f64,
    #[serde(default)]
    pub milestone: bool,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// Natural language goal → task breakdown with durations and dependencies.
pub fn plan_project(provider: &dyn AiProvider, goal: &str) -> Result<Vec<PlannedTask>> {
    let system = "Tu planifies des projets en tâches ordonnancées (chemin critique).";
    let prompt = format!(
        "Décompose cet objectif en 3 à 12 tâches enchaînées. \
Chaque tâche : title (str), duration_days (nombre > 0), milestone (bool), depends_on = titres des tâches prérequises. \
Objectif : « {} » \
JSON attendu : {{\"tasks\": [{{\"title\": str, \"duration_days\": number, \"milestone\": bool, \"depends_on\": [str]}}]}}",
        goal
    );
    let v = ask_json(provider, system, &prompt)?;
    let tasks: Vec<PlannedTask> = v
        .get("tasks")
        .cloned()
        .ok_or_else(|| Error::Ai("champ tasks manquant".into()))
        .and_then(|t| serde_json::from_value(t).map_err(|e| Error::Ai(format!("tasks invalides: {e}"))))?;
    if tasks.is_empty() {
        return Err(Error::Ai("plan vide".into()));
    }
    Ok(tasks)
}

/// Board summary for the dashboard AI panel.
pub fn summarize_board(provider: &dyn AiProvider, board_data: &serde_json::Value) -> Result<String> {
    let system = "Tu résumes l'état de projets pour un tableau de bord.";
    let prompt = format!(
        "Résume en 3 phrases maximum l'état de ce board (avancement, risques, échéances critiques) :\n{}",
        serde_json::to_string(board_data).map_err(|e| Error::Ai(e.to_string()))?
    );
    provider.generate(Some(system), &prompt, Some(1024))
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct PrioritySuggestion {
    pub card_id: String,
    pub priority: String,
    pub reason: String,
}

/// Suggest priorities for a set of cards.
pub fn suggest_priorities(
    provider: &dyn AiProvider,
    cards: &[serde_json::Value],
) -> Result<Vec<PrioritySuggestion>> {
    let system = "Tu priorises des tâches de façon pragmatique.";
    let prompt = format!(
        "Propose une priorité (none|low|medium|high|urgent) et une raison courte pour chaque carte :\n{} \
JSON attendu : {{\"suggestions\": [{{\"card_id\": str, \"priority\": str, \"reason\": str}}]}}",
        serde_json::to_string(cards).map_err(|e| Error::Ai(e.to_string()))?
    );
    let v = ask_json(provider, system, &prompt)?;
    let suggestions: Vec<PrioritySuggestion> = v
        .get("suggestions")
        .cloned()
        .ok_or_else(|| Error::Ai("champ suggestions manquant".into()))
        .and_then(|s| serde_json::from_value(s).map_err(|e| Error::Ai(format!("suggestions invalides: {e}"))))?;
    for s in &suggestions {
        if !PRIORITIES.contains(&s.priority.as_str()) {
            return Err(Error::Ai(format!("priorité invalide: {}", s.priority)));
        }
    }
    Ok(suggestions)
}

/// Rewrite/clarify a card description.
pub fn rewrite_description(provider: &dyn AiProvider, text: &str) -> Result<String> {
    let system = "Tu reformules des descriptions de tâches : claires, concises, structurées (Markdown léger).";
    provider.generate(Some(system), &format!("Reformule :\n{text}"), Some(1024))
}

/// Generate a checklist from a title/description.
pub fn generate_checklist(provider: &dyn AiProvider, title: &str, context: &str) -> Result<Vec<String>> {
    let system = "Tu crées des checklists actionnables.";
    let prompt = format!(
        "Génère 3 à 8 étapes concrètes pour cette tâche : « {} ». Contexte : {} \
JSON attendu : {{\"items\": [str]}}",
        title, context
    );
    let v = ask_json(provider, system, &prompt)?;
    let items: Vec<String> = v
        .get("items")
        .cloned()
        .ok_or_else(|| Error::Ai("champ items manquant".into()))
        .and_then(|i| serde_json::from_value(i).map_err(|e| Error::Ai(format!("items invalides: {e}"))))?;
    if items.is_empty() {
        return Err(Error::Ai("checklist vide".into()));
    }
    Ok(items)
}

/// Convenience: build compact JSON describing a board for AI prompts.
pub fn board_context(service: &Service, board_id: &str) -> Result<serde_json::Value> {
    let board = service.get_board(board_id)?;
    let lists = service.list_lists(board_id, false)?;
    let cards = service.list_cards(board_id, false)?;
    let cards_json: Vec<serde_json::Value> = cards
        .iter()
        .map(|c| {
            serde_json::json!({
                "id": c.id,
                "title": c.title,
                "list": lists.iter().find(|l| l.id == c.list_id).map(|l| l.title.clone()),
                "due": c.due,
                "priority": c.priority,
                "pct_complete": c.pct_complete,
                "milestone": c.is_milestone,
            })
        })
        .collect();
    Ok(serde_json::json!({
        "board": { "title": board.title, "start": board.start_date, "deadline": board.deadline },
        "lists": lists.iter().map(|l| l.title.clone()).collect::<Vec<_>>(),
        "cards": cards_json,
    }))
}
