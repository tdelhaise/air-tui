// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La vérification des deux segmentations d'air-tui contre les essais officiels.
//!
//! # Pourquoi un outil, et pas un test
//!
//! Les règles de l'annexe 29 se relisent mal et se vérifient très bien : le Consortium
//! publie `GraphemeBreakTest.txt` et `WordBreakTest.txt`, qui énumèrent des milliers de
//! cas avec la segmentation attendue. Une implémentation écrite à la main sans les passer
//! est une implémentation dont personne ne sait si elle est juste.
//!
//! Ces fichiers ne sont pas committés — ce sont des données du référentiel, pas du code —
//! donc la vérification ne peut pas être un `#[test]` : un test qui se tairait quand le
//! fichier manque donnerait exactement l'assurance fausse que les deux NO GO de ce dépôt
//! interdisent. C'est donc une commande, à lancer avec le référentiel sous la main :
//!
//! ```sh
//! cargo xtask download-ucd --version 18.0.0 --output-directory /tmp/ucd
//! cargo xtask check-segmentation --ucd-directory /tmp/ucd
//! ```

use std::fmt::Write as _;
use std::path::Path;

use crate::ucd::UcdError;

/// Le fichier d'essais de la segmentation en grappes.
const GRAPHEME_BREAK_TEST_FILE: &str = "GraphemeBreakTest.txt";
/// Le fichier d'essais de la segmentation en mots.
const WORD_BREAK_TEST_FILE: &str = "WordBreakTest.txt";

/// Un cas d'essai : le texte d'entrée et la segmentation attendue.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Case {
    /// Le numéro de ligne dans le fichier, compté depuis un.
    line_number: usize,
    /// Le texte reconstitué depuis les points de code de la ligne.
    text: String,
    /// Les segments attendus, dans l'ordre.
    expected: Vec<String>,
}

/// Le compte rendu d'une passe de vérification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Le nom du fichier d'essais.
    pub file: String,
    /// Le nombre de cas examinés.
    pub examined: usize,
    /// Les écarts constatés, au plus les premiers.
    pub failures: Vec<String>,
}

/// Le nombre d'écarts rapportés avant de s'arrêter d'en détailler.
///
/// Au-delà, le compte suffit : une liste de mille écarts ne se lit pas, et le premier
/// suffit presque toujours à trouver la règle fautive.
const REPORTED_FAILURE_LIMIT: usize = 10;

/// Vérifie les deux segmentations contre les fichiers d'essais du référentiel.
///
/// # Errors
///
/// Si l'un des deux fichiers manque, est illisible ou porte une ligne malformée.
pub fn check(ucd_directory: &Path) -> Result<Vec<Report>, UcdError> {
    let grapheme_path = ucd_directory.join(GRAPHEME_BREAK_TEST_FILE);
    let word_path = ucd_directory.join(WORD_BREAK_TEST_FILE);
    let grapheme_cases = read_cases(&grapheme_path)?;
    let word_cases = read_cases(&word_path)?;

    Ok(vec![
        run(GRAPHEME_BREAK_TEST_FILE, &grapheme_cases, |text| {
            air_tui::unicode::graphemes(text)
                .map(str::to_owned)
                .collect()
        }),
        run(WORD_BREAK_TEST_FILE, &word_cases, |text| {
            air_tui::unicode::words(text).map(str::to_owned).collect()
        }),
    ])
}

/// Applique une segmentation à tous les cas et rend le compte rendu.
fn run(file: &str, cases: &[Case], segment: impl Fn(&str) -> Vec<String>) -> Report {
    let mut failures: Vec<String> = Vec::new();
    for case in cases {
        let produced = segment(&case.text);
        if produced != case.expected {
            if failures.len() < REPORTED_FAILURE_LIMIT {
                failures.push(format!(
                    "{file}:{} — attendu {}, produit {}",
                    case.line_number,
                    describe(&case.expected),
                    describe(&produced)
                ));
            } else {
                // Le compte continue de monter ; seuls les détails s'arrêtent.
                failures.push(String::new());
            }
        }
    }
    Report {
        file: file.to_owned(),
        examined: cases.len(),
        failures,
    }
}

/// Décrit une segmentation en points de code, seule forme lisible pour de l'invisible.
fn describe(segments: &[String]) -> String {
    let mut description = String::from("[");
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            description.push_str(" | ");
        }
        for (position, character) in segment.chars().enumerate() {
            if position > 0 {
                description.push(' ');
            }
            let _ = write!(description, "{:04X}", u32::from(character));
        }
    }
    description.push(']');
    description
}

/// Lit un fichier d'essais et rend ses cas.
fn read_cases(path: &Path) -> Result<Vec<Case>, UcdError> {
    let content = std::fs::read_to_string(path).map_err(|cause| UcdError::Unreadable {
        path: path.display().to_string(),
        cause: cause.to_string(),
    })?;
    let mut cases: Vec<Case> = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let line_number = index.saturating_add(1);
        let payload = line.split('#').next().unwrap_or("").trim();
        if payload.is_empty() {
            continue;
        }
        let case = parse_case(payload, line_number).ok_or_else(|| UcdError::MalformedLine {
            path: path.display().to_string(),
            line_number,
            content: payload.to_owned(),
        })?;
        cases.push(case);
    }
    Ok(cases)
}

/// Interprète une ligne d'essai.
///
/// La forme est une alternance de marques et de points de code : `÷` coupe, `×` ne coupe
/// pas. La ligne commence et finit toujours par une coupure — les règles `WB1`, `WB2`,
/// `GB1` et `GB2` —, et c'est ce qui permet de reconstituer les segments sans ambiguïté.
fn parse_case(payload: &str, line_number: usize) -> Option<Case> {
    let mut text = String::new();
    let mut expected: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut started = false;

    for token in payload.split_whitespace() {
        match token {
            "÷" => {
                if started && !current.is_empty() {
                    expected.push(std::mem::take(&mut current));
                }
                started = true;
            }
            "×" => {
                if !started {
                    return None;
                }
            }
            code => {
                if !started {
                    return None;
                }
                let value = u32::from_str_radix(code, 16).ok()?;
                let character = char::from_u32(value)?;
                text.push(character);
                current.push(character);
            }
        }
    }
    if !current.is_empty() {
        // Une ligne qui ne finit pas par une coupure : le fichier n'en produit pas, mais
        // la refuser en silence serait perdre un cas.
        return None;
    }
    Some(Case {
        line_number,
        text,
        expected,
    })
}

#[cfg(test)]
mod tests {
    use super::{Case, describe, parse_case, run};

    #[test]
    fn une_ligne_d_essai_se_reconstitue_en_segments() {
        let case = parse_case("÷ 0041 × 0301 ÷ 0042 ÷", 7).expect("ligne bien formée");
        assert_eq!(case.line_number, 7);
        assert_eq!(case.text, "A\u{301}B");
        assert_eq!(case.expected, vec!["A\u{301}".to_owned(), "B".to_owned()]);
    }

    #[test]
    fn une_ligne_sans_marque_initiale_est_refusee() {
        assert_eq!(parse_case("0041 ÷", 1), None);
    }

    #[test]
    fn une_ligne_qui_ne_finit_pas_par_une_coupure_est_refusee() {
        assert_eq!(parse_case("÷ 0041 × 0301", 1), None);
    }

    #[test]
    fn un_point_de_code_invalide_est_refuse() {
        assert_eq!(parse_case("÷ 110000 ÷", 1), None);
        assert_eq!(parse_case("÷ ZZZZ ÷", 1), None);
    }

    #[test]
    fn une_ligne_vide_de_segments_est_admise() {
        let case = parse_case("÷", 1).expect("ligne bien formée");
        assert_eq!(case.expected, [] as [String; 0]);
        assert_eq!(case.text, "");
    }

    #[test]
    fn une_segmentation_conforme_ne_rapporte_aucun_ecart() {
        let cases = vec![Case {
            line_number: 1,
            text: "ab".to_owned(),
            expected: vec!["a".to_owned(), "b".to_owned()],
        }];
        let report = run("essai", &cases, |text| {
            text.chars()
                .map(|character| character.to_string())
                .collect()
        });
        assert_eq!(report.examined, 1);
        assert_eq!(report.failures, [] as [String; 0]);
    }

    #[test]
    fn une_segmentation_divergente_rapporte_l_ecart_en_points_de_code() {
        let cases = vec![Case {
            line_number: 42,
            text: "ab".to_owned(),
            expected: vec!["ab".to_owned()],
        }];
        let report = run("essai", &cases, |text| {
            text.chars()
                .map(|character| character.to_string())
                .collect()
        });
        assert_eq!(report.failures.len(), 1);
        let first = report.failures.first().expect("un écart");
        assert!(first.contains("essai:42"), "rendu : {first}");
        assert!(first.contains("[0061 0062]"), "rendu : {first}");
        assert!(first.contains("[0061 | 0062]"), "rendu : {first}");
    }

    #[test]
    fn la_description_d_une_segmentation_vide_est_une_paire_de_crochets() {
        assert_eq!(describe(&[]), "[]");
    }
}
