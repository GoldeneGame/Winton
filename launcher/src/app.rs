//! Interface du launcher.

use crate::model::{self, Game, LauncherData, Store};
use crate::theme::{
    self, bold_font, heading_font, title_font, Surface, ACCENT, DANGER, GOLD, GOLD_BRIGHT, GOLD_DIM, INK, INK_SUB,
    PLAY, RUNNING, SUB, TEXT,
};
use crate::updater::{self, Release};
use crate::win;

use eframe::egui::{
    self, pos2, vec2, Align, Align2, Button, Color32, FontId, Frame, Key, Layout, Margin, Rect, RichText, Rounding,
    ScrollArea, Sense, Shape, Stroke, TextEdit, TextureHandle, Ui,
};

/// Peint une texture derrière le contenu d'un panneau, une fois sa taille connue.
fn textured<R>(
    ui: &mut Ui,
    margin: Margin,
    surface: Surface,
    use_max: bool,
    tint: Color32,
    add: impl FnOnce(&mut Ui) -> R,
) -> (R, Rect) {
    let idx = ui.painter().add(Shape::Noop);
    let r = add(ui);
    let inner = if use_max { ui.max_rect() } else { ui.min_rect() };
    let full =
        Rect::from_min_max(inner.min - vec2(margin.left, margin.top), inner.max + vec2(margin.right, margin.bottom));
    ui.painter().set(idx, theme::surface_shape(surface, full, tint));
    (r, full)
}
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, AtomicUsize, Ordering::SeqCst};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Fréquence de vérification des mises à jour des jeux gérés.
const UPDATE_CHECK_EVERY: Duration = Duration::from_secs(15 * 60);

// ------------------------------------------------------------------ mises à jour

/// État de la mise à jour d'un jeu géré (récupéré depuis les releases GitHub).
enum Upd {
    Checking,
    UpToDate(String),
    NoRelease,
    Available(Release),
    Downloading { tag: String, done: Arc<AtomicU64>, total: u64 },
    Failed(String),
}

enum UpdMsg {
    Checked(String, Result<Option<Release>, String>),
    Installed(String, String, Result<std::path::PathBuf, String>),
}

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

/// Fenêtre « Rejoindre une partie » : le joueur colle le lien du MJ.
struct JoinForm {
    id: String,
    url: String,
    error: String,
    focused: bool,
}

enum Modal {
    None,
    Edit(EditForm),
    Join(JoinForm),
    ConfirmRemove(String),
    Missing(String),
    AlreadyRunning(String),
    Message(String, String),
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
    Update(String),
    AskJoin(String),
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
    // Jeux gérés : état des mises à jour par id de jeu.
    updates: HashMap<String, Upd>,
    last_check: Option<Instant>,
    upd_tx: Sender<UpdMsg>,
    upd_rx: Receiver<UpdMsg>,
}

impl LauncherApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::setup(&cc.egui_ctx);
        let store = Store::new();
        let (mut data, warning) = store.load();
        if warning.is_none() && model::attach_managed(&mut data) {
            let _ = store.save(&data);
        }
        let (tx, rx) = channel();
        let (upd_tx, upd_rx) = channel();
        LauncherApp {
            updates: HashMap::new(),
            last_check: None,
            upd_tx,
            upd_rx,
            store,
            data,
            selected: None,
            search: String::new(),
            status: "Astuce : glisse un .exe dans la fenêtre pour l'ajouter.".into(),
            tex: TexCache::default(),
            modal: warning.map_or(Modal::None, |w| Modal::Message("Winton".into(), w)),
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
        }
    }

    fn game(&self, id: &str) -> Option<&Game> {
        self.data.games.iter().find(|g| g.id == id)
    }

    /// Action du double-clic, du sceau et de la touche Entrée : pour un jeu en ligne c'est
    /// « Rejoindre » (un joueur ne doit pas lancer le serveur par mégarde), sinon « Jouer ».
    fn primary_action(&self, id: &str) -> Action {
        if self.game(id).is_some_and(Game::joinable) {
            Action::AskJoin(id.to_string())
        } else {
            Action::Launch(id.to_string())
        }
    }

    fn save(&mut self) {
        if let Err(e) = self.store.save(&self.data) {
            self.status = format!("⚠ Sauvegarde impossible : {e}");
        }
    }

    /// Indices des jeux affichés (recherche + tri).
    fn visible(&self) -> Vec<usize> {
        let q = self.search.trim().to_lowercase();
        let games = &self.data.games;
        let mut v: Vec<usize> =
            (0..games.len()).filter(|&i| q.is_empty() || games[i].name.to_lowercase().contains(&q)).collect();
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
        if let Some(s) = &g.source {
            self.data.removed_managed.push(s.repo.clone());
        }
        self.updates.remove(id);
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

        // Jeu géré pas encore installé : « Jouer » l'installe d'abord.
        if g.source.is_some() && !g.installed() {
            self.start_update(ctx, id);
            return;
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

    /// Ouvre la partie du MJ dans le navigateur : aucun serveur n'est lancé sur ce PC.
    /// Renvoie true si la fenêtre « Rejoindre » peut se fermer.
    fn join(&mut self, f: &mut JoinForm) -> bool {
        let url = match model::parse_join_url(&f.url) {
            Ok(u) => u,
            Err(e) => {
                f.error = e;
                return false;
            }
        };
        if let Err(e) = win::shell_launch(&url, "", None) {
            f.error = format!("Impossible d'ouvrir le navigateur : {e}");
            return false;
        }
        if let Some(g) = self.data.games.iter_mut().find(|g| g.id == f.id) {
            g.join_url = url;
            g.last_played = Some(model::now());
            self.status = format!("Partie de « {} » ouverte dans ton navigateur.", g.name);
        }
        self.save();
        true
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

    // ------------------------------------------------------------------ mises à jour

    /// Interroge GitHub pour chaque jeu géré (en arrière-plan).
    fn check_updates(&mut self, ctx: &egui::Context) {
        self.last_check = Some(Instant::now());
        for g in &self.data.games {
            let Some(src) = &g.source else { continue };
            if matches!(self.updates.get(&g.id), Some(Upd::Downloading { .. })) {
                continue;
            }
            self.updates.entry(g.id.clone()).or_insert(Upd::Checking);
            let (id, repo, asset, tx, ctx) = (g.id.clone(), src.repo.clone(), src.asset.clone(), self.upd_tx.clone(), ctx.clone());
            std::thread::spawn(move || {
                let _ = tx.send(UpdMsg::Checked(id, updater::latest(&repo, &asset)));
                ctx.request_repaint();
            });
        }
    }

    fn poll_updates(&mut self) {
        while let Ok(msg) = self.upd_rx.try_recv() {
            match msg {
                UpdMsg::Checked(id, result) => {
                    let Some(g) = self.game(&id) else { continue };
                    if matches!(self.updates.get(&id), Some(Upd::Downloading { .. })) {
                        continue;
                    }
                    let version = g.source.as_ref().map(|s| s.version.clone()).unwrap_or_default();
                    let state = match result {
                        Ok(None) => Upd::NoRelease,
                        Ok(Some(r)) if r.tag == version && g.installed() => Upd::UpToDate(r.tag),
                        Ok(Some(r)) => Upd::Available(r),
                        Err(e) => Upd::Failed(e),
                    };
                    self.updates.insert(id, state);
                }
                UpdMsg::Installed(id, tag, result) => match result {
                    Ok(exe) => {
                        if let Some(g) = self.data.games.iter_mut().find(|g| g.id == id) {
                            g.path = model::make_portable(&exe.to_string_lossy());
                            if let Some(s) = g.source.as_mut() {
                                s.version = tag.clone();
                            }
                            self.status = format!("« {} » est à jour ({tag}).", g.name);
                        }
                        self.updates.insert(id, Upd::UpToDate(tag));
                        self.save();
                    }
                    Err(e) => {
                        let name = self.game(&id).map(|g| g.name.clone()).unwrap_or_default();
                        self.updates.insert(id, Upd::Failed(e.clone()));
                        self.modal = Modal::Message("Mise à jour impossible".into(), format!("« {name} » n'a pas pu être installé :\n\n{e}"));
                    }
                },
            }
        }
    }

    /// Télécharge et installe la dernière version d'un jeu géré.
    fn start_update(&mut self, ctx: &egui::Context, id: &str) {
        let Some(g) = self.game(id).cloned() else { return };
        let Some(src) = g.source.clone() else { return };
        if self.running.contains_key(id) {
            self.modal = Modal::Message("Jeu en cours".into(), format!("Ferme « {} » avant de le mettre à jour.", g.name));
            return;
        }
        let rel = match self.updates.get(id) {
            Some(Upd::Available(r)) => r.clone(),
            Some(Upd::Downloading { .. }) => return,
            Some(Upd::Checking) | None => {
                self.status = "Recherche de la dernière version… réessaie dans un instant.".into();
                return;
            }
            Some(Upd::NoRelease) => {
                self.modal = Modal::Message(
                    "Pas encore disponible".into(),
                    format!("Aucune version de « {} » n'a encore été publiée sur GitHub ({}).", g.name, src.repo),
                );
                return;
            }
            Some(Upd::Failed(e)) => {
                self.modal = Modal::Message("Mise à jour impossible".into(), e.clone());
                self.check_updates(ctx);
                return;
            }
            Some(Upd::UpToDate(_)) => return,
        };
        let done = Arc::new(AtomicU64::new(0));
        self.updates.insert(id.to_string(), Upd::Downloading { tag: rel.tag.clone(), done: done.clone(), total: rel.size });
        self.status = format!("Téléchargement de « {} » {}…", g.name, rel.tag);
        let dir = self.store.game_dir(&src.repo);
        let previous = g.installed().then(|| std::path::PathBuf::from(g.full_path()));
        let (id, tx, ctx) = (id.to_string(), self.upd_tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let result = updater::install(&rel, &src.asset, &dir, previous, &done);
            let _ = tx.send(UpdMsg::Installed(id, rel.tag, result));
            ctx.request_repaint();
        });
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
                Action::Update(id) => self.start_update(ctx, &id),
                Action::AskJoin(id) => {
                    let url = self.game(&id).map(|g| g.join_url.clone()).unwrap_or_default();
                    self.modal = Modal::Join(JoinForm { id, url, error: String::new(), focused: false });
                }
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
                let a = self.primary_action(&id);
                self.actions.push(a);
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
        let margin = Margin { left: 24.0, right: 24.0, top: 14.0, bottom: 17.0 };
        egui::TopBottomPanel::top("header")
            .show_separator_line(false)
            .frame(Frame::none().inner_margin(margin))
            .show(ctx, |ui| {
                let (_, full) = textured(ui, margin, Surface::Wood, false, Color32::WHITE, |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    ui.horizontal(|ui| {
                        theme::logo(ui, 38.0);
                        ui.add_space(6.0);
                        ui.label(RichText::new("Winton").font(title_font(26.0)).color(GOLD));
                        let count = match self.data.games.len() {
                            0 => String::new(),
                            1 => "· 1 jeu".into(),
                            n => format!("· {n} jeux"),
                        };
                        ui.add_space(4.0);
                        ui.label(RichText::new(count).font(heading_font(13.0)).color(SUB));

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let add = theme::filled_button("+  Ajouter un jeu", ACCENT).min_size(vec2(0.0, 36.0));
                            if ui.add(add).on_hover_text("Ctrl+N").clicked() {
                                self.actions.push(Action::Add(None));
                            }
                            ui.add_space(8.0);
                            self.update_button(ui);
                            let resp = ui.add(
                                TextEdit::singleline(&mut self.search)
                                    .id(egui::Id::new("search"))
                                    .hint_text("Rechercher…  (Ctrl+F)")
                                    .desired_width(240.0)
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
                                    if let Some(id) = self.selected.clone() {
                                        let a = self.primary_action(&id);
                                        self.actions.push(a);
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
                theme::gold_rule(ui.painter(), full.left(), full.right(), full.bottom() - 2.5);
            });
        if sort_changed {
            self.save();
        }
    }

    fn footer(&mut self, ctx: &egui::Context, enabled: bool) {
        let mut changed = false;
        let margin = Margin { left: 24.0, right: 24.0, top: 9.0, bottom: 7.0 };
        egui::TopBottomPanel::bottom("footer")
            .show_separator_line(false)
            .frame(Frame::none().inner_margin(margin))
            .show(ctx, |ui| {
                let (_, full) = textured(ui, margin, Surface::Wood, false, Color32::from_gray(165), |ui| {
                    ui.add_enabled_ui(enabled, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&self.status).color(SUB).size(13.0));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let mut v = self.data.minimize_on_launch;
                                let text = RichText::new("Réduire le launcher pendant que je joue").color(SUB).size(13.0);
                                if ui.checkbox(&mut v, text).changed() {
                                    self.data.minimize_on_launch = v;
                                    changed = true;
                                }
                            });
                        });
                    });
                });
                theme::gold_rule(ui.painter(), full.left(), full.right(), full.top() + 2.0);
            });
        if changed {
            self.save();
        }
    }

    fn details(&mut self, ctx: &egui::Context, enabled: bool) {
        let margin = Margin { left: 30.0, right: 24.0, top: 24.0, bottom: 24.0 };
        egui::SidePanel::right("details")
            .exact_width(350.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(Frame::none().inner_margin(margin))
            .show(ctx, |ui| {
                let (_, full) = textured(ui, margin, Surface::Parchment, true, Color32::WHITE, |ui| {
                // ombres sur les bords du parchemin
                let r = ui.max_rect();
                let full = Rect::from_min_max(r.min - vec2(margin.left, margin.top), r.max + vec2(margin.right, margin.bottom));
                theme::vignette(ui.painter(), full, 40.0, 70);
                ui.add_enabled_ui(enabled, |ui| {
                    let Some(g) = self.selected.as_deref().and_then(|id| self.game(id)).cloned() else {
                        if !self.data.games.is_empty() {
                            ui.centered_and_justified(|ui| {
                                ui.label(RichText::new("Choisis un jeu").font(heading_font(16.0)).color(INK_SUB))
                            });
                        }
                        return;
                    };
                    let cover = self.tex.cover(ctx, &model::resolve(&g.cover_path));
                    let icon = self.tex.icon(ctx, &g.full_path());
                    let running = self.running.contains_key(&g.id);

                    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                        let w = ui.available_width();
                        let (rect, _) = ui.allocate_exact_size(vec2(w, 220.0), Sense::hover());
                        ui.painter().rect_filled(rect.translate(vec2(0.0, 4.0)), 3.0, Color32::from_black_alpha(90));
                        theme::paint_art(ui, rect, cover.as_ref(), icon.as_ref(), &g.initial(), 3.0, 96.0, 0.0);
                        theme::ornate_frame(ui.painter(), rect, GOLD, 5.0);
                        ui.add_space(12.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new(&g.name).font(heading_font(22.0)).color(INK));
                        });
                        let installed = g.installed();
                        let path_text = if installed || g.source.is_none() { g.full_path() } else { "Pas encore installé".into() };
                        ui.add(egui::Label::new(RichText::new(&path_text).color(INK_SUB).size(11.5)).truncate())
                            .on_hover_text(&path_text);
                        theme::divider(ui, INK_SUB);
                        ui.add_space(4.0);

                        let downloading = matches!(self.updates.get(&g.id), Some(Upd::Downloading { .. }));
                        if g.joinable() {
                            // Joueurs : on ouvre le lien du MJ. MJ : on lance le serveur (l'exe).
                            let join = theme::grand_button(ui, vec2(w, 54.0), "Rejoindre une partie", PLAY)
                                .on_hover_text("Colle le lien envoyé par le MJ : la partie s'ouvre dans ton navigateur.\nRien n'est installé ni lancé sur ce PC.");
                            if join.clicked() {
                                self.actions.push(Action::AskJoin(g.id.clone()));
                            }
                            ui.add_space(8.0);
                            let text = if running {
                                "Partie hébergée — en cours"
                            } else if !installed {
                                if downloading { "Installation…" } else { "Installer pour héberger (MJ)" }
                            } else {
                                "Héberger la partie (MJ)"
                            };
                            let host = ui
                                .add(theme::filled_button(text, if running { RUNNING } else { ACCENT }).min_size(vec2(w, 38.0)))
                                .on_hover_text("Réservé au MJ : ton PC devient le serveur de la partie.\nEnvoie ensuite le lien affiché aux joueurs.");
                            if host.clicked() && !downloading {
                                self.actions.push(Action::Launch(g.id.clone()));
                            }
                            if installed || downloading {
                                ui.add_space(8.0);
                                self.update_box(ui, &g, w);
                            }
                        } else {
                        let (text, fill) = if running {
                            ("En cours…", RUNNING)
                        } else if g.source.is_some() && !installed {
                            (if downloading { "Installation…" } else { "Installer" }, ACCENT)
                        } else {
                            ("Jouer", PLAY)
                        };
                        if theme::grand_button(ui, vec2(w, 54.0), text, fill).clicked() && !downloading {
                            self.actions.push(Action::Launch(g.id.clone()));
                        }
                        if g.source.is_some() {
                            ui.add_space(8.0);
                            self.update_box(ui, &g, w);
                        }
                        }
                        ui.add_space(12.0);

                        Frame::none()
                            .fill(Color32::from_rgba_unmultiplied(0x6E, 0x48, 0x1E, 28))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0x6E, 0x52, 0x33, 120)))
                            .rounding(2.0)
                            .inner_margin(Margin::symmetric(14.0, 10.0))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                let rows = [
                                    ("Temps de jeu", model::format_duration(g.play_seconds)),
                                    ("Dernière session", g.last_played_text()),
                                    ("Ajouté le", g.added_text()),
                                ];
                                for (i, (k, v)) in rows.into_iter().enumerate() {
                                    if i > 0 {
                                        let x = ui.max_rect().x_range();
                                        let y = ui.cursor().top() - 1.0;
                                        ui.painter().hline(x, y, Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0x6E, 0x52, 0x33, 70)));
                                    }
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(k).color(INK_SUB));
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            ui.label(RichText::new(v).font(bold_font(15.0)).color(INK));
                                        });
                                    });
                                }
                            });
                        ui.add_space(10.0);

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
                // bordure gauche : poutre de bois et filet doré
                let p = ui.painter();
                let beam = Rect::from_min_max(full.left_top(), pos2(full.left() + 7.0, full.bottom()));
                p.add(theme::surface_shape(Surface::Wood, beam, Color32::from_gray(150)));
                p.vline(beam.right(), full.y_range(), Stroke::new(1.0_f32, GOLD_DIM));
                p.vline(beam.left() + 0.5, full.y_range(), Stroke::new(1.0_f32, Color32::from_black_alpha(200)));
                theme::horizontal_gradient(
                    p,
                    Rect::from_min_max(pos2(beam.right(), full.top()), pos2(beam.right() + 14.0, full.bottom())),
                    Color32::from_black_alpha(80),
                    Color32::TRANSPARENT,
                );
            });
    }

    /// Bouton « Mettre à jour Winton » de la barre du haut : visible dès qu'une nouvelle version
    /// d'un jeu installé est publiée ; il installe toutes les mises à jour d'un coup.
    fn update_button(&mut self, ui: &mut Ui) {
        // Téléchargements en cours : progression globale à la place du bouton.
        let (mut got, mut total, mut busy) = (0u64, 0u64, false);
        for u in self.updates.values() {
            if let Upd::Downloading { done, total: t, .. } = u {
                busy = true;
                got += done.load(SeqCst);
                total += *t;
            }
        }
        if busy {
            let pct = if total > 0 { got * 100 / total } else { 0 };
            ui.label(RichText::new(format!("⟳  Mise à jour… {pct} %")).font(heading_font(13.0)).color(GOLD_BRIGHT));
            ui.ctx().request_repaint_after(Duration::from_millis(150));
            ui.add_space(12.0);
            return;
        }
        let mut lines = Vec::new();
        for g in self.data.games.iter().filter(|g| g.installed()) {
            if let Some(Upd::Available(r)) = self.updates.get(&g.id) {
                let mut line = format!("• {} → {}", g.name, r.tag);
                if !r.notes.is_empty() {
                    line.push_str(&format!("\n   {}", r.notes.lines().next().unwrap_or_default()));
                }
                lines.push((g.id.clone(), line));
            }
        }
        if lines.is_empty() {
            return;
        }
        let hover = format!(
            "Nouveautés disponibles :\n{}\n\nTa configuration et tes parties sont conservées.",
            lines.iter().map(|(_, l)| l.as_str()).collect::<Vec<_>>().join("\n")
        );
        let button = theme::filled_button("⟳  Mettre à jour Winton", GOLD).min_size(vec2(0.0, 36.0));
        if ui.add(button).on_hover_text(hover).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            for (id, _) in lines {
                self.actions.push(Action::Update(id));
            }
        }
        ui.add_space(8.0);
    }

    /// Version d'un jeu géré (état de sa mise à jour).
    fn update_box(&mut self, ui: &mut Ui, g: &Game, w: f32) {
        let version = g.source.as_ref().map(|s| s.version.clone()).unwrap_or_default();
        let note = |ui: &mut Ui, text: String| {
            ui.vertical_centered(|ui| ui.label(RichText::new(text).color(INK_SUB).size(12.5)));
        };
        match self.updates.get(&g.id) {
            None | Some(Upd::Checking) => note(ui, "Recherche de mises à jour…".into()),
            Some(Upd::UpToDate(tag)) => note(ui, format!("✔ À jour  ·  version {tag}")),
            Some(Upd::NoRelease) => note(ui, "Aucune version publiée pour l'instant.".into()),
            Some(Upd::Failed(e)) => {
                let e = e.clone();
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(format!("⚠ {e}")).color(DANGER).size(12.5));
                    if ui.small_button("Réessayer").clicked() {
                        self.last_check = None;
                    }
                });
            }
            Some(Upd::Downloading { tag, done, total }) => {
                let (got, total) = (done.load(SeqCst) as f32, (*total).max(1) as f32);
                let frac = (got / total).clamp(0.0, 1.0);
                let text = format!("Téléchargement {tag} · {:.0} / {:.0} Mo", got / 1e6, total / 1e6);
                ui.add(egui::ProgressBar::new(frac).desired_width(w).text(RichText::new(text).color(TEXT).size(12.5)));
                ui.ctx().request_repaint_after(Duration::from_millis(150));
            }
            Some(Upd::Available(r)) => {
                if g.installed() {
                    let installed = if version.is_empty() { String::new() } else { format!(" (installée : {version})") };
                    note(ui, format!("Nouvelle version {}{installed}\n→ bouton « Mettre à jour Winton » en haut", r.tag));
                } else {
                    note(ui, format!("Version disponible : {}", r.tag));
                }
            }
        }
    }

    fn grid(&mut self, ctx: &egui::Context, enabled: bool, visible: &[usize]) {
        egui::CentralPanel::default().frame(Frame::none().inner_margin(Margin::same(24.0))).show(ctx, |ui| {
            let r = ui.max_rect().expand(24.0);
            ui.painter().add(theme::surface_shape(Surface::Stone, r, Color32::WHITE));
            // lueur chaude au centre, ombre sur les bords
            theme::vertical_gradient(
                ui.painter(),
                r,
                Color32::from_rgba_unmultiplied(0x60, 0x38, 0x18, 22),
                Color32::TRANSPARENT,
            );
            theme::vignette(ui.painter(), r, 90.0, 140);
            ui.add_enabled_ui(enabled, |ui| {
                if visible.is_empty() {
                    self.empty_state(ui);
                    return;
                }
                ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(20.0, 20.0);
                    // marge pour que les ornements des cadres ne soient pas rognés
                    Frame::none().inner_margin(Margin::same(8.0)).show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for &i in visible {
                                self.card(ui, i);
                            }
                        });
                    });
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
            ui.add_space((ui.available_height() / 2.0 - 110.0).max(0.0));
            let (r, _) = ui.allocate_exact_size(vec2(66.0, 78.0), Sense::hover());
            theme::shield(ui.painter(), r, Color32::from_rgb(0x3A, 0x26, 0x18), GOLD_DIM);
            theme::play_triangle(ui.painter(), r.center() + vec2(3.0, -6.0), 13.0, GOLD_DIM);
            ui.add_space(16.0);
            ui.label(RichText::new(title).font(heading_font(22.0)).color(GOLD));
            let (dr, _) = ui.allocate_exact_size(vec2(280.0, 14.0), Sense::hover());
            theme::paint_divider(ui.painter(), dr.left(), dr.right(), dr.center().y, GOLD_DIM, 5.0);
            ui.label(RichText::new(sub).color(SUB));
        });
    }

    fn card(&mut self, ui: &mut Ui, index: usize) {
        let size = vec2(180.0, 252.0);
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        let g = &self.data.games[index];
        let id = g.id.clone();

        if ui.is_rect_visible(rect) {
            let (name, sub, initial) = (g.name.clone(), g.card_subtitle(), g.initial());
            let (cover_path, full_path) = (model::resolve(&g.cover_path), g.full_path());
            let ctx = ui.ctx().clone();
            let cover = self.tex.cover(&ctx, &cover_path);
            let icon = self.tex.icon(&ctx, &full_path);

            ui.painter().rect_filled(rect.translate(vec2(0.0, 5.0)).expand(1.0), 4.0, Color32::from_black_alpha(130));
            theme::paint_art(ui, rect, cover.as_ref(), icon.as_ref(), &initial, 3.0, 80.0, -25.0);

            // Dégradé + plaque de nom en bas de la carte
            let p = ui.painter();
            let dark = Color32::from_rgba_unmultiplied(0x0E, 0x08, 0x05, 235);
            let grad_bottom = rect.bottom() - 12.0;
            theme::vertical_gradient(
                p,
                Rect::from_min_max(pos2(rect.left(), rect.bottom() - 100.0), pos2(rect.right(), grad_bottom)),
                Color32::TRANSPARENT,
                dark,
            );
            p.rect_filled(
                Rect::from_min_max(pos2(rect.left(), grad_bottom), rect.max),
                Rounding { nw: 0.0, ne: 0.0, sw: 3.0, se: 3.0 },
                dark,
            );
            theme::paint_divider(p, rect.left() + 18.0, rect.right() - 18.0, rect.bottom() - 60.0, GOLD_DIM, 3.5);
            let name_g = theme::truncated(ui, &name, heading_font(14.5), GOLD_BRIGHT, size.x - 24.0);
            p.galley(pos2(rect.center().x - name_g.size().x / 2.0, rect.bottom() - 50.0), name_g, GOLD_BRIGHT);
            let sub_color = Color32::from_rgb(0xC4, 0xAE, 0x86);
            let sub_g = theme::truncated(ui, &sub, FontId::proportional(12.0), sub_color, size.x - 24.0);
            p.galley(pos2(rect.center().x - sub_g.size().x / 2.0, rect.bottom() - 28.0), sub_g, sub_color);

            if self.running.contains_key(&id) {
                let badge = theme::truncated(ui, "EN COURS", heading_font(10.5), TEXT, 200.0);
                let br = Rect::from_min_size(rect.min + vec2(12.0, 12.0), badge.size() + vec2(18.0, 8.0));
                p.rect_filled(br, 2.0, RUNNING);
                p.rect_stroke(br, 2.0, Stroke::new(1.0_f32, GOLD));
                p.galley(br.min + vec2(9.0, 4.0), badge, TEXT);
            }
            let upd_badge = match self.updates.get(&id) {
                Some(Upd::Downloading { .. }) => Some("TÉLÉCHARGEMENT"),
                Some(Upd::Available(_)) if !self.data.games[index].installed() => Some("À INSTALLER"),
                _ => None,
            };
            if let Some(text) = upd_badge {
                let badge = theme::truncated(ui, text, heading_font(10.5), INK, 200.0);
                let br = Rect::from_min_size(
                    pos2(rect.right() - 12.0 - badge.size().x - 18.0, rect.top() + 12.0),
                    badge.size() + vec2(18.0, 8.0),
                );
                p.rect_filled(br, 2.0, GOLD_BRIGHT);
                p.rect_stroke(br, 2.0, Stroke::new(1.0_f32, Color32::from_rgb(0x5E, 0x44, 0x1C)));
                p.galley(br.min + vec2(9.0, 4.0), badge, INK);
            }
        }

        let circle = rect.center() - vec2(0.0, 20.0);
        let on_circle = resp.hover_pos().is_some_and(|pos| pos.distance(circle) <= 30.0);
        let selected = self.selected.as_deref() == Some(id.as_str());
        if resp.hovered() {
            let p = ui.painter();
            p.rect_filled(rect, 3.0, Color32::from_black_alpha(70));
            theme::wax_seal(p, circle, 30.0, on_circle);
        }
        if ui.is_rect_visible(rect) {
            let p = ui.painter();
            let gold = if selected {
                GOLD_BRIGHT
            } else if resp.hovered() {
                GOLD
            } else {
                GOLD_DIM
            };
            if selected {
                p.rect_stroke(rect.expand(3.0), 4.0, Stroke::new(3.0_f32, Color32::from_rgba_unmultiplied(0xF2, 0xD2, 0x7C, 70)));
            }
            theme::ornate_frame(p, rect, gold, 4.5);
        }

        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.double_clicked() || (resp.clicked() && on_circle) {
            self.actions.push(Action::Select(id.clone()));
            let a = self.primary_action(&id);
            self.actions.push(a);
        } else if resp.clicked() || resp.secondary_clicked() {
            self.actions.push(Action::Select(id.clone()));
        }
        let joinable = self.data.games[index].joinable();
        resp.context_menu(|ui| {
            if joinable && ui.button("Rejoindre une partie…").clicked() {
                self.actions.push(Action::AskJoin(id.clone()));
                ui.close_menu();
            }
            let items = [
                (if joinable { "Héberger la partie (MJ)" } else { "Jouer" }, Action::Launch(id.clone())),
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

    // ------------------------------------------------------------------ fenêtres modales

    fn show_modal(&mut self, ctx: &egui::Context) {
        let mut modal = std::mem::replace(&mut self.modal, Modal::None);
        let keep = match &mut modal {
            Modal::None => false,
            Modal::Edit(f) => self.edit_window(ctx, f),
            Modal::Join(f) => self.join_window(ctx, f),
            Modal::ConfirmRemove(id) => {
                let name = self.game(id).map(|g| g.name.clone()).unwrap_or_default();
                let text = format!("Retirer « {name} » du launcher ?\n\nLe jeu lui-même ne sera pas supprimé de ton disque.");
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
        modal_window(ctx, "confirm", 440.0, |ui| {
            modal_title(ui, title);
            ui.label(RichText::new(text).color(TEXT));
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
            modal_title(ui, title);
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

    fn join_window(&mut self, ctx: &egui::Context, f: &mut JoinForm) -> bool {
        let (enter, esc) = ctx.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
        let mut keep = !esc;
        let mut go = enter;
        let name = self.game(&f.id).map(|g| g.name.clone()).unwrap_or_default();
        modal_window(ctx, "join", 520.0, |ui| {
            modal_title(ui, "Rejoindre une partie");
            ui.label(
                RichText::new(format!(
                    "Colle le lien de la partie « {name} » envoyé par le MJ (Discord, SMS…).\n\
                     Elle s'ouvrira dans ton navigateur : ton PC n'héberge rien."
                ))
                .color(TEXT),
            );
            field_label(ui, "Lien de la partie");
            let resp = ui.add(
                TextEdit::singleline(&mut f.url)
                    .hint_text("https://xxxx.trycloudflare.com")
                    .desired_width(f32::INFINITY)
                    .margin(vec2(10.0, 8.0)),
            );
            if !f.focused {
                resp.request_focus();
                f.focused = true;
            }
            if resp.changed() {
                f.error.clear();
            }
            ui.label(
                RichText::new("Le lien change à chaque séance : demande le nouveau au MJ s'il ne marche plus.")
                    .color(SUB)
                    .size(12.5),
            );
            if !f.error.is_empty() {
                ui.add_space(8.0);
                ui.label(RichText::new(&f.error).color(DANGER));
            }
            ui.add_space(16.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(theme::filled_button("Rejoindre", PLAY).min_size(vec2(130.0, 36.0))).clicked() {
                    go = true;
                }
                if ui.add(Button::new("Annuler").min_size(vec2(110.0, 36.0))).clicked() {
                    keep = false;
                }
            });
        });
        if keep && go && self.join(f) {
            keep = false;
        }
        keep
    }

    fn edit_window(&mut self, ctx: &egui::Context, f: &mut EditForm) -> bool {
        let (enter, esc) = ctx.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
        let mut keep = !esc;
        let mut try_save = enter;
        let cover = self.tex.cover(ctx, &f.cover);
        let icon = self.tex.icon(ctx, &model::resolve(f.path.trim().trim_matches('"')));
        let title = if f.existing.is_some() { "Modifier le jeu" } else { "Ajouter un jeu" };

        modal_window(ctx, "edit", 600.0, |ui| {
            modal_title(ui, title);
            ui.horizontal_top(|ui| {
                // Jaquette
                ui.vertical(|ui| {
                    ui.set_width(160.0);
                    ui.add_space(10.0);
                    let (rect, _) = ui.allocate_exact_size(vec2(160.0, 224.0), Sense::hover());
                    let initial = model::initial_of(&f.name);
                    theme::paint_art(ui, rect, cover.as_ref(), icon.as_ref(), &initial, 10.0, 64.0, -5.0);
                    ui.add_space(4.0);
                    if ui.add_sized([160.0, 30.0], Button::new(RichText::new("Choisir une image…").font(heading_font(12.0)))).clicked() {
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
                    let remove = theme::danger_button("Retirer l'image").min_size(vec2(160.0, 30.0));
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

fn field_label(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text).color(SUB).size(13.0));
}

/// Titre de fenêtre : capitales dorées et séparateur orné.
fn modal_title(ui: &mut Ui, title: &str) {
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(title).font(heading_font(21.0)).color(GOLD_BRIGHT));
    });
    theme::divider(ui, GOLD_DIM);
    ui.add_space(4.0);
}

fn modal_window<R>(ctx: &egui::Context, id: &str, width: f32, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    let shown = egui::Window::new(id)
        .id(egui::Id::new(id))
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(Frame::window(&ctx.style()).fill(theme::CARD).stroke(Stroke::NONE).inner_margin(Margin::same(28.0)).rounding(3.0))
        .show(ctx, |ui| {
            ui.set_width(width);
            // Emplacement réservé pour le fond, rempli une fois la taille de la fenêtre connue.
            let idx = ui.painter().add(Shape::Noop);
            (add(ui), ui.layer_id(), idx)
        })?;
    let (r, layer, idx) = shown.inner?;
    let full = shown.response.rect;
    let mut shapes = vec![theme::surface_shape(Surface::Stone, full, Color32::WHITE)];
    shapes.push(Shape::rect_filled(full, 0.0, Color32::from_rgba_unmultiplied(0x6A, 0x40, 0x1C, 40)));
    shapes.extend(theme::ornate_frame_shapes(full.shrink(6.0), GOLD, 5.0));
    ctx.graphics_mut(|g| g.entry(layer).set(idx, full, Shape::Vec(shapes)));
    Some(r)
}

// ------------------------------------------------------------------ boucle principale

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.hwnd.load(SeqCst) == 0 {
            self.hwnd.store(win::own_window(), SeqCst);
        }
        self.poll_finished();
        self.poll_updates();
        if self.last_check.map_or(true, |t| t.elapsed() >= UPDATE_CHECK_EVERY) {
            self.check_updates(ctx);
        }
        ctx.request_repaint_after(Duration::from_secs(60));
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
            p.rect_filled(r, 3.0, Color32::from_rgba_unmultiplied(0x14, 0x0C, 0x06, 200));
            theme::ornate_frame(&p, r, GOLD, 7.0);
            p.text(r.center(), Align2::CENTER_CENTER, "Dépose ici pour ajouter le jeu", heading_font(26.0), GOLD_BRIGHT);
        }

        self.process_actions(ctx);
    }
}
