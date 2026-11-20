//! Interface du launcher.

use crate::model::{self, Game, LauncherData, Store};
use crate::theme::{self, bold_font, ACCENT, BG, CARD, DANGER, PANEL, PLAY, SUB, TEXT};
use crate::update::{self, CatalogEntry, Progress, Release, Remote};
use crate::win;

use eframe::egui::{
    self, pos2, vec2, Align, Align2, Button, Color32, FontId, Frame, Key, Layout, Margin, Rect, RichText, Rounding,
    ScrollArea, Sense, Stroke, TextEdit, TextureHandle, Ui,
};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering::SeqCst};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

const CARD_SIZE: egui::Vec2 = egui::Vec2::new(180.0, 252.0);

// ------------------------------------------------------------------ images

#[derive(Default)]
struct TexCache(HashMap<String, Option<TextureHandle>>);

impl TexCache {
    fn cover(&mut self, ctx: &egui::Context, path: &str) -> Option<TextureHandle> {
        self.get(ctx, "img", path, load_image)
    }

    fn icon(&mut self, ctx: &egui::Context, path: &str) -> Option<TextureHandle> {
        if model::is_uri(path) {
            return None;
        }
        self.get(ctx, "ico", path, win::extract_icon)
    }

    /// Oublie l'icône d'un fichier (après une mise à jour du jeu).
    fn forget_icon(&mut self, path: &str) {
        self.0.remove(&format!("ico:{path}"));
    }

    fn get(
        &mut self,
        ctx: &egui::Context,
        kind: &str,
        path: &str,
        load: fn(&str) -> Option<egui::ColorImage>,
    ) -> Option<TextureHandle> {
        if path.trim().is_empty() {
            return None;
        }
        let key = format!("{kind}:{path}");
        if let Some(t) = self.0.get(&key) {
            return t.clone();
        }
        let t = load(path).map(|img| ctx.load_texture(&key, img, egui::TextureOptions::LINEAR));
        self.0.insert(key, t.clone());
        t
    }
}

fn load_image(path: &str) -> Option<egui::ColorImage> {
    let img = image::open(path).ok()?;
    let img = if img.width() > 600 { img.thumbnail(600, 1200) } else { img };
    let rgba = img.to_rgba8();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    ))
}

// ------------------------------------------------------------------ état

struct EditForm {
    existing: Option<String>,
    new_id: String,
    name: String,
    path: String,
    args: String,
    workdir: String,
    cover: String, // chemin absolu de l'image choisie ("" = aucune)
    error: String,
    focused: bool,
}

enum Modal {
    None,
    Edit(EditForm),
    ConfirmRemove(String),
    Missing(String),
    AlreadyRunning(String),
    Message(String, String),
    /// Installer ou mettre à jour un jeu du catalogue (id du catalogue).
    InstallGame(String),
    LauncherUpdate,
}

enum Action {
    Select(String),
    Launch(String),
    ForceLaunch(String),
    Add(Option<String>),
    Edit(String),
    ConfirmRemove(String),
    Remove(String),
    Folder(String),
    AskInstall(String),
    Install(String),
    AskLauncherUpdate,
    UpdateLauncher,
    CheckUpdates,
}

/// Messages des threads de mise à jour.
enum UpdateMsg {
    Checked(Result<Remote, String>),
    GameInstalled { entry: CatalogEntry, version: String, result: Result<PathBuf, String> },
    LauncherInstalled(Result<(), String>),
}

/// Téléchargement en cours (un seul à la fois).
struct Job {
    label: String,
    progress: Arc<Progress>,
}

pub struct LauncherApp {
    store: Store,
    data: LauncherData,
    selected: Option<String>,
    search: String,
    status: String,
    tex: TexCache,
    modal: Modal,
    actions: Vec<Action>,
    pending_adds: VecDeque<String>,
    focus_search: bool,
    last_launch: Option<(String, Instant)>,
    // Suivi des jeux en cours (les threads de surveillance renvoient (id, secondes jouées)).
    running: HashMap<String, usize>,
    running_count: Arc<AtomicUsize>,
    minimized_by_us: Arc<AtomicBool>,
    hwnd: Arc<AtomicIsize>,
    tx: Sender<(String, f64)>,
    rx: Receiver<(String, f64)>,
    // Mises à jour
    auto_update: bool,
    remote: Option<Remote>,
    checking: bool,
    last_check: Option<Instant>,
    launcher_dismissed: bool,
    job: Option<Job>,
    upd_tx: Sender<UpdateMsg>,
    upd_rx: Receiver<UpdateMsg>,
}

impl LauncherApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::setup(&cc.egui_ctx);
        let store = Store::new();
        let (data, warning) = store.load();
        let (tx, rx) = channel();
        let (upd_tx, upd_rx) = channel();
        LauncherApp {
            store,
            data,
            selected: None,
            search: String::new(),
            status: "Astuce : glisse un .exe dans la fenêtre pour l'ajouter.".into(),
            tex: TexCache::default(),
            modal: warning.map_or(Modal::None, |w| Modal::Message("Launcher".into(), w)),
            actions: Vec::new(),
            pending_adds: VecDeque::new(),
            focus_search: false,
            last_launch: None,
            running: HashMap::new(),
            running_count: Arc::new(AtomicUsize::new(0)),
            minimized_by_us: Arc::new(AtomicBool::new(false)),
            hwnd: Arc::new(AtomicIsize::new(0)),
            tx,
            rx,
            auto_update: std::env::var_os("WINTON_NO_UPDATE").is_none(),
            remote: None,
            checking: false,
            last_check: None,
            launcher_dismissed: false,
            job: None,
            upd_tx,
            upd_rx,
        }
    }

    fn game(&self, id: &str) -> Option<&Game> {
        self.data.games.iter().find(|g| g.id == id)
    }

    fn save(&mut self) {
        if let Err(e) = self.store.save(&self.data) {
            self.status = format!("⚠ Sauvegarde impossible : {e}");
        }
    }

    fn matches_search(&self, name: &str) -> bool {
        let q = self.search.trim().to_lowercase();
        q.is_empty() || name.to_lowercase().contains(&q)
    }

    /// Indices des jeux affichés (recherche + tri).
    fn visible(&self) -> Vec<usize> {
        let games = &self.data.games;
        let mut v: Vec<usize> = (0..games.len()).filter(|&i| self.matches_search(&games[i].name)).collect();
        v.sort_by(|&a, &b| {
            let (a, b) = (&games[a], &games[b]);
            let primary = match self.data.sort.as_str() {
                "recent" => b.last_played.cmp(&a.last_played),
                "time" => b.play_seconds.partial_cmp(&a.play_seconds).unwrap_or(std::cmp::Ordering::Equal),
                _ => std::cmp::Ordering::Equal,
            };
            primary.then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        v
    }

    // ------------------------------------------------------------------ mises à jour

    fn release(&self, component: &str) -> Option<&Release> {
        self.remote.as_ref()?.releases.get(component)
    }

    fn catalog_entry(&self, id: &str) -> Option<&CatalogEntry> {
        self.remote.as_ref()?.catalog.iter().find(|e| e.id == id)
    }

    /// Nouvelle version du launcher, s'il y en a une.
    fn launcher_update(&self) -> Option<&Release> {
        self.release(update::LAUNCHER).filter(|r| update::is_newer(&r.version, update::VERSION))
    }

    /// Nouvelle version d'un jeu installé depuis le catalogue, s'il y en a une.
    fn game_update(&self, g: &Game) -> Option<&Release> {
        if g.catalog_id.is_empty() {
            return None;
        }
        self.release(&g.catalog_id).filter(|r| g.version.is_empty() || update::is_newer(&r.version, &g.version))
    }

    /// Jeux du catalogue publiés mais pas encore installés.
    fn installable(&self) -> Vec<(CatalogEntry, String)> {
        let Some(remote) = &self.remote else { return Vec::new() };
        remote
            .catalog
            .iter()
            .filter(|e| !self.data.games.iter().any(|g| g.catalog_id == e.id))
            .filter(|e| self.matches_search(&e.name))
            .filter_map(|e| remote.releases.get(&e.id).map(|r| (e.clone(), r.version.clone())))
            .collect()
    }

    fn maybe_check(&mut self, ctx: &egui::Context) {
        if !self.auto_update || self.checking {
            return;
        }
        match self.last_check {
            Some(t) if t.elapsed() < update::CHECK_INTERVAL => {
                ctx.request_repaint_after(update::CHECK_INTERVAL.saturating_sub(t.elapsed()));
            }
            _ => self.check_now(ctx),
        }
    }

    fn check_now(&mut self, ctx: &egui::Context) {
        if self.checking {
            return;
        }
        self.checking = true;
        self.last_check = Some(Instant::now());
        let (tx, ctx) = (self.upd_tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(UpdateMsg::Checked(update::fetch_remote()));
            ctx.request_repaint();
        });
    }

    fn start_install(&mut self, ctx: &egui::Context, id: &str) {
        if self.job.is_some() {
            return;
        }
        let (Some(entry), Some(release)) = (self.catalog_entry(id).cloned(), self.release(id).cloned()) else {
            return;
        };
        if let Some(g) = self.data.games.iter().find(|g| g.catalog_id == id) {
            if self.running.contains_key(&g.id) {
                self.modal = Modal::Message(
                    "Jeu ouvert".into(),
                    format!("Ferme « {} » avant de le mettre à jour.", g.name),
                );
                return;
            }
        }
        let progress = Arc::new(Progress::default());
        self.job = Some(Job { label: format!("{} {}", entry.name, release.version), progress: progress.clone() });
        let (tx, ctx, games_dir) = (self.upd_tx.clone(), ctx.clone(), self.store.games_dir());
        std::thread::spawn(move || {
            let result = update::install_game(&entry, &release, &games_dir, &progress);
            let _ = tx.send(UpdateMsg::GameInstalled { entry, version: release.version, result });
            ctx.request_repaint();
        });
    }

    fn start_launcher_update(&mut self, ctx: &egui::Context) {
        if self.job.is_some() {
            return;
        }
        let Some(release) = self.launcher_update().cloned() else { return };
        let progress = Arc::new(Progress::default());
        self.job = Some(Job { label: format!("Launcher {}", release.version), progress: progress.clone() });
        let (tx, ctx) = (self.upd_tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(UpdateMsg::LauncherInstalled(update::install_launcher(&release, &progress)));
            ctx.request_repaint();
        });
    }

    fn poll_updates(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.upd_rx.try_recv() {
            match msg {
                UpdateMsg::Checked(Ok(remote)) => {
                    self.checking = false;
                    self.remote = Some(remote);
                }
                UpdateMsg::Checked(Err(e)) => {
                    self.checking = false;
                    self.status = format!("Mises à jour : vérification impossible ({e}).");
                }
                UpdateMsg::GameInstalled { entry, version, result } => {
                    self.job = None;
                    match result {
                        Ok(exe) => self.on_game_installed(&entry, &version, &exe),
                        Err(e) => {
                            self.modal = Modal::Message(
                                "Installation impossible".into(),
                                format!("« {} » n'a pas pu être installé :\n\n{e}", entry.name),
                            )
                        }
                    }
                }
                UpdateMsg::LauncherInstalled(result) => {
                    self.job = None;
                    match result.and_then(|_| update::relaunch()) {
                        Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                        Err(e) => {
                            self.modal = Modal::Message(
                                "Mise à jour impossible".into(),
                                format!("Le launcher n'a pas pu être mis à jour :\n\n{e}"),
                            )
                        }
                    }
                }
            }
        }
        if self.job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }

    fn on_game_installed(&mut self, entry: &CatalogEntry, version: &str, exe: &Path) {
        let exe = exe.to_string_lossy().to_string();
        self.tex.forget_icon(&exe);
        let path = model::make_portable(&exe);
        let id = match self.data.games.iter_mut().find(|g| g.catalog_id == entry.id) {
            Some(g) => {
                g.version = version.to_string();
                g.path = path;
                self.status = format!("« {} » a été mis à jour en version {version}.", g.name);
                g.id.clone()
            }
            None => {
                let g = Game {
                    name: entry.name.clone(),
                    path,
                    catalog_id: entry.id.clone(),
                    version: version.to_string(),
                    ..Default::default()
                };
                self.status = format!("« {} » {version} a été installé.", g.name);
                let id = g.id.clone();
                self.data.games.push(g);
                id
            }
        };
        self.selected = Some(id);
        self.save();
    }

    // ------------------------------------------------------------------ actions

    fn open_add(&mut self, path: Option<String>) {
        let mut f = EditForm {
            existing: None,
            new_id: model::new_id(),
            name: String::new(),
            path: String::new(),
            args: String::new(),
            workdir: String::new(),
            cover: String::new(),
            error: String::new(),
            focused: false,
        };
        if let Some(p) = path {
            f.name = model::guess_name(&p);
            f.path = p;
        }
        self.modal = Modal::Edit(f);
    }

    fn open_edit(&mut self, id: &str) {
        let Some(g) = self.game(id) else { return };
        self.modal = Modal::Edit(EditForm {
            existing: Some(g.id.clone()),
            new_id: String::new(),
            name: g.name.clone(),
            path: g.full_path(),
            args: g.arguments.clone(),
            workdir: model::resolve(&g.working_directory),
            cover: model::resolve(&g.cover_path),
            error: String::new(),
            focused: false,
        });
    }

    /// Valide et enregistre le formulaire. Renvoie true si la fenêtre peut se fermer.
    fn save_form(&mut self, f: &mut EditForm) -> bool {
        let name = f.name.trim().to_string();
        let path = f.path.trim().trim_matches('"').to_string();
        let workdir = f.workdir.trim().trim_matches('"').to_string();

        if name.is_empty() {
            f.error = "Donne un nom au jeu.".into();
            return false;
        }
        if path.is_empty() {
            f.error = "Choisis le fichier qui lance le jeu.".into();
            return false;
        }
        let full = model::resolve(&path);
        if !model::is_uri(&full) && !Path::new(&full).is_file() {
            f.error = format!("Ce fichier n'existe pas :\n{full}");
            return false;
        }
        if !workdir.is_empty() && !Path::new(&model::resolve(&workdir)).is_dir() {
            f.error = "Le dossier de travail n'existe pas.".into();
            return false;
        }

        let id = f.existing.clone().unwrap_or_else(|| f.new_id.clone());
        let old_cover = self.game(&id).map(|g| g.cover_path.clone()).unwrap_or_default();
        let mut cover_path = old_cover.clone();
        if !f.cover.eq_ignore_ascii_case(&model::resolve(&old_cover)) {
            let new_cover = if f.cover.is_empty() {
                String::new()
            } else {
                match self.store.import_cover(&f.cover, &id) {
                    Ok(p) => p,
                    Err(e) => {
                        f.error = format!("Impossible de copier l'image : {e}");
                        return false;
                    }
                }
            };
            self.store.delete_owned_cover(&old_cover);
            cover_path = model::make_portable(&new_cover);
        }

        let is_new = f.existing.is_none();
        if is_new {
            self.data.games.push(Game { id: id.clone(), ..Default::default() });
        }
        let Some(g) = self.data.games.iter_mut().find(|g| g.id == id) else { return true };
        g.name = name.clone();
        g.path = model::make_portable(&full);
        g.arguments = f.args.trim().to_string();
        g.working_directory = model::make_portable(&workdir);
        g.cover_path = cover_path;

        self.selected = Some(id);
        if is_new {
            self.search.clear();
            self.status = format!("« {name} » a été ajouté.");
        }
        self.save();
        true
    }

    fn remove(&mut self, id: &str) {
        let Some(pos) = self.data.games.iter().position(|g| g.id == id) else { return };
        let g = self.data.games.remove(pos);
        self.store.delete_owned_cover(&g.cover_path);
        // Jeu du catalogue : on désinstalle ses fichiers (ses sauvegardes sont rangées ailleurs).
        if !g.catalog_id.is_empty() {
            let _ = std::fs::remove_dir_all(self.store.games_dir().join(&g.catalog_id));
        }
        self.status = format!("« {} » a été retiré.", g.name);
        self.save();
    }

    fn open_folder(&mut self, id: &str) {
        let Some(full) = self.game(id).map(|g| g.full_path()) else { return };
        if model::is_uri(&full) {
            self.status = "Ce jeu est un lien, il n'a pas de dossier.".into();
        } else if Path::new(&full).is_file() {
            win::open_in_explorer(&full);
        } else if let Some(dir) = Path::new(&full).parent().filter(|d| d.is_dir()) {
            win::open_in_explorer(&dir.to_string_lossy());
        } else {
            self.status = "Le dossier du jeu est introuvable.".into();
        }
    }

    fn launch(&mut self, ctx: &egui::Context, id: &str, force: bool) {
        let Some(g) = self.game(id).cloned() else { return };

        // Évite un double lancement (double-clic sur le bouton lecture).
        if let Some((last, t)) = &self.last_launch {
            if !force && last == id && t.elapsed().as_secs_f32() < 2.0 {
                return;
            }
        }

        let target = g.full_path();
        let uri = model::is_uri(&target);
        if !uri && !Path::new(&target).is_file() {
            self.modal = Modal::Missing(id.into());
            return;
        }
        if !force && self.running.contains_key(id) {
            self.modal = Modal::AlreadyRunning(id.into());
            return;
        }

        let dir = if uri {
            None
        } else {
            let w = model::resolve(&g.working_directory);
            if !w.is_empty() && Path::new(&w).is_dir() {
                Some(w)
            } else {
                Path::new(&target).parent().map(|p| p.to_string_lossy().to_string())
            }
        };

        let handle = match win::shell_launch(&target, &g.arguments, dir.as_deref()) {
            Ok(h) => h,
            Err(e) => {
                self.modal = Modal::Message("Erreur".into(), format!("Impossible de lancer « {} » :\n\n{e}", g.name));
                return;
            }
        };

        self.last_launch = Some((id.to_string(), Instant::now()));
        if let Some(gm) = self.data.games.iter_mut().find(|x| x.id == id) {
            gm.last_played = Some(model::now());
        }
        self.status = format!("« {} » lancé à {}.", g.name, chrono::Local::now().format("%H:%M"));
        self.save();

        // Pas de processus (lien steam://, raccourci…) : pas de suivi du temps de jeu.
        let Some(process) = handle else { return };

        *self.running.entry(id.to_string()).or_insert(0) += 1;
        self.running_count.fetch_add(1, SeqCst);
        let (tx, ctx2, count, minimized, hwnd) = (
            self.tx.clone(),
            ctx.clone(),
            self.running_count.clone(),
            self.minimized_by_us.clone(),
            self.hwnd.clone(),
        );
        let game_id = id.to_string();
        let started = Instant::now();
        std::thread::spawn(move || {
            win::wait_and_close(process);
            let _ = tx.send((game_id, started.elapsed().as_secs_f64()));
            if count.fetch_sub(1, SeqCst) == 1 && minimized.swap(false, SeqCst) {
                win::restore_window(hwnd.load(SeqCst));
            }
            ctx2.request_repaint();
        });

        if self.data.minimize_on_launch {
            self.minimized_by_us.store(true, SeqCst);
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
    }

    fn poll_finished(&mut self) {
        let mut any = false;
        while let Ok((id, secs)) = self.rx.try_recv() {
            any = true;
            if let Some(c) = self.running.get_mut(&id) {
                *c -= 1;
                if *c == 0 {
                    self.running.remove(&id);
                }
            }
            if let Some(g) = self.data.games.iter_mut().find(|g| g.id == id) {
                g.play_seconds += secs;
                self.status =
                    format!("Session terminée : {} sur « {} ».", model::format_duration(secs), g.name);
            }
        }
        if any {
            self.save();
        }
    }

    fn process_actions(&mut self, ctx: &egui::Context) {
        for action in std::mem::take(&mut self.actions) {
            match action {
                Action::Select(id) => self.selected = Some(id),
                Action::Launch(id) => self.launch(ctx, &id, false),
                Action::ForceLaunch(id) => self.launch(ctx, &id, true),
                Action::Add(path) => self.open_add(path),
                Action::Edit(id) => self.open_edit(&id),
                Action::ConfirmRemove(id) => self.modal = Modal::ConfirmRemove(id),
                Action::Remove(id) => self.remove(&id),
                Action::Folder(id) => self.open_folder(&id),
                Action::AskInstall(id) => {
                    if self.job.is_none() {
                        self.modal = Modal::InstallGame(id)
                    }
                }
                Action::Install(id) => self.start_install(ctx, &id),
                Action::AskLauncherUpdate => {
                    if self.job.is_none() {
                        self.modal = Modal::LauncherUpdate
                    }
                }
                Action::UpdateLauncher => self.start_launcher_update(ctx),
                Action::CheckUpdates => self.check_now(ctx),
            }
        }
    }

    // ------------------------------------------------------------------ entrées

    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped: Vec<String> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.as_ref())
                .filter(|p| p.is_file())
                .map(|p| p.to_string_lossy().to_string())
                .collect()
        });
        self.pending_adds.extend(dropped);
        if matches!(self.modal, Modal::None) {
            if let Some(p) = self.pending_adds.pop_front() {
                self.open_add(Some(p));
            }
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let (ctrl_f, ctrl_n, enter, del, f2) = ctx.input(|i| {
            (
                i.modifiers.command && i.key_pressed(Key::F),
                i.modifiers.command && i.key_pressed(Key::N),
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::Delete),
                i.key_pressed(Key::F2),
            )
        });
        if ctrl_f {
            self.focus_search = true;
        }
        if ctrl_n {
            self.actions.push(Action::Add(None));
        }
        if ctx.wants_keyboard_input() {
            return;
        }
        if let Some(id) = self.selected.clone() {
            if enter {
                self.actions.push(Action::Launch(id.clone()));
            }
            if del {
                self.actions.push(Action::ConfirmRemove(id.clone()));
            }
            if f2 {
                self.actions.push(Action::Edit(id));
            }
        }
    }

    // ------------------------------------------------------------------ panneaux

    fn header(&mut self, ctx: &egui::Context, enabled: bool) {
        let mut sort_changed = false;
        let launcher_update = if self.launcher_dismissed { None } else { self.launcher_update().map(|r| r.version.clone()) };
        egui::TopBottomPanel::top("header")
            .frame(Frame::none().fill(PANEL).inner_margin(Margin::symmetric(24.0, 14.0)))
            .show(ctx, |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    ui.horizontal(|ui| {
                        theme::logo(ui, 30.0);
                        ui.add_space(4.0);
                        ui.label(RichText::new("LAUNCHER").font(bold_font(21.0)).color(TEXT));
                        let count = match self.data.games.len() {
                            0 => String::new(),
                            1 => "1 jeu".into(),
                            n => format!("{n} jeux"),
                        };
                        ui.add_space(6.0);
                        ui.label(RichText::new(count).color(SUB).size(13.0));

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let add = theme::filled_button("+  Ajouter un jeu", ACCENT).min_size(vec2(0.0, 36.0));
                            if ui.add(add).on_hover_text("Ctrl+N").clicked() {
                                self.actions.push(Action::Add(None));
                            }

                            // Téléchargement en cours, ou mise à jour du launcher disponible
                            if let Some(job) = &self.job {
                                ui.add_space(8.0);
                                let pct = (job.progress.fraction() * 100.0).round();
                                ui.add(
                                    egui::ProgressBar::new(job.progress.fraction())
                                        .desired_width(220.0)
                                        .fill(ACCENT)
                                        .text(format!("{} — {pct} %", job.label)),
                                );
                            } else if let Some(v) = &launcher_update {
                                ui.add_space(8.0);
                                let b = theme::filled_button(&format!("Mise à jour {v}"), PLAY).min_size(vec2(0.0, 36.0));
                                if ui.add(b).on_hover_text("Une nouvelle version du launcher est disponible").clicked() {
                                    self.actions.push(Action::AskLauncherUpdate);
                                }
                            }

                            ui.add_space(8.0);
                            let resp = ui.add(
                                TextEdit::singleline(&mut self.search)
                                    .id(egui::Id::new("search"))
                                    .hint_text("Rechercher…  (Ctrl+F)")
                                    .desired_width(220.0)
                                    .margin(vec2(10.0, 8.0)),
                            );
                            if self.focus_search {
                                resp.request_focus();
                                self.focus_search = false;
                            }
                            if resp.lost_focus() {
                                let (enter, esc) =
                                    ui.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
                                if enter {
                                    if let Some(id) = &self.selected {
                                        self.actions.push(Action::Launch(id.clone()));
                                    }
                                } else if esc {
                                    self.search.clear();
                                }
                            }
                            ui.add_space(16.0);
                            for (key, label) in [("time", "Temps de jeu"), ("recent", "Récents"), ("name", "A – Z")] {
                                if theme::pill(ui, label, self.data.sort == key).clicked() && self.data.sort != key {
                                    self.data.sort = key.into();
                                    sort_changed = true;
                                }
                            }
                        });
                    });
                });
            });
        if sort_changed {
            self.save();
        }
    }

    fn footer(&mut self, ctx: &egui::Context, enabled: bool) {
        let mut changed = false;
        egui::TopBottomPanel::bottom("footer")
            .frame(Frame::none().fill(PANEL).inner_margin(Margin::symmetric(24.0, 7.0)))
            .show(ctx, |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&self.status).color(SUB).size(12.5));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let mut v = self.data.minimize_on_launch;
                            let text = RichText::new("Réduire le launcher pendant que je joue").color(SUB);
                            if ui.checkbox(&mut v, text).changed() {
                                self.data.minimize_on_launch = v;
                                changed = true;
                            }
                            ui.add_space(16.0);
                            let label = if self.checking { "Vérification…" } else { "Vérifier les mises à jour" };
                            let check = Button::new(RichText::new(label).color(SUB).size(12.5)).frame(false);
                            if ui.add_enabled(!self.checking, check).clicked() {
                                self.actions.push(Action::CheckUpdates);
                            }
                            ui.label(RichText::new(format!("v{}", update::VERSION)).color(SUB).size(12.5));
                        });
                    });
                });
            });
        if changed {
            self.save();
        }
    }

    fn details(&mut self, ctx: &egui::Context, enabled: bool) {
        egui::SidePanel::right("details")
            .exact_width(340.0)
            .resizable(false)
            .frame(Frame::none().fill(PANEL).inner_margin(Margin::same(24.0)))
            .show(ctx, |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    let Some(g) = self.selected.as_deref().and_then(|id| self.game(id)).cloned() else {
                        if !self.data.games.is_empty() {
                            ui.centered_and_justified(|ui| ui.label(RichText::new("Sélectionne un jeu").color(SUB)));
                        }
                        return;
                    };
                    let cover = self.tex.cover(ctx, &model::resolve(&g.cover_path));
                    let icon = self.tex.icon(ctx, &g.full_path());
                    let running = self.running.contains_key(&g.id);
                    let new_version = self.game_update(&g).map(|r| r.version.clone());
                    let busy = self.job.is_some();

                    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                        let w = ui.available_width();
                        let (rect, _) = ui.allocate_exact_size(vec2(w, 220.0), Sense::hover());
                        theme::paint_art(ui, rect, cover.as_ref(), icon.as_ref(), &g.initial(), 12.0, 96.0, 0.0);
                        ui.add_space(10.0);
                        ui.label(RichText::new(&g.name).font(bold_font(22.0)).color(TEXT));
                        ui.label(RichText::new(g.full_path()).color(SUB).size(11.5));
                        ui.add_space(12.0);

                        if let Some(v) = &new_version {
                            let b = theme::filled_button(&format!("Mettre à jour  ({v})"), ACCENT);
                            if ui.add_enabled(!busy, b.min_size(vec2(w, 40.0))).clicked() {
                                self.actions.push(Action::AskInstall(g.catalog_id.clone()));
                            }
                            ui.add_space(4.0);
                        }

                        let (text, fill) = if running { ("EN COURS…", ACCENT) } else { ("▶   JOUER", PLAY) };
                        let play = Button::new(RichText::new(text).color(Color32::WHITE).font(bold_font(17.0)))
                            .fill(fill)
                            .stroke(Stroke::NONE)
                            .rounding(8.0);
                        if ui.add_sized([w, 52.0], play).clicked() {
                            self.actions.push(Action::Launch(g.id.clone()));
                        }
                        ui.add_space(8.0);

                        Frame::none().fill(CARD).rounding(10.0).inner_margin(Margin::symmetric(16.0, 12.0)).show(
                            ui,
                            |ui| {
                                ui.set_width(ui.available_width());
                                let mut rows = vec![
                                    ("Temps de jeu", model::format_duration(g.play_seconds)),
                                    ("Dernière session", g.last_played_text()),
                                    ("Ajouté le", g.added_text()),
                                ];
                                if !g.catalog_id.is_empty() {
                                    rows.push(("Version", if g.version.is_empty() { "—".into() } else { g.version.clone() }));
                                }
                                for (k, v) in rows {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(k).color(SUB));
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            ui.label(RichText::new(v).font(bold_font(14.0)).color(TEXT));
                                        });
                                    });
                                }
                            },
                        );
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            let bw = (w - 16.0) / 3.0;
                            if ui.add_sized([bw, 34.0], Button::new("Modifier")).on_hover_text("F2").clicked() {
                                self.actions.push(Action::Edit(g.id.clone()));
                            }
                            if ui
                                .add_sized([bw, 34.0], Button::new("Dossier"))
                                .on_hover_text("Ouvrir l'emplacement du jeu")
                                .clicked()
                            {
                                self.actions.push(Action::Folder(g.id.clone()));
                            }
                            if ui.add_sized([bw, 34.0], theme::danger_button("Retirer")).on_hover_text("Suppr").clicked()
                            {
                                self.actions.push(Action::ConfirmRemove(g.id.clone()));
                            }
                        });
                    });
                });
            });
    }

    fn grid(&mut self, ctx: &egui::Context, enabled: bool, visible: &[usize]) {
        let installable = self.installable();
        egui::CentralPanel::default().frame(Frame::none().fill(BG).inner_margin(Margin::same(24.0))).show(ctx, |ui| {
            ui.add_enabled_ui(enabled, |ui| {
                if visible.is_empty() && installable.is_empty() {
                    self.empty_state(ui);
                    return;
                }
                ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(16.0, 16.0);
                    if !visible.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            for &i in visible {
                                self.card(ui, i);
                            }
                        });
                    }
                    if !installable.is_empty() {
                        ui.add_space(8.0);
                        ui.label(RichText::new("À installer").font(bold_font(16.0)).color(TEXT));
                        ui.horizontal_wrapped(|ui| {
                            for (entry, version) in &installable {
                                self.install_card(ui, entry, version);
                            }
                        });
                    }
                });
            });
        });
    }

    fn empty_state(&self, ui: &mut Ui) {
        let (title, sub) = if self.data.games.is_empty() {
            ("Ta bibliothèque est vide", "Clique sur « + Ajouter un jeu »\nou glisse un fichier .exe dans cette fenêtre.")
        } else {
            ("Aucun résultat", "Essaie un autre terme de recherche.")
        };
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() / 2.0 - 90.0).max(0.0));
            let (r, _) = ui.allocate_exact_size(vec2(72.0, 72.0), Sense::hover());
            ui.painter().rect_filled(r, 18.0, CARD);
            theme::play_triangle(ui.painter(), r.center() + vec2(3.0, 0.0), 14.0, SUB);
            ui.add_space(14.0);
            ui.label(RichText::new(title).font(bold_font(20.0)).color(TEXT));
            ui.label(RichText::new(sub).color(SUB));
        });
    }

    /// Bas de carte : dégradé, nom et sous-titre.
    fn card_caption(ui: &Ui, rect: Rect, name: &str, sub: &str) {
        let p = ui.painter();
        let dark = Color32::from_black_alpha(225);
        let grad_bottom = rect.bottom() - 12.0;
        theme::vertical_gradient(
            p,
            Rect::from_min_max(pos2(rect.left(), rect.bottom() - 95.0), pos2(rect.right(), grad_bottom)),
            Color32::TRANSPARENT,
            dark,
        );
        p.rect_filled(
            Rect::from_min_max(pos2(rect.left(), grad_bottom), rect.max),
            Rounding { nw: 0.0, ne: 0.0, sw: 12.0, se: 12.0 },
            dark,
        );
        let name_g = theme::truncated(ui, name, bold_font(14.0), Color32::WHITE, rect.width() - 24.0);
        p.galley(pos2(rect.left() + 12.0, rect.bottom() - 50.0), name_g, Color32::WHITE);
        let sub_color = Color32::from_rgb(0xB4, 0xBC, 0xCC);
        let sub_g = theme::truncated(ui, sub, FontId::proportional(11.5), sub_color, rect.width() - 24.0);
        p.galley(pos2(rect.left() + 12.0, rect.bottom() - 28.0), sub_g, sub_color);
    }

    fn badge(ui: &Ui, rect: Rect, text: &str, color: Color32, right: bool) {
        let g = theme::truncated(ui, text, bold_font(10.0), Color32::WHITE, 200.0);
        let size = g.size() + vec2(16.0, 6.0);
        let min = if right { pos2(rect.right() - 10.0 - size.x, rect.top() + 10.0) } else { rect.min + vec2(10.0, 10.0) };
        let br = Rect::from_min_size(min, size);
        ui.painter().rect_filled(br, 6.0, color);
        ui.painter().galley(br.min + vec2(8.0, 3.0), g, Color32::WHITE);
    }

    fn card(&mut self, ui: &mut Ui, index: usize) {
        let (rect, resp) = ui.allocate_exact_size(CARD_SIZE, Sense::click());
        let g = &self.data.games[index];
        let id = g.id.clone();

        if ui.is_rect_visible(rect) {
            let (name, sub, initial) = (g.name.clone(), g.card_subtitle(), g.initial());
            let (cover_path, full_path) = (model::resolve(&g.cover_path), g.full_path());
            let has_update = self.game_update(g).is_some();
            let ctx = ui.ctx().clone();
            let cover = self.tex.cover(&ctx, &cover_path);
            let icon = self.tex.icon(&ctx, &full_path);

            theme::paint_art(ui, rect, cover.as_ref(), icon.as_ref(), &initial, 12.0, 80.0, -25.0);
            Self::card_caption(ui, rect, &name, &sub);
            if self.running.contains_key(&id) {
                Self::badge(ui, rect, "EN COURS", PLAY, false);
            }
            if has_update {
                Self::badge(ui, rect, "MISE À JOUR", ACCENT, true);
            }
        }

        let circle = rect.center() - vec2(0.0, 20.0);
        let on_circle = resp.hover_pos().is_some_and(|pos| pos.distance(circle) <= 29.0);
        if resp.hovered() {
            let p = ui.painter();
            p.rect_filled(rect, 12.0, Color32::from_black_alpha(64));
            let fill = if on_circle { Color32::from_rgb(0x3D, 0xD6, 0x84) } else { PLAY };
            p.circle_filled(circle, 29.0, fill);
            theme::play_triangle(p, circle + vec2(2.0, 0.0), 11.0, Color32::WHITE);
        }
        if self.selected.as_deref() == Some(id.as_str()) {
            ui.painter().rect_stroke(rect.shrink(1.5), 12.0, Stroke::new(3.0_f32, ACCENT));
        }

        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.double_clicked() || (resp.clicked() && on_circle) {
            self.actions.push(Action::Select(id.clone()));
            self.actions.push(Action::Launch(id.clone()));
        } else if resp.clicked() || resp.secondary_clicked() {
            self.actions.push(Action::Select(id.clone()));
        }
        resp.context_menu(|ui| {
            let items = [
                ("Jouer", Action::Launch(id.clone())),
                ("Modifier…", Action::Edit(id.clone())),
                ("Ouvrir l'emplacement du jeu", Action::Folder(id.clone())),
                ("Retirer du launcher", Action::ConfirmRemove(id.clone())),
            ];
            for (label, action) in items {
                if ui.button(label).clicked() {
                    self.actions.push(action);
                    ui.close_menu();
                }
            }
        });
    }

    /// Carte d'un jeu du catalogue pas encore installé.
    fn install_card(&mut self, ui: &mut Ui, entry: &CatalogEntry, version: &str) {
        let (rect, resp) = ui.allocate_exact_size(CARD_SIZE, Sense::click());
        if ui.is_rect_visible(rect) {
            theme::paint_art(ui, rect, None, None, &model::initial_of(&entry.name), 12.0, 80.0, -25.0);
            Self::card_caption(ui, rect, &entry.name, &format!("Version {version} — à installer"));
        }
        if resp.hovered() {
            let p = ui.painter();
            p.rect_filled(rect, 12.0, Color32::from_black_alpha(64));
            let c = rect.center() - vec2(0.0, 20.0);
            p.circle_filled(c, 29.0, ACCENT);
            // flèche « télécharger »
            p.line_segment([c + vec2(0.0, -12.0), c + vec2(0.0, 6.0)], Stroke::new(3.0_f32, Color32::WHITE));
            p.add(egui::Shape::convex_polygon(
                vec![c + vec2(-8.0, 2.0), c + vec2(8.0, 2.0), c + vec2(0.0, 12.0)],
                Color32::WHITE,
                Stroke::NONE,
            ));
        }
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        let resp = if entry.description.is_empty() { resp } else { resp.on_hover_text(&entry.description) };
        if resp.clicked() {
            self.actions.push(Action::AskInstall(entry.id.clone()));
        }
    }

    // ------------------------------------------------------------------ fenêtres modales

    fn show_modal(&mut self, ctx: &egui::Context) {
        let mut modal = std::mem::replace(&mut self.modal, Modal::None);
        let keep = match &mut modal {
            Modal::None => false,
            Modal::Edit(f) => self.edit_window(ctx, f),
            Modal::ConfirmRemove(id) => {
                let (name, catalog) = self.game(id).map(|g| (g.name.clone(), !g.catalog_id.is_empty())).unwrap_or_default();
                let text = if catalog {
                    format!("Désinstaller « {name} » ?\n\nSes fichiers seront supprimés du dossier Jeux.\nSes sauvegardes sont conservées.")
                } else {
                    format!("Retirer « {name} » du launcher ?\n\nLe jeu lui-même ne sera pas supprimé de ton disque.")
                };
                self.confirm(ctx, "Retirer le jeu", &text, "Retirer", DANGER, Action::Remove(id.clone()))
            }
            Modal::Missing(id) => {
                let path = self.game(id).map(|g| g.full_path()).unwrap_or_default();
                let text = format!("Le fichier du jeu est introuvable :\n{path}\n\nVeux-tu modifier son emplacement ?");
                self.confirm(ctx, "Jeu introuvable", &text, "Modifier", ACCENT, Action::Edit(id.clone()))
            }
            Modal::AlreadyRunning(id) => {
                let name = self.game(id).map(|g| g.name.clone()).unwrap_or_default();
                let text = format!("« {name} » est déjà lancé. En ouvrir une autre instance ?");
                self.confirm(ctx, "Déjà en cours", &text, "Lancer", PLAY, Action::ForceLaunch(id.clone()))
            }
            Modal::Message(title, text) => {
                let (title, text) = (title.clone(), text.clone());
                self.message(ctx, &title, &text)
            }
            Modal::InstallGame(id) => {
                let id = id.clone();
                match (self.catalog_entry(&id).cloned(), self.release(&id).cloned()) {
                    (Some(entry), Some(release)) => {
                        let installed = self.data.games.iter().find(|g| g.catalog_id == id).map(|g| g.version.clone());
                        let (title, ok, intro) = match &installed {
                            Some(v) => (
                                "Mettre à jour",
                                "Mettre à jour",
                                format!("{} {} → {}", entry.name, if v.is_empty() { "?" } else { v }, release.version),
                            ),
                            None => ("Installer", "Installer", format!("{} {}", entry.name, release.version)),
                        };
                        let text = notes_text(&intro, &release.notes);
                        self.confirm(ctx, title, &text, ok, ACCENT, Action::Install(id))
                    }
                    _ => false,
                }
            }
            Modal::LauncherUpdate => match self.launcher_update().cloned() {
                Some(release) => {
                    let intro = format!(
                        "Nouvelle version du launcher : {} (tu as la {}).\nLe launcher redémarrera tout seul.",
                        release.version,
                        update::VERSION
                    );
                    let text = notes_text(&intro, &release.notes);
                    let keep = self.confirm(ctx, "Mise à jour du launcher", &text, "Installer et redémarrer", PLAY, Action::UpdateLauncher);
                    if !keep && !matches!(self.actions.last(), Some(Action::UpdateLauncher)) {
                        self.launcher_dismissed = true; // « Plus tard »
                    }
                    keep
                }
                None => false,
            },
        };
        if keep && matches!(self.modal, Modal::None) {
            self.modal = modal;
        }
    }

    fn confirm(
        &mut self,
        ctx: &egui::Context,
        title: &str,
        text: &str,
        ok: &str,
        ok_color: Color32,
        action: Action,
    ) -> bool {
        let (enter, esc) = ctx.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
        let (mut accepted, mut cancelled) = (enter, esc);
        modal_window(ctx, "confirm", 460.0, |ui| {
            ui.label(RichText::new(title).font(bold_font(18.0)).color(TEXT));
            ui.add_space(6.0);
            ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                ui.label(RichText::new(text).color(TEXT));
            });
            ui.add_space(16.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(theme::filled_button(ok, ok_color).min_size(vec2(110.0, 34.0))).clicked() {
                    accepted = true;
                }
                if ui.add(Button::new("Annuler").min_size(vec2(110.0, 34.0))).clicked() {
                    cancelled = true;
                }
            });
        });
        if accepted && !cancelled {
            self.actions.push(action);
            return false;
        }
        !cancelled
    }

    fn message(&mut self, ctx: &egui::Context, title: &str, text: &str) -> bool {
        let mut close = ctx.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Escape));
        modal_window(ctx, "message", 440.0, |ui| {
            ui.label(RichText::new(title).font(bold_font(18.0)).color(TEXT));
            ui.add_space(6.0);
            ui.label(RichText::new(text).color(TEXT));
            ui.add_space(16.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(theme::filled_button("OK", ACCENT).min_size(vec2(110.0, 34.0))).clicked() {
                    close = true;
                }
            });
        });
        !close
    }

    fn edit_window(&mut self, ctx: &egui::Context, f: &mut EditForm) -> bool {
        let (enter, esc) = ctx.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
        let mut keep = !esc;
        let mut try_save = enter;
        let cover = self.tex.cover(ctx, &f.cover);
        let icon = self.tex.icon(ctx, &model::resolve(f.path.trim().trim_matches('"')));
        let title = if f.existing.is_some() { "Modifier le jeu" } else { "Ajouter un jeu" };

        modal_window(ctx, "edit", 600.0, |ui| {
            ui.label(RichText::new(title).font(bold_font(20.0)).color(TEXT));
            ui.add_space(8.0);
            ui.horizontal_top(|ui| {
                // Jaquette
                ui.vertical(|ui| {
                    ui.set_width(140.0);
                    ui.add_space(10.0);
                    let (rect, _) = ui.allocate_exact_size(vec2(140.0, 196.0), Sense::hover());
                    let initial = model::initial_of(&f.name);
                    theme::paint_art(ui, rect, cover.as_ref(), icon.as_ref(), &initial, 10.0, 64.0, -5.0);
                    ui.add_space(4.0);
                    if ui.add_sized([140.0, 30.0], Button::new("Choisir une image…")).clicked() {
                        let picked = rfd::FileDialog::new()
                            .set_title("Choisir une jaquette")
                            .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp", "ico"])
                            .pick_file();
                        if let Some(p) = picked {
                            let s = p.to_string_lossy().to_string();
                            if load_image(&s).is_some() {
                                f.cover = s;
                                f.error.clear();
                            } else {
                                f.error = "Cette image n'a pas pu être lue.".into();
                            }
                        }
                    }
                    let remove = theme::danger_button("Retirer l'image").min_size(vec2(140.0, 30.0));
                    if ui.add_enabled(!f.cover.is_empty(), remove).clicked() {
                        f.cover.clear();
                    }
                });
                ui.add_space(14.0);

                // Champs
                ui.vertical(|ui| {
                    let w = ui.available_width();
                    let field_w = w - 110.0;

                    field_label(ui, "Nom du jeu");
                    let name = ui.add(TextEdit::singleline(&mut f.name).desired_width(w).margin(vec2(10.0, 8.0)));
                    if !f.focused {
                        name.request_focus();
                        f.focused = true;
                    }

                    field_label(ui, "Fichier du jeu  (.exe, .bat, .lnk… ou lien steam://…)");
                    ui.horizontal(|ui| {
                        ui.add(TextEdit::singleline(&mut f.path).desired_width(field_w).margin(vec2(10.0, 8.0)));
                        if ui.add_sized([102.0, 34.0], Button::new("Parcourir…")).clicked() {
                            let mut dlg = rfd::FileDialog::new()
                                .set_title("Choisir le jeu")
                                .add_filter("Jeux et programmes", &["exe", "bat", "cmd", "lnk", "url"])
                                .add_filter("Tous les fichiers", &["*"]);
                            let current = model::resolve(f.path.trim().trim_matches('"'));
                            if let Some(dir) = Path::new(&current).parent().filter(|d| d.is_dir()) {
                                dlg = dlg.set_directory(dir);
                            }
                            if let Some(p) = dlg.pick_file() {
                                let s = p.to_string_lossy().to_string();
                                if f.name.trim().is_empty() {
                                    f.name = model::guess_name(&s);
                                }
                                f.path = s;
                            }
                        }
                    });

                    field_label(ui, "Arguments de lancement  (optionnel)");
                    ui.add(TextEdit::singleline(&mut f.args).desired_width(w).margin(vec2(10.0, 8.0)));

                    field_label(ui, "Dossier de travail  (optionnel — par défaut celui du jeu)");
                    ui.horizontal(|ui| {
                        ui.add(TextEdit::singleline(&mut f.workdir).desired_width(field_w).margin(vec2(10.0, 8.0)));
                        if ui.add_sized([102.0, 34.0], Button::new("Parcourir…")).clicked() {
                            if let Some(p) = rfd::FileDialog::new().set_title("Dossier de travail du jeu").pick_folder() {
                                f.workdir = p.to_string_lossy().to_string();
                            }
                        }
                    });
                });
            });

            if !f.error.is_empty() {
                ui.add_space(10.0);
                ui.label(RichText::new(&f.error).color(DANGER));
            }
            ui.add_space(16.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(theme::filled_button("Enregistrer", ACCENT).min_size(vec2(130.0, 36.0))).clicked() {
                    try_save = true;
                }
                if ui.add(Button::new("Annuler").min_size(vec2(110.0, 36.0))).clicked() {
                    keep = false;
                }
            });
        });

        if keep && try_save && self.save_form(f) {
            keep = false;
        }
        keep
    }
}

/// Texte d'une fenêtre d'installation : en-tête + notes de version (si publiées).
fn notes_text(intro: &str, notes: &str) -> String {
    if notes.trim().is_empty() {
        intro.to_string()
    } else {
        format!("{intro}\n\nNouveautés :\n{}", notes.trim())
    }
}

fn field_label(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text).color(SUB).size(12.0));
}

fn modal_window<R>(ctx: &egui::Context, id: &str, width: f32, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    egui::Window::new(id)
        .id(egui::Id::new(id))
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(Frame::window(&ctx.style()).fill(BG).inner_margin(Margin::same(24.0)).rounding(12.0))
        .show(ctx, |ui| {
            ui.set_width(width);
            add(ui)
        })
        .and_then(|r| r.inner)
}

// ------------------------------------------------------------------ boucle principale

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.hwnd.load(SeqCst) == 0 {
            self.hwnd.store(win::own_window(), SeqCst);
        }
        self.poll_finished();
        self.poll_updates(ctx);
        self.maybe_check(ctx);
        self.handle_drops(ctx);

        let modal_open = !matches!(self.modal, Modal::None);
        if !modal_open {
            self.handle_keys(ctx);
        }

        let visible = self.visible();
        let selected_visible =
            visible.iter().any(|&i| Some(self.data.games[i].id.as_str()) == self.selected.as_deref());
        if !selected_visible {
            self.selected = visible.first().map(|&i| self.data.games[i].id.clone());
        }

        self.header(ctx, !modal_open);
        self.footer(ctx, !modal_open);
        self.details(ctx, !modal_open);
        self.grid(ctx, !modal_open, &visible);

        if modal_open {
            ctx.layer_painter(egui::LayerId::new(egui::Order::PanelResizeLine, egui::Id::new("dim")))
                .rect_filled(ctx.screen_rect(), 0.0, Color32::from_black_alpha(150));
        }
        self.show_modal(ctx);

        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop")));
            let r = ctx.screen_rect().shrink(12.0);
            p.rect_filled(r, 16.0, Color32::from_rgba_unmultiplied(0x5B, 0x8C, 0xFF, 40));
            p.rect_stroke(r, 16.0, Stroke::new(2.0_f32, ACCENT));
            p.text(r.center(), Align2::CENTER_CENTER, "Dépose ici pour ajouter le jeu", bold_font(22.0), TEXT);
        }

        self.process_actions(ctx);
    }
}
