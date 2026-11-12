//! Tests de génération des raccourcis macOS (.shortcut) pour Siri/Spotlight.

use std::io::Cursor;
use std::path::Path;

use krello_core::shortcut::{
    build_task_shortcut, shell_quote, task_add_script, url_encode_component, write_pack,
    ShortcutTarget,
};

#[test]
fn url_encoding_component() {
    assert_eq!(url_encode_component("À faire"), "%C3%80%20faire");
    assert_eq!(url_encode_component("a-b_c.d~"), "a-b_c.d~");
    assert_eq!(url_encode_component("100%"), "100%25");
}

#[test]
fn shell_quoting() {
    assert_eq!(shell_quote("Courses"), "'Courses'");
    assert_eq!(shell_quote("l Courses"), "'l Courses'");
    assert_eq!(shell_quote("l'été"), "'l'\\''été'");
}

#[test]
fn task_add_script_embeds_list_exe_db_and_deeplink() {
    let script = task_add_script(
        "Courses",
        Path::new("/Applications/Krello.app/Contents/MacOS/Krello"),
        Path::new("/Users/x/Library/Application Support/com.krello.app"),
        Path::new("/Users/x/Library/Application Support/com.krello.app/krello.db"),
    );
    assert!(script.contains("krello://task/new?list=Courses"));
    assert!(script.contains("siri-inbox.txt"));
    assert!(script.contains("'/Applications/Krello.app/Contents/MacOS/Krello' task add --title \"$TITLE\" --list 'Courses'"));
    assert!(script.contains("--db '/Users/x/Library/Application Support/com.krello.app/krello.db'"));
    // liste avec espace encodée dans l'URL
    let s2 = task_add_script("À faire", Path::new("/bin/krello"), Path::new("/d"), Path::new("/d/k.db"));
    assert!(s2.contains("krello://task/new?list=%C3%80%20faire"));
    assert!(s2.contains("--list 'À faire'"));
}

#[test]
fn build_shortcut_produces_valid_binary_plist_with_two_actions() {
    let script = task_add_script("Courses", Path::new("/x/Krello"), Path::new("/data"), Path::new("/data/krello.db"));
    let bytes = build_task_shortcut("Krello · Courses", "Courses", &script).unwrap();
    // re-parse depuis le binaire
    let v = plist::Value::from_reader(Cursor::new(&bytes)).expect("plist binaire lisible");
    let root = v.as_dictionary().unwrap();
    assert_eq!(root.get("WFWorkflowName").unwrap().as_string().unwrap(), "Krello · Courses");
    let actions = root.get("WFWorkflowActions").unwrap().as_array().unwrap();
    assert_eq!(actions.len(), 2);
    let a0 = actions[0].as_dictionary().unwrap();
    assert_eq!(
        a0.get("WFWorkflowActionIdentifier").unwrap().as_string().unwrap(),
        "is.workflow.actions.ask"
    );
    let p0 = a0.get("WFWorkflowActionParameters").unwrap().as_dictionary().unwrap();
    assert_eq!(p0.get("WFInputType").unwrap().as_string().unwrap(), "Text");
    assert!(p0.get("WFAskActionPrompt").unwrap().as_string().unwrap().contains("Courses"));
    let a1 = actions[1].as_dictionary().unwrap();
    assert_eq!(
        a1.get("WFWorkflowActionIdentifier").unwrap().as_string().unwrap(),
        "is.workflow.actions.runshellscript"
    );
    let p1 = a1.get("WFWorkflowActionParameters").unwrap().as_dictionary().unwrap();
    assert_eq!(p1.get("WFShellScriptShell").unwrap().as_string().unwrap(), "/bin/sh");
    assert_eq!(p1.get("WFInput").unwrap().as_string().unwrap(), "to stdin");
    let src = p1.get("WFShellScript").unwrap().as_string().unwrap();
    assert!(src.contains("krello://task/new?list=Courses"));
    // UUIDs présents et distincts
    let u0 = p0.get("UUID").unwrap().as_string().unwrap().to_string();
    let u1 = p1.get("UUID").unwrap().as_string().unwrap().to_string();
    assert_ne!(u0, u1);
    assert!(u0.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
}

#[test]
fn write_pack_writes_one_shortcut_per_list_plus_readme() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("pack");
    let targets = vec![
        ShortcutTarget { board_title: "Perso".into(), list_title: "Courses".into() },
        ShortcutTarget { board_title: "Travail".into(), list_title: "Courses".into() }, // collision → préfixe board
        ShortcutTarget { board_title: "Perso".into(), list_title: "À faire/A/B".into() }, // assainissement
    ];
    let files = write_pack(
        &out,
        &targets,
        Path::new("/x/Krello"),
        Path::new("/data"),
        Path::new("/data/krello.db"),
    )
    .unwrap();
    assert_eq!(files.len(), 4);
    let names: Vec<String> = files.iter().map(|f| f.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert!(names.contains(&"Krello · Courses.shortcut".to_string()));
    assert!(names.contains(&"Krello · Travail · Courses.shortcut".to_string()));
    assert!(names.iter().any(|n| n.contains("À faire-A-B")));
    assert!(names.contains(&"LISEZMOI.txt".to_string()));
    // chaque .shortcut est un plist binaire valide
    for f in files.iter().filter(|f| f.extension().map(|e| e == "shortcut").unwrap_or(false)) {
        let bytes = std::fs::read(f).unwrap();
        let v = plist::Value::from_reader(Cursor::new(&bytes)).unwrap();
        assert!(v.as_dictionary().unwrap().get("WFWorkflowActions").is_some());
    }
    let readme = std::fs::read_to_string(out.join("LISEZMOI.txt")).unwrap();
    assert!(readme.contains("Spotlight"));
    assert!(readme.contains("Ajouter à Siri"));
}
