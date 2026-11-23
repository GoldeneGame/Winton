//! Jeux distribués par les releases GitHub : recherche de la dernière version et installation.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Duration;

#[derive(Clone)]
pub struct Release {
    pub tag: String,
    /// Texte de la release (le message du commit qui l'a produite).
    pub notes: String,
    pub url: String,
    pub size: u64,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .user_agent("Winton")
        .build()
}

fn describe(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(403 | 429, _) => "GitHub limite les vérifications pour le moment, réessaie plus tard.".into(),
        ureq::Error::Status(code, _) => format!("GitHub a répondu avec l'erreur {code}."),
        ureq::Error::Transport(_) => "Impossible de joindre GitHub (pas de connexion ?).".into(),
    }
}

/// Dernière release du dépôt `propriétaire/nom` contenant le fichier `asset`.
/// `Ok(None)` : le dépôt n'a encore publié aucune version.
pub fn latest(repo: &str, asset: &str) -> Result<Option<Release>, String> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let resp = match agent().get(&url).set("Accept", "application/vnd.github+json").call() {
        Ok(r) => r,
        Err(ureq::Error::Status(404, _)) => return Ok(None),
        Err(e) => return Err(describe(e)),
    };
    let v: serde_json::Value =
        serde_json::from_reader(resp.into_reader()).map_err(|e| format!("Réponse de GitHub illisible ({e})."))?;
    let tag = v["tag_name"].as_str().unwrap_or_default().to_string();
    let file = v["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|x| x["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(asset))))
        .ok_or_else(|| format!("La version {tag} ne contient pas {asset}."))?;
    Ok(Some(Release {
        tag,
        notes: v["body"].as_str().unwrap_or_default().trim().to_string(),
        url: file["browser_download_url"].as_str().unwrap_or_default().to_string(),
        size: file["size"].as_u64().unwrap_or(0),
    }))
}

/// Télécharge la release dans `dir` et renvoie le chemin de l'exécutable installé.
/// `done` compte les octets reçus (barre de progression).
/// `previous` : l'ancien exécutable du jeu ; si Winton installe le jeu ailleurs pour la première fois,
/// les fichiers qui l'accompagnaient (configuration, sauvegardes…) sont recopiés à côté du nouveau.
pub fn install(rel: &Release, asset: &str, dir: &Path, previous: Option<PathBuf>, done: &AtomicU64) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| format!("Impossible de créer {} ({e}).", dir.display()))?;
    let exe = dir.join(asset);
    let part = dir.join(format!("{asset}.telechargement"));

    let resp = agent().get(&rel.url).call().map_err(describe)?;
    let mut reader = resp.into_reader();
    let mut file = fs::File::create(&part).map_err(|e| format!("Écriture impossible ({e})."))?;
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = match reader.read(&mut buf) {
            Ok(n) => n,
            Err(e) => {
                drop(file);
                let _ = fs::remove_file(&part);
                return Err(format!("Téléchargement interrompu ({e})."));
            }
        };
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| format!("Écriture impossible ({e})."))?;
        done.fetch_add(n as u64, Relaxed);
    }
    drop(file);
    let got = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    if rel.size > 0 && got != rel.size {
        let _ = fs::remove_file(&part);
        return Err("Téléchargement incomplet, réessaie.".into());
    }

    // Ancienne version : supprimée, ou mise de côté si Windows refuse (exe encore ouvert).
    let stem = Path::new(asset).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let set_aside = format!("{stem}.ancien-");
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().starts_with(&set_aside) {
                let _ = fs::remove_file(e.path());
            }
        }
    }
    if exe.exists() && fs::remove_file(&exe).is_err() {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        fs::rename(&exe, dir.join(format!("{set_aside}{stamp}.exe")))
            .map_err(|e| format!("L'ancienne version n'a pas pu être remplacée ({e})."))?;
    }
    fs::rename(&part, &exe).map_err(|e| format!("Installation impossible ({e})."))?;

    if let Some(old_dir) = previous.as_deref().and_then(Path::parent) {
        let same = fs::canonicalize(old_dir).ok() == fs::canonicalize(dir).ok();
        if !same && old_dir.is_dir() {
            copy_companions(old_dir, dir, asset, &set_aside);
        }
    }
    Ok(exe)
}

/// Recopie ce qui accompagnait l'ancien exe (sans écraser l'existant).
fn copy_companions(from: &Path, to: &Path, asset: &str, set_aside: &str) {
    let Ok(entries) = fs::read_dir(from) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.eq_ignore_ascii_case(asset) || name.starts_with(set_aside) {
            continue;
        }
        let dest = to.join(&name);
        if !dest.exists() {
            let _ = copy_all(&e.path(), &dest);
        }
    }
}

fn copy_all(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        fs::create_dir_all(to)?;
        for e in fs::read_dir(from)?.flatten() {
            copy_all(&e.path(), &to.join(e.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(from, to).map(|_| ())
    }
}
