//! Templates de cartes : conversion, instanciation (copie profonde),
//! réversibilité, indépendance template/instances, keepFromSource réduit.

mod common;

use common::*;
use krello_core::error::Error;
use krello_core::models::*;
use krello_core::service::Service;

fn toggle(s: &Service, id: &str, on: bool) -> Card {
    s.set_template(id, SetTemplate { is_template: on, template_description: None }).unwrap()
}

/// (titre checklist, [(titre item, coché, due)]) d'une carte.
type ChecklistSummary = Vec<(String, Vec<(String, bool, Option<String>)>)>;

fn checklists_of(s: &Service, card_id: &str) -> ChecklistSummary {
    let d = s.card_detail(card_id).unwrap();
    d["checklists"]
        .as_array()
        .unwrap()
        .iter()
        .map(|cl| {
            let title = cl["title"].as_str().unwrap().to_string();
            let items = cl["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|it| {
                    (
                        it["title"].as_str().unwrap().to_string(),
                        it["checked"].as_bool().unwrap_or(false),
                        it["due"].as_str().map(|s| s.to_string()),
                    )
                })
                .collect();
            (title, items)
        })
        .collect()
}

/// Carte riche : dates, complétion, priorité, label, champ perso, checklist
/// (2 items dont 1 coché), pièce jointe, commentaire, couverture couleur.
fn rich_card(s: &Service, board_id: &str, list_id: &str) -> Card {
    let c = s
        .create_card(
            board_id,
            CreateCard {
                title: "Revue hebdo".into(),
                description: "Contenu à copier".into(),
                list_id: list_id.into(),
                due: Some("2026-12-01T10:00:00".into()),
                start: Some("2026-11-20".into()),
                due_complete: Some(true),
                pct_complete: Some(100),
                priority: Some("high".into()),
                duration_days: Some(3.0),
                ..Default::default()
            },
        )
        .unwrap();
    let label = s
        .create_label(board_id, CreateLabel { name: "Qualité".into(), color: "#10b981".into() })
        .unwrap();
    s.attach_label(&c.id, &label.id).unwrap();
    let field = s
        .create_field_def(board_id, CreateFieldDef { name: "Criticité".into(), kind: "text".into(), options: None })
        .unwrap();
    s.set_field_value(&c.id, &field.id, "haute").unwrap();
    let cl = s.create_checklist(&c.id, CreateChecklist { title: "Étapes".into(), position: None }).unwrap();
    let i1 = s.create_checklist_item(&cl.id, CreateChecklistItem { title: "Préparer".into(), position: None, due: None }).unwrap();
    s.create_checklist_item(&cl.id, CreateChecklistItem { title: "Envoyer".into(), position: None, due: None }).unwrap();
    s.update_checklist_item(&i1.id, UpdateChecklistItem { checked: Some(true), ..Default::default() }).unwrap();
    s.add_attachment(&c.id, "note.txt", "text/plain", b"contenu piece jointe").unwrap();
    s.add_comment(&c.id, "un commentaire").unwrap();
    s.set_color_cover(&c.id, Some("#6366f1")).unwrap();
    c
}

#[test]
fn conversion_toggle_and_gallery_listing() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let c = make_card(&s, &l, "Modèle de revue");

    let t = s
        .set_template(&c.id, SetTemplate { is_template: true, template_description: Some("Contexte du modèle".into()) })
        .unwrap();
    assert!(t.is_template);
    assert_eq!(t.template_description.as_deref(), Some("Contexte du modèle"));
    let tpls = s.list_templates(&b).unwrap();
    assert_eq!(tpls.len(), 1);
    assert_eq!(tpls[0].id, c.id);

    // Reconversion réversible à volonté, aide purgée
    let back = s.set_template(&c.id, SetTemplate { is_template: false, template_description: None }).unwrap();
    assert!(!back.is_template);
    assert_eq!(back.template_description, None);
    assert!(s.list_templates(&b).unwrap().is_empty());
}

#[test]
fn instantiate_deep_copy_with_healthy_defaults() {
    let s = test_service();
    let (b, l1) = board_with_list(&s);
    let l2 = s.create_list(&b, CreateList { title: "Cible".into(), ..Default::default() }).unwrap();
    let tpl_card = rich_card(&s, &b, &l1);
    let old_cl = &checklists_of(&s, &tpl_card.id)[0];
    let old_att = &s.list_attachments(&tpl_card.id).unwrap()[0];

    let t = toggle(&s, &tpl_card.id, true);
    let card = s
        .instantiate_from_template(
            &t.id,
            InstantiateTemplate { list_id: l2.id.clone(), ..Default::default() },
        )
        .unwrap();

    // Toujours copié : titre (défaut = modèle) + description
    assert_eq!(card.title, "Revue hebdo");
    assert_eq!(card.description, "Contenu à copier");
    assert_ne!(card.id, t.id);

    // Jamais copié : dates, statut, progression, historique social
    assert!(card.start.is_none());
    assert!(card.due.is_none());
    assert!(!card.due_complete);
    assert!(card.completed_at.is_none());
    assert!(card.reminders.is_empty(), "les rappels ne sont jamais copiés");
    assert_eq!(card.pct_complete, 0);
    let detail = s.card_detail(&card.id).unwrap();
    assert!(detail["comments"].as_array().unwrap().is_empty());
    assert!(detail["custom_fields"].is_null() || detail["custom_fields"].as_object().map(|m| !m.is_empty()).unwrap_or(false));

    // Copié : labels, champs personnalisés, couverture
    let labels = detail["labels"].as_array().unwrap();
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0]["name"], "Qualité");
    assert_eq!(detail["custom_fields"].as_object().map(|m| m.len()).unwrap_or(0), 1);
    let cover = detail["card"]["cover"].as_str().unwrap();
    assert!(cover.contains("#6366f1"), "cover copied: {cover}");

    // Copié : checklists neuves, items tous décochés, sans date
    let cls = checklists_of(&s, &card.id);
    assert_eq!(cls.len(), 1);
    assert_eq!(cls[0].0, old_cl.0);
    assert_eq!(
        cls[0].1.iter().map(|(t, _, _)| t.clone()).collect::<Vec<_>>(),
        vec!["Préparer".to_string(), "Envoyer".to_string()]
    );
    assert!(cls[0].1.iter().all(|(_, checked, due)| !checked && due.is_none()));

    // Copié : pièce jointe dupliquée (nouvel id, octets copiés sur disque)
    let atts = s.list_attachments(&card.id).unwrap();
    assert_eq!(atts.len(), 1);
    assert_ne!(atts[0].id, old_att.id);
    let bytes = s
        .read_attachment_file(s.attachments_dir().join(&atts[0].stored_path).to_str().unwrap())
        .unwrap();
    assert_eq!(bytes, b"contenu piece jointe");

    // Position : top par défaut dans une liste vide
    assert_eq!(card.position, 0.0);

    // Renommage côté instance ne touche jamais le modèle
    s.update_card(&card.id, UpdateCard { title: Some("Revue instanciée".into()), ..Default::default() }).unwrap();
    assert_eq!(s.get_card(&t.id).unwrap().title, "Revue hebdo");
}

#[test]
fn instance_edits_never_propagate_to_template() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let tpl_card = rich_card(&s, &b, &l);
    let t = toggle(&s, &tpl_card.id, true);
    let card = s
        .instantiate_from_template(&t.id, InstantiateTemplate { list_id: l.clone(), ..Default::default() })
        .unwrap();

    // Modifier un item de l'instance ne change pas le modèle…
    let inst_detail = s.card_detail(&card.id).unwrap();
    let inst_cl = &inst_detail["checklists"].as_array().unwrap()[0];
    let item_id = inst_cl["items"].as_array().unwrap()[0]["id"].as_str().unwrap().to_string();
    s.update_checklist_item(&item_id, UpdateChecklistItem { checked: Some(true), ..Default::default() }).unwrap();
    // Le modèle garde exactement son état d'origine (Préparer coché chez lui, jamais transféré)
    let tpl_cl = &checklists_of(&s, &t.id)[0];
    assert_eq!(
        tpl_cl.1,
        vec![("Préparer".to_string(), true, None), ("Envoyer".to_string(), false, None)]
    );
    // …et réciproque
    let tpl_item_id = s.card_detail(&t.id).unwrap()["checklists"].as_array().unwrap()[0]
        ["items"].as_array().unwrap()[1]["id"].as_str().unwrap().to_string();
    s.update_checklist_item(&tpl_item_id, UpdateChecklistItem { title: Some("Modifié".into()), ..Default::default() }).unwrap();
    let inst_titles: Vec<String> = checklists_of(&s, &card.id)[0].1.iter().map(|(t, _, _)| t.clone()).collect();
    assert_eq!(inst_titles, vec!["Préparer".to_string(), "Envoyer".to_string()]);
}

#[test]
fn revert_then_dates_editable_and_delete_keeps_instances() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let tpl_card = rich_card(&s, &b, &l);
    let t = toggle(&s, &tpl_card.id, true);
    let card = s
        .instantiate_from_template(&t.id, InstantiateTemplate { list_id: l.clone(), ..Default::default() })
        .unwrap();

    // Reconvertir en carte : rééditable avec dates
    let back = toggle(&s, &t.id, false);
    s.update_card(&back.id, UpdateCard { due: Some("2027-01-15".into()), ..Default::default() }).unwrap();
    assert_eq!(s.get_card(&back.id).unwrap().due.as_deref(), Some("2027-01-15"));

    // Supprimer le modèle : les instances déjà créées restent intactes
    s.delete_card(&back.id).unwrap();
    let kept = s.get_card(&card.id).unwrap();
    assert_eq!(kept.title, "Revue hebdo");
    assert_eq!(s.list_attachments(&card.id).unwrap().len(), 1);
    assert_eq!(checklists_of(&s, &card.id).len(), 1);
}

#[test]
fn keep_subset_copies_only_requested_sections() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let tpl_card = rich_card(&s, &b, &l);
    let t = toggle(&s, &tpl_card.id, true);
    let card = s
        .instantiate_from_template(
            &t.id,
            InstantiateTemplate {
                list_id: l.clone(),
                keep: Some(vec![CopySection::Labels]),
                ..Default::default()
            },
        )
        .unwrap();

    let d = s.card_detail(&card.id).unwrap();
    assert_eq!(d["labels"].as_array().unwrap().len(), 1, "labels copiés");
    assert_eq!(checklists_of(&s, &card.id).len(), 0, "checklists exclues");
    assert_eq!(s.list_attachments(&card.id).unwrap().len(), 0, "pièces jointes exclues");
    assert_eq!(d["custom_fields"].as_object().map(|m| m.len()).unwrap_or(0), 0, "champs exclus");
    assert!(d["card"]["cover"].as_str().is_none(), "couverture exclue");
    // Titre + description toujours copiés
    assert_eq!(d["card"]["description"], "Contenu à copier");
}

#[test]
fn cover_attachment_is_remapped_to_the_copied_file() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let tpl_card = rich_card(&s, &b, &l);
    let att = s.list_attachments(&tpl_card.id).unwrap()[0].clone();
    s.set_cover(&tpl_card.id, &att.id).unwrap();
    let t = toggle(&s, &tpl_card.id, true);
    let card = s
        .instantiate_from_template(&t.id, InstantiateTemplate { list_id: l.clone(), ..Default::default() })
        .unwrap();

    let cover: serde_json::Value =
        serde_json::from_str(s.get_card(&card.id).unwrap().cover.as_deref().unwrap()).unwrap();
    assert_eq!(cover["type"], "attachment");
    let new_att_id = cover["value"].as_str().unwrap();
    assert_ne!(new_att_id, att.id, "la couverture pointe vers la copie, pas l'original");
    let atts = s.list_attachments(&card.id).unwrap();
    assert!(atts.iter().any(|a| a.id == new_att_id));
}

#[test]
fn position_top_bottom_and_explicit() {
    let s = test_service();
    let (_b, l) = board_with_list(&s);
    let c = make_card(&s, &l, "Modèle");
    let t = toggle(&s, &c.id, true);
    make_card(&s, &l, "Existante"); // position 1.0

    let top = s
        .instantiate_from_template(&t.id, InstantiateTemplate { list_id: l.clone(), at: Some("top".into()), ..Default::default() })
        .unwrap();
    let bottom = s
        .instantiate_from_template(&t.id, InstantiateTemplate { list_id: l.clone(), at: Some("bottom".into()), ..Default::default() })
        .unwrap();
    let explicit = s
        .instantiate_from_template(&t.id, InstantiateTemplate { list_id: l.clone(), position: Some(2.5), ..Default::default() })
        .unwrap();
    assert!(top.position < 1.0, "top insère au-dessus de l'existant");
    assert!(bottom.position > top.position);
    assert_eq!(explicit.position, 2.5);
}

#[test]
fn errors_and_undo() {
    let s = test_service();
    let (b, l) = board_with_list(&s);
    let c = make_card(&s, &l, "Carte ordinaire");

    // Non-template → validation
    let err = s
        .instantiate_from_template(&c.id, InstantiateTemplate { list_id: l.clone(), ..Default::default() })
        .unwrap_err();
    assert!(matches!(err, Error::Validation(_)));

    // Template inexistant → 404
    let err = s
        .instantiate_from_template("card-inexistant", InstantiateTemplate { list_id: l.clone(), ..Default::default() })
        .unwrap_err();
    assert!(matches!(err, Error::NotFound(_)));

    // Liste hors board → validation
    let (b2, l2) = board_with_list(&s);
    let t = toggle(&s, &c.id, true);
    let err = s
        .instantiate_from_template(&t.id, InstantiateTemplate { list_id: l2.clone(), ..Default::default() })
        .unwrap_err();
    assert!(matches!(err, Error::Validation(_)));
    assert_ne!(b, b2);

    // Undo de l'instanciation supprime la carte créée (pas d'orphelin)
    let card = s
        .instantiate_from_template(&t.id, InstantiateTemplate { list_id: l.clone(), ..Default::default() })
        .unwrap();
    s.undo().unwrap();
    assert!(matches!(s.get_card(&card.id), Err(Error::NotFound(_))));

    // Modèle supprimé pendant le dialogue → revalidation au moment du POST
    s.delete_card(&t.id).unwrap();
    let err = s
        .instantiate_from_template(&t.id, InstantiateTemplate { list_id: l.clone(), ..Default::default() })
        .unwrap_err();
    assert!(matches!(err, Error::NotFound(_)));
}
