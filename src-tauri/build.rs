fn main() {
    println!("cargo:rerun-if-changed=../frontend");
    tauri_build::build();
}
