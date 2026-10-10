// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le registre des thèmes compilés — `theme/skins/mod.rs`, **généré**.
//!
//! # Pourquoi un registre, et pourquoi généré
//!
//! Le dépôt ne portait qu'**un** artefact de thème, et `AirTheme::midnight_commander()`
//! le rendait. Les trente-neuf skins livrés avec `mc` sont importables depuis le jalon
//! M7g, mais pas simultanément : importer le second effaçait le premier.
//!
//! Le registre lève cette limite **sans** toucher à l'ADR-008 : rien n'est lu à
//! l'exécution, les thèmes sont toujours compilés, et le choix se fait par un nom sur la
//! ligne de commande plutôt que par un fichier ouvert au démarrage.
//!
//! Il est généré pour une raison de cohérence : la liste des modules et la table des noms
//! disent la même chose deux fois. Écrites à la main, elles divergeraient — un artefact
//! ajouté sans son entrée dans la table serait du code mort que `clippy` signalerait, et
//! une entrée sans son artefact ne compilerait pas. Les deux sont donc produites du même
//! balayage du répertoire.

use std::fmt::Write as _;
use std::path::Path;

/// Le nom d'un thème compilé, depuis le nom du fichier de skin.
///
/// Le nom est **transposé en identifiant Rust**, et c'est une perte qu'il faut assumer :
/// `modarcon16-defbg.ini` donne `modarcon16_defbg`, et `seasons-autumn16M.ini` donne
/// `seasons_autumn16m`. Un module Rust n'admet ni tiret ni majuscule — `non_snake_case`
/// est un avertissement que `clippy -D warnings` refuse —, et faire porter au registre
/// deux noms, celui du fichier et celui du module, obligerait à les tenir d'accord.
///
/// Personne n'a donc à devancer la transposition : `--list-themes` donne les noms exacts
/// à taper, et c'est lui qui fait foi.
#[must_use]
pub fn module_name(skin_name: &str) -> String {
    skin_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// Un artefact trouvé dans le répertoire des thèmes compilés.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Compiled {
    /// Le nom du skin, tel que la ligne de commande le désigne.
    pub name: String,
    /// Le nom du module Rust.
    pub module: String,
}

/// Les artefacts présents dans le répertoire, par ordre de nom.
///
/// # Errors
///
/// Si le répertoire est illisible.
pub fn present(directory: &Path) -> Result<Vec<Compiled>, String> {
    let entries = std::fs::read_dir(directory)
        .map_err(|cause| format!("{} illisible : {cause}", directory.display()))?;
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|cause| format!("entrée illisible : {cause}"))?;
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension != "rs") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if stem == "mod" {
            continue;
        }
        found.push(Compiled {
            name: stem.to_owned(),
            module: stem.to_owned(),
        });
    }
    found.sort();
    Ok(found)
}

/// Produit le source du registre.
///
/// Le nom de la ligne de commande est le **nom de fichier du module**, et non le nom du
/// skin d'origine : le nom du skin porte des tirets que le module ne peut pas porter, et
/// faire porter les deux au registre demanderait de les tenir d'accord. Le module fait
/// donc foi, et `mc:modarcon16_defbg` est le nom que l'on tape.
#[must_use]
pub fn emit(compiled: &[Compiled]) -> String {
    let mut output = String::new();
    output.push_str(
        "// This Source Code Form is subject to the terms of the Mozilla Public\n\
         // License, v. 2.0. If a copy of the MPL was not distributed with this\n\
         // file, You can obtain one at https://mozilla.org/MPL/2.0/.\n\n",
    );
    output.push_str(
        "//! Les thèmes **compilés** depuis un skin de Midnight Commander — **fichier\n\
         //! généré, ne pas éditer**.\n\
         //!\n\
         //! # Pourquoi compilés\n\
         //!\n\
         //! L'ADR-008 tranche : « les thèmes sont compilés, pas analysés au runtime ». Un\n\
         //! skin est un texte, et le Principe 11 d'Air interdit qu'un texte entre par la\n\
         //! porte d'exécution : un outil dédié le compile en artefact vérifié. Un skin mal\n\
         //! formé est donc un refus de `cargo xtask import-mc-skin`, jamais un comportement\n\
         //! dégradé dans une application.\n\
         //!\n\
         //! Conséquence directe : air-tui ne sait pas lire un `.ini`, n'embarque aucun\n\
         //! analyseur, et n'ouvre aucun fichier pour afficher un thème.\n\
         //!\n\
         //! # Plusieurs skins à la fois\n\
         //!\n\
         //! Ce fichier est le **registre** : il déclare les modules et les nomme. Jusqu'au\n\
         //! jalon M7i, le dépôt n'en portait qu'un, et importer le second effaçait le\n\
         //! premier. Le choix se fait désormais par un nom — `mc:<nom>` sur la ligne de\n\
         //! commande des applications —, ce qui lève la limite sans rien lire à\n\
         //! l'exécution.\n\
         //!\n\
         //! Le registre est généré plutôt qu'écrit : la liste des modules et la table des\n\
         //! noms disent la même chose deux fois, et deux listes tenues à la main\n\
         //! divergent.\n\
         //!\n\
         //! # Ce que contient ce répertoire\n\
         //!\n\
         //! Des fichiers **générés**, qui ne s'éditent jamais à la main — la règle des\n\
         //! fichiers générés de `docs/CONVENTIONS-fr.md` s'y applique telle quelle,\n\
         //! `--check` compris. Leur origine et sa licence sont dans `NOTICE`.\n\n",
    );
    for entry in compiled {
        let _ = writeln!(output, "pub(crate) mod {};", entry.module);
    }
    output.push_str(
        "\nuse crate::theme::AirCompiledSkin;\n\n\
         /// Les thèmes compilés que ce dépôt porte, par ordre de nom.\n\
         ///\n\
         /// Une table de pointeurs de fonction plutôt qu'un `match` : la table se parcourt\n\
         /// pour **énumérer** les noms, ce qu'un `match` ne permet pas, et c'est ce dont\n\
         /// `--list-themes` a besoin.\n\
         pub(crate) const COMPILED: &[AirCompiledSkin] = &[\n",
    );
    for entry in compiled {
        let module = &entry.module;
        let _ = write!(
            output,
            "    AirCompiledSkin {{\n        \
             name: \"{}\",\n        \
             description: {module}::DESCRIPTION,\n        \
             palette: {module}::palette,\n        \
             glyphs: {module}::glyphs,\n    \
             }},\n",
            entry.name
        );
    }
    output.push_str("];\n");
    output
}

#[cfg(test)]
mod tests {
    use super::{Compiled, emit, module_name};

    /// Deux artefacts, dont un au nom à tirets.
    fn two() -> Vec<Compiled> {
        vec![
            Compiled {
                name: "default".to_owned(),
                module: "default".to_owned(),
            },
            Compiled {
                name: "modarcon16_defbg".to_owned(),
                module: "modarcon16_defbg".to_owned(),
            },
        ]
    }

    #[test]
    fn un_tiret_devient_un_souligne() {
        assert_eq!(module_name("modarcon16-defbg"), "modarcon16_defbg");
        assert_eq!(module_name("default"), "default");
    }

    #[test]
    fn une_majuscule_est_abaissee() {
        // `non_snake_case` est un avertissement, et `clippy -D warnings` le refuse : un
        // module `seasons_autumn16M` ferait échouer la compilation du dépôt.
        assert_eq!(module_name("seasons-autumn16M"), "seasons_autumn16m");
    }

    #[test]
    fn tout_ce_qui_n_est_pas_alphanumerique_devient_un_souligne() {
        // `--skin default.ini` pourrait arriver avec son extension, et un nom venu d'un
        // skin tiers pourrait porter n'importe quoi.
        assert_eq!(module_name("default.ini"), "default_ini");
        assert_eq!(module_name("thème+exotique"), "th_me_exotique");
    }

    #[test]
    fn le_registre_declare_chaque_module() {
        let source = emit(&two());
        assert!(source.contains("pub(crate) mod default;"));
        assert!(source.contains("pub(crate) mod modarcon16_defbg;"));
    }

    #[test]
    fn le_registre_nomme_chaque_theme_et_pointe_ses_deux_fonctions() {
        let source = emit(&two());
        assert!(source.contains("name: \"default\","), "{source}");
        assert!(source.contains("palette: default::palette,"), "{source}");
        assert!(source.contains("glyphs: default::glyphs,"), "{source}");
        assert!(
            source.contains("description: modarcon16_defbg::DESCRIPTION,"),
            "{source}"
        );
    }

    #[test]
    fn le_registre_porte_l_en_tete_de_licence_et_se_declare_genere() {
        let source = emit(&two());
        assert!(source.starts_with("// This Source Code Form"));
        assert!(source.contains("fichier\n//! généré, ne pas éditer"));
    }

    #[test]
    fn un_registre_vide_compile_quand_meme() {
        // Le cas d'un dépôt dont on aurait retiré tous les artefacts : une table vide
        // vaut mieux qu'un fichier qui ne compile pas.
        let source = emit(&[]);
        assert!(source.contains("pub(crate) const COMPILED: &[AirCompiledSkin] = &[\n];"));
    }

    #[test]
    fn la_generation_est_deterministe() {
        assert_eq!(emit(&two()), emit(&two()));
    }
}
