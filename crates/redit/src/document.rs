// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le document : un tampon de texte, un chemin, et les deux accès au disque.
//!
//! # Pourquoi le disque est ici et pas dans le modèle
//!
//! [`air_tui::model::AirTextBuffer`] ne lit ni n'écrit aucun fichier, et c'est voulu : un
//! modèle qui touche au disque n'est plus testable sans disque. Le chargement se traduit
//! en [`AirTextAction::Load`], l'enregistrement lit
//! [`air_tui::model::AirTextBuffer::to_text`], et c'est cette couche-ci qui fait les deux.

use std::fmt;
use std::path::{Path, PathBuf};

use air_tui::model::{AirChangeSet, AirModel, AirTextAction, AirTextBuffer, AirTextChange};

/// Ce qui peut empêcher un accès au disque.
#[derive(Debug)]
pub enum DocumentError {
    /// Le fichier n'a pas pu être lu.
    Unreadable {
        /// Le chemin en cause.
        path: PathBuf,
        /// La cause système.
        cause: String,
    },
    /// Le fichier n'a pas pu être écrit.
    Unwritable {
        /// Le chemin en cause.
        path: PathBuf,
        /// La cause système.
        cause: String,
    },
    /// Le contenu n'est pas de l'UTF-8.
    ///
    /// `redit` est en UTF-8 par décision de projet, et il le dit plutôt que de remplacer
    /// les octets invalides par un caractère de substitution : enregistrer après une telle
    /// substitution détruirait le fichier d'origine.
    NotUtf8 {
        /// Le chemin en cause.
        path: PathBuf,
    },
    /// Aucun chemin n'est associé au document.
    NoPath,
}

impl fmt::Display for DocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { path, cause } => {
                write!(formatter, "{} illisible : {cause}", path.display())
            }
            Self::Unwritable { path, cause } => {
                write!(formatter, "{} inécrivable : {cause}", path.display())
            }
            Self::NotUtf8 { path } => {
                write!(formatter, "{} n'est pas de l'UTF-8", path.display())
            }
            Self::NoPath => formatter.write_str("aucun nom de fichier"),
        }
    }
}

/// Un document en cours d'édition.
#[derive(Debug)]
pub struct Document {
    /// Le tampon de texte — le modèle.
    buffer: AirTextBuffer,
    /// Le chemin du fichier, s'il en a un.
    path: Option<PathBuf>,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    /// Un document vide, sans nom.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: AirTextBuffer::new(),
            path: None,
        }
    }

    /// Le tampon, en lecture.
    #[must_use]
    pub const fn buffer(&self) -> &AirTextBuffer {
        &self.buffer
    }

    /// Le tampon, en écriture.
    pub const fn buffer_mut(&mut self) -> &mut AirTextBuffer {
        &mut self.buffer
    }

    /// Le chemin du fichier, s'il en a un.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Le nom à afficher : le dernier élément du chemin, ou « sans nom ».
    #[must_use]
    pub fn display_name(&self) -> String {
        match self.path.as_deref().and_then(Path::file_name) {
            Some(name) => name.to_string_lossy().into_owned(),
            None => "sans nom".to_owned(),
        }
    }

    /// Remplace le contenu par celui d'un fichier.
    ///
    /// Un fichier **absent** n'est pas une erreur : c'est la création d'un fichier neuf,
    /// ce que fait tout éditeur appelé sur un nom qui n'existe pas encore. Le chemin est
    /// retenu, le tampon reste vide, et l'enregistrement créera le fichier.
    ///
    /// # Errors
    ///
    /// Si le fichier existe mais ne peut pas être lu, ou s'il n'est pas de l'UTF-8.
    pub fn load(
        &mut self,
        path: &Path,
        changes: &mut AirChangeSet<AirTextChange>,
    ) -> Result<bool, DocumentError> {
        let lines = match std::fs::read(path) {
            Ok(bytes) => {
                let text = String::from_utf8(bytes).map_err(|_| DocumentError::NotUtf8 {
                    path: path.to_path_buf(),
                })?;
                Some(split_lines(&text))
            }
            Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => None,
            Err(cause) => {
                return Err(DocumentError::Unreadable {
                    path: path.to_path_buf(),
                    cause: cause.to_string(),
                });
            }
        };
        let existed = lines.is_some();
        self.buffer
            .apply(AirTextAction::Load(lines.unwrap_or_default()), changes);
        self.path = Some(path.to_path_buf());
        Ok(existed)
    }

    /// Écrit le contenu dans le fichier du document.
    ///
    /// # Errors
    ///
    /// Si le document n'a pas de chemin, ou si l'écriture échoue.
    pub fn save(&mut self, changes: &mut AirChangeSet<AirTextChange>) -> Result<(), DocumentError> {
        let path = self.path.clone().ok_or(DocumentError::NoPath)?;
        self.save_as(&path, changes)
    }

    /// Écrit le contenu dans un fichier, qui devient celui du document.
    ///
    /// Un saut de ligne final est ajouté s'il manque : c'est la convention POSIX, et un
    /// fichier qui n'en a pas gêne tous les outils de la chaîne.
    ///
    /// # Errors
    ///
    /// Si l'écriture échoue.
    pub fn save_as(
        &mut self,
        path: &Path,
        changes: &mut AirChangeSet<AirTextChange>,
    ) -> Result<(), DocumentError> {
        let mut text = self.buffer.to_text();
        if !text.ends_with('\n') {
            text.push('\n');
        }
        std::fs::write(path, text.as_bytes()).map_err(|cause| DocumentError::Unwritable {
            path: path.to_path_buf(),
            cause: cause.to_string(),
        })?;
        self.path = Some(path.to_path_buf());
        self.buffer.apply(AirTextAction::MarkSaved, changes);
        Ok(())
    }
}

/// Coupe un texte en lignes, en retirant le saut final s'il y en a un.
///
/// Sans ce retrait, un fichier qui finit par un saut de ligne — c'est-à-dire presque tous
/// — gagnerait une ligne vide à chaque ouverture, et en perdrait une à chaque
/// enregistrement suivi d'une réouverture.
fn split_lines(text: &str) -> Vec<String> {
    let trimmed = text.strip_suffix('\n').unwrap_or(text);
    trimmed
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use air_tui::model::{AirChangeSet, AirModel, AirTextAction};

    use super::{Document, DocumentError, split_lines};

    /// Un chemin de fichier temporaire, propre à ce processus et à ce nom d'essai.
    fn temporary_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("redit-essai-{}-{name}", std::process::id()));
        path
    }

    #[test]
    fn un_document_neuf_est_vide_et_sans_nom() {
        let document = Document::new();
        assert_eq!(document.buffer().line_count(), 1);
        assert_eq!(document.path(), None);
        assert_eq!(document.display_name(), "sans nom");
        assert!(!document.buffer().is_modified());
    }

    #[test]
    fn le_document_par_defaut_est_le_document_neuf() {
        assert_eq!(Document::default().display_name(), "sans nom");
    }

    #[test]
    fn un_texte_perd_son_saut_de_ligne_final() {
        // Sans cela, un aller-retour disque ajouterait une ligne vide à chaque fois.
        assert_eq!(split_lines("une\ndeux\n"), ["une", "deux"]);
        assert_eq!(split_lines("une\ndeux"), ["une", "deux"]);
        assert_eq!(split_lines(""), [""]);
    }

    #[test]
    fn les_fins_de_ligne_windows_sont_acceptees() {
        assert_eq!(split_lines("une\r\ndeux\r\n"), ["une", "deux"]);
    }

    #[test]
    fn un_aller_retour_sur_le_disque_conserve_le_texte() {
        let path = temporary_path("aller-retour");
        let mut changes = AirChangeSet::new();
        let mut written = Document::new();
        written
            .load(&path, &mut changes)
            .expect("un fichier absent n'est pas une erreur");
        written.buffer_mut().apply(
            AirTextAction::Load(vec!["une".to_owned(), "deux".to_owned()]),
            &mut changes,
        );
        written.save(&mut changes).expect("l'écriture aboutit");

        let mut read = Document::new();
        let existed = read.load(&path, &mut changes).expect("la lecture aboutit");
        assert!(existed);
        assert_eq!(read.buffer().lines(), ["une", "deux"]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn un_enregistrement_ajoute_le_saut_de_ligne_final() {
        let path = temporary_path("saut-final");
        let mut changes = AirChangeSet::new();
        let mut document = Document::new();
        document
            .buffer_mut()
            .apply(AirTextAction::Load(vec!["une".to_owned()]), &mut changes);
        document
            .save_as(&path, &mut changes)
            .expect("l'écriture aboutit");
        let content = std::fs::read_to_string(&path).expect("le fichier est lisible");
        assert_eq!(content, "une\n");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn un_enregistrement_eteint_le_drapeau_de_modification() {
        let path = temporary_path("drapeau");
        let mut changes = AirChangeSet::new();
        let mut document = Document::new();
        document
            .buffer_mut()
            .apply(AirTextAction::InsertCharacter('a'), &mut changes);
        assert!(document.buffer().is_modified());
        document
            .save_as(&path, &mut changes)
            .expect("l'écriture aboutit");
        assert!(!document.buffer().is_modified());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn un_fichier_absent_ouvre_un_document_neuf_portant_son_nom() {
        let path = temporary_path("absent");
        let _ = std::fs::remove_file(&path);
        let mut changes = AirChangeSet::new();
        let mut document = Document::new();
        let existed = document.load(&path, &mut changes).expect("pas une erreur");
        assert!(!existed);
        assert_eq!(document.buffer().lines(), [""]);
        assert_eq!(document.path(), Some(path.as_path()));
    }

    #[test]
    fn un_enregistrement_sans_nom_est_refuse() {
        let mut changes = AirChangeSet::new();
        let mut document = Document::new();
        let error = document
            .save(&mut changes)
            .expect_err("il n'y a pas de nom");
        assert!(matches!(error, DocumentError::NoPath));
        assert_eq!(error.to_string(), "aucun nom de fichier");
    }

    #[test]
    fn un_contenu_non_utf8_est_refuse_et_non_substitue() {
        // Substituer détruirait le fichier d'origine au premier enregistrement.
        let path = temporary_path("non-utf8");
        std::fs::write(&path, [0xFF_u8, 0xFE]).expect("l'écriture aboutit");
        let mut changes = AirChangeSet::new();
        let mut document = Document::new();
        let error = document
            .load(&path, &mut changes)
            .expect_err("ce n'est pas de l'UTF-8");
        assert!(matches!(error, DocumentError::NotUtf8 { .. }));
        assert!(error.to_string().contains("n'est pas de l'UTF-8"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn une_lecture_impossible_est_rapportee_avec_son_chemin() {
        // Un répertoire n'est pas un fichier : la lecture échoue sans être une absence.
        let mut changes = AirChangeSet::new();
        let mut document = Document::new();
        let directory = std::env::temp_dir();
        let error = document
            .load(&directory, &mut changes)
            .expect_err("un répertoire n'est pas lisible comme un fichier");
        assert!(matches!(error, DocumentError::Unreadable { .. }));
        assert!(error.to_string().contains("illisible"));
    }

    #[test]
    fn une_ecriture_impossible_est_rapportee_avec_son_chemin() {
        let mut changes = AirChangeSet::new();
        let mut document = Document::new();
        let mut path = std::env::temp_dir();
        path.push("redit-essai-repertoire-absent");
        path.push("fichier.txt");
        let error = document
            .save_as(&path, &mut changes)
            .expect_err("le répertoire parent n'existe pas");
        assert!(matches!(error, DocumentError::Unwritable { .. }));
        assert!(error.to_string().contains("inécrivable"));
    }

    #[test]
    fn le_nom_affiche_est_le_dernier_element_du_chemin() {
        let path = temporary_path("nom-affiche");
        let mut changes = AirChangeSet::new();
        let mut document = Document::new();
        document.load(&path, &mut changes).expect("pas une erreur");
        assert_eq!(
            document.display_name(),
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        );
    }
}
