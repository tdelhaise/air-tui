// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! L'outillage du dépôt air-tui.
//!
//! Du Rust compilé par la même toolchain pinnée que le framework, et non des scripts :
//! l'outillage se relit et se teste comme le reste.
//!
//! ```sh
//! cargo xtask download-ucd --version 18.0.0 --output-directory /tmp/ucd
//! cargo xtask generate-unicode-tables --ucd-directory /tmp/ucd
//! cargo xtask generate-unicode-tables --ucd-directory /tmp/ucd --check
//! cargo xtask check-segmentation --ucd-directory /tmp/ucd
//! ```

// `expect` et `unwrap` sont interdits en production (lints de l'espace de travail) et
// légitimes dans un test, où une précondition non tenue doit faire échouer le test
// bruyamment. Même choix que dans `air-tui`.
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

mod segmentation_conformance;
mod ucd;
mod unicode_tables;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Le chemin du fichier de tables, relatif à la racine du dépôt.
const TABLES_PATH: &str = "crates/air-tui/src/unicode/tables.rs";

/// Les fichiers du référentiel à télécharger, avec leur chemin dans l'arborescence amont.
const UCD_FILES: &[&str] = &[
    "EastAsianWidth.txt",
    "DerivedCoreProperties.txt",
    "auxiliary/GraphemeBreakProperty.txt",
    "auxiliary/WordBreakProperty.txt",
    "auxiliary/GraphemeBreakTest.txt",
    "auxiliary/WordBreakTest.txt",
    "emoji/emoji-data.txt",
];

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let command = arguments.first().map(String::as_str);
    let outcome = match command {
        Some("download-ucd") => download_ucd(&arguments),
        Some("generate-unicode-tables") => generate_unicode_tables(&arguments),
        Some("check-segmentation") => check_segmentation(&arguments),
        Some(other) => Err(format!("commande inconnue : {other}\n\n{}", usage())),
        None => Err(usage()),
    };
    match outcome {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

/// Le mode d'emploi.
fn usage() -> String {
    "usage :\n  \
     cargo xtask download-ucd --version <version> --output-directory <répertoire>\n  \
     cargo xtask generate-unicode-tables --ucd-directory <répertoire> [--check]\n  \
     cargo xtask check-segmentation --ucd-directory <répertoire>"
        .to_owned()
}

/// La valeur d'une option nommée.
fn option_value(arguments: &[String], name: &str) -> Option<String> {
    let position = arguments.iter().position(|argument| argument == name)?;
    arguments.get(position.saturating_add(1)).cloned()
}

/// Vrai si l'option est présente.
fn has_flag(arguments: &[String], name: &str) -> bool {
    arguments.iter().any(|argument| argument == name)
}

/// Télécharge les fichiers du référentiel pour une édition donnée.
///
/// L'URL porte la **version**, jamais `latest` : une génération doit pouvoir être rejouée
/// à l'identique des années plus tard (ADR-025).
fn download_ucd(arguments: &[String]) -> Result<String, String> {
    let version = option_value(arguments, "--version")
        .ok_or_else(|| format!("--version est obligatoire\n\n{}", usage()))?;
    let directory = option_value(arguments, "--output-directory")
        .ok_or_else(|| format!("--output-directory est obligatoire\n\n{}", usage()))?;
    let directory = PathBuf::from(directory);
    std::fs::create_dir_all(&directory)
        .map_err(|cause| format!("création de {} impossible : {cause}", directory.display()))?;

    for relative in UCD_FILES {
        let name = Path::new(relative)
            .file_name()
            .ok_or_else(|| format!("chemin amont sans nom de fichier : {relative}"))?;
        let destination = directory.join(name);
        let url = format!("https://www.unicode.org/Public/{version}/ucd/{relative}");
        // `curl` comme processus plutôt qu'une pile TLS en dépendance (ADR-005).
        let status = std::process::Command::new("curl")
            .arg("--silent")
            .arg("--show-error")
            .arg("--fail")
            .arg("--output")
            .arg(&destination)
            .arg(&url)
            .status()
            .map_err(|cause| format!("curl n'a pas pu être lancé : {cause}"))?;
        if !status.success() {
            return Err(format!("téléchargement de {url} en échec"));
        }
    }
    Ok(format!(
        "{} fichiers du référentiel Unicode {version} écrits dans {}",
        UCD_FILES.len(),
        directory.display()
    ))
}

/// Produit le fichier de tables, ou vérifie que celui qui est committé correspond.
///
/// # Pourquoi le formateur a le dernier mot
///
/// Le fichier généré est soumis aux mêmes contrôles que le code écrit à la main, et
/// `cargo fmt` le reformate — il découpe les tuples de la table de coupure, qui dépassent
/// son seuil de largeur d'élément. Deviner ces seuils dans le générateur serait les
/// réinventer et les voir dériver à la prochaine version de `rustfmt`.
///
/// La génération passe donc par `rustfmt`, et la vérification compare au **produit
/// formaté**. Sans cela, `--check` échouerait systématiquement juste après un
/// `cargo fmt` — ce qui s'est produit le 2026-10-10.
fn generate_unicode_tables(arguments: &[String]) -> Result<String, String> {
    let directory = option_value(arguments, "--ucd-directory")
        .ok_or_else(|| format!("--ucd-directory est obligatoire\n\n{}", usage()))?;
    let generated = unicode_tables::generate(Path::new(&directory))
        .map_err(|cause| format!("génération impossible : {cause}"))?;
    let destination = repository_root()?.join(TABLES_PATH);

    if has_flag(arguments, "--check") {
        let committed = std::fs::read_to_string(&destination)
            .map_err(|cause| format!("{} illisible : {cause}", destination.display()))?;
        let reference = format_through_rustfmt(&generated, &destination)?;
        if committed == reference {
            return Ok(format!("{TABLES_PATH} est à jour"));
        }
        return Err(format!(
            "{TABLES_PATH} ne correspond plus au référentiel.\n\
             Relancer : cargo xtask generate-unicode-tables --ucd-directory {directory}"
        ));
    }

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|cause| format!("création de {} impossible : {cause}", parent.display()))?;
    }
    std::fs::write(&destination, &generated)
        .map_err(|cause| format!("écriture de {} impossible : {cause}", destination.display()))?;
    run_rustfmt(&destination)?;
    let written = std::fs::metadata(&destination)
        .map_err(|cause| format!("{} illisible : {cause}", destination.display()))?;
    Ok(format!("{TABLES_PATH} écrit ({} octets)", written.len()))
}

/// Confronte les deux segmentations de l'annexe 29 aux essais officiels.
///
/// Les fichiers d'essais ne sont pas committés : ce sont des données du référentiel. La
/// vérification est donc une commande et non un test, pour qu'elle ne puisse pas passer en
/// silence faute de données.
fn check_segmentation(arguments: &[String]) -> Result<String, String> {
    let directory = option_value(arguments, "--ucd-directory")
        .ok_or_else(|| format!("--ucd-directory est obligatoire\n\n{}", usage()))?;
    let reports = segmentation_conformance::check(Path::new(&directory))
        .map_err(|cause| format!("vérification impossible : {cause}"))?;

    let mut summary = String::new();
    let mut failed = false;
    for report in &reports {
        let detailed: Vec<&String> = report
            .failures
            .iter()
            .filter(|failure| !failure.is_empty())
            .collect();
        if report.failures.is_empty() {
            let _ = writeln!(
                summary,
                "{} : {} cas, aucun écart",
                report.file, report.examined
            );
            continue;
        }
        failed = true;
        let _ = writeln!(
            summary,
            "{} : {} cas, {} écart(s)",
            report.file,
            report.examined,
            report.failures.len()
        );
        for failure in detailed {
            let _ = writeln!(summary, "  {failure}");
        }
    }
    if failed {
        return Err(summary);
    }
    Ok(summary.trim_end().to_owned())
}

/// Passe un fichier au formateur de la toolchain pinnée.
fn run_rustfmt(path: &Path) -> Result<(), String> {
    let status = std::process::Command::new("rustfmt")
        .arg("--edition")
        .arg("2024")
        .arg(path)
        .status()
        .map_err(|cause| format!("rustfmt n'a pas pu être lancé : {cause}"))?;
    if status.success() {
        return Ok(());
    }
    Err(format!("rustfmt a échoué sur {}", path.display()))
}

/// Le texte tel que le formateur l'écrirait, obtenu par un fichier temporaire.
///
/// Le temporaire est posé à côté de la destination : il hérite ainsi de la même
/// configuration de formatage, et il est retiré même quand le formatage échoue.
fn format_through_rustfmt(source: &str, destination: &Path) -> Result<String, String> {
    let temporary = destination.with_extension("rs.verification");
    std::fs::write(&temporary, source)
        .map_err(|cause| format!("{} inécrivable : {cause}", temporary.display()))?;
    let formatted = run_rustfmt(&temporary).and_then(|()| {
        std::fs::read_to_string(&temporary)
            .map_err(|cause| format!("{} illisible : {cause}", temporary.display()))
    });
    let _ = std::fs::remove_file(&temporary);
    formatted
}

/// La racine du dépôt, déduite de l'emplacement de cette crate.
fn repository_root() -> Result<PathBuf, String> {
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|cause| format!("CARGO_MANIFEST_DIR absent : {cause}"))?;
    Path::new(&manifest)
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("{manifest} n'a pas de répertoire parent"))
}

#[cfg(test)]
mod tests {
    use super::{has_flag, option_value, usage};

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn une_option_nommee_rend_sa_valeur() {
        let given = arguments(&["generate-unicode-tables", "--ucd-directory", "/tmp/ucd"]);
        assert_eq!(
            option_value(&given, "--ucd-directory").as_deref(),
            Some("/tmp/ucd")
        );
    }

    #[test]
    fn une_option_absente_ne_rend_rien() {
        let given = arguments(&["generate-unicode-tables"]);
        assert_eq!(option_value(&given, "--ucd-directory"), None);
    }

    #[test]
    fn une_option_sans_valeur_ne_rend_rien() {
        let given = arguments(&["generate-unicode-tables", "--ucd-directory"]);
        assert_eq!(option_value(&given, "--ucd-directory"), None);
    }

    #[test]
    fn un_drapeau_est_detecte() {
        let given = arguments(&["generate-unicode-tables", "--check"]);
        assert!(has_flag(&given, "--check"));
        assert!(!has_flag(&given, "--autre"));
    }

    #[test]
    fn le_mode_d_emploi_cite_les_trois_commandes() {
        let text = usage();
        assert!(text.contains("download-ucd"));
        assert!(text.contains("generate-unicode-tables"));
        assert!(text.contains("check-segmentation"));
    }
}
