//! Attachment storage: round-trip read, path-traversal guard, size limit.

mod common;

use common::*;
use krello_core::Error;
use krello_core::service::attachments::MAX_ATTACHMENT_BYTES;

#[test]
fn attachment_upload_then_read_roundtrip() {
    let s = test_service();
    let (_board_id, list_id) = board_with_list(&s);
    let card = make_card(&s, &list_id, "avec pièce jointe");
    let att = s
        .add_attachment(&card.id, "rapport Final.PDF", "application/pdf", b"hello")
        .unwrap();
    assert!(att.stored_path.ends_with(".pdf"), "extension conservée en minuscules");
    let data = s.read_attachment_file(&att.stored_path).unwrap();
    assert_eq!(data, b"hello");
    // Le chemin résolu reste dans le répertoire des pièces jointes.
    let p = s.attachment_path_checked(&att.stored_path).unwrap();
    let dir = s.attachments_dir().canonicalize().unwrap();
    assert!(p.starts_with(&dir));
}
#[test]
fn path_traversal_outside_attachments_dir_is_rejected() {
    let s = test_service();
    std::fs::create_dir_all(s.attachments_dir()).unwrap();
    // Chemin absolu hors du répertoire → refusé.
    assert!(s.attachment_path_checked("/etc/passwd").is_err());
    assert!(s.read_attachment_file("/etc/passwd").is_err());
    // Chemin relatif qui remonte → refusé.
    let escaping = format!("{}/../../etc/passwd", s.attachments_dir().display());
    assert!(s.attachment_path_checked(&escaping).is_err());
    // Fichier inexistant → NotFound, pas une erreur de validation.
    let inside = s.attachments_dir().join("nope.bin");
    let err = s.attachment_path_checked(inside.to_str().unwrap()).unwrap_err();
    assert!(matches!(err, Error::NotFound(_)));
}

#[test]
fn attachment_over_64mo_is_rejected() {
    let s = test_service();
    let (_board_id, list_id) = board_with_list(&s);
    let card = make_card(&s, &list_id, "gros fichier");
    let too_big = vec![0u8; (MAX_ATTACHMENT_BYTES + 1) as usize];
    let err = s
        .add_attachment(&card.id, "gros.bin", "application/octet-stream", &too_big)
        .unwrap_err();
    assert!(matches!(err, Error::Validation(_)), "attendu Validation, reçu {err:?}");
    // À la limite exacte → accepté.
    let ok = vec![0u8; MAX_ATTACHMENT_BYTES as usize];
    s.add_attachment(&card.id, "limite.bin", "application/octet-stream", &ok).unwrap();
}
