// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le contrôle de couverture de l'ADR-012 : un codec est à 100 %, ou la PR est refusée.
//!
//! # Les trois mesures exigées
//!
//! **Régions, fonctions et lignes** — celles du tableau d'état de l'ADR-012. Les branches
//! et le MC/DC que `llvm-cov` rend par ailleurs ne sont pas exigés : l'ADR ne les nomme
//! pas, et un contrôle ne doit pas exiger plus que la décision qu'il applique.
//!
//! # Quels fichiers, et pourquoi ceux-là
//!
//! [`CODEC_FILES`] est la liste close des fichiers soumis à la règle, chacun avec son
//! rôle. Elle est explicite plutôt que déduite d'un motif de chemin : `input/` contient le
//! décodeur **et** le modèle d'événements qu'il produit, et un motif les confondrait avec
//! le reste du cœur.
//!
//! Un fichier **absent** du relevé est un manquement, pas un succès. C'est le cas qui
//! compte : un fichier renommé sans que cette liste suive sortirait silencieusement de la
//! règle, et le contrôle dirait que tout va bien.
//!
//! # Un décodeur qui n'est pas dans le cœur
//!
//! `xtask/src/mc_skin.rs` y figure, et c'est une décision à motiver. L'ADR-008 place
//! l'importeur de skins **hors** du chemin d'exécution : un skin est compilé en artefact,
//! jamais analysé par une application. On pourrait donc soutenir que l'ADR-012 ne le
//! vise pas, puisque son argument porte sur « des octets venus de l'extérieur » à
//! l'exécution.
//!
//! La règle est pourtant écrite large — « toute conversion d'un flux de données » —, et la
//! tenir a coûté exactement ce que l'ADR-012 annonce : les branches défensives
//! inatteignables ont été **supprimées** par le typage plutôt que couvertes. Les trois
//! chiffres d'une couleur du cube et les six d'une vraie couleur se reconnaissent par
//! motif de tranche, et la valeur d'un chiffre hexadécimal vient d'une table plutôt que
//! d'un `to_digit` suivi d'une conversion qui ne peut pas échouer. Le fichier est plus
//! simple qu'avant la règle, pas plus long.
//!
//! `xtask/src/mc_theme.rs`, qui **produit** du Rust à partir d'un skin déjà lu, n'y figure
//! pas : ce n'est pas une conversion de flux, c'est de la génération.
//!
//! # Les deux modules que cette liste ne contient pas
//!
//! `crates/air-tui/src/codec/mod.rs` est une façade : deux `pub mod` et un `pub use`,
//! aucune ligne exécutable. `llvm-cov` ne le relève donc pas du tout, et l'exiger
//! reviendrait à exiger une mesure qui ne peut pas exister.
//!
//! `crates/air-tui-platform/src/codec.rs` porte le **contrat** — `Decoder` et `Encoder`,
//! deux traits sans méthode par défaut, donc sans une seule ligne exécutable hors
//! `#[cfg(test)]`. Mesuré le 2026-10-10, il est à 84/90 régions, et les six régions
//! manquantes sont les fermetures `|item| collected.push(item)` des lignes 125 et 144 :
//! deux essais où le décodeur ne doit **rien** produire, donc où la fermeture ne peut pas
//! être appelée. Exiger 100 % là mesurerait le double d'essai et non un codec. L'ADR-012
//! écrit « un module de `codec/` » ; ce fichier n'est pas sous un répertoire `codec/`, et
//! l'exclure est conforme à sa lettre comme à son intention.

use std::path::Path;

/// Un fichier soumis à la règle, et ce qui l'y soumet.
struct Codec {
    /// Le chemin, relatif à la racine du dépôt.
    path: &'static str,
    /// Le rôle qui en fait un codec.
    role: &'static str,
}

/// Les fichiers soumis à la règle des 100 %.
const CODEC_FILES: &[Codec] = &[
    Codec {
        path: "crates/air-tui/src/codec/decimal.rs",
        role: "encodeur décimal — la conversion d'un entier en chiffres ASCII",
    },
    Codec {
        path: "crates/air-tui/src/codec/sgr.rs",
        role: "encodeur des séquences SGR — impl Encoder",
    },
    Codec {
        path: "crates/air-tui/src/input/ansi.rs",
        role: "décodeur des séquences ANSI d'entrée — impl Decoder",
    },
    Codec {
        path: "crates/air-tui/src/input/scanning.rs",
        role: "le balayage des paramètres, étape du décodage",
    },
    Codec {
        path: "crates/air-tui/src/input/kitty.rs",
        role: "le décodage du protocole clavier Kitty",
    },
    Codec {
        path: "crates/air-tui/src/input/event.rs",
        role: "le modèle d'événements que le décodeur produit",
    },
    Codec {
        path: "xtask/src/mc_skin.rs",
        role: "décodeur des skins de Midnight Commander — hors exécution, mais c'est bien \
               une conversion de flux",
    },
];

/// Ce que le relevé dit d'un fichier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measurement {
    /// Régions couvertes sur régions totales.
    pub regions: (u64, u64),
    /// Fonctions couvertes sur fonctions totales.
    pub functions: (u64, u64),
    /// Lignes couvertes sur lignes totales.
    pub lines: (u64, u64),
}

impl Measurement {
    /// Vrai si les trois mesures sont intégrales.
    const fn is_complete(&self) -> bool {
        self.regions.0 == self.regions.1
            && self.functions.0 == self.functions.1
            && self.lines.0 == self.lines.1
    }
}

/// Un manquement relevé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Le fichier en cause.
    pub file: String,
    /// Ce qui manque.
    pub detail: String,
}

/// Le compte rendu d'un contrôle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Le nombre de fichiers exigés par la liste.
    pub required: usize,
    /// Les manquements.
    pub violations: Vec<Violation>,
}

/// Contrôle la couverture des codecs.
///
/// Sans `prepared`, la mesure est prise sur place par `cargo llvm-cov`. Avec, elle est
/// lue dans un relevé déjà produit — ce que fait l'intégration continue, qui mesure une
/// fois et contrôle ensuite.
///
/// # Errors
///
/// Rend une erreur si `cargo llvm-cov` est absent ou échoue, et si le relevé n'est pas
/// lisible. L'outil absent n'est **jamais** un succès : c'est précisément le cas où une
/// règle se croirait outillée sans l'être.
pub fn check(repository: &Path, prepared: Option<&Path>) -> Result<Report, String> {
    let json = match prepared {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|cause| format!("{} illisible : {cause}", path.display()))?,
        None => measure(repository)?,
    };
    let measurements = parse(&json)?;
    let mut report = Report {
        required: CODEC_FILES.len(),
        violations: Vec::new(),
    };
    for codec in CODEC_FILES {
        let found = measurements
            .iter()
            .find(|(filename, _)| filename.ends_with(codec.path));
        let Some((_, measurement)) = found else {
            report.violations.push(Violation {
                file: codec.path.to_owned(),
                detail: format!(
                    "absent du relevé de couverture ({}) — fichier renommé, ou essais non \
                     exécutés",
                    codec.role
                ),
            });
            continue;
        };
        if !measurement.is_complete() {
            report.violations.push(Violation {
                file: codec.path.to_owned(),
                detail: format!(
                    "régions {}/{}, fonctions {}/{}, lignes {}/{} — l'ADR-012 exige 100 % \
                     des trois",
                    measurement.regions.0,
                    measurement.regions.1,
                    measurement.functions.0,
                    measurement.functions.1,
                    measurement.lines.0,
                    measurement.lines.1
                ),
            });
        }
    }
    Ok(report)
}

/// Prend la mesure par `cargo llvm-cov`.
fn measure(repository: &Path) -> Result<String, String> {
    let output = std::process::Command::new("cargo")
        .current_dir(repository)
        .args(["llvm-cov", "--workspace", "--summary-only", "--json"])
        .output()
        .map_err(|cause| format!("cargo n'a pas pu être lancé : {cause}"))?;
    if !output.status.success() {
        let complaint = String::from_utf8_lossy(&output.stderr);
        if complaint.contains("no such command") || complaint.contains("llvm-cov") {
            return Err(format!(
                "cargo llvm-cov est indisponible :\n{}\n\
                 L'installer : cargo install cargo-llvm-cov --locked",
                complaint.trim()
            ));
        }
        return Err(format!("cargo llvm-cov a échoué :\n{}", complaint.trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// La clé qui ouvre la description d'un fichier dans le relevé.
const FILENAME_KEY: &str = "\"filename\":\"";

/// Lit un relevé `llvm-cov` et rend la mesure de chaque fichier.
///
/// Le relevé est produit par un outil dont la forme est documentée et stable ; la lecture
/// se limite donc aux trois résumés nécessaires, sans analyseur JSON général — ADR-005,
/// aucune dépendance tierce.
///
/// # Errors
///
/// Rend une erreur si le relevé ne porte aucun fichier : un relevé vide n'est pas un
/// relevé propre.
fn parse(json: &str) -> Result<Vec<(String, Measurement)>, String> {
    let mut found = Vec::new();
    let mut remainder = json;
    while let Some(position) = remainder.find(FILENAME_KEY) {
        let after = remainder
            .get(position.saturating_add(FILENAME_KEY.len())..)
            .unwrap_or("");
        let Some((filename, rest)) = after.split_once('"') else {
            break;
        };
        let extent = rest
            .find(FILENAME_KEY)
            .and_then(|next| rest.get(..next))
            .unwrap_or(rest);
        if let (Some(regions), Some(functions), Some(lines)) = (
            metric(extent, "regions"),
            metric(extent, "functions"),
            metric(extent, "lines"),
        ) {
            found.push((
                filename.to_owned(),
                Measurement {
                    regions,
                    functions,
                    lines,
                },
            ));
        }
        remainder = rest;
    }
    if found.is_empty() {
        return Err("le relevé de couverture ne décrit aucun fichier".to_owned());
    }
    Ok(found)
}

/// Les valeurs `covered` et `count` d'un résumé nommé.
fn metric(section: &str, name: &str) -> Option<(u64, u64)> {
    let key = format!("\"{name}\":{{");
    let position = section.find(&key)?;
    let after = section.get(position.saturating_add(key.len())..)?;
    let end = after.find('}')?;
    let body = after.get(..end)?;
    Some((number(body, "covered")?, number(body, "count")?))
}

/// Un entier sous une clé.
fn number(body: &str, key: &str) -> Option<u64> {
    let key = format!("\"{key}\":");
    let position = body.find(&key)?;
    let after = body.get(position.saturating_add(key.len())..)?;
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{CODEC_FILES, Measurement, check, metric, number, parse};
    use std::path::{Path, PathBuf};

    /// Un relevé de la forme que produit `cargo llvm-cov --summary-only --json`.
    fn coverage_report(regions_covered: u64, regions_count: u64) -> String {
        format!(
            r#"{{"data":[{{"files":[
            {{"filename":"/home/essai/crates/air-tui/src/codec/decimal.rs","summary":{{
              "branches":{{"count":0,"covered":0,"notcovered":0,"percent":0}},
              "functions":{{"count":15,"covered":15,"percent":100}},
              "lines":{{"count":76,"covered":76,"percent":100}},
              "regions":{{"count":{regions_count},"covered":{regions_covered},"notcovered":0,"percent":99}}}}}},
            {{"filename":"/home/essai/crates/air-tui/src/codec/sgr.rs","summary":{{
              "functions":{{"count":17,"covered":17,"percent":100}},
              "lines":{{"count":202,"covered":202,"percent":100}},
              "regions":{{"count":249,"covered":249,"notcovered":0,"percent":100}}}}}}],
            "totals":{{"functions":{{"count":999,"covered":1,"percent":1}},
              "lines":{{"count":999,"covered":1,"percent":1}},
              "regions":{{"count":999,"covered":1,"percent":1}}}}}}],
            "type":"llvm.coverage.json.export","version":"2.0.1"}}"#
        )
    }

    #[test]
    fn un_entier_se_lit_sous_sa_cle() {
        assert_eq!(number(r#""count":90,"covered":84"#, "count"), Some(90));
        assert_eq!(number(r#""count":90,"covered":84"#, "covered"), Some(84));
    }

    #[test]
    fn notcovered_ne_repond_pas_pour_covered() {
        // Le piège de la lecture par sous-chaîne : `notcovered` contient `covered`.
        let body = r#""count":90,"notcovered":6,"covered":84"#;
        assert_eq!(number(body, "covered"), Some(84));
    }

    #[test]
    fn un_resume_nomme_rend_ses_deux_valeurs() {
        let section = r#""regions":{"count":90,"covered":84,"percent":93.3}"#;
        assert_eq!(metric(section, "regions"), Some((84, 90)));
        assert_eq!(metric(section, "lines"), None);
    }

    #[test]
    fn le_releve_se_lit_fichier_par_fichier() {
        let measurements = parse(&coverage_report(122, 122)).expect("relevé lisible");
        assert_eq!(measurements.len(), 2);
        let (filename, measurement) = measurements.first().expect("un premier fichier");
        assert!(filename.ends_with("codec/decimal.rs"), "{filename}");
        assert_eq!(measurement.regions, (122, 122));
        assert_eq!(measurement.functions, (15, 15));
        assert_eq!(measurement.lines, (76, 76));
    }

    #[test]
    fn les_totaux_ne_sont_pas_pris_pour_un_fichier() {
        // `totals` porte les mêmes clés que le résumé d'un fichier, sans `filename`.
        let measurements = parse(&coverage_report(122, 122)).expect("relevé lisible");
        assert!(
            measurements
                .iter()
                .all(|(_, measure)| measure.lines.1 != 999),
            "les totaux ont été lus comme un fichier"
        );
    }

    #[test]
    fn un_releve_vide_est_une_erreur() {
        assert!(parse(r#"{"data":[{"files":[]}]}"#).is_err());
    }

    #[test]
    fn une_mesure_integrale_est_complete() {
        let measure = Measurement {
            regions: (10, 10),
            functions: (2, 2),
            lines: (8, 8),
        };
        assert!(measure.is_complete());
    }

    #[test]
    fn une_region_manquante_suffit_a_rendre_la_mesure_incomplete() {
        let measure = Measurement {
            regions: (9, 10),
            functions: (2, 2),
            lines: (8, 8),
        };
        assert!(!measure.is_complete());
    }

    /// Écrit un relevé dans un fichier jetable.
    fn prepared_report(name: &str, content: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("air-tui-couverture-{name}.json"));
        std::fs::write(&path, content).expect("écriture du relevé d'essai");
        path
    }

    #[test]
    fn un_codec_sous_cent_pour_cent_est_refuse() {
        let path = prepared_report("incomplet", &coverage_report(121, 122));
        let report = check(Path::new("."), Some(&path)).expect("contrôle");
        let first = report
            .violations
            .iter()
            .find(|violation| violation.file.ends_with("decimal.rs"))
            .expect("le manquement attendu");
        assert!(first.detail.contains("121/122"), "{}", first.detail);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn un_codec_absent_du_releve_est_un_manquement() {
        // Le cas qui compte : un fichier renommé sortirait sinon de la règle en silence.
        let path = prepared_report("absent", &coverage_report(122, 122));
        let report = check(Path::new("."), Some(&path)).expect("contrôle");
        assert_eq!(report.required, CODEC_FILES.len());
        assert!(
            report
                .violations
                .iter()
                .any(|violation| violation.detail.contains("absent du relevé")),
            "un fichier manquant doit être relevé"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn chaque_codec_de_la_liste_porte_son_role() {
        for codec in CODEC_FILES {
            assert!(
                Path::new(codec.path)
                    .extension()
                    .is_some_and(|extension| extension == "rs"),
                "{}",
                codec.path
            );
            assert!(!codec.role.is_empty(), "{} sans rôle", codec.path);
        }
    }
}
