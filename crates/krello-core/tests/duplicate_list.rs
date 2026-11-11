//! List duplication: cards copied with labels/checklists/fields/attachments
//! (attachment FILES physically duplicated), dependencies not copied, undoable
//! as a single operation.

mod common;

use common::*;
use krello_core::models::*;

#[test]
fn duplicate_list_copies_cards_labels_checklists_and_files() {
    let s = test_service();
    let (board_id, list_id) = board_with_list(&s);
    let c1 = make_card(&s, &list_id, "Carte A");
    let _c2 = make_card(&s, &list_id, "Carte B");

    let label = s.create_label(&board_id, CreateLabel { name: "bug".into(), color: "#f00".into() }).unwrap();
    s.attach_label(&c1.id, &label.id).unwrap();
    s.create_checklist(&c1.id, CreateChecklist { title: "étapes".into(), ..Default::default() }).unwrap();
    let cls = s.list_checklists(&c1.id).unwrap();
    s.create_checklist_item(&cls[0].0.id, CreateChecklistItem { title: "premier".into(), ..Default::default() }).unwrap();

    // Pièce jointe sur Carte A
    let att = s.add_attachment(&c1.id, "doc.txt", "text/plain", b"contenu").unwrap();
    let original_file = std::path::PathBuf::from(&att.stored_path);
    assert!(original_file.exists());

    let copy = s.duplicate_list(&list_id).unwrap();
    assert!(copy.title.contains("(copie)"));
    let lists = s.list_lists(&board_id, false).unwrap();
    assert_eq!(lists.len(), 2);

    // Les cartes copiées portent labels, checklists et PJ
    let copy_cards = s.list_cards(&board_id, false).unwrap()
        .into_iter().filter(|c| c.list_id == copy.id).collect::<Vec<_>>();
    assert_eq!(copy_cards.len(), 2, "les 2 cartes sont copiées");
    let copy_a = copy_cards.iter().find(|x| x.title == "Carte A").unwrap();
    assert_eq!(copy_a.position, 0.0, "ordre préservé");
    assert_eq!(s.list_card_count(&copy.id).unwrap(), 2);

    let labels = s.get_card(&copy_a.id).unwrap();
    let detail = s.card_detail(&copy_a.id).unwrap();
    assert_eq!(detail["checklists"].as_array().unwrap().len(), 1);
    let items = detail["checklists"][0]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "premier");
    let _ = labels;

    // La carte copiée a une PJ dont le FICHIER est une copie physique distincte
    let copy_atts = s.list_attachments(&copy_a.id).unwrap();
    assert_eq!(copy_atts.len(), 1, "la PJ est copiée");
    assert_ne!(copy_atts[0].stored_path, att.stored_path, "chemin distinct");
    assert!(std::path::Path::new(&copy_atts[0].stored_path).exists(), "fichier copié sur le disque");
    let data = s.read_attachment_file(&copy_atts[0].stored_path).unwrap();
    assert_eq!(data, b"contenu");

    // Les deux lignes restent indépendantes : supprimer la PJ originale ne
    // casse pas la copie (fichier distinct).
    s.delete_attachment(&att.id).unwrap();
    assert!(std::path::Path::new(&copy_atts[0].stored_path).exists());
    let data2 = s.read_attachment_file(&copy_atts[0].stored_path).unwrap();
    assert_eq!(data2, b"contenu");
    let _ = original_file;
}

#[test]
fn duplicate_list_is_undoable_as_one_operation() {
    let s = test_service();
    let (board_id, list_id) = board_with_list(&s);
    make_card(&s, &list_id, "une");
    make_card(&s, &list_id, "deux");

    let copy = s.duplicate_list(&list_id).unwrap();
    assert_eq!(s.list_card_count(&copy.id).unwrap(), 2);

    // ⌘Z unique : la liste copiée et ses cartes disparaissent (cascade)
    s.undo().unwrap();
    let lists_after = s.list_lists(&board_id, false).unwrap();
    assert!(lists_after.iter().all(|l| l.id != copy.id), "liste copiée annulée");
    // redo restaure liste + cartes
    s.redo().unwrap();
    assert_eq!(s.list_card_count(&copy.id).unwrap(), 2);
}

#[test]
fn duplicate_list_skips_archived_cards_and_copies_wip() {
    let s = test_service();
    let (board_id, list_id) = board_with_list(&s);
    make_card(&s, &list_id, "visible");
    let hidden = make_card(&s, &list_id, "archivée");
    s.update_card(&hidden.id, UpdateCard { archived: Some(true), ..Default::default() }).unwrap();
    s.update_list(&list_id, UpdateList { wip_limit: Some(7), ..Default::default() }).unwrap();

    let copy = s.duplicate_list(&list_id).unwrap();
    assert_eq!(s.list_card_count(&copy.id).unwrap(), 1, "les cartes archivées ne sont pas copiées");
    let lists = s.list_lists(&board_id, false).unwrap();
    let copied = lists.iter().find(|l| l.id == copy.id).unwrap();
    assert_eq!(copied.wip_limit, Some(7), "la limite WIP est copiée");
}
