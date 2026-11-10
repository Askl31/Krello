//! Compile the Swift Apple Intelligence helper and embed it.
//!
//! On macOS with the FoundationModels framework available, `swiftc` produces a
//! small helper binary that the Rust core spawns over stdin/stdout (JSON lines).
//! On any other platform (or if swiftc is missing / compilation fails), an empty
//! stub is embedded so everything still builds — the AI provider then reports
//! itself as unavailable.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=swift/main.swift");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let bin = out_dir.join("krello-ai-helper");

    let on_macos = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos");
    let mut compiled = false;
    if on_macos {
        // `xcrun swiftc` resolves the active macOS SDK (FoundationModels needs SDK 26+).
        let compilers: [&[&str]; 2] = [&["xcrun", "swiftc"], &["swiftc"]];
        for compiler in compilers {
            compiled = Command::new(compiler[0])
                .args(&compiler[1..])
                .arg("-O")
                .arg("-o")
                .arg(&bin)
                .arg("swift/main.swift")
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if compiled {
                break;
            }
        }
    }

    if !compiled {
        std::fs::write(&bin, b"").expect("write stub helper");
        if on_macos {
            println!("cargo:warning=krello: swiftc n'a pas pu compiler le helper Apple Intelligence — fonctionnalités IA désactivées (Xcode 26+ requis)");
        }
    }

    println!("cargo:rustc-env=KRELLO_AI_HELPER_PATH={}", bin.display());
}
