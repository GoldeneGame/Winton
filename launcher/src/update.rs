//! Mises à jour du launcher et des jeux du catalogue.
//!
//! Même principe que Ronnie (github.com/Jellfedora/ronnie, src/update.rs), avec GitHub comme serveur :
//!  - les versions sont des Releases GitHub taguées `<composant>-vX.Y.Z` (ex. `la-taverne-v1.2.0`) ;
//!  - chaque Release contient `<composant>-windows.zip`, son empreinte `.sha256` et sa signature `.sig`
//!    (Ed25519, faite par la GitHub Action avec une clé privée secrète) ;
//!  - rien n'est extrait tant que l'empreinte ET la signature ne sont pas vérifiées ;
//!  - le launcher se remplace avec `self_replace`, puis se relance ;
//!  - un jeu est extrait à côté, puis échangé d'un coup avec l'ancien dossier.

use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPO: &str = "GoldeneGame/Winton";
/// Nom du composant launcher dans les Releases (tags winton-vX.Y.Z, archive winton-windows.zip).
pub const LAUNCHER: &str = "winton";
/// Délai entre deux vérifications automatiques : au démarrage, puis toutes les heures.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Clé publique Ed25519 (base64) ; la clé privée correspondante est le secret
/// GitHub `WINTON_UPDATE_KEY`, utilisé par .github/workflows/release.yml.
const UPDATE_KEY: &str = "05HwO+DRDgWRrAeSTIiiTChGOjp6pQ9vNXqCYGMH6v8=";
const MAX_DOWNLOAD: u64 = 500 * 1024 * 1024;

/// Un jeu proposé par le dépôt (catalogue.json à la racine du dépôt).
#[derive(Clone, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    /// Exécutable à lancer, relatif au dossier du jeu.
    pub exe: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Clone)]
pub struct Release {
    pub version: String,
    pub notes: String,
    zip_url: String,
    sha_url: String,
    sig_url: String,
    size: u64,
}

#[derive(Clone, Default)]
pub struct Remote {
    pub catalog: Vec<CatalogEntry>,
    /// Dernière version publiée de chaque composant (« launcher », « la-taverne »…).
    pub releases: HashMap<String, Release>,
}

/// Avancement d'un téléchargement (octets reçus / total), lu par l'interface.
#[derive(Default)]
pub struct Progress {
    pub done: AtomicU64,
    pub total: AtomicU64,
}

impl Progress {
    pub fn fraction(&self) -> f32 {
        let total = self.total.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        (self.done.load(Ordering::Relaxed) as f64 / total as f64).min(1.0) as f32
    }
}

// ------------------------------------------------------------------ versions

/// « v1.2.3-beta.1 » -> ((1, 2, 3), true). Renvoie None si ce n'est pas une version.
fn parse_version(v: &str) -> Option<((u64, u64, u64), bool)> {
    let v = v.trim().trim_start_matches('v');
    let (core, pre) = match v.find(['-', '+']) {
        Some(i) => (&v[..i], v[i..].starts_with('-')),
        None => (v, false),
    };
    let mut it = core.split('.');
    let a = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    let c = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some(((a, b, c), pre))
}

/// `candidate` est-elle plus récente que `current` ? Une version finale passe devant sa pré-version.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some((c, c_pre)), Some((u, u_pre))) => c > u || (c == u && u_pre && !c_pre),
        _ => false,
    }
}

// ------------------------------------------------------------------ réseau

fn agent() -> Result<ureq::Agent, String> {
    let tls = native_tls::TlsConnector::new().map_err(|e| format!("TLS : {e}"))?;
    Ok(ureq::AgentBuilder::new()
        .tls_connector(Arc::new(tls))
        .timeout_connect(Duration::from_secs(15))
        .user_agent(&format!("winton-launcher/{VERSION}"))
        .build())
}

fn http_err(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(403, _) | ureq::Error::Status(429, _) => {
            "GitHub limite le nombre de vérifications, réessaie plus tard.".into()
        }
        ureq::Error::Status(code, r) => format!("HTTP {code} ({})", r.get_url()),
        ureq::Error::Transport(t) => format!("pas de connexion ({t})"),
    }
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

/// Lit le catalogue et les Releases du dépôt (à appeler depuis un thread).
pub fn fetch_remote() -> Result<Remote, String> {
    let agent = agent()?;

    let catalog_url = format!("https://raw.githubusercontent.com/{REPO}/main/catalogue.json");
    let catalog: Vec<CatalogEntry> = match agent.get(&catalog_url).timeout(Duration::from_secs(30)).call() {
        Ok(r) => r.into_json().map_err(|e| format!("catalogue.json illisible : {e}"))?,
        Err(ureq::Error::Status(404, _)) => Vec::new(),
        Err(e) => return Err(http_err(e)),
    };

    let api = format!("https://api.github.com/repos/{REPO}/releases?per_page=100");
    let list: Vec<GhRelease> = agent
        .get(&api)
        .set("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(30))
        .call()
        .map_err(http_err)?
        .into_json()
        .map_err(|e| format!("réponse GitHub illisible : {e}"))?;

    let mut releases: HashMap<String, Release> = HashMap::new();
    for r in list.into_iter().filter(|r| !r.draft && !r.prerelease) {
        let Some((component, version)) = r.tag_name.rsplit_once("-v") else { continue };
        if parse_version(version).is_none() {
            continue;
        }
        let zip_name = format!("{component}-windows.zip");
        let find = |name: &str| r.assets.iter().find(|a| a.name.eq_ignore_ascii_case(name));
        let (Some(zip), Some(sha), Some(sig)) =
            (find(&zip_name), find(&format!("{zip_name}.sha256")), find(&format!("{zip_name}.sig")))
        else {
            continue;
        };
        let newer = releases.get(component).map_or(true, |known| is_newer(version, &known.version));
        if newer {
            releases.insert(
                component.to_string(),
                Release {
                    version: version.to_string(),
                    notes: r.body.clone().unwrap_or_default().trim().to_string(),
                    zip_url: zip.browser_download_url.clone(),
                    sha_url: sha.browser_download_url.clone(),
                    sig_url: sig.browser_download_url.clone(),
                    size: zip.size,
                },
            );
        }
    }
    Ok(Remote { catalog, releases })
}

/// Télécharge l'archive d'une Release et vérifie son empreinte et sa signature.
fn download_verified(release: &Release, progress: &Progress) -> Result<Vec<u8>, String> {
    let agent = agent()?;
    let text = |url: &str| -> Result<String, String> {
        agent.get(url).timeout(Duration::from_secs(60)).call().map_err(http_err)?.into_string().map_err(|e| e.to_string())
    };
    let sha_hex = text(&release.sha_url)?.split_whitespace().next().unwrap_or("").to_lowercase();
    if sha_hex.len() != 64 {
        return Err("la version publiée n'a pas d'empreinte SHA-256 valide".into());
    }
    let signature = base64::engine::general_purpose::STANDARD
        .decode(text(&release.sig_url)?.trim())
        .map_err(|_| "signature illisible".to_string())?;

    let resp = agent.get(&release.zip_url).timeout(Duration::from_secs(900)).call().map_err(http_err)?;
    let total = resp.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(release.size);
    if total > MAX_DOWNLOAD {
        return Err("archive trop volumineuse".into());
    }
    progress.total.store(total, Ordering::Relaxed);
    progress.done.store(0, Ordering::Relaxed);

    let mut reader = resp.into_reader().take(MAX_DOWNLOAD + 1);
    let mut bytes = Vec::with_capacity(total as usize);
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("téléchargement interrompu : {e}"))?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..n]);
        progress.done.store(bytes.len() as u64, Ordering::Relaxed);
    }
    if bytes.len() as u64 > MAX_DOWNLOAD {
        return Err("archive trop volumineuse".into());
    }
    verify(&bytes, &sha_hex, &signature)?;
    Ok(bytes)
}

/// Empreinte SHA-256 puis signature Ed25519 : refus au moindre écart.
fn verify(bytes: &[u8], sha_hex: &str, signature: &[u8]) -> Result<(), String> {
    let digest: String = Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect();
    if digest != sha_hex {
        return Err("l'empreinte de l'archive ne correspond pas (fichier corrompu ?)".into());
    }
    let key: [u8; 32] = base64::engine::general_purpose::STANDARD
        .decode(UPDATE_KEY)
        .ok()
        .and_then(|k| k.try_into().ok())
        .ok_or("clé publique invalide")?;
    let key = VerifyingKey::from_bytes(&key).map_err(|_| "clé publique invalide")?;
    let sig: [u8; 64] = signature.try_into().map_err(|_| "signature de taille invalide".to_string())?;
    key.verify_strict(bytes, &Signature::from_bytes(&sig))
        .map_err(|_| "signature invalide : la mise à jour n'a pas été publiée par le dépôt officiel".into())
}

fn extract(bytes: &[u8], dest: &Path) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(io::Cursor::new(bytes)).map_err(|e| format!("archive illisible : {e}"))?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = file.enclosed_name() else { continue }; // refuse les chemins « ../ »
        let out = dest.join(rel);
        if file.is_dir() {
            fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut f = fs::File::create(&out).map_err(|e| e.to_string())?;
            io::copy(&mut file, &mut f).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ installation

/// Installe (ou met à jour) un jeu dans `games_dir\<id>\`. Ses sauvegardes, rangées ailleurs, ne sont pas touchées.
pub fn install_game(
    entry: &CatalogEntry,
    release: &Release,
    games_dir: &Path,
    progress: &Progress,
) -> Result<PathBuf, String> {
    let bytes = download_verified(release, progress)?;
    let pid = std::process::id();
    fs::create_dir_all(games_dir).map_err(|e| e.to_string())?;
    let target = games_dir.join(&entry.id);
    let staging = games_dir.join(format!(".{}.new-{pid}", entry.id));
    let old = games_dir.join(format!(".{}.old-{pid}", entry.id));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;

    let result = (|| {
        extract(&bytes, &staging)?;
        if !staging.join(&entry.exe).is_file() {
            return Err(format!("l'archive ne contient pas {}", entry.exe));
        }
        fs::write(staging.join("version.txt"), &release.version).map_err(|e| e.to_string())?;
        if target.exists() {
            fs::rename(&target, &old).map_err(|_| "le jeu est peut-être encore ouvert : ferme-le puis réessaie".to_string())?;
        }
        if let Err(e) = fs::rename(&staging, &target) {
            let _ = fs::rename(&old, &target); // on remet l'ancienne version
            return Err(e.to_string());
        }
        Ok(())
    })();
    let _ = fs::remove_dir_all(&staging);
    let _ = fs::remove_dir_all(&old);
    result.map(|_| target.join(&entry.exe))
}

/// Remplace le launcher en cours d'exécution par la nouvelle version (il faut ensuite le relancer).
pub fn install_launcher(release: &Release, progress: &Progress) -> Result<(), String> {
    let bytes = download_verified(release, progress)?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("dossier du launcher introuvable")?;
    let staging = dir.join(format!(".launcher-update-{}", std::process::id()));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let result = (|| {
        extract(&bytes, &staging)?;
        let new_exe = staging.join("Launcher.exe");
        if !new_exe.is_file() {
            return Err("l'archive ne contient pas Launcher.exe".to_string());
        }
        self_replace::self_replace(&new_exe).map_err(|e| format!("impossible de remplacer le launcher : {e}"))
    })();
    let _ = fs::remove_dir_all(&staging);
    result
}

/// Relance le launcher ; la nouvelle instance attend la fermeture de celle-ci (voir main.rs).
pub fn relaunch() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::process::Command::new(exe)
        .arg("--after-update")
        .arg(std::process::id().to_string())
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(is_newer("v2.0.0", "1.99.99"));
        assert!(is_newer("0.2.0", "0.2.0-beta.1"));
        assert!(!is_newer("0.2.0-beta.1", "0.2.0"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("n'importe quoi", "1.0.0"));
        assert!(!is_newer("1.0", "0.9.0"));
    }

    #[test]
    fn rejects_bad_digest_or_signature() {
        let data = b"archive";
        let good: String = Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect();
        assert!(verify(data, &"0".repeat(64), &[0u8; 64]).unwrap_err().contains("empreinte"));
        assert!(verify(data, &good, &[0u8; 64]).unwrap_err().contains("signature"));
    }

    /// Vérifie une archive signée par openssl comme le fait la GitHub Action.
    /// WINTON_TEST_FILE / WINTON_TEST_SIG : fichier et signature base64 (test ignoré sinon).
    #[test]
    fn accepts_openssl_signature() {
        let (Ok(file), Ok(sig)) = (std::env::var("WINTON_TEST_FILE"), std::env::var("WINTON_TEST_SIG")) else {
            return;
        };
        let data = fs::read(file).unwrap();
        let sig = base64::engine::general_purpose::STANDARD.decode(fs::read_to_string(sig).unwrap().trim()).unwrap();
        let sha: String = Sha256::digest(&data).iter().map(|b| format!("{b:02x}")).collect();
        verify(&data, &sha, &sig).unwrap();
    }
}
