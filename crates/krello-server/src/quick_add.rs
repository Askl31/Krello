//! macOS Quick Add helper: compiles and installs `Ajouter à Krello.app`
//! in `~/Applications/` for instant Spotlight invocation (⌘ + Espace).

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use krello_core::error::{Error, Result};

pub const APP_NAME: &str = "Ajouter à Krello.app";
pub const BUNDLE_ID: &str = "com.krello.quick-add";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickAddStatus {
    pub supported: bool,
    pub installed: bool,
    pub path: Option<String>,
    pub app_name: String,
}

/// Directory where user applications are indexed by Spotlight: `~/Applications`.
pub fn app_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join("Applications")
    } else {
        PathBuf::from("/Applications")
    }
}

/// Full path to the bundle: `~/Applications/Ajouter à Krello.app`.
pub fn app_path() -> PathBuf {
    app_dir().join(APP_NAME)
}

/// Checks whether `Ajouter à Krello.app` is currently installed.
pub fn status() -> QuickAddStatus {
    let supported = cfg!(target_os = "macos");
    let path = app_path();
    let installed = supported && path.exists() && path.join("Contents/MacOS/Ajouter à Krello").exists();
    QuickAddStatus {
        supported,
        installed,
        path: if installed { Some(path.to_string_lossy().into_owned()) } else { None },
        app_name: APP_NAME.to_string(),
    }
}

/// Installs (or updates) `Ajouter à Krello.app` in `~/Applications/`.
pub fn install() -> Result<PathBuf> {
    if !cfg!(target_os = "macos") {
        return Err(Error::Validation("L'ajout rapide Spotlight est réservé à macOS".into()));
    }

    let apps_dir = app_dir();
    std::fs::create_dir_all(&apps_dir)?;

    let bundle = app_path();
    let macos_dir = bundle.join("Contents/MacOS");
    let resources_dir = bundle.join("Contents/Resources");
    std::fs::create_dir_all(&macos_dir)?;
    std::fs::create_dir_all(&resources_dir)?;

    // 1. Info.plist with LSUIElement = true (no Dock icon, stays over active app)
    let plist_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>Ajouter à Krello</string>
    <key>CFBundleIdentifier</key>
    <string>com.krello.quick-add</string>
    <key>CFBundleName</key>
    <string>Ajouter à Krello</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>LSUIElement</key>
    <true/>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
</dict>
</plist>
"#;
    std::fs::write(bundle.join("Contents/Info.plist"), plist_content)?;

    // 2. Icon (AppIcon.icns)
    let icon_bytes: &[u8] = include_bytes!("../../../src-tauri/icons/icon.icns");
    let _ = std::fs::write(resources_dir.join("AppIcon.icns"), icon_bytes);

    // 3. Compile embedded Swift source
    let swift_source = include_str!("../swift/quick_add.swift");
    let temp_swift = std::env::temp_dir().join(format!("krello_quick_add_{}.swift", std::process::id()));
    std::fs::write(&temp_swift, swift_source)?;

    let target_binary = macos_dir.join("Ajouter à Krello");
    let output = std::process::Command::new("swiftc")
        .arg("-O")
        .arg(&temp_swift)
        .arg("-o")
        .arg(&target_binary)
        .output();

    let _ = std::fs::remove_file(&temp_swift);

    match output {
        Ok(out) if out.status.success() => {},
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(Error::Validation(format!("Échec de la compilation de l'applet Swift : {err}")));
        }
        Err(e) => {
            return Err(Error::Validation(format!("Compilateur swiftc introuvable ou erreur d'exécution : {e}")));
        }
    }

    // 4. Ad-hoc codesign
    let _ = std::process::Command::new("codesign")
        .arg("-f")
        .arg("-s")
        .arg("-")
        .arg(&bundle)
        .status();

    // 5. Register with LaunchServices for instant Spotlight indexing
    let lsregister = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
    if Path::new(lsregister).exists() {
        let _ = std::process::Command::new(lsregister)
            .arg("-f")
            .arg(&bundle)
            .status();
    }

    Ok(bundle)
}

/// Uninstalls `Ajouter à Krello.app` from `~/Applications/`.
pub fn uninstall() -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err(Error::Validation("L'ajout rapide Spotlight est réservé à macOS".into()));
    }
    let bundle = app_path();
    if bundle.exists() {
        let lsregister = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
        if Path::new(lsregister).exists() {
            let _ = std::process::Command::new(lsregister)
                .arg("-u")
                .arg(&bundle)
                .status();
        }
        std::fs::remove_dir_all(&bundle)?;
    }
    Ok(())
}

/// Tests launching the installed applet (`open -a`).
pub fn test_launch() -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err(Error::Validation("L'ajout rapide Spotlight est réservé à macOS".into()));
    }
    let bundle = app_path();
    if !bundle.exists() {
        return Err(Error::NotFound("L'application « Ajouter à Krello » n'est pas encore installée".into()));
    }
    std::process::Command::new("open")
        .arg(&bundle)
        .spawn()?;
    Ok(())
}
