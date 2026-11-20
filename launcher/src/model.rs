//! Données du launcher : jeux, chemins, sauvegarde sur disque.
//! Le format JSON est identique à celui de l'ancienne version (LauncherData\launcher.json).

use chrono::{Local, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Game {
    pub id: String,
    pub name: String,
    /// Exécutable (relatif au launcher s'il est dans son dossier) ou lien (steam://…).
    pub path: String,
    pub arguments: String,
    pub working_directory: String,
    pub cover_path: String,
    #[serde(with = "date")]
    pub added_at: Option<NaiveDateTime>,
    #[serde(with = "date")]
    pub last_played: Option<NaiveDateTime>,
    pub play_seconds: f64,
    /// Identifiant dans catalogue.json pour un jeu installé depuis le dépôt ("" = jeu ajouté à la main).
    pub catalog_id: String,
    /// Version installée (jeux du catalogue).
    pub version: String,
}

impl Default for Game {
    fn default() -> Self {
        Game {
            id: new_id(),
            name: String::new(),
            path: String::new(),
            arguments: String::new(),
            working_directory: String::new(),
            cover_path: String::new(),
            added_at: Some(now()),
            last_played: None,
            play_seconds: 0.0,
            catalog_id: String::new(),
            version: String::new(),
        }
    }
}

impl Game {
    pub fn full_path(&self) -> String {
        resolve(&self.path)
    }

    pub fn initial(&self) -> String {
        initial_of(&self.name)
    }

    pub fn last_played_text(&self) -> String {
        match self.last_played {
            None => "Jamais".into(),
            Some(d) => {
                let today = Local::now().date_naive();
                if d.date() == today {
                    format!("Aujourd'hui à {}", d.format("%H:%M"))
                } else if Some(d.date()) == today.pred_opt() {
                    format!("Hier à {}", d.format("%H:%M"))
                } else {
                    d.format("%d/%m/%Y").to_string()
                }
            }
        }
    }

    pub fn added_text(&self) -> String {
        self.added_at.map(|d| d.format("%d/%m/%Y").to_string()).unwrap_or_else(|| "—".into())
    }

    pub fn card_subtitle(&self) -> String {
        if self.last_played.is_none() {
            "Jamais lancé".into()
        } else if self.play_seconds >= 60.0 {
            format!("{} de jeu", format_duration(self.play_seconds))
        } else {
            format!("Lancé {}", self.last_played_text().to_lowercase())
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct LauncherData {
    pub minimize_on_launch: bool,
    pub sort: String,
    pub games: Vec<Game>,
}

impl Default for LauncherData {
    fn default() -> Self {
        LauncherData { minimize_on_launch: true, sort: "name".into(), games: Vec::new() }
    }
}

// ------------------------------------------------------------------ utilitaires

pub fn now() -> NaiveDateTime {
    Local::now().naive_local()
}

pub fn new_id() -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}{:04x}", nanos, COUNTER.fetch_add(1, Ordering::Relaxed) & 0xffff)
}

pub fn initial_of(name: &str) -> String {
    name.trim()
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_else(|| "?".into())
}

pub fn format_duration(seconds: f64) -> String {
    if seconds < 1.0 {
        return "0 min".into();
    }
    let mins = (seconds / 60.0).floor() as u64;
    if mins == 0 {
        "< 1 min".into()
    } else if mins < 60 {
        format!("{mins} min")
    } else {
        format!("{} h {:02}", mins / 60, mins % 60)
    }
}

/// « LaTaverne » -> « La Taverne », « mon_jeu » -> « mon jeu ».
pub fn guess_name(path: &str) -> String {
    if is_uri(path) {
        return String::new();
    }
    let stem = Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut out = String::new();
    let mut prev_lower = false;
    for c in stem.chars() {
        let c = if c == '_' || c == '-' { ' ' } else { c };
        if c.is_uppercase() && prev_lower {
            out.push(' ');
        }
        prev_lower = c.is_lowercase();
        out.push(c);
    }
    out.trim().to_string()
}

// ------------------------------------------------------------------ chemins

/// Dossier du launcher.exe : les jeux qui y sont rangés sont enregistrés en relatif (launcher portable).
pub fn base_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn is_uri(p: &str) -> bool {
    p.contains("://")
}

pub fn resolve(p: &str) -> String {
    let p = p.trim();
    if p.is_empty() || is_uri(p) {
        return p.to_string();
    }
    let path = Path::new(p);
    if path.is_absolute() {
        return p.to_string();
    }
    let joined = base_dir().join(path);
    std::path::absolute(&joined).unwrap_or(joined).to_string_lossy().to_string()
}

pub fn make_portable(p: &str) -> String {
    let p = p.trim();
    if p.is_empty() || is_uri(p) {
        return p.to_string();
    }
    let full = PathBuf::from(resolve(p));
    match full.strip_prefix(base_dir()) {
        Ok(rel) => rel.to_string_lossy().to_string(),
        Err(_) => full.to_string_lossy().to_string(),
    }
}

// ------------------------------------------------------------------ stockage

pub struct Store {
    pub dir: PathBuf,
}

impl Store {
    pub fn new() -> Self {
        let local = base_dir().join("LauncherData");
        let writable = fs::create_dir_all(&local).is_ok() && {
            let probe = local.join(".write-test");
            let ok = fs::write(&probe, b"").is_ok();
            let _ = fs::remove_file(&probe);
            ok
        };
        if writable {
            return Store { dir: local };
        }
        // Dossier protégé (ex. Program Files) : on se rabat sur %APPDATA%.
        let appdata = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        let dir = appdata.join("GameLauncher");
        let _ = fs::create_dir_all(&dir);
        Store { dir }
    }

    fn file(&self) -> PathBuf {
        self.dir.join("launcher.json")
    }

    pub fn covers_dir(&self) -> PathBuf {
        self.dir.join("covers")
    }

    /// Dossier où sont installés les jeux du catalogue (Jeux\ à côté de LauncherData\).
    pub fn games_dir(&self) -> PathBuf {
        match self.dir.parent() {
            Some(parent) if self.dir.file_name().is_some_and(|n| n == "LauncherData") => parent.join("Jeux"),
            _ => self.dir.join("Jeux"), // repli dans %APPDATA%\GameLauncher
        }
    }

    /// Renvoie les données et un éventuel avertissement à afficher.
    pub fn load(&self) -> (LauncherData, Option<String>) {
        let file = self.file();
        let Ok(text) = fs::read_to_string(&file) else {
            return (LauncherData::default(), None);
        };
        match serde_json::from_str::<LauncherData>(text.trim_start_matches('\u{feff}')) {
            Ok(d) => (d, None),
            Err(e) => {
                let backup = self.dir.join("launcher.json.corrompu.bak");
                let _ = fs::copy(&file, &backup);
                (
                    LauncherData::default(),
                    Some(format!(
                        "Impossible de lire la bibliothèque ({e}).\nUne copie a été gardée dans :\n{}",
                        backup.display()
                    )),
                )
            }
        }
    }

    pub fn save(&self, data: &LauncherData) -> io::Result<()> {
        let tmp = self.dir.join("launcher.json.tmp");
        let json = serde_json::to_string_pretty(data).map_err(io::Error::other)?;
        fs::write(&tmp, json)?;
        fs::rename(&tmp, self.file())
    }

    /// Copie la jaquette choisie dans LauncherData\covers pour qu'elle ne se perde pas.
    pub fn import_cover(&self, source: &str, game_id: &str) -> io::Result<String> {
        fs::create_dir_all(self.covers_dir())?;
        let ext = Path::new(source)
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
            .unwrap_or_default();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let dest = self.covers_dir().join(format!("{game_id}_{stamp}{ext}"));
        fs::copy(source, &dest)?;
        Ok(dest.to_string_lossy().to_string())
    }

    /// Supprime la jaquette si c'est une copie gérée par le launcher.
    pub fn delete_owned_cover(&self, cover_path: &str) {
        let f = PathBuf::from(resolve(cover_path));
        if cover_path.trim().is_empty() || !f.is_file() {
            return;
        }
        let covers = std::path::absolute(self.covers_dir()).unwrap_or_else(|_| self.covers_dir());
        let parent = f.parent().map(|p| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()));
        if parent.is_some_and(|p| p.to_string_lossy().eq_ignore_ascii_case(&covers.to_string_lossy())) {
            let _ = fs::remove_file(f);
        }
    }
}

// ------------------------------------------------------------------ dates (compatibles .NET)

mod date {
    use chrono::{Local, NaiveDateTime};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &Option<NaiveDateTime>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(d) => s.serialize_str(&d.format("%Y-%m-%dT%H:%M:%S").to_string()),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<NaiveDateTime>, D::Error> {
        let s: Option<String> = Option::deserialize(d)?;
        Ok(s.and_then(|s| parse(&s)))
    }

    fn parse(s: &str) -> Option<NaiveDateTime> {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
            return Some(dt.with_timezone(&Local).naive_local());
        }
        NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f").ok()
    }
}
