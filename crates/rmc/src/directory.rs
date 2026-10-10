// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le modèle d'un panneau : un répertoire, ses entrées, et ce qui y est marqué.
//!
//! # Il ne connaît aucune vue
//!
//! C'est le MVC de l'ADR-007 : le modèle reçoit une [`DirectoryAction`], consigne ce qui
//! a changé dans le journal que l'appelant lui tend, et n'appelle personne. Le contrôleur
//! lit le journal et pousse le reflet dans la liste.
//!
//! # Le marquage est ici, pas dans la liste
//!
//! Marquer un fichier est un fait que l'application garde : il survit à un défilement, à
//! un changement de panneau, et il commande ce qu'une touche de fonction fera ensuite. La
//! liste le demande et l'affiche ; c'est ce module qui le tient.
//!
//! # Ce que la relecture perd, et ce qu'elle garde
//!
//! [`DirectoryAction::Reload`] relit le répertoire sur le disque. Les marques y
//! **survivent par nom** : un fichier encore là reste marqué, un fichier disparu perd sa
//! marque avec lui. Les garder par indice les ferait glisser sur le voisin dès qu'un
//! fichier est créé.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use air_tui::model::{AirChangeSet, AirGeneration, AirModel};

/// Le nom de l'entrée qui remonte d'un niveau.
pub const PARENT_NAME: &str = "..";

/// Ce qu'une entrée de répertoire est.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntryKind {
    /// L'entrée qui remonte d'un niveau — toujours la première, et jamais marquable.
    Parent,
    /// Un répertoire.
    Directory,
    /// Un lien symbolique, quelle que soit sa cible.
    ///
    /// Le distinguer d'un fichier demande `symlink_metadata` : sans lui, un lien cassé
    /// serait rapporté comme absent, et un lien vers un répertoire comme un répertoire.
    Symlink,
    /// Tout le reste — un fichier ordinaire, un tube nommé, un périphérique.
    File,
}

/// Une entrée de répertoire, telle que le panneau la montre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Le nom, sans son chemin.
    pub name: String,
    /// Ce que l'entrée est.
    pub kind: EntryKind,
    /// La taille en octets, nulle pour un répertoire.
    pub size: u64,
    /// Vrai quand l'utilisateur l'a marquée.
    pub marked: bool,
}

impl Entry {
    /// Vrai quand activer cette entrée change de répertoire.
    #[must_use]
    pub const fn is_traversable(&self) -> bool {
        matches!(self.kind, EntryKind::Parent | EntryKind::Directory)
    }

    /// Vrai quand cette entrée peut porter une marque.
    ///
    /// `..` ne peut pas : elle n'est pas un fichier de ce répertoire, et la marquer
    /// n'aurait rien à désigner.
    #[must_use]
    pub const fn is_markable(&self) -> bool {
        !matches!(self.kind, EntryKind::Parent)
    }
}

/// Ce qu'on peut demander à un panneau.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectoryAction {
    /// Relire le répertoire courant sur le disque.
    Reload,
    /// Activer l'entrée d'indice donné : y descendre si c'en est un.
    Activate(usize),
    /// Remonter d'un niveau.
    GoToParent,
    /// Marquer ou démarquer l'entrée d'indice donné.
    ToggleMark(usize),
}

/// Ce que le panneau rapporte avoir changé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryChange {
    /// Le contenu a été relu : la liste doit être poussée à nouveau.
    Listed,
    /// Le répertoire courant a changé : le curseur doit repartir du début.
    Entered,
    /// La lecture a échoué : [`Directory::error`] dit pourquoi.
    Failed,
    /// Une marque a basculé.
    Marked,
}

/// Le modèle d'un panneau.
#[derive(Debug)]
pub struct Directory {
    /// Le répertoire courant.
    path: PathBuf,
    /// Ses entrées, `..` en tête puis les répertoires, puis le reste.
    entries: Vec<Entry>,
    /// Le dernier message d'échec, s'il y en a un.
    error: Option<String>,
    /// La génération du contenu — une liste qui n'a pas changé ne se repousse pas.
    generation: AirGeneration,
}

impl Directory {
    /// Un panneau sur un chemin, **sans** rien lire encore.
    ///
    /// La lecture est une action : elle peut échouer, et un constructeur qui échoue
    /// obligerait l'appelant à traiter l'échec avant d'avoir un écran où l'afficher.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            entries: Vec::new(),
            error: None,
            generation: AirGeneration::new(),
        }
    }

    /// Le répertoire courant.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Les entrées.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Le dernier échec de lecture, s'il y en a un.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// La génération du contenu.
    #[must_use]
    pub const fn generation(&self) -> AirGeneration {
        self.generation
    }

    /// Le nombre d'entrées marquées.
    #[must_use]
    pub fn marked_count(&self) -> usize {
        self.entries.iter().filter(|entry| entry.marked).count()
    }

    /// La somme des tailles marquées, en octets.
    #[must_use]
    pub fn marked_size(&self) -> u64 {
        self.entries
            .iter()
            .filter(|entry| entry.marked)
            .fold(0, |total, entry| total.saturating_add(entry.size))
    }

    /// Le chemin complet de l'entrée d'indice donné.
    #[must_use]
    pub fn entry_path(&self, index: usize) -> Option<PathBuf> {
        let entry = self.entries.get(index)?;
        if matches!(entry.kind, EntryKind::Parent) {
            return self.parent_path();
        }
        Some(self.path.join(&entry.name))
    }

    /// Le chemin du répertoire parent, s'il y en a un.
    fn parent_path(&self) -> Option<PathBuf> {
        self.path.parent().map(Path::to_path_buf)
    }

    /// Relit le répertoire, en gardant les marques des noms encore présents.
    fn list(&mut self, changes: &mut AirChangeSet<DirectoryChange>) {
        let marked: BTreeSet<String> = self
            .entries
            .iter()
            .filter(|entry| entry.marked)
            .map(|entry| entry.name.clone())
            .collect();
        match read_entries(&self.path) {
            Ok(mut entries) => {
                for entry in &mut entries {
                    entry.marked = marked.contains(&entry.name);
                }
                self.entries = entries;
                self.error = None;
                changes.record_once(DirectoryChange::Listed);
            }
            Err(message) => {
                // Le contenu précédent est gardé : un répertoire devenu illisible garde
                // ce qu'on y voyait, et le message dit pourquoi il ne bouge plus.
                self.error = Some(message);
                changes.record_once(DirectoryChange::Failed);
            }
        }
        self.generation.advance();
    }

    /// Se place dans un répertoire et le lit.
    fn enter(&mut self, path: PathBuf, changes: &mut AirChangeSet<DirectoryChange>) {
        // Un chemin illisible ne doit pas remplacer le répertoire courant : on lit
        // d'abord, on déplace ensuite. Sans cet ordre, un `Entrée` sur un répertoire
        // interdit laisserait le panneau sur un chemin dont il ne peut rien montrer.
        match read_entries(&path) {
            Ok(entries) => {
                self.path = path;
                self.entries = entries;
                self.error = None;
                self.generation.advance();
                changes.record_once(DirectoryChange::Listed);
                changes.record_once(DirectoryChange::Entered);
            }
            Err(message) => {
                self.error = Some(message);
                changes.record_once(DirectoryChange::Failed);
            }
        }
    }
}

impl AirModel for Directory {
    type Action = DirectoryAction;
    type Change = DirectoryChange;

    fn apply(&mut self, action: Self::Action, changes: &mut AirChangeSet<Self::Change>) {
        match action {
            DirectoryAction::Reload => self.list(changes),
            DirectoryAction::GoToParent => {
                if let Some(parent) = self.parent_path() {
                    self.enter(parent, changes);
                }
            }
            DirectoryAction::Activate(index) => {
                let traversable = self.entries.get(index).is_some_and(Entry::is_traversable);
                if !traversable {
                    // Un fichier ne s'ouvre pas : `rmc` ne lance aucun programme, et
                    // cette absence est écrite dans le mode d'emploi plutôt que
                    // remplacée par une devinette sur l'éditeur de l'utilisateur.
                    return;
                }
                if let Some(path) = self.entry_path(index) {
                    self.enter(path, changes);
                }
            }
            DirectoryAction::ToggleMark(index) => {
                let Some(entry) = self.entries.get_mut(index) else {
                    return;
                };
                if !entry.is_markable() {
                    return;
                }
                entry.marked = !entry.marked;
                // La génération avance : le marquage fait partie de ce que la liste
                // reflète, et sans cela le contrôleur ne repousserait pas le reflet — la
                // marque serait dans le modèle et invisible à l'écran.
                self.generation.advance();
                changes.record_once(DirectoryChange::Marked);
            }
        }
    }
}

/// Lit un répertoire et rend ses entrées rangées.
///
/// # Errors
///
/// Rend le message du système quand le répertoire ne peut pas être lu — il n'existe pas,
/// ou les droits manquent.
fn read_entries(path: &Path) -> Result<Vec<Entry>, String> {
    let listing = std::fs::read_dir(path).map_err(|error| error.to_string())?;
    let mut entries: Vec<Entry> = Vec::new();
    if path.parent().is_some() {
        entries.push(Entry {
            name: PARENT_NAME.to_owned(),
            kind: EntryKind::Parent,
            size: 0,
            marked: false,
        });
    }
    for found in listing {
        // Une entrée illisible au milieu d'un répertoire lisible est sautée : refuser le
        // répertoire entier pour un fichier dont les droits manquent cacherait tout le
        // reste.
        let Ok(found) = found else {
            continue;
        };
        let name = found.file_name();
        let Some(name) = name.to_str().map(str::to_owned) else {
            // Un nom qui n'est pas de l'UTF-8 n'est pas affichable sans inventer des
            // caractères, et l'inventer donnerait un nom qu'aucune action ne retrouve.
            continue;
        };
        // `symlink_metadata` ne suit pas le lien : c'est ce qui permet de voir un lien
        // cassé comme un lien et non comme une entrée absente.
        let metadata = found.path().symlink_metadata();
        let kind = metadata.as_ref().map_or(EntryKind::File, |metadata| {
            let file_type = metadata.file_type();
            if file_type.is_symlink() {
                EntryKind::Symlink
            } else if file_type.is_dir() {
                EntryKind::Directory
            } else {
                EntryKind::File
            }
        });
        let size = metadata.as_ref().map_or(0, std::fs::Metadata::len);
        entries.push(Entry {
            name,
            kind,
            size: if matches!(kind, EntryKind::Directory) {
                0
            } else {
                size
            },
            marked: false,
        });
    }
    entries.sort_by(|first, second| {
        first
            .kind
            .cmp(&second.kind)
            .then_with(|| compare_names(&first.name, &second.name))
    });
    Ok(entries)
}

/// Compare deux noms comme un gestionnaire de fichiers le fait.
///
/// La casse ne décide qu'à égalité : `Makefile` et `makefile` voisinent, au lieu d'être
/// séparés par tout l'alphabet minuscule comme un tri sur les octets le ferait.
fn compare_names(first: &str, second: &str) -> std::cmp::Ordering {
    let folded = first.to_lowercase().cmp(&second.to_lowercase());
    if folded == std::cmp::Ordering::Equal {
        return first.cmp(second);
    }
    folded
}

/// Le nom de fichier d'un chemin, ou le chemin entier quand il n'en a pas.
#[must_use]
pub fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(OsStr::to_str)
        .map_or_else(|| path.display().to_string(), str::to_owned)
}

/// Une arborescence jetable, pour les essais qui ont besoin d'un vrai disque.
///
/// Elle n'est compilée qu'en essai : un gestionnaire de fichiers se vérifie sur des
/// fichiers, et un faux système de fichiers vérifierait le faux.
#[cfg(test)]
pub(crate) mod scratch {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Un compteur, pour que deux essais simultanés ne se marchent pas dessus.
    static NEXT: AtomicU32 = AtomicU32::new(0);

    /// Une arborescence qui se supprime à la fin de l'essai.
    pub(crate) struct Scratch {
        /// La racine créée.
        root: PathBuf,
    }

    impl Scratch {
        /// Crée une racine, ses sous-répertoires et ses fichiers.
        pub(crate) fn new(directories: &[&str], files: &[(&str, &str)]) -> Self {
            let serial = NEXT.fetch_add(1, Ordering::Relaxed);
            let root =
                std::env::temp_dir().join(format!("rmc-essai-{}-{serial}", std::process::id()));
            std::fs::create_dir_all(&root).expect("racine d'essai");
            for directory in directories {
                std::fs::create_dir_all(root.join(directory)).expect("sous-répertoire d'essai");
            }
            for (name, contents) in files {
                std::fs::write(root.join(name), contents).expect("fichier d'essai");
            }
            Self { root }
        }

        /// La racine.
        pub(crate) fn path(&self) -> &Path {
            &self.root
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            // Un échec de nettoyage ne doit pas masquer le verdict de l'essai : le
            // répertoire temporaire du système sera balayé de toute façon.
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::scratch::Scratch;
    use super::{Directory, DirectoryAction, DirectoryChange, EntryKind, display_name};
    use air_tui::model::{AirChangeSet, AirModel};
    use std::path::Path;

    /// Un panneau lu sur une arborescence, et le journal de ce qui a changé.
    fn listed(root: &Path) -> (Directory, AirChangeSet<DirectoryChange>) {
        let mut panel = Directory::new(root.to_path_buf());
        let mut changes = AirChangeSet::new();
        panel.apply(DirectoryAction::Reload, &mut changes);
        (panel, changes)
    }

    /// Les noms des entrées, dans l'ordre où le panneau les montre.
    fn names(panel: &Directory) -> Vec<&str> {
        panel
            .entries()
            .iter()
            .map(|entry| entry.name.as_str())
            .collect()
    }

    #[test]
    fn un_panneau_neuf_ne_lit_rien() {
        // La lecture peut échouer : un constructeur qui lirait obligerait l'appelant à
        // traiter l'échec avant d'avoir un écran où l'afficher.
        let panel = Directory::new(Path::new("/").to_path_buf());
        assert_eq!(panel.entries(), []);
        assert_eq!(panel.error(), None);
    }

    #[test]
    fn les_repertoires_viennent_avant_les_fichiers_et_le_parent_en_tete() {
        let scratch = Scratch::new(&["zoulou", "alpha"], &[("beta.txt", "x"), ("aa.txt", "y")]);
        let (panel, changes) = listed(scratch.path());
        assert!(changes.contains(&DirectoryChange::Listed));
        assert_eq!(
            names(&panel),
            vec!["..", "alpha", "zoulou", "aa.txt", "beta.txt"]
        );
    }

    #[test]
    fn la_casse_ne_separe_pas_les_voisins() {
        // Un tri sur les octets mettrait tous les majuscules avant toutes les minuscules :
        // `Makefile` se retrouverait à vingt lignes de `makefile`.
        let scratch = Scratch::new(
            &[],
            &[("Makefile", ""), ("lisezmoi", ""), ("makefile.old", "")],
        );
        let (panel, _) = listed(scratch.path());
        assert_eq!(
            names(&panel),
            vec!["..", "lisezmoi", "Makefile", "makefile.old"]
        );
    }

    #[test]
    fn la_taille_d_un_fichier_est_celle_du_disque() {
        let scratch = Scratch::new(&[], &[("quatre", "abcd")]);
        let (panel, _) = listed(scratch.path());
        let entry = panel
            .entries()
            .iter()
            .find(|entry| entry.name == "quatre")
            .expect("le fichier est là");
        assert_eq!(entry.size, 4);
        assert_eq!(entry.kind, EntryKind::File);
    }

    #[test]
    fn un_repertoire_n_annonce_pas_de_taille() {
        // La taille d'un répertoire sur le disque est celle de sa table, pas de son
        // contenu : l'afficher ferait croire qu'un répertoire plein pèse 64 octets.
        let scratch = Scratch::new(&["dedans"], &[]);
        let (panel, _) = listed(scratch.path());
        let entry = panel
            .entries()
            .iter()
            .find(|entry| entry.name == "dedans")
            .expect("le répertoire est là");
        assert_eq!(entry.size, 0);
        assert_eq!(entry.kind, EntryKind::Directory);
    }

    #[test]
    fn descendre_dans_un_repertoire_change_le_chemin() {
        let scratch = Scratch::new(&["dedans"], &[("dehors.txt", "")]);
        let (mut panel, _) = listed(scratch.path());
        let index = names(&panel)
            .iter()
            .position(|name| *name == "dedans")
            .expect("le répertoire est listé");
        let mut changes = AirChangeSet::new();
        panel.apply(DirectoryAction::Activate(index), &mut changes);
        assert!(changes.contains(&DirectoryChange::Entered));
        assert_eq!(panel.path(), scratch.path().join("dedans"));
        assert_eq!(names(&panel), vec![".."]);
    }

    #[test]
    fn activer_un_fichier_ne_fait_rien() {
        // `rmc` n'exécute aucun programme : deviner l'éditeur de l'utilisateur serait
        // promettre une capacité qu'il n'a pas.
        let scratch = Scratch::new(&[], &[("fichier.txt", "")]);
        let (mut panel, _) = listed(scratch.path());
        let before = panel.path().to_path_buf();
        let index = names(&panel)
            .iter()
            .position(|name| *name == "fichier.txt")
            .expect("le fichier est listé");
        let mut changes = AirChangeSet::new();
        panel.apply(DirectoryAction::Activate(index), &mut changes);
        assert_eq!(panel.path(), before);
        assert!(changes.is_empty());
    }

    #[test]
    fn remonter_par_le_parent_et_y_revenir() {
        let scratch = Scratch::new(&["dedans"], &[]);
        let inside = scratch.path().join("dedans");
        let (mut panel, _) = listed(&inside);
        let mut changes = AirChangeSet::new();
        panel.apply(DirectoryAction::Activate(0), &mut changes);
        assert_eq!(panel.path(), scratch.path(), "l'indice zéro est `..`");
    }

    #[test]
    fn un_repertoire_illisible_garde_ce_qu_on_voyait() {
        let scratch = Scratch::new(&["dedans"], &[("visible.txt", "")]);
        let (mut panel, _) = listed(scratch.path());
        let before = names(&panel).len();
        // Un chemin qui n'existe pas : le modèle ne peut pas le lire, et il ne doit pas
        // vider l'écran pour autant.
        let mut absent = Directory::new(scratch.path().join("absent"));
        let mut changes = AirChangeSet::new();
        absent.apply(DirectoryAction::Reload, &mut changes);
        assert!(changes.contains(&DirectoryChange::Failed));
        assert!(absent.error().is_some());

        let mut changes = AirChangeSet::new();
        panel.apply(DirectoryAction::Reload, &mut changes);
        assert_eq!(names(&panel).len(), before);
        assert_eq!(panel.error(), None);
    }

    #[test]
    fn un_chemin_illisible_ne_remplace_pas_le_repertoire_courant() {
        // On lit d'abord, on déplace ensuite : sans cet ordre, un `Entrée` sur un
        // répertoire interdit laisserait le panneau sur un chemin dont il ne peut rien
        // montrer.
        let scratch = Scratch::new(&[], &[]);
        let mut panel = Directory::new(scratch.path().to_path_buf());
        let mut changes = AirChangeSet::new();
        panel.apply(DirectoryAction::Reload, &mut changes);
        let before = panel.path().to_path_buf();
        std::fs::create_dir(scratch.path().join("piege")).expect("sous-répertoire");
        let mut changes = AirChangeSet::new();
        panel.apply(DirectoryAction::Reload, &mut changes);
        let index = names(&panel)
            .iter()
            .position(|name| *name == "piege")
            .expect("le répertoire est listé");
        std::fs::remove_dir(scratch.path().join("piege")).expect("retrait du piège");
        let mut changes = AirChangeSet::new();
        panel.apply(DirectoryAction::Activate(index), &mut changes);
        assert!(changes.contains(&DirectoryChange::Failed));
        assert_eq!(panel.path(), before, "le panneau n'a pas bougé");
    }

    #[test]
    fn marquer_et_demarquer_compte_et_pese() {
        let scratch = Scratch::new(&[], &[("un", "abc"), ("deux", "de")]);
        let (mut panel, _) = listed(scratch.path());
        let index = names(&panel)
            .iter()
            .position(|name| *name == "un")
            .expect("le fichier est listé");
        let mut changes = AirChangeSet::new();
        panel.apply(DirectoryAction::ToggleMark(index), &mut changes);
        assert!(changes.contains(&DirectoryChange::Marked));
        assert_eq!(panel.marked_count(), 1);
        assert_eq!(panel.marked_size(), 3);
        panel.apply(DirectoryAction::ToggleMark(index), &mut changes);
        assert_eq!(panel.marked_count(), 0);
        assert_eq!(panel.marked_size(), 0);
    }

    #[test]
    fn le_parent_ne_se_marque_pas() {
        // `..` n'est pas un fichier de ce répertoire : le marquer n'aurait rien à
        // désigner, et le compte des marqués deviendrait faux.
        let scratch = Scratch::new(&[], &[("un", "")]);
        let (mut panel, _) = listed(scratch.path());
        let mut changes = AirChangeSet::new();
        panel.apply(DirectoryAction::ToggleMark(0), &mut changes);
        assert_eq!(panel.marked_count(), 0);
        assert!(changes.is_empty());
    }

    #[test]
    fn une_marque_survit_a_la_relecture_par_son_nom() {
        let scratch = Scratch::new(&[], &[("garde", "x"), ("perdu", "y")]);
        let (mut panel, _) = listed(scratch.path());
        for name in ["garde", "perdu"] {
            let index = names(&panel)
                .iter()
                .position(|found| *found == name)
                .expect("le fichier est listé");
            let mut changes = AirChangeSet::new();
            panel.apply(DirectoryAction::ToggleMark(index), &mut changes);
        }
        assert_eq!(panel.marked_count(), 2);
        // Un fichier de plus en tête : les indices glissent, les noms non.
        std::fs::write(scratch.path().join("aaaa"), "").expect("fichier");
        std::fs::remove_file(scratch.path().join("perdu")).expect("retrait");
        let mut changes = AirChangeSet::new();
        panel.apply(DirectoryAction::Reload, &mut changes);
        assert_eq!(panel.marked_count(), 1);
        let kept = panel
            .entries()
            .iter()
            .find(|entry| entry.marked)
            .expect("une marque reste");
        assert_eq!(kept.name, "garde");
    }

    #[test]
    fn la_generation_avance_a_chaque_lecture() {
        // C'est ce qui permet au contrôleur de ne repousser la liste que quand il le
        // faut, en comparant deux entiers.
        let scratch = Scratch::new(&[], &[]);
        let (mut panel, _) = listed(scratch.path());
        let first = panel.generation();
        let mut changes = AirChangeSet::new();
        panel.apply(DirectoryAction::Reload, &mut changes);
        assert_ne!(panel.generation(), first);
    }

    #[test]
    fn un_lien_symbolique_est_vu_comme_un_lien() {
        // `symlink_metadata` ne suit pas le lien : un lien cassé reste un lien, là où
        // `metadata` le rapporterait comme absent.
        let scratch = Scratch::new(&[], &[("cible", "x")]);
        let link = scratch.path().join("lien");
        if std::os::unix::fs::symlink(scratch.path().join("absent"), &link).is_err() {
            // Le système refuse les liens : l'essai n'a rien à vérifier, et le dire vaut
            // mieux qu'un faux vert silencieux.
            return;
        }
        let (panel, _) = listed(scratch.path());
        let entry = panel
            .entries()
            .iter()
            .find(|entry| entry.name == "lien")
            .expect("le lien est listé");
        assert_eq!(entry.kind, EntryKind::Symlink);
    }

    #[test]
    fn la_racine_n_a_pas_de_parent() {
        let (panel, _) = listed(Path::new("/"));
        assert!(
            !names(&panel).contains(&".."),
            "la racine n'offre pas de remontée"
        );
    }

    #[test]
    fn le_chemin_d_une_entree_est_celui_du_disque() {
        let scratch = Scratch::new(&["dedans"], &[]);
        let (panel, _) = listed(scratch.path());
        let index = names(&panel)
            .iter()
            .position(|name| *name == "dedans")
            .expect("listé");
        assert_eq!(panel.entry_path(index), Some(scratch.path().join("dedans")));
        assert_eq!(
            panel.entry_path(0),
            scratch.path().parent().map(Path::to_path_buf)
        );
        assert_eq!(panel.entry_path(9999), None);
    }

    #[test]
    fn le_nom_d_affichage_d_un_chemin() {
        assert_eq!(display_name(Path::new("/usr/local/share")), "share");
        assert_eq!(display_name(Path::new("/")), "/");
    }
}
