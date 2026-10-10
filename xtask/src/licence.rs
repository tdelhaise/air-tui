// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le contrôle des en-têtes de licence : les trois lignes MPL en tête de chaque `.rs`.
//!
//! # Pourquoi en tête, et pourquoi à l'octet
//!
//! La MPL-2.0 demande que chaque fichier porte l'avis ; un dépôt où quelques fichiers
//! l'ont perdu n'est pas un dépôt à la licence floue, c'est un dépôt dont la licence ne
//! vaut plus pour ces fichiers. La comparaison est donc **exacte** sur les trois
//! premières lignes : un avis reformulé n'est pas l'avis, et le laisser passer demanderait
//! de décider à partir de quand une reformulation reste l'avis.
//!
//! # Ce que ce contrôle trouve aujourd'hui
//!
//! **Rien** — mesuré sur les soixante-dix-sept fichiers du dépôt avant de l'écrire. C'est
//! de la prévention pure, et il faut le dire : il garde une règle tenue à la main depuis
//! le premier commit, il ne répare rien. Son intérêt est qu'un fichier **nouveau** est
//! exactement le moment où l'en-tête s'oublie, et c'est aussi le moment où personne ne
//! relit les trois premières lignes des soixante-seize autres.

use std::path::Path;

/// L'avis MPL-2.0, ligne pour ligne.
///
/// Transcrit depuis les fichiers du dépôt, qui le portent tous à l'identique. C'est la
/// forme que `mozilla.org` publie pour les sources, et elle n'est pas paramétrable : ni
/// nom d'auteur, ni année, donc rien à faire varier d'un fichier à l'autre.
const HEADER: &[&str] = &[
    "// This Source Code Form is subject to the terms of the Mozilla Public",
    "// License, v. 2.0. If a copy of the MPL was not distributed with this",
    "// file, You can obtain one at https://mozilla.org/MPL/2.0/.",
];

/// Un manquement relevé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Le fichier en cause, relatif à la racine du dépôt.
    pub subject: String,
    /// Ce qui manque, ou ce qui est écrit à la place.
    pub detail: String,
}

/// Le compte rendu d'un contrôle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Le nombre de fichiers examinés.
    pub examined: usize,
    /// Les manquements.
    pub violations: Vec<Violation>,
}

/// Contrôle les en-têtes de tous les `.rs` du dépôt.
///
/// # Errors
///
/// Si un répertoire ou un fichier est illisible — un fichier qu'on ne peut pas lire ne
/// doit pas compter comme un fichier conforme.
pub fn check(repository: &Path) -> Result<Report, String> {
    let mut files = Vec::new();
    for root in crate::naming::ROOTS {
        let directory = repository.join(root);
        if !directory.is_dir() {
            continue;
        }
        crate::naming::collect_rust_files(&directory, &mut files)?;
    }
    files.sort();

    let mut violations = Vec::new();
    for file in &files {
        let source = std::fs::read_to_string(file)
            .map_err(|cause| format!("{} illisible : {cause}", file.display()))?;
        let relative = file
            .strip_prefix(repository)
            .unwrap_or(file)
            .display()
            .to_string();
        if let Some(detail) = missing_header(&source) {
            violations.push(Violation {
                subject: relative,
                detail,
            });
        }
    }
    Ok(Report {
        examined: files.len(),
        violations,
    })
}

/// Ce qui manque à l'en-tête d'un source, s'il manque quelque chose.
///
/// La fin de ligne est rognée avant la comparaison : un dépôt cloné sous Windows porterait
/// des `\r`, et refuser un avis juste pour une fin de ligne serait refuser pour la mauvaise
/// raison.
fn missing_header(source: &str) -> Option<String> {
    let mut lines = source.lines();
    for (index, expected) in HEADER.iter().enumerate() {
        let number = index.saturating_add(1);
        let found = lines.next().map(str::trim_end);
        match found {
            None => {
                return Some(format!(
                    "le fichier n'a que {index} ligne(s) : l'avis MPL en demande trois"
                ));
            }
            Some(line) if line == *expected => {}
            Some(line) => {
                return Some(format!(
                    "ligne {number} : « {line} »\n    attendu : « {expected} »"
                ));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{HEADER, missing_header};

    /// L'avis complet, suivi d'un corps quelconque.
    fn with_header(body: &str) -> String {
        format!("{}\n{body}", HEADER.join("\n"))
    }

    #[test]
    fn un_fichier_avec_l_avis_ne_manque_de_rien() {
        assert_eq!(missing_header(&with_header("\n//! Un module.\n")), None);
    }

    #[test]
    fn l_avis_seul_suffit() {
        // Un fichier qui ne porte que l'avis est conforme : c'est le corps qui est
        // facultatif, pas l'avis.
        assert_eq!(missing_header(&HEADER.join("\n")), None);
    }

    #[test]
    fn un_fichier_sans_avis_est_refuse() {
        let refusal = missing_header("//! Un module sans avis.\n").expect("refus attendu");
        assert!(refusal.contains("ligne 1"), "{refusal}");
        assert!(refusal.contains("attendu"), "{refusal}");
    }

    #[test]
    fn un_avis_tronque_est_refuse_et_la_ligne_est_nommee() {
        let source = format!("{}\n", HEADER.get(..2).unwrap_or_default().join("\n"));
        let refusal = missing_header(&source).expect("refus attendu");
        assert!(refusal.contains("ligne(s)"), "{refusal}");
    }

    #[test]
    fn un_avis_reformule_est_refuse() {
        // « Un avis reformulé n'est pas l'avis » : sans cette exigence, il faudrait
        // décider à partir de quand une reformulation reste l'avis.
        let source = format!(
            "// This Source Code Form is subject to the Mozilla Public License\n{}\n",
            HEADER.get(1..).unwrap_or_default().join("\n")
        );
        let refusal = missing_header(&source).expect("refus attendu");
        assert!(refusal.contains("ligne 1"), "{refusal}");
    }

    #[test]
    fn une_fin_de_ligne_windows_ne_fait_pas_echouer() {
        let mut source = String::new();
        for line in HEADER {
            source.push_str(line);
            source.push_str("\r\n");
        }
        assert_eq!(missing_header(&source), None);
    }

    #[test]
    fn un_fichier_vide_est_refuse() {
        let refusal = missing_header("").expect("refus attendu");
        assert!(refusal.contains("0 ligne"), "{refusal}");
    }

    #[test]
    fn l_avis_est_bien_celui_de_ce_fichier() {
        // Le contrôle doit se tenir lui-même : ce fichier porte l'avis qu'il exige.
        let own = include_str!("licence.rs");
        assert_eq!(missing_header(own), None);
    }
}
