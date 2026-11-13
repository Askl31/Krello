//! Génération des raccourcis macOS (`.shortcut` = plist binaire) pour l'ajout
//! rapide de tâches via **Spotlight** et **Siri**.
//!
//! Un raccourci par liste : [Demander la saisie] → [Exécuter un script shell].
//! Le script reçoit le texte (saisie clavier depuis Spotlight, dictée Siri)
//! sur stdin, le dépose dans l'inbox Siri, puis ouvre le deep link
//! `krello://task/new?list=…` (l'app consomme l'inbox et crée la carte avec
//! notification WS). Repli si le scheme n'est pas enregistré :
//! `<binaire> task add --title … --list … --db …` (écriture SQLite directe).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use plist::{Dictionary, Value};

use crate::deeplink::INBOX_FILE;
use crate::error::{Error, Result};

/// Cible d'un raccourci : une liste d'un board.
#[derive(Debug, Clone)]
pub struct ShortcutTarget {
    pub board_title: String,
    pub list_title: String,
}

/// Percent-encoding d'un composant query (conserve les « unreserved » RFC 3986).
pub fn url_encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Échappement shell entre single quotes (`'` → `'\''`).
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Script shell embarqué dans le raccourci (reçoit le titre sur stdin).
pub fn task_add_script(list_title: &str, exe: &Path, data_dir: &Path, db_path: &Path) -> String {
    let inbox = data_dir.join(INBOX_FILE);
    let list_enc = url_encode_component(list_title);
    format!(
        "TITLE=$(cat)\n\
         [ -z \"$TITLE\" ] && exit 0\n\
         INBOX={inbox_q}\n\
         mkdir -p \"$(dirname \"$INBOX\")\"\n\
         printf '%s' \"$TITLE\" > \"$INBOX\" || exit 1\n\
         open \"krello://task/new?list={list_enc}\" 2>/dev/null && exit 0\n\
         {exe_q} task add --title \"$TITLE\" --list {list_q} --db {db_q}\n\
         rm -f \"$INBOX\"\n",
        inbox_q = shell_quote(&inbox.to_string_lossy()),
        exe_q = shell_quote(&exe.to_string_lossy()),
        list_q = shell_quote(list_title),
        db_q = shell_quote(&db_path.to_string_lossy()),
    )
}

fn action_dict(identifier: &str, params: Dictionary) -> Value {
    let mut d = Dictionary::new();
    d.insert(
        "WFWorkflowActionIdentifier".into(),
        Value::String(identifier.into()),
    );
    d.insert("WFWorkflowActionParameters".into(), Value::Dictionary(params));
    Value::Dictionary(d)
}

/// Construit le plist binaire d'un raccourci « Nouvelle tâche dans <liste> » :
/// action `ask` (saisie texte) puis `runshellscript` (stdin = saisie).
pub fn build_task_shortcut(name: &str, list_title: &str, script: &str) -> Result<Vec<u8>> {
    let ask_uuid = uuid::Uuid::new_v4().to_string().to_uppercase();
    let shell_uuid = uuid::Uuid::new_v4().to_string().to_uppercase();

    let mut ask = Dictionary::new();
    ask.insert("UUID".into(), Value::String(ask_uuid));
    ask.insert(
        "WFTextActionText".into(),
        Value::String("Ask for Input".into()),
    );
    ask.insert(
        "WFAskActionPrompt".into(),
        Value::String(format!("Nouvelle tâche dans « {list_title} » ?")),
    );
    ask.insert(
        "WFAskActionDefaultAnswer".into(),
        Value::String(String::new()),
    );
    ask.insert("WFInputType".into(), Value::String("Text".into()));

    let mut shell = Dictionary::new();
    shell.insert("UUID".into(), Value::String(shell_uuid));
    shell.insert(
        "WFTextActionText".into(),
        Value::String("Run Shell Script".into()),
    );
    // Paramètres modernes Apple Shortcuts (macOS 12+)
    shell.insert("Script".into(), Value::String(script.to_string()));
    shell.insert("Shell".into(), Value::String("/bin/sh".into()));
    shell.insert("InputMode".into(), Value::String("to stdin".into()));
    // Clés historiques (rétro-compatibilité)
    shell.insert("WFShellScript".into(), Value::String(script.to_string()));
    shell.insert(
        "WFShellScriptShell".into(),
        Value::String("/bin/sh".into()),
    );
    shell.insert("WFInput".into(), Value::String("to stdin".into()));

    let mut icon = Dictionary::new();
    icon.insert("WFWorkflowIconGlyphNumber".into(), Value::Integer(59511_i64.into()));
    icon.insert("WFWorkflowIconStartColor".into(), Value::Integer(4282601983_i64.into()));

    let mut root = Dictionary::new();
    root.insert(
        "WFWorkflowActions".into(),
        Value::Array(vec![
            action_dict("is.workflow.actions.ask", ask),
            action_dict("is.workflow.actions.runshellscript", shell),
        ]),
    );
    root.insert(
        "WFWorkflowClientVersion".into(),
        Value::String("2700.0.4".into()),
    );
    root.insert("WFWorkflowMinimumClientVersion".into(), Value::Integer(900_i64.into()));
    root.insert(
        "WFWorkflowMinimumClientVersionString".into(),
        Value::String("900".into()),
    );
    root.insert("WFWorkflowIcon".into(), Value::Dictionary(icon));
    root.insert("WFWorkflowName".into(), Value::String(name.to_string()));
    root.insert("WFWorkflowHasOutputFallback".into(), Value::Boolean(false));
    root.insert(
        "WFWorkflowHasShortcutInputVariables".into(),
        Value::Boolean(false),
    );
    root.insert("WFWorkflowImportQuestions".into(), Value::Array(vec![]));
    root.insert(
        "WFWorkflowInputContentItemClasses".into(),
        Value::Array(vec![
            Value::String("WFStringContentItem".into()),
            Value::String("WFURLContentItem".into()),
        ]),
    );
    root.insert(
        "WFWorkflowOutputContentItemClasses".into(),
        Value::Array(vec![]),
    );
    root.insert("WFWorkflowTypes".into(), Value::Array(vec![]));

    let mut buf = Vec::new();
    Value::Dictionary(root)
        .to_writer_binary(&mut buf)
        .map_err(|e| Error::Validation(format!("génération plist: {e}")))?;
    Ok(buf)
}

/// Assainit un nom de fichier (caractères interdits HFS+/APFS → `-`).
fn sanitize_filename(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| match c {
            '/' | ':' | '\\' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            _ => c,
        })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.').trim();
    let cleaned = if cleaned.is_empty() { "liste" } else { cleaned };
    cleaned.chars().take(80).collect()
}

fn lisezmoi(targets: &[ShortcutTarget]) -> String {
    let mut list = String::new();
    for t in targets {
        list.push_str(&format!("- Krello · {}.shortcut  (board « {} »)\n", t.list_title, t.board_title));
    }
    format!(
        "Krello — raccourcis Siri & Spotlight\n\
         =====================================\n\n\
         Chaque fichier .shortcut ajoute UNE tâche dans UNE liste déterminée.\n\n\
         1. Importez un raccourci : double-cliquez sur le fichier .shortcut\n\
         \x20   → la fenêtre « Ajouter le raccourci » s'ouvre → cliquez « Ajouter ».\n\
         2. Spotlight : ⌘Espace, tapez « Krello », choisissez le raccourci,\n\
         \x20   saisissez le titre de la tâche, Entrée → la carte est créée dans Krello.\n\
         3. Siri : ouvrez le raccourci dans l'app Raccourcis → « Ajouter à Siri »\n\
         \x20   → dictez votre phrase (ex. « Nouvelle tâche Krello »), puis dites\n\
         \x20   « Siri, nouvelle tâche Krello » en dictant la tâche.\n\
         4. Pour cibler une autre liste : dupliquez le raccourci dans Raccourcis\n\
         \x20   et changez le nom de la liste dans l'action « Exécuter un script shell »\n\
         \x20   (paramètre --list et krello://task/new?list=...).\n\n\
         Raccourcis générés :\n{list}\n\
         La création passe par l'app Krello (deep link krello://) si elle est\n\
         installée ; sinon le binaire en ligne de commande écrit directement dans\n\
         la base SQLite. Annulation possible avec ⌘Z dans Krello.\n"
    )
}

/// Signe un fichier .shortcut sur macOS en utilisant `shortcuts sign --mode anyone`.
/// Remplace le fichier plist brut par l'archive signée reconnue par macOS.
pub fn sign_shortcut_file(path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let tmp_signed = path.with_extension("signed.tmp");
        if let Ok(status) = std::process::Command::new("shortcuts")
            .arg("sign")
            .arg("--mode")
            .arg("anyone")
            .arg("--input")
            .arg(path)
            .arg("--output")
            .arg(&tmp_signed)
            .status()
        {
            if status.success() && tmp_signed.exists() {
                let _ = std::fs::rename(&tmp_signed, path);
            } else {
                let _ = std::fs::remove_file(&tmp_signed);
            }
        }
    }
    Ok(())
}

/// Ouvre un fichier ou un dossier avec l'utilitaire système macOS `open`.
pub fn open_path(path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()?;
    }
    Ok(())
}

/// Génère et signe un raccourci individuel pour une cible donnée.
pub fn write_single_shortcut(
    dir: &Path,
    target: &ShortcutTarget,
    exe: &Path,
    data_dir: &Path,
    db_path: &Path,
) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let base = sanitize_filename(&format!("Krello · {}", target.list_title));
    let path = dir.join(format!("{base}.shortcut"));
    let script = task_add_script(&target.list_title, exe, data_dir, db_path);
    let bytes = build_task_shortcut(&format!("Krello · {}", target.list_title), &target.list_title, &script)?;
    std::fs::write(&path, bytes)?;
    let _ = sign_shortcut_file(&path);
    Ok(path)
}

/// Écrit le pack de raccourcis dans `dir` : un `.shortcut` par liste + LISEZMOI.txt.
/// Retourne la liste des fichiers écrits (les .shortcut sont signés pour macOS).
pub fn write_pack(
    dir: &Path,
    targets: &[ShortcutTarget],
    exe: &Path,
    data_dir: &Path,
    db_path: &Path,
) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let mut written = Vec::new();
    let mut used: HashSet<String> = HashSet::new();
    for t in targets {
        let base = sanitize_filename(&format!("Krello · {}", t.list_title));
        let mut fname = base.clone();
        if used.contains(&fname) {
            fname = sanitize_filename(&format!("Krello · {} · {}", t.board_title, t.list_title));
        }
        if used.contains(&fname) {
            fname = format!("{fname} ({})", used.len());
        }
        used.insert(fname.clone());
        let path = dir.join(format!("{fname}.shortcut"));
        let script = task_add_script(&t.list_title, exe, data_dir, db_path);
        let bytes = build_task_shortcut(&format!("Krello · {}", t.list_title), &t.list_title, &script)?;
        std::fs::write(&path, bytes)?;
        let _ = sign_shortcut_file(&path);
        written.push(path);
    }
    let readme = dir.join("LISEZMOI.txt");
    std::fs::write(&readme, lisezmoi(targets))?;
    written.push(readme);
    Ok(written)
}
