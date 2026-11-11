//! All REST routes.

use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post, put};
use axum::{Json, Router};
use tower::ServiceExt;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{ApiError, ApiResult};
use crate::state::SharedState;
use krello_core::ai;
use krello_core::models::*;
use krello_core::search::{search_cards, SearchFilters};

pub fn router(state: SharedState) -> Router {
    let api = Router::new()
        // health & undo
        .route("/health", get(health))
        .route("/undo", post(undo).delete(clear_undo).get(undo_labels))
        .route("/redo", post(redo))
        // boards
        .route("/boards", get(list_boards).post(create_board).delete(delete_all_boards))
        .route("/boards/{id}", get(get_board).patch(update_board).delete(delete_board))
        .route("/boards/{id}/duplicate", post(duplicate_board))
        .route("/boards/{id}/activity", get(board_activity))
        .route("/boards/{id}/lists", get(list_lists).post(create_list))
        .route("/boards/{id}/cards", get(list_cards).post(create_card))
        .route("/boards/{id}/cards/meta", get(list_cards_meta))
        .route("/boards/{id}/templates", get(list_board_templates))
        .route("/boards/{id}/labels", get(list_labels).post(create_label))
        .route("/boards/{id}/fields", get(list_fields).post(create_field))
        .route("/boards/{id}/dependencies", get(list_deps).post(add_dep))
        .route("/boards/{id}/schedule", get(get_schedule).post(recompute_schedule))
        .route("/boards/{id}/baselines", get(list_baselines).post(create_baseline))
        .route("/boards/{id}/automations", get(list_automations).post(create_automation))
        .route("/boards/{id}/automations/runs", get(list_board_automation_runs))
        .route("/boards/{id}/export", get(export_board))
        .route("/boards/{id}/views/{view}", put(set_view_settings))
        .route("/boards/{id}/views", get(get_view_settings))
        // settings / backup
        .route("/settings/backup", get(get_backup_settings).put(put_backup_settings))
        .route("/settings/backup/run", post(run_backup_now))
        .route("/settings/backup/pick", post(pick_backup_dir))
        // lists
        .route("/lists/{id}", patch(update_list).delete(delete_list).get(get_list))
        .route("/lists/{id}/count", get(list_card_count))
        .route("/lists/{id}/duplicate", post(duplicate_list_route))
        // cards
        .route("/cards/{id}", get(get_card).patch(update_card).delete(delete_card))
        .route("/cards/{id}/move", post(move_card))
        .route("/cards/{id}/duplicate", post(duplicate_card))
        .route("/cards/{id}/template", patch(set_card_template))
        .route("/cards/{id}/instantiate", post(instantiate_card))
        .route("/cards/{id}/labels", put(set_card_labels))
        .route("/cards/{id}/labels/{label_id}", post(attach_label).delete(detach_label))
        .route("/cards/{id}/cover", post(set_cover_color))
        .route("/cards/{id}/fields/{field_id}", put(set_field_value))
        .route("/cards/{id}/checklists", get(list_checklists).post(create_checklist))
        .route("/cards/{id}/comments", get(list_comments).post(add_comment))
        .route("/cards/{id}/attachments", get(list_attachments).post(upload_attachment))
        // checklists
        .route("/checklists/{id}", delete(delete_checklist))
        .route("/checklists/{id}/items", post(create_checklist_item))
        .route("/checklist-items/{id}", patch(update_checklist_item).delete(delete_checklist_item))
        // labels / fields / deps / baselines / automations / calendars / comments / attachments
        .route("/labels/{id}", patch(update_label).delete(delete_label))
        .route("/fields/{id}", patch(update_field).delete(delete_field))
        .route("/dependencies/{id}", patch(update_dep).delete(remove_dep))
        .route("/baselines/{id}", delete(delete_baseline))
        .route("/baselines/{id}/compare", get(compare_baseline))
        .route("/automations/{id}", patch(update_automation).delete(delete_automation))
        .route("/automations/{id}/run", post(run_automation))
        .route("/automations/{id}/runs", get(list_automation_runs))
        .route("/calendars", get(list_calendars).post(create_calendar))
        .route("/calendars/{id}", patch(update_calendar).delete(delete_calendar))
        .route("/comments/{id}", delete(delete_comment))
        .route("/attachments/{id}", delete(delete_attachment))
        .route("/attachments/{id}/download", get(download_attachment))
        .route("/attachments/{id}/cover", post(set_attachment_cover))
        // search & import
        .route("/search", get(search))
        .route("/import/trello", post(import_trello_route))
        .route("/import/krello", post(import_krello_route))
        // AI
        .route("/ai/status", get(ai_status))
        .route("/ai/parse-task", post(ai_parse_task))
        .route("/ai/plan", post(ai_plan))
        .route("/ai/summarize", post(ai_summarize))
        .route("/ai/priorities", post(ai_priorities))
        .route("/ai/rewrite", post(ai_rewrite))
        .route("/ai/checklist", post(ai_checklist))
        .layer(DefaultBodyLimit::max(80 * 1024 * 1024));

    Router::new()
        .route("/api/v1/ws", get(crate::ws::ws_handler))
        .nest("/api/v1", api)
        .with_state(state)
}

// ---------- helpers ----------

fn ok_json(v: Value) -> Response {
    (StatusCode::OK, Json(v)).into_response()
}

// ---------- health / undo ----------

async fn health(State(state): State<SharedState>) -> ApiResult<Response> {
    let boards = state.service.list_boards(false)?.len();
    Ok(ok_json(json!({
        "ok": true,
        "name": "krello",
        "version": env!("CARGO_PKG_VERSION"),
        "boards": boards,
        "ai": state.ai.status(),
    })))
}

async fn undo(State(state): State<SharedState>) -> ApiResult<Response> {
    match state.service.undo()? {
        Some(label) => Ok(ok_json(json!({ "undone": label }))),
        None => Ok(ok_json(json!({ "undone": null }))),
    }
}

async fn redo(State(state): State<SharedState>) -> ApiResult<Response> {
    match state.service.redo()? {
        Some(label) => Ok(ok_json(json!({ "redone": label }))),
        None => Ok(ok_json(json!({ "redone": null }))),
    }
}

async fn undo_labels(State(state): State<SharedState>) -> ApiResult<Response> {
    Ok(ok_json(json!({ "labels": state.service.undo_labels() })))
}

async fn clear_undo(State(state): State<SharedState>) -> ApiResult<Response> {
    state.service.clear_undo();
    Ok(ok_json(json!({ "cleared": true })))
}

// ---------- boards ----------

#[derive(Deserialize, Default)]
#[serde(default)]
struct IncludeArchived {
    include_archived: Option<bool>,
}

async fn list_boards(State(state): State<SharedState>, Query(q): Query<IncludeArchived>) -> ApiResult<Response> {
    let boards = state.service.list_boards(q.include_archived.unwrap_or(false))?;
    Ok(Json(boards).into_response())
}

async fn create_board(State(state): State<SharedState>, Json(c): Json<CreateBoard>) -> ApiResult<Response> {
    let board = state.service.create_board(c)?;
    Ok((StatusCode::CREATED, Json(board)).into_response())
}

async fn get_board(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let board = state.service.get_board(&id)?;
    Ok(Json(board).into_response())
}

async fn update_board(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(u): Json<UpdateBoard>,
) -> ApiResult<Response> {
    let board = state.service.update_board(&id, u)?;
    Ok(Json(board).into_response())
}

async fn delete_board(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    state.service.delete_board(&id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn delete_all_boards(State(state): State<SharedState>) -> ApiResult<Response> {
    let n = state.service.delete_all_boards()?;
    Ok(ok_json(json!({ "deleted": n })))
}

async fn duplicate_board(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let board = state.service.duplicate_board(&id)?;
    Ok((StatusCode::CREATED, Json(board)).into_response())
}

/// Bound a client-provided page limit (1..=500, fallback to `default`).
fn clamp_limit(v: Option<i64>, default: i64) -> i64 {
    v.unwrap_or(default).clamp(1, 500)
}

async fn board_activity(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Query(q): Query<ActivityQuery>,
) -> ApiResult<Response> {
    let acts = match &q.card_id {
        Some(card_id) => state.service.card_activity(&id, card_id, clamp_limit(q.limit, 100))?,
        None => state.service.board_activity(&id, clamp_limit(q.limit, 100))?,
    };
    Ok(Json(acts).into_response())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ActivityQuery {
    limit: Option<i64>,
    card_id: Option<String>,
}

// ---------- lists ----------

async fn list_lists(State(state): State<SharedState>, Path(id): Path<String>, Query(q): Query<IncludeArchived>) -> ApiResult<Response> {
    let lists = state.service.list_lists(&id, q.include_archived.unwrap_or(false))?;
    Ok(Json(lists).into_response())
}

async fn create_list(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(c): Json<CreateList>,
) -> ApiResult<Response> {
    let list = state.service.create_list(&id, c)?;
    Ok((StatusCode::CREATED, Json(list)).into_response())
}

async fn get_list(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let list = state.service.get_list(&id)?;
    Ok(Json(list).into_response())
}

async fn update_list(State(state): State<SharedState>, Path(id): Path<String>, Json(u): Json<UpdateList>) -> ApiResult<Response> {
    let list = state.service.update_list(&id, u)?;
    Ok(Json(list).into_response())
}

async fn delete_list(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    state.service.delete_list(&id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn list_card_count(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let count = state.service.list_card_count(&id)?;
    let exceeded = state.service.wip_exceeded(&id)?;
    Ok(ok_json(json!({ "count": count, "wip_exceeded": exceeded })))
}

// ---------- cards ----------

async fn list_cards(State(state): State<SharedState>, Path(id): Path<String>, Query(q): Query<IncludeArchived>) -> ApiResult<Response> {
    let cards = state.service.list_cards(&id, q.include_archived.unwrap_or(false))?;
    Ok(Json(cards).into_response())
}

/// Per-card metadata (label ids + checklist/comment/attachment counts) for a
/// whole board in a fixed number of queries — replaces N × GET /cards/{id}.
async fn list_cards_meta(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let meta = state.service.cards_meta(&id)?;
    Ok(Json(meta).into_response())
}

async fn create_card(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(c): Json<CreateCard>,
) -> ApiResult<Response> {
    let card = state.service.create_card(&id, c)?;
    Ok((StatusCode::CREATED, Json(card)).into_response())
}

async fn get_card(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let detail = state.service.card_detail(&id)?;
    Ok(Json(detail).into_response())
}

async fn update_card(State(state): State<SharedState>, Path(id): Path<String>, Json(u): Json<UpdateCard>) -> ApiResult<Response> {
    let card = state.service.update_card(&id, u)?;
    Ok(Json(card).into_response())
}

async fn delete_card(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    state.service.delete_card(&id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn move_card(State(state): State<SharedState>, Path(id): Path<String>, Json(m): Json<MoveCard>) -> ApiResult<Response> {
    let card = state.service.move_card(&id, m)?;
    Ok(Json(card).into_response())
}

async fn duplicate_card(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let card = state.service.duplicate_card(&id)?;
    Ok((StatusCode::CREATED, Json(card)).into_response())
}

async fn set_card_template(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(s): Json<SetTemplate>,
) -> ApiResult<Response> {
    let card = state.service.set_template(&id, s)?;
    Ok(Json(card).into_response())
}

async fn instantiate_card(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(r): Json<InstantiateTemplate>,
) -> ApiResult<Response> {
    let card = state.service.instantiate_from_template(&id, r)?;
    Ok((StatusCode::CREATED, Json(card)).into_response())
}

async fn list_board_templates(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let cards = state.service.list_templates(&id)?;
    Ok(Json(cards).into_response())
}

async fn set_card_labels(State(state): State<SharedState>, Path(id): Path<String>, Json(body): Json<Vec<String>>) -> ApiResult<Response> {
    state.service.set_card_labels(&id, body)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn attach_label(State(state): State<SharedState>, Path((id, label_id)): Path<(String, String)>) -> ApiResult<Response> {
    state.service.attach_label(&id, &label_id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn detach_label(State(state): State<SharedState>, Path((id, label_id)): Path<(String, String)>) -> ApiResult<Response> {
    state.service.detach_label(&id, &label_id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct CoverColor {
    color: Option<String>,
}

async fn set_cover_color(State(state): State<SharedState>, Path(id): Path<String>, Json(body): Json<CoverColor>) -> ApiResult<Response> {
    state.service.set_color_cover(&id, body.color.as_deref())?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn set_field_value(
    State(state): State<SharedState>,
    Path((id, field_id)): Path<(String, String)>,
    Json(body): Json<SetFieldValue>,
) -> ApiResult<Response> {
    state.service.set_field_value(&id, &field_id, &body.value)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ---------- labels ----------

async fn list_labels(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let labels = state.service.list_labels(&id)?;
    Ok(Json(labels).into_response())
}

async fn create_label(State(state): State<SharedState>, Path(id): Path<String>, Json(c): Json<CreateLabel>) -> ApiResult<Response> {
    let label = state.service.create_label(&id, c)?;
    Ok((StatusCode::CREATED, Json(label)).into_response())
}

async fn update_label(State(state): State<SharedState>, Path(id): Path<String>, Json(u): Json<UpdateLabel>) -> ApiResult<Response> {
    let label = state.service.update_label(&id, u)?;
    Ok(Json(label).into_response())
}

async fn delete_label(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    state.service.delete_label(&id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ---------- checklists ----------

async fn list_checklists(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let cls = state.service.list_checklists(&id)?;
    let json: Vec<Value> = cls
        .into_iter()
        .map(|(cl, items)| {
            let mut v = serde_json::to_value(&cl).unwrap_or_default();
            if let Value::Object(map) = &mut v {
                map.insert("items".into(), serde_json::to_value(&items).unwrap_or_default());
            }
            v
        })
        .collect();
    Ok(Json(json).into_response())
}

async fn create_checklist(State(state): State<SharedState>, Path(id): Path<String>, Json(c): Json<CreateChecklist>) -> ApiResult<Response> {
    let cl = state.service.create_checklist(&id, c)?;
    Ok((StatusCode::CREATED, Json(cl)).into_response())
}

async fn delete_checklist(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    state.service.delete_checklist(&id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn create_checklist_item(State(state): State<SharedState>, Path(id): Path<String>, Json(c): Json<CreateChecklistItem>) -> ApiResult<Response> {
    let item = state.service.create_checklist_item(&id, c)?;
    Ok((StatusCode::CREATED, Json(item)).into_response())
}

async fn update_checklist_item(State(state): State<SharedState>, Path(id): Path<String>, Json(u): Json<UpdateChecklistItem>) -> ApiResult<Response> {
    let item = state.service.update_checklist_item(&id, u)?;
    Ok(Json(item).into_response())
}

async fn delete_checklist_item(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    state.service.delete_checklist_item(&id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ---------- custom fields ----------

async fn list_fields(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let defs = state.service.list_field_defs(&id)?;
    Ok(Json(defs).into_response())
}

async fn create_field(State(state): State<SharedState>, Path(id): Path<String>, Json(c): Json<CreateFieldDef>) -> ApiResult<Response> {
    let def = state.service.create_field_def(&id, c)?;
    Ok((StatusCode::CREATED, Json(def)).into_response())
}

async fn update_field(State(state): State<SharedState>, Path(id): Path<String>, Json(u): Json<UpdateFieldDef>) -> ApiResult<Response> {
    let def = state.service.update_field_def(&id, u)?;
    Ok(Json(def).into_response())
}

async fn delete_field(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    state.service.delete_field_def(&id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ---------- comments ----------

async fn list_comments(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let comments = state.service.list_comments(&id)?;
    Ok(Json(comments).into_response())
}

#[derive(Deserialize)]
struct CommentBody {
    body: String,
}

async fn add_comment(State(state): State<SharedState>, Path(id): Path<String>, Json(body): Json<CommentBody>) -> ApiResult<Response> {
    let comment = state.service.add_comment(&id, &body.body)?;
    Ok((StatusCode::CREATED, Json(comment)).into_response())
}

async fn delete_comment(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    state.service.delete_comment(&id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ---------- attachments ----------

async fn upload_attachment(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> ApiResult<Response> {
    let mut result: Option<krello_core::models::Attachment> = None;
    while let Some(field) = multipart.next_field().await.map_err(|e| ApiError(krello_core::Error::Validation(format!("multipart: {e}"))))? {
        if field.name() == Some("file") {
            let filename = field.file_name().unwrap_or("fichier").to_string();
            let mime = field.content_type().unwrap_or("application/octet-stream").to_string();
            let data = field.bytes().await.map_err(|e| ApiError(krello_core::Error::Validation(format!("lecture: {e}"))))?;
            let att = state.service.add_attachment(&id, &filename, &mime, &data)?;
            result = Some(att);
        }
    }
    match result {
        Some(att) => Ok((StatusCode::CREATED, Json(att)).into_response()),
        None => Err(ApiError(krello_core::Error::Validation("champ 'file' manquant".into()))),
    }
}

async fn list_attachments(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let atts = state.service.list_attachments(&id)?;
    Ok(Json(atts).into_response())
}

async fn download_attachment(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    req: Request,
) -> ApiResult<Response> {
    let att = state.service.get_attachment(&id).map_err(ApiError)?;
    let path = state.service.attachment_path_checked(&att.stored_path).map_err(ApiError)?;
    // Stream the file (supports HTTP Range) instead of buffering up to 64 MiB.
    let res = tower_http::services::ServeFile::new(path)
        .oneshot(req)
        .await
        .map_err(|e| ApiError(krello_core::Error::NotFound(format!("fichier illisible: {e}"))))?;
    let mut res = res.map(axum::body::Body::new);
    let mime = axum::http::HeaderValue::from_str(&att.mime)
        .unwrap_or_else(|_| axum::http::HeaderValue::from_static("application/octet-stream"));
    let disposition = format!(
        "attachment; filename=\"{}\"",
        att.filename.replace(['"', '\r', '\n', '\\'], "")
    );
    res.headers_mut().insert(axum::http::header::CONTENT_TYPE, mime);
    if let Ok(v) = axum::http::HeaderValue::from_str(&disposition) {
        res.headers_mut()
            .insert(axum::http::header::CONTENT_DISPOSITION, v);
    }
    Ok(res)
}

async fn delete_attachment(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    state.service.delete_attachment(&id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn set_attachment_cover(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let att = state.service.get_attachment(&id).map_err(ApiError)?;
    state.service.set_cover(&att.card_id, &id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ---------- dependencies & schedule ----------

async fn list_deps(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let deps = state.service.list_dependencies(&id)?;
    Ok(Json(deps).into_response())
}

async fn add_dep(State(state): State<SharedState>, Path(id): Path<String>, Json(c): Json<CreateDependency>) -> ApiResult<Response> {
    let dep = state.service.add_dependency(&id, c)?;
    Ok((StatusCode::CREATED, Json(dep)).into_response())
}

async fn update_dep(State(state): State<SharedState>, Path(id): Path<String>, Json(u): Json<UpdateDependency>) -> ApiResult<Response> {
    let dep = state.service.update_dependency(&id, u)?;
    Ok(Json(dep).into_response())
}

async fn remove_dep(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    state.service.remove_dependency(&id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn get_schedule(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let summary = state.service.schedule_for_board(&id)?;
    Ok(Json(summary).into_response())
}

async fn recompute_schedule(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let summary = state.service.recompute_and_notify_public(&id)?;
    Ok(Json(summary).into_response())
}

// ---------- baselines ----------

async fn list_baselines(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let baselines = state.service.list_baselines(&id)?;
    Ok(Json(baselines).into_response())
}

#[derive(Deserialize)]
struct BaselineBody {
    name: Option<String>,
}

async fn create_baseline(State(state): State<SharedState>, Path(id): Path<String>, Json(body): Json<BaselineBody>) -> ApiResult<Response> {
    let name = body.name.as_deref().unwrap_or("baseline");
    let baseline = state.service.create_baseline(&id, name)?;
    Ok((StatusCode::CREATED, Json(baseline)).into_response())
}

async fn delete_baseline(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    state.service.delete_baseline(&id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn compare_baseline(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let diffs = state.service.compare_baseline(&id)?;
    Ok(Json(diffs).into_response())
}

// ---------- automations ----------

async fn list_automations(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let autos = state.service.list_automations(&id)?;
    Ok(Json(autos).into_response())
}

async fn create_automation(State(state): State<SharedState>, Path(id): Path<String>, Json(c): Json<CreateAutomation>) -> ApiResult<Response> {
    let auto = state.service.create_automation(&id, c)?;
    Ok((StatusCode::CREATED, Json(auto)).into_response())
}

async fn update_automation(State(state): State<SharedState>, Path(id): Path<String>, Json(u): Json<UpdateAutomation>) -> ApiResult<Response> {
    let auto = state.service.update_automation(&id, u)?;
    Ok(Json(auto).into_response())
}

async fn delete_automation(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    state.service.delete_automation(&id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn run_automation(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let count = state.service.run_automation(&id)?;
    Ok(ok_json(json!({ "cards_touched": count })))
}

async fn list_automation_runs(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Query(q): Query<RunsQuery>,
) -> ApiResult<Response> {
    let runs = state.service.list_automation_runs(&id, clamp_limit(q.limit, 50))?;
    Ok(Json(runs).into_response())
}

async fn list_board_automation_runs(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Query(q): Query<RunsQuery>,
) -> ApiResult<Response> {
    let runs = state.service.list_board_automation_runs(&id, clamp_limit(q.limit, 100))?;
    Ok(Json(runs).into_response())
}

#[derive(serde::Deserialize)]
struct RunsQuery {
    limit: Option<i64>,
}

// ---------- calendars ----------

async fn list_calendars(State(state): State<SharedState>) -> ApiResult<Response> {
    let cals = state.service.list_calendars()?;
    Ok(Json(cals).into_response())
}

async fn create_calendar(State(state): State<SharedState>, Json(c): Json<CreateCalendar>) -> ApiResult<Response> {
    let cal = state.service.create_calendar(c)?;
    Ok((StatusCode::CREATED, Json(cal)).into_response())
}

async fn update_calendar(State(state): State<SharedState>, Path(id): Path<String>, Json(u): Json<UpdateCalendar>) -> ApiResult<Response> {
    let cal = state.service.update_calendar(&id, u)?;
    Ok(Json(cal).into_response())
}

async fn delete_calendar(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    state.service.delete_calendar(&id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ---------- search ----------

#[derive(Deserialize, Default)]
#[serde(default)]
struct SearchQuery {
    q: Option<String>,
    board_id: Option<String>,
    list_id: Option<String>,
    label_id: Option<String>,
    priority: Option<String>,
    due_before: Option<String>,
    due_after: Option<String>,
    has_due: Option<bool>,
    include_archived: Option<bool>,
}

async fn search(State(state): State<SharedState>, Query(q): Query<SearchQuery>) -> ApiResult<Response> {
    let filters = SearchFilters {
        board_id: q.board_id,
        list_id: q.list_id,
        label_id: q.label_id,
        priority: q.priority,
        due_before: q.due_before,
        due_after: q.due_after,
        has_due: q.has_due,
        include_archived: q.include_archived.unwrap_or(false),
        limit: 200,
    };
    let cards = state
        .service
        .with_db(|conn| search_cards(conn, q.q.as_deref(), &filters))
        .map_err(ApiError)?;
    Ok(Json(cards).into_response())
}

// ---------- export / import ----------

#[derive(Deserialize)]
struct ExportQuery {
    format: Option<String>,
}

async fn export_board(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Query(q): Query<ExportQuery>,
) -> ApiResult<Response> {
    let format = krello_core::exim::ExportFormat::parse(q.format.as_deref().unwrap_or("json"))?;
    let (filename, mime, body) = state
        .service
        .with_db(|conn| krello_core::exim::export_board(conn, &id, format))
        .map_err(ApiError)?;
    Ok((
        [
            ("Content-Type", mime),
            ("Content-Disposition", format!("attachment; filename=\"{filename}\"").as_str()),
        ],
        body,
    )
        .into_response())
}

async fn import_trello_route(State(state): State<SharedState>, body: String) -> ApiResult<Response> {
    let board = krello_core::exim::import_trello(&state.service, &body).map_err(ApiError)?;
    Ok((StatusCode::CREATED, Json(board)).into_response())
}

async fn import_krello_route(State(state): State<SharedState>, body: String) -> ApiResult<Response> {
    let board = krello_core::exim::import_krello(&state.service, &body).map_err(ApiError)?;
    Ok((StatusCode::CREATED, Json(board)).into_response())
}

// ---------- views ----------

#[derive(Deserialize)]
struct ViewSettings {
    settings: Value,
}

async fn set_view_settings(
    State(state): State<SharedState>,
    Path((id, view)): Path<(String, String)>,
    Json(body): Json<ViewSettings>,
) -> ApiResult<Response> {
    if !VIEWS.contains(&view.as_str()) {
        return Err(ApiError(krello_core::Error::Validation(format!("vue inconnue: {view}"))));
    }
    let settings = serde_json::to_string(&body.settings).map_err(|e| ApiError(krello_core::Error::Validation(e.to_string())))?;
    state
        .service
        .set_view_settings(&id, &view, &settings)
        .map_err(ApiError)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn get_view_settings(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let settings = state.service.get_view_settings(&id).map_err(ApiError)?;
    let json: Vec<Value> = settings
        .into_iter()
        .map(|(view, settings)| {
            let parsed = serde_json::from_str::<Value>(&settings).unwrap_or(Value::Null);
            json!({ "view": view, "settings": parsed })
        })
        .collect();
    Ok(Json(json).into_response())
}

// ---------- AI ----------

async fn ai_status(State(state): State<SharedState>) -> ApiResult<Response> {
    Ok(ok_json(serde_json::to_value(state.ai.status()).unwrap_or_default()))
}

#[derive(Deserialize)]
struct ParseTaskBody {
    input: String,
    list_id: String,
    #[serde(default)]
    create: bool,
}

async fn ai_parse_task(State(state): State<SharedState>, Json(body): Json<ParseTaskBody>) -> ApiResult<Response> {
    let parsed = ai::parse_task(state.ai.as_ref(), &body.input).map_err(ApiError)?;
    if body.create {
        let card = state.service.create_card(
            &state.service.get_list(&body.list_id).map_err(ApiError)?.board_id,
            ai::parsed_task_to_create_card(&parsed, &body.list_id),
        )?;
        return Ok((StatusCode::CREATED, Json(json!({ "parsed": parsed, "card": card }))).into_response());
    }
    Ok(ok_json(json!({ "parsed": parsed })))
}

#[derive(Deserialize)]
struct PlanBody {
    goal: String,
    board_id: String,
    list_id: String,
    #[serde(default)]
    apply: bool,
}

async fn ai_plan(State(state): State<SharedState>, Json(body): Json<PlanBody>) -> ApiResult<Response> {
    let plan = ai::plan_project(state.ai.as_ref(), &body.goal).map_err(ApiError)?;
    if !body.apply {
        return Ok(ok_json(json!({ "plan": plan })));
    }
    // create cards then FS dependencies following depends_on titles
    let board = state.service.get_board(&body.board_id).map_err(ApiError)?;
    let mut created = Vec::new();
    let mut title_to_id: std::collections::HashMap<String, String> = Default::default();
    for t in &plan {
        let mut c = ai::parsed_task_to_create_card(
            &ai::ParsedTask {
                title: t.title.clone(),
                description: String::new(),
                start: None,
                due: None,
                priority: None,
                labels: Vec::new(),
                checklist: Vec::new(),
            },
            &body.list_id,
        );
        c.duration_days = Some(t.duration_days.max(0.0));
        c.is_milestone = Some(t.milestone);
        let card = state.service.create_card(&board.id, c)?;
        title_to_id.insert(t.title.clone(), card.id.clone());
        created.push(card);
    }
    let mut deps = Vec::new();
    for t in &plan {
        for pred_title in &t.depends_on {
            if let (Some(pred), Some(succ)) = (title_to_id.get(pred_title), title_to_id.get(&t.title)) {
                let dep = state
                    .service
                    .add_dependency(
                        &board.id,
                        CreateDependency {
                            predecessor_id: pred.clone(),
                            successor_id: succ.clone(),
                            kind: "FS".into(),
                            lag_days: None,
                        },
                    )
                    .map_err(ApiError)?;
                deps.push(dep);
            }
        }
    }
    let summary = state.service.recompute_and_notify_public(&board.id).map_err(ApiError)?;
    Ok((StatusCode::CREATED, Json(json!({ "plan": plan, "cards": created, "dependencies": deps, "schedule": summary }))).into_response())
}

#[derive(Deserialize)]
struct SummarizeBody {
    board_id: String,
}

async fn ai_summarize(State(state): State<SharedState>, Json(body): Json<SummarizeBody>) -> ApiResult<Response> {
    let context = ai::board_context(&state.service, &body.board_id).map_err(ApiError)?;
    let summary = ai::summarize_board(state.ai.as_ref(), &context).map_err(ApiError)?;
    Ok(ok_json(json!({ "summary": summary })))
}

#[derive(Deserialize)]
struct PrioritiesBody {
    board_id: String,
    #[serde(default)]
    apply: bool,
}

async fn ai_priorities(State(state): State<SharedState>, Json(body): Json<PrioritiesBody>) -> ApiResult<Response> {
    let context = ai::board_context(&state.service, &body.board_id).map_err(ApiError)?;
    let cards = context
        .get("cards")
        .cloned()
        .unwrap_or_else(|| Value::Array(vec![]));
    let arr = cards.as_array().cloned().unwrap_or_default();
    let suggestions = ai::suggest_priorities(state.ai.as_ref(), &arr).map_err(ApiError)?;
    if body.apply {
        for s in &suggestions {
            if let Err(e) = state.service.update_card(
                &s.card_id,
                UpdateCard { priority: Some(s.priority.clone()), ..Default::default() },
            ) {
                tracing::warn!("ai_priorities: apply carte {} échoué: {e}", s.card_id);
            }
        }
    }
    Ok(ok_json(json!({ "suggestions": suggestions, "applied": body.apply })))
}

#[derive(Deserialize)]
struct RewriteBody {
    text: String,
}

async fn ai_rewrite(State(state): State<SharedState>, Json(body): Json<RewriteBody>) -> ApiResult<Response> {
    let text = ai::rewrite_description(state.ai.as_ref(), &body.text).map_err(ApiError)?;
    Ok(ok_json(json!({ "text": text })))
}

#[derive(Deserialize)]
struct ChecklistBody {
    card_id: String,
    #[serde(default)]
    apply: bool,
}

async fn ai_checklist(State(state): State<SharedState>, Json(body): Json<ChecklistBody>) -> ApiResult<Response> {
    let card = state.service.get_card(&body.card_id).map_err(ApiError)?;
    let items = ai::generate_checklist(state.ai.as_ref(), &card.title, &card.description).map_err(ApiError)?;
    if body.apply {
        let cl = state
            .service
            .create_checklist(
                &body.card_id,
                CreateChecklist { title: "Checklist IA".into(), ..Default::default() },
            )
            .map_err(ApiError)?;
        let mut created = Vec::new();
        for item in items {
            let it = state
                .service
                .create_checklist_item(&cl.id, CreateChecklistItem { title: item, ..Default::default() })
                .map_err(ApiError)?;
            created.push(it);
        }
        return Ok((StatusCode::CREATED, Json(json!({ "checklist": cl, "items": created }))).into_response());
    }
    Ok(ok_json(json!({ "items": items })))
}

// ---------- settings / backup ----------

#[derive(Deserialize, Default)]
#[serde(default)]
struct BackupSettingsBody {
    backup_dir: Option<String>,
    interval_hours: Option<i64>,
    keep: Option<i64>,
}

async fn get_backup_settings(State(state): State<SharedState>) -> ApiResult<Response> {
    let s = state.service.backup_settings()?;
    Ok(ok_json(serde_json::to_value(s).unwrap_or_default()))
}

async fn put_backup_settings(
    State(state): State<SharedState>,
    Json(b): Json<BackupSettingsBody>,
) -> ApiResult<Response> {
    let s = state.service.set_backup_settings(b.backup_dir, b.interval_hours, b.keep)?;
    Ok(ok_json(serde_json::to_value(s).unwrap_or_default()))
}

async fn run_backup_now(State(state): State<SharedState>) -> ApiResult<Response> {
    let path = state.service.run_backup_now()?;
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Ok(ok_json(json!({ "file": path.to_string_lossy(), "size": size })))
}

// ---------- list duplicate ----------

async fn duplicate_list_route(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Response> {
    let list = state.service.duplicate_list(&id)?;
    Ok((StatusCode::CREATED, Json(list)).into_response())
}

/// Native directory picker (Finder / Explorer). Only available when the server
/// runs inside the Tauri app (`KRELLO_NATIVE_DIALOG=1` set by src-tauri), since
/// a headless CLI process has no window environment. The dialog is opened by
/// the shared in-process server, so no webview IPC is needed.
async fn pick_backup_dir() -> ApiResult<Response> {
    if std::env::var("KRELLO_NATIVE_DIALOG").as_deref() != Ok("1") {
        return Err(ApiError(krello_core::Error::Validation(
            "le sélecteur de répertoire natif n'est disponible que dans l'app Krello (fenêtre native) — saisissez le chemin manuellement en mode navigateur".into(),
        )));
    }
    let picked = rfd::AsyncFileDialog::new()
        .set_title("Choisir le répertoire de sauvegarde")
        .pick_folder()
        .await
        .map(|h| h.path().to_string_lossy().into_owned());
    Ok(ok_json(json!({ "dir": picked })))
}
