// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le contrôle de versionnement de l'ADR-009 : une crate touchée est une crate
//! incrémentée.
//!
//! # Les deux contrôles
//!
//! 1. **Incrément** : pour chaque crate de l'espace de travail dont un fichier change sur
//!    la plage `<base>..HEAD`, la version du manifeste doit différer de celle de la base.
//!    Une crate qui n'existe pas à la base est neuve : rien à incrémenter.
//! 2. **Cohérence des dépendances par chemin** : `air-tui = { path = "…", version = "0.7.0" }`
//!    doit déclarer la version que porte réellement `air-tui`. C'est la conséquence de
//!    l'ADR-009 la plus facile à oublier — le `path` continue de compiler, et c'est la
//!    publication qui casse, bien plus tard.
//!
//! # Ce que la plage voit, et ce qu'elle ne voit pas
//!
//! La plage ne voit que ce qui est **committé**. Un contrôle qui ne regarderait qu'elle
//! passerait donc en silence sur un travail en cours, ce qui est le pire moment pour
//! rassurer : l'arbre de travail est donc joint à la plage, et le compte rendu dit
//! toujours d'où viennent les fichiers qu'il a vus.

use std::collections::BTreeMap;
use std::path::Path;

/// Une crate de l'espace de travail, telle que son manifeste la décrit.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CrateInformation {
    /// Le nom du paquet.
    name: String,
    /// Le répertoire, relatif à la racine du dépôt.
    directory: String,
    /// La version déclarée.
    version: String,
}

/// Un manquement relevé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Ce qui est en cause — une crate, ou un couple de crates.
    pub subject: String,
    /// Ce qui manque, et ce qui est attendu.
    pub detail: String,
}

/// Le compte rendu d'un contrôle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// La référence de comparaison, telle qu'elle a été résolue.
    pub base: String,
    /// Le nombre de crates de l'espace de travail.
    pub examined: usize,
    /// Les crates touchées, par leur nom.
    pub touched: Vec<String>,
    /// Vrai si des fichiers non committés ont été joints à la plage.
    pub working_tree_joined: bool,
    /// Les manquements.
    pub violations: Vec<Violation>,
}

/// Contrôle l'espace de travail contre une référence de comparaison.
///
/// # Errors
///
/// Rend une erreur si `git` échoue, si la référence est introuvable, ou si un manifeste
/// n'est pas compris — un manifeste qu'on ne comprend pas ne doit pas passer en silence.
pub fn check(repository: &Path, base: &str) -> Result<Report, String> {
    let merge_base = git(repository, &["merge-base", base, "HEAD"]).map_err(|cause| {
        format!(
            "impossible de situer {base} par rapport à HEAD : {cause}\n\
             Sur un clone superficiel, récupérer la base d'abord :\n  \
             git fetch --depth=1 origin main"
        )
    })?;
    let merge_base = merge_base.trim().to_owned();

    let root_manifest = read_file(repository, "Cargo.toml")?;
    let members = workspace_members(&root_manifest);
    let mut crates = Vec::new();
    for member in &members {
        let manifest_path = format!("{member}/Cargo.toml");
        let manifest = read_file(repository, &manifest_path)?;
        crates.push(CrateInformation {
            name: package_name(&manifest)
                .ok_or_else(|| format!("{manifest_path} ne déclare pas de nom de paquet"))?,
            directory: member.clone(),
            version: package_version(&manifest)
                .map_err(|cause| format!("{manifest_path} : {cause}"))?,
        });
    }

    let committed = changed_files(repository, &merge_base)?;
    let pending = working_tree_files(repository)?;
    let working_tree_joined = !pending.is_empty();
    let mut changed = committed;
    changed.extend(pending);

    let mut report = Report {
        base: format!("{base} ({})", short(&merge_base)),
        examined: crates.len(),
        touched: Vec::new(),
        working_tree_joined,
        violations: Vec::new(),
    };

    for information in &crates {
        if !touches(&changed, &information.directory) {
            continue;
        }
        report.touched.push(information.name.clone());
        let manifest_path = format!("{}/Cargo.toml", information.directory);
        let Some(previous) = file_at_revision(repository, &merge_base, &manifest_path) else {
            // Crate neuve sur la plage : il n'y a pas de version antérieure à dépasser.
            continue;
        };
        let previous_version = package_version(&previous)
            .map_err(|cause| format!("{manifest_path} à la base : {cause}"))?;
        if previous_version == information.version {
            report.violations.push(Violation {
                subject: information.name.clone(),
                detail: format!(
                    "touchée sans incrément : toujours {previous_version}. ADR-009 — \
                     rupture d'API en y, tout le reste en z"
                ),
            });
        }
    }

    let actual: BTreeMap<&str, &str> = crates
        .iter()
        .map(|information| (information.name.as_str(), information.version.as_str()))
        .collect();
    for information in &crates {
        let manifest_path = format!("{}/Cargo.toml", information.directory);
        let manifest = read_file(repository, &manifest_path)?;
        for (dependency, declared) in
            path_dependencies(&manifest).map_err(|cause| format!("{manifest_path} : {cause}"))?
        {
            let Some(real) = actual.get(dependency.as_str()) else {
                continue;
            };
            if let Some(violation) = drift(&information.name, &dependency, &declared, real) {
                report.violations.push(violation);
            }
        }
    }
    Ok(report)
}

/// Le manquement d'une dépendance par chemin dont la version déclarée a dérivé.
///
/// L'égalité est **exacte**, comme l'exemple de l'ADR-009. Cargo, lui, se contente d'une
/// compatibilité : déclarer `0.7.0` alors que la crate porte `0.7.1` compile sans un mot,
/// et c'est la publication qui livrerait une borne fausse. C'est ce cas-là que ce contrôle
/// existe pour attraper — l'incompatible, cargo le refuse déjà tout seul.
fn drift(consumer: &str, dependency: &str, declared: &str, real: &str) -> Option<Violation> {
    if declared == real {
        return None;
    }
    Some(Violation {
        subject: format!("{consumer} → {dependency}"),
        detail: format!(
            "dépendance par chemin déclarée en {declared} alors que {dependency} porte {real}"
        ),
    })
}

/// Les sept premiers caractères d'un identifiant de commit.
fn short(revision: &str) -> String {
    revision.chars().take(7).collect()
}

/// Lit un fichier du dépôt.
fn read_file(repository: &Path, relative: &str) -> Result<String, String> {
    let path = repository.join(relative);
    std::fs::read_to_string(&path)
        .map_err(|cause| format!("{} illisible : {cause}", path.display()))
}

/// Le contenu d'un fichier à une révision, ou `None` s'il n'y existait pas.
///
/// `git show` échoue aussi bien pour un chemin absent que pour une révision inconnue. La
/// révision vient d'être résolue par `merge-base` : il ne reste que le chemin absent,
/// c'est-à-dire une crate neuve. L'absence n'est donc pas une erreur ici.
fn file_at_revision(repository: &Path, revision: &str, relative: &str) -> Option<String> {
    let specification = format!("{revision}:{relative}");
    git(repository, &["show", &specification]).ok()
}

/// Les fichiers modifiés entre une révision et `HEAD`.
fn changed_files(repository: &Path, revision: &str) -> Result<Vec<String>, String> {
    let output = git(repository, &["diff", "--name-only", revision, "HEAD"])?;
    Ok(output.lines().map(str::to_owned).collect())
}

/// Les fichiers modifiés dans l'arbre de travail, suivis ou non.
fn working_tree_files(repository: &Path) -> Result<Vec<String>, String> {
    let output = git(repository, &["status", "--porcelain"])?;
    Ok(output.lines().filter_map(porcelain_path).collect())
}

/// Le chemin d'une ligne de `git status --porcelain`.
///
/// Les deux premières colonnes sont l'état ; un renommage porte `ancien -> nouveau`, et
/// c'est le nouveau nom qui compte.
fn porcelain_path(line: &str) -> Option<String> {
    let remainder = line.get(3..)?.trim();
    let path = remainder
        .rsplit(" -> ")
        .next()
        .unwrap_or(remainder)
        .trim_matches('"');
    if path.is_empty() {
        return None;
    }
    Some(path.to_owned())
}

/// Vrai si l'un des chemins est dans le répertoire d'une crate.
fn touches(changed: &[String], directory: &str) -> bool {
    let prefix = format!("{directory}/");
    changed.iter().any(|path| path.starts_with(&prefix))
}

/// Les membres déclarés par le manifeste de l'espace de travail.
fn workspace_members(manifest: &str) -> Vec<String> {
    let mut members = Vec::new();
    let mut inside = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("members") && trimmed.contains('[') {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if trimmed.starts_with(']') {
            break;
        }
        if let Some(value) = between_quotes(trimmed) {
            members.push(value.to_owned());
        }
    }
    members
}

/// Le texte entre les premiers guillemets d'une ligne.
fn between_quotes(line: &str) -> Option<&str> {
    let (_, after) = line.split_once('"')?;
    let (value, _) = after.split_once('"')?;
    Some(value)
}

/// Le nom de paquet d'un manifeste.
fn package_name(manifest: &str) -> Option<String> {
    value_of_key(manifest, "name").map(str::to_owned)
}

/// La version d'un manifeste.
///
/// # Errors
///
/// Une version absente, ou héritée de `[workspace.package]`, est refusée : l'héritage
/// rendrait la règle de l'ADR-009 inapplicable, et c'est pour cela que le manifeste
/// racine ne déclare pas de `version`.
fn package_version(manifest: &str) -> Result<String, String> {
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("version.workspace") {
            return Err(
                "version héritée de l'espace de travail, ce qu'interdit l'ADR-009".to_owned(),
            );
        }
    }
    value_of_key(manifest, "version")
        .map(str::to_owned)
        .ok_or_else(|| "aucune version déclarée".to_owned())
}

/// La valeur entre guillemets de la première clé de ce nom, hors clé composée.
fn value_of_key<'manifest>(manifest: &'manifest str, key: &str) -> Option<&'manifest str> {
    for line in manifest.lines() {
        let trimmed = line.trim();
        let Some(remainder) = trimmed.strip_prefix(key) else {
            continue;
        };
        // `rust-version` ne doit pas répondre pour `version`, ni `version.workspace`.
        if !remainder.trim_start().starts_with('=') {
            continue;
        }
        if let Some(value) = between_quotes(remainder) {
            return Some(value);
        }
    }
    None
}

/// Les dépendances par chemin d'un manifeste, avec la version qu'elles déclarent.
///
/// # Errors
///
/// Une table en ligne coupée sur plusieurs lignes n'est pas comprise, et le dire est
/// préférable à l'ignorer : un contrôle muet sur ce qu'il n'a pas lu ne contrôle rien.
fn path_dependencies(manifest: &str) -> Result<Vec<(String, String)>, String> {
    let mut found = Vec::new();
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        let Some((name, remainder)) = trimmed.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if !remainder.trim_start().starts_with('{') {
            continue;
        }
        if !remainder.contains('}') {
            return Err(format!(
                "la dépendance `{name}` est déclarée sur plusieurs lignes, forme que ce \
                 contrôle ne lit pas"
            ));
        }
        if !remainder.contains("path") {
            continue;
        }
        let Some(version) = inline_value(remainder, "version") else {
            return Err(format!(
                "la dépendance par chemin `{name}` ne déclare pas de version, ce \
                 qu'exige l'ADR-009"
            ));
        };
        found.push((name.to_owned(), version.to_owned()));
    }
    Ok(found)
}

/// La valeur d'une clé dans une table en ligne.
fn inline_value<'table>(table: &'table str, key: &str) -> Option<&'table str> {
    let (_, after) = table.split_once(key)?;
    let remainder = after.trim_start();
    let equals = remainder.strip_prefix('=')?;
    between_quotes(equals)
}

/// Lance `git` dans le dépôt et rend sa sortie standard.
fn git(repository: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .current_dir(repository)
        .args(arguments)
        .output()
        .map_err(|cause| format!("git n'a pas pu être lancé : {cause}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        drift, package_name, package_version, path_dependencies, porcelain_path, touches,
        value_of_key, workspace_members,
    };

    const ROOT: &str = r#"
[workspace]
resolver = "3"
members = [
    "crates/air-tui-platform",
    "crates/air-tui",
    "xtask",
]

[workspace.package]
edition = "2024"
rust-version = "1.99"
"#;

    const MEMBER: &str = r#"
[package]
name = "air-tui"
description = "Le cœur du framework."
version = "0.7.0"
edition.workspace = true
rust-version.workspace = true

[dependencies]
air-tui-platform = { path = "../air-tui-platform", version = "0.2.0" }
"#;

    #[test]
    fn les_membres_se_lisent_dans_le_manifeste_racine() {
        assert_eq!(
            workspace_members(ROOT),
            vec!["crates/air-tui-platform", "crates/air-tui", "xtask"]
        );
    }

    #[test]
    fn le_nom_et_la_version_se_lisent() {
        assert_eq!(package_name(MEMBER).as_deref(), Some("air-tui"));
        assert_eq!(package_version(MEMBER).as_deref(), Ok("0.7.0"));
    }

    #[test]
    fn rust_version_ne_repond_pas_pour_version() {
        // Le piège de la lecture ligne à ligne : `rust-version` contient `version`.
        let manifest = "[package]\nrust-version = \"1.99\"\nversion = \"0.3.0\"\n";
        assert_eq!(value_of_key(manifest, "version"), Some("0.3.0"));
    }

    #[test]
    fn une_version_heritee_est_refusee() {
        // ADR-009 : l'héritage rendrait la règle inapplicable, donc il est interdit.
        let manifest = "[package]\nname = \"essai\"\nversion.workspace = true\n";
        assert!(package_version(manifest).is_err());
    }

    #[test]
    fn une_version_absente_est_refusee() {
        assert!(package_version("[package]\nname = \"essai\"\n").is_err());
    }

    #[test]
    fn une_dependance_par_chemin_rend_la_version_declaree() {
        let found = path_dependencies(MEMBER).expect("manifeste compris");
        assert_eq!(
            found,
            vec![("air-tui-platform".to_owned(), "0.2.0".to_owned())]
        );
    }

    #[test]
    fn une_dependance_sans_chemin_n_est_pas_concernee() {
        let manifest = "[dependencies]\nserde = { version = \"1\", features = [] }\n";
        assert_eq!(path_dependencies(manifest), Ok(Vec::new()));
    }

    #[test]
    fn une_dependance_par_chemin_sans_version_est_refusee() {
        let manifest = "[dependencies]\nair-tui = { path = \"../air-tui\" }\n";
        assert!(path_dependencies(manifest).is_err());
    }

    #[test]
    fn une_table_coupee_sur_plusieurs_lignes_est_refusee_et_non_ignoree() {
        // Un contrôle muet sur ce qu'il n'a pas lu ne contrôle rien.
        let manifest = "[dependencies]\nair-tui = {\n    path = \"../air-tui\",\n}\n";
        assert!(path_dependencies(manifest).is_err());
    }

    #[test]
    fn un_commentaire_ne_declare_pas_de_dependance() {
        let manifest = "# air-tui = { path = \"../air-tui\" }\n";
        assert_eq!(path_dependencies(manifest), Ok(Vec::new()));
    }

    #[test]
    fn une_ligne_de_statut_rend_son_chemin() {
        assert_eq!(
            porcelain_path(" M crates/air-tui/src/view/mod.rs").as_deref(),
            Some("crates/air-tui/src/view/mod.rs")
        );
        assert_eq!(
            porcelain_path("?? xtask/src/naming.rs").as_deref(),
            Some("xtask/src/naming.rs")
        );
    }

    #[test]
    fn un_renommage_rend_le_nouveau_nom() {
        assert_eq!(
            porcelain_path("R  ancien.rs -> nouveau.rs").as_deref(),
            Some("nouveau.rs")
        );
    }

    #[test]
    fn un_prefixe_de_crate_ne_mord_pas_sur_sa_voisine() {
        // `crates/air-tui` est un préfixe de `crates/air-tui-platform` : sans la barre
        // oblique, toucher la plateforme ferait croire que le cœur a changé.
        let changed = vec!["crates/air-tui-platform/src/codec.rs".to_owned()];
        assert!(touches(&changed, "crates/air-tui-platform"));
        assert!(!touches(&changed, "crates/air-tui"));
    }

    #[test]
    fn une_version_declaree_exacte_ne_derive_pas() {
        assert_eq!(drift("xtask", "air-tui", "0.7.0", "0.7.0"), None);
    }

    #[test]
    fn une_derive_compatible_est_relevee() {
        // Cargo accepte `^0.7.0` pour une crate en 0.7.1 : c'est exactement la dérive que
        // ce contrôle existe pour voir, puisque rien d'autre ne la verra avant la
        // publication.
        let found = drift("xtask", "air-tui", "0.7.0", "0.7.1").expect("une dérive attendue");
        assert_eq!(found.subject, "xtask → air-tui");
        assert!(found.detail.contains("0.7.1"), "{}", found.detail);
    }

    #[test]
    fn un_fichier_hors_crate_ne_touche_personne() {
        let changed = vec!["docs/JOURNAL.md".to_owned()];
        assert!(!touches(&changed, "crates/air-tui"));
        assert!(!touches(&changed, "xtask"));
    }
}
