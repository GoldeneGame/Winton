// Intègre l'icône dans Winton.exe (Explorateur, raccourcis, barre des tâches).
// windres est appelé directement, avec des chemins relatifs : il gère mal les
// espaces dans le chemin du projet (« Projet Claude »).
use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/winton.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let icon = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets").join("winton.ico");
    fs::copy(&icon, out.join("winton.ico")).unwrap();
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let v = version.replace('.', ",") + ",0";
    let rc = format!(
        r#"1 ICON "winton.ico"
1 VERSIONINFO
FILEVERSION {v}
PRODUCTVERSION {v}
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040C04B0"
    BEGIN
      VALUE "FileDescription", "Winton"
      VALUE "ProductName", "Winton"
      VALUE "FileVersion", "{version}"
      VALUE "ProductVersion", "{version}"
      VALUE "OriginalFilename", "Winton.exe"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x040C, 1200
  END
END
"#
    );
    fs::write(out.join("winton.rc"), rc).unwrap();
    let status = Command::new("windres")
        .current_dir(&out)
        .args(["-O", "coff", "-i", "winton.rc", "-o", "winton_res.o"])
        .status()
        .expect("windres introuvable (MinGW doit être dans le PATH)");
    assert!(status.success(), "windres a échoué");
    println!("cargo:rustc-link-arg-bins={}", out.join("winton_res.o").display());
}
