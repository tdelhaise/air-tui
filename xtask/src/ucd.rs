// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La lecture des fichiers du référentiel de caractères Unicode.
//!
//! # Pourquoi ce parseur a le droit d'exister
//!
//! Le Principe 11 d'Air borne les parseurs tolérants **sur un chemin d'entrée de
//! décision**. Celui-ci n'en est pas un : il tourne hors de l'exécution du framework, et
//! son produit est un artefact Rust committé que le compilateur vérifie. Son échec n'est
//! qu'un refus d'import — exactement le régime que le principe prévoit pour un texte
//! rédigé par un humain.
//!
//! Il reste strict : une ligne qu'il ne comprend pas est une erreur, jamais une ligne
//! ignorée en silence.

use std::fmt;
use std::path::Path;

/// Une plage de points de code et la valeur de propriété qu'elle porte.
///
/// Les fichiers du référentiel emploient **deux ou trois** champs. Deux pour une propriété
/// à valeur unique — `0300..036F ; Grapheme_Extend` —, trois pour une propriété
/// énumérée — `094D ; InCB ; Linker`. Confondre les deux formes ferait soit refuser à tort
/// les secondes, soit prendre leur nom de propriété pour une valeur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyRange {
    /// Premier point de code de la plage, inclus.
    pub first: u32,
    /// Dernier point de code de la plage, inclus.
    pub last: u32,
    /// Le deuxième champ : la valeur, ou le nom de la propriété énumérée.
    pub value: String,
    /// Le troisième champ, quand la propriété est énumérée.
    pub qualifier: Option<String>,
}

/// Ce qui peut empêcher la lecture d'un fichier du référentiel.
#[derive(Debug)]
pub enum UcdError {
    /// Le fichier n'a pas pu être lu.
    Unreadable {
        /// Le chemin en cause.
        path: String,
        /// La cause système.
        cause: String,
    },
    /// Une ligne ne respecte pas la forme attendue.
    MalformedLine {
        /// Le chemin du fichier.
        path: String,
        /// Le numéro de ligne, compté depuis un.
        line_number: usize,
        /// Le contenu de la ligne.
        content: String,
    },
    /// La version n'a pas pu être lue dans l'en-tête du fichier.
    MissingVersion {
        /// Le chemin du fichier.
        path: String,
    },
}

impl fmt::Display for UcdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { path, cause } => {
                write!(formatter, "{path} illisible : {cause}")
            }
            Self::MalformedLine {
                path,
                line_number,
                content,
            } => write!(
                formatter,
                "{path}:{line_number} : ligne malformée : {content:?}"
            ),
            Self::MissingVersion { path } => {
                write!(formatter, "{path} : version absente de l'en-tête")
            }
        }
    }
}

impl std::error::Error for UcdError {}

/// Lit un fichier de propriétés du référentiel.
///
/// Les lignes vides et les commentaires sont écartés ; tout le reste doit avoir la forme
/// `début[..fin] ; valeur [# commentaire]`.
///
/// # Errors
///
/// Si le fichier est illisible, ou si une ligne ne respecte pas cette forme.
pub fn read_property_file(path: &Path) -> Result<Vec<PropertyRange>, UcdError> {
    let displayed = path.display().to_string();
    let content = std::fs::read_to_string(path).map_err(|cause| UcdError::Unreadable {
        path: displayed.clone(),
        cause: cause.to_string(),
    })?;

    let mut ranges = Vec::new();
    for (index, raw_line) in content.lines().enumerate() {
        let line_number = index.saturating_add(1);
        let without_comment = raw_line.split('#').next().unwrap_or("").trim();
        if without_comment.is_empty() {
            continue;
        }
        let Some(range) = parse_property_line(without_comment) else {
            return Err(UcdError::MalformedLine {
                path: displayed,
                line_number,
                content: raw_line.to_owned(),
            });
        };
        ranges.push(range);
    }
    Ok(ranges)
}

/// Lit une ligne déjà débarrassée de son commentaire.
fn parse_property_line(line: &str) -> Option<PropertyRange> {
    let mut fields = line.split(';');
    let code_points = fields.next()?.trim();
    let value = fields.next()?.trim();
    if value.is_empty() {
        return None;
    }
    let qualifier = match fields.next() {
        Some(third) => {
            let trimmed = third.trim();
            if trimmed.is_empty() {
                return None;
            }
            Some(trimmed.to_owned())
        }
        None => None,
    };
    // Au-delà de trois champs, la forme du fichier a changé : on arrête plutôt que de
    // produire des tables à moitié justes.
    if fields.next().is_some() {
        return None;
    }
    let (first, last) = parse_code_point_range(code_points)?;
    Some(PropertyRange {
        first,
        last,
        value: value.to_owned(),
        qualifier,
    })
}

/// Lit `0041` ou `0041..005A`.
fn parse_code_point_range(field: &str) -> Option<(u32, u32)> {
    let Some((start, end)) = field.split_once("..") else {
        let only = u32::from_str_radix(field.trim(), 16).ok()?;
        return Some((only, only));
    };
    let first = u32::from_str_radix(start.trim(), 16).ok()?;
    let last = u32::from_str_radix(end.trim(), 16).ok()?;
    if last < first {
        return None;
    }
    Some((first, last))
}

/// Lit la version d'Unicode dans l'en-tête d'un fichier du référentiel.
///
/// L'en-tête commence par `# NomDuFichier-18.0.0.txt`. La version est inscrite dans le
/// fichier généré pour qu'on sache toujours de quelle édition les tables proviennent.
///
/// # Errors
///
/// Si le fichier est illisible, ou si aucune version ne figure dans ses premières lignes.
pub fn read_version(path: &Path) -> Result<String, UcdError> {
    let displayed = path.display().to_string();
    let content = std::fs::read_to_string(path).map_err(|cause| UcdError::Unreadable {
        path: displayed.clone(),
        cause: cause.to_string(),
    })?;
    for line in content.lines().take(8) {
        if let Some(version) = extract_version(line) {
            return Ok(version);
        }
    }
    Err(UcdError::MissingVersion { path: displayed })
}

/// Extrait `18.0.0` de `# EastAsianWidth-18.0.0.txt`.
fn extract_version(line: &str) -> Option<String> {
    let trimmed = line.trim_start_matches('#').trim();
    let stem = trimmed.strip_suffix(".txt")?;
    let (_, version) = stem.rsplit_once('-')?;
    let mut parts = version.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;
    let patch = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let all_numeric = [major, minor, patch]
        .iter()
        .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()));
    if all_numeric {
        Some(version.to_owned())
    } else {
        None
    }
}

/// Les plages d'une propriété à valeur unique, fusionnées.
///
/// Les lignes à trois champs sont écartées : ce sont des propriétés énumérées, qui se
/// sélectionnent par [`select_qualified`].
pub fn select(ranges: &[PropertyRange], wanted: &[&str]) -> Vec<(u32, u32)> {
    let mut selected: Vec<(u32, u32)> = ranges
        .iter()
        .filter(|range| range.qualifier.is_none() && wanted.contains(&range.value.as_str()))
        .map(|range| (range.first, range.last))
        .collect();
    selected.sort_unstable();
    merge_adjacent(selected)
}

/// Les plages d'une propriété énumérée qui portent l'une des valeurs demandées.
pub fn select_qualified(
    ranges: &[PropertyRange],
    property: &str,
    wanted: &[&str],
) -> Vec<(u32, u32)> {
    let mut selected: Vec<(u32, u32)> = ranges
        .iter()
        .filter(|range| {
            range.value == property
                && range
                    .qualifier
                    .as_deref()
                    .is_some_and(|value| wanted.contains(&value))
        })
        .map(|range| (range.first, range.last))
        .collect();
    selected.sort_unstable();
    merge_adjacent(selected)
}

/// Fusionne les plages contiguës ou qui se recouvrent.
///
/// Les tables rétrécissent d'un bon tiers, et la recherche binaire a moins d'entrées à
/// traverser.
pub fn merge_adjacent(sorted: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    let mut merged: Vec<(u32, u32)> = Vec::with_capacity(sorted.len());
    for (first, last) in sorted {
        match merged.last_mut() {
            Some(previous) if first <= previous.1.saturating_add(1) => {
                previous.1 = previous.1.max(last);
            }
            _ => merged.push((first, last)),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::{
        PropertyRange, extract_version, merge_adjacent, parse_code_point_range,
        parse_property_line, select, select_qualified,
    };

    fn range(first: u32, last: u32, value: &str) -> PropertyRange {
        PropertyRange {
            first,
            last,
            value: value.to_owned(),
            qualifier: None,
        }
    }

    fn qualified(first: u32, last: u32, value: &str, qualifier: &str) -> PropertyRange {
        PropertyRange {
            first,
            last,
            value: value.to_owned(),
            qualifier: Some(qualifier.to_owned()),
        }
    }

    #[test]
    fn un_point_de_code_seul_est_une_plage_d_un_element() {
        assert_eq!(parse_code_point_range("0041"), Some((0x41, 0x41)));
    }

    #[test]
    fn une_plage_est_lue_dans_les_deux_bornes() {
        assert_eq!(parse_code_point_range("0041..005A"), Some((0x41, 0x5A)));
    }

    #[test]
    fn une_plage_inversee_est_refusee() {
        assert_eq!(parse_code_point_range("005A..0041"), None);
    }

    #[test]
    fn un_point_de_code_non_hexadecimal_est_refuse() {
        assert_eq!(parse_code_point_range("00ZZ"), None);
        assert_eq!(parse_code_point_range(""), None);
        assert_eq!(parse_code_point_range("0041..00ZZ"), None);
    }

    #[test]
    fn une_ligne_de_propriete_est_lue() {
        assert_eq!(
            parse_property_line("0041..005A ; Na"),
            Some(range(0x41, 0x5A, "Na"))
        );
    }

    #[test]
    fn une_ligne_sans_valeur_est_refusee() {
        assert_eq!(parse_property_line("0041"), None);
        assert_eq!(parse_property_line("0041 ;"), None);
    }

    #[test]
    fn une_ligne_a_trois_champs_porte_une_propriete_enumeree() {
        // `094D ; InCB ; Linker` : le deuxième champ nomme la propriété, le troisième
        // porte la valeur. Prendre `InCB` pour une valeur serait une erreur silencieuse.
        assert_eq!(
            parse_property_line("094D ; InCB ; Linker"),
            Some(qualified(0x94D, 0x94D, "InCB", "Linker"))
        );
    }

    #[test]
    fn une_ligne_a_quatre_champs_est_refusee() {
        // La forme du fichier a changé : on arrête plutôt que de deviner.
        assert_eq!(parse_property_line("0041 ; Na ; autre ; encore"), None);
    }

    #[test]
    fn un_troisieme_champ_vide_est_refuse() {
        assert_eq!(parse_property_line("0041 ; InCB ;"), None);
    }

    #[test]
    fn la_version_est_extraite_de_l_en_tete() {
        assert_eq!(
            extract_version("# EastAsianWidth-18.0.0.txt").as_deref(),
            Some("18.0.0")
        );
        assert_eq!(
            extract_version("# GraphemeBreakProperty-17.1.2.txt").as_deref(),
            Some("17.1.2")
        );
    }

    #[test]
    fn une_en_tete_sans_version_ne_donne_rien() {
        // `emoji-data.txt` est dans ce cas : sa version vient d'un autre fichier.
        assert_eq!(extract_version("# emoji-data.txt"), None);
        assert_eq!(extract_version("# Date: 2026-06-29"), None);
        assert_eq!(extract_version("# Fichier-18.0.txt"), None);
        assert_eq!(extract_version("# Fichier-18.0.0.0.txt"), None);
        assert_eq!(extract_version("# Fichier-18.x.0.txt"), None);
    }

    #[test]
    fn la_selection_ne_retient_que_les_valeurs_demandees() {
        let ranges = vec![
            range(0x10, 0x1F, "W"),
            range(0x30, 0x3F, "N"),
            range(0x40, 0x4F, "F"),
        ];
        assert_eq!(
            select(&ranges, &["W", "F"]),
            vec![(0x10, 0x1F), (0x40, 0x4F)]
        );
    }

    #[test]
    fn la_selection_simple_ecarte_les_proprietes_enumerees() {
        // Sans cela, une ligne `… ; InCB ; Linker` serait retenue par une recherche de la
        // valeur « InCB », ce qui n'a pas de sens.
        let ranges = vec![
            range(0x10, 0x1F, "Grapheme_Extend"),
            qualified(0x94D, 0x94D, "Grapheme_Extend", "Linker"),
        ];
        assert_eq!(select(&ranges, &["Grapheme_Extend"]), vec![(0x10, 0x1F)]);
    }

    #[test]
    fn la_selection_qualifiee_ne_retient_que_la_propriete_demandee() {
        let ranges = vec![
            qualified(0x94D, 0x94D, "InCB", "Linker"),
            qualified(0x300, 0x300, "InCB", "Extend"),
            qualified(0x400, 0x400, "Autre", "Linker"),
            range(0x500, 0x500, "InCB"),
        ];
        assert_eq!(
            select_qualified(&ranges, "InCB", &["Linker"]),
            vec![(0x94D, 0x94D)]
        );
        assert_eq!(
            select_qualified(&ranges, "InCB", &["Linker", "Extend"]),
            vec![(0x300, 0x300), (0x94D, 0x94D)]
        );
        assert_eq!(
            select_qualified(&ranges, "InCB", &["Consonant"]),
            Vec::new()
        );
    }

    #[test]
    fn les_plages_contigues_sont_fusionnees() {
        assert_eq!(
            merge_adjacent(vec![(0, 10), (11, 20), (30, 40)]),
            vec![(0, 20), (30, 40)]
        );
    }

    #[test]
    fn les_plages_qui_se_recouvrent_sont_fusionnees() {
        assert_eq!(merge_adjacent(vec![(0, 20), (10, 30)]), vec![(0, 30)]);
        assert_eq!(merge_adjacent(vec![(0, 30), (10, 20)]), vec![(0, 30)]);
    }

    #[test]
    fn une_liste_vide_reste_vide() {
        assert_eq!(merge_adjacent(Vec::new()), Vec::new());
    }
}
