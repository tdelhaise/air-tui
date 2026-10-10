// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `redit` — l'éditeur de texte d'air-tui, à la manière de `tvedit`.
//!
//! # Ce qu'il fait
//!
//! ```sh
//! redit                        # un document vide
//! redit fichier.txt            # un fichier, existant ou à créer
//! redit --theme mc fichier.txt # aux couleurs de Midnight Commander
//! ```
//!
//! - `F2` enregistre, `F3` ouvre, `F10` quitte ;
//! - `Alt`+`F`, `Alt`+`E`, `Alt`+`A` ouvrent les menus ; `Échap` les referme ;
//! - l'édition travaille par **grappes de graphème** : un `é` décomposé ou un émoji à
//!   jointures s'effacent d'une touche ;
//! - `Ctrl`+flèche se déplace **par mot**, au sens de l'annexe 29 d'Unicode — ce qui garde
//!   `3,14` et `l'enfant` entiers là où une coupure sur la ponctuation les casserait ;
//! - `Ctrl`+`Y` supprime la ligne, comme dans Turbo Vision ;
//! - `--theme mc` échange la palette et les glyphes pour ceux du skin par défaut de
//!   Midnight Commander — l'éditeur reste `redit`, seules les couleurs et les traits
//!   changent, ce qui est précisément la preuve que le thème est une donnée ;
//! - les deux ascenseurs suivent le texte, la molette défile sans déplacer le curseur ;
//! - « Enregistrer » est **grisé** sur un document intact, dans le menu comme dans la
//!   barre de statut : une seule source, deux chemins (ADR-007).
//!
//! # Ce qu'il ne fait pas encore
//!
//! Pas de **sélection**, donc pas de copier-coller interne ni de `Maj`+flèche : le cadre
//! n'a qu'un curseur, pas d'étendue. Pas de **recherche**, pas d'**annulation**, pas de
//! **tabulation**, et pas de deuxième fenêtre — le bureau sait en tenir plusieurs, mais
//! rien ne les crée encore.
//!
//! Ces absences sont écrites ici et dans `docs/etat-avancement.md` plutôt que découvertes
//! à l'usage.

#![forbid(unsafe_code)]
// `expect` et `unwrap` sont interdits en production (lints de l'espace de travail) et
// légitimes dans un test, où une précondition non tenue doit faire échouer le test
// bruyamment. Même choix que dans `air-tui`.
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

mod document;
mod editor;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use air_tui::application::{AirApplication, AirPumpOutcome};
use air_tui::command::AirCommand;
use air_tui::theme::AirTheme;
use air_tui::view::AirView;
use air_tui::{PlatformError, PlatformErrorKind};
use air_tui_platform_posix::PosixTerminal;

use crate::document::Document;
use crate::editor::Editor;

/// Le délai d'attente d'un tour de boucle.
///
/// Un quart de seconde : assez court pour que le redimensionnement du terminal soit vu
/// sans gestionnaire de signal, assez long pour que la boucle dorme la plupart du temps.
const TURN_TIMEOUT: Duration = Duration::from_millis(250);

/// Le délai laissé au terminal pour répondre à la négociation du protocole clavier.
const PROTOCOL_TIMEOUT: Duration = Duration::from_millis(100);

/// Le thème par défaut, quand la ligne de commande n'en nomme pas.
///
/// Nommé plutôt que numéroté : `--theme 2` ne dirait rien à personne. La résolution d'un
/// nom en thème vit dans `air-tui`, parce que les deux applications la demandent — écrite
/// deux fois, elle finirait par ne plus l'être qu'une.
const DEFAULT_THEME: &str = "tv";

/// Ce que la ligne de commande demande.
#[derive(Debug, PartialEq, Eq)]
enum Request {
    /// Éditer, éventuellement un fichier, avec un thème.
    Edit {
        /// Le fichier à ouvrir, s'il y en a un.
        path: Option<PathBuf>,
        /// Le nom du thème demandé, déjà validé.
        theme: String,
    },
    /// Afficher le mode d'emploi.
    Usage,
    /// Afficher les noms de thèmes acceptés.
    ListThemes,
    /// Afficher la version.
    Version,
    /// Une option inconnue.
    Unknown(String),
    /// Un thème inconnu — distinct d'une option inconnue, parce que le message diffère.
    UnknownTheme(String),
}

/// Interprète la ligne de commande.
///
/// Un seul fichier : `redit` n'ouvre qu'une fenêtre, et accepter plusieurs noms
/// promettrait des onglets qui n'existent pas.
fn parse(arguments: &[String]) -> Request {
    let mut path: Option<PathBuf> = None;
    let mut theme = DEFAULT_THEME.to_owned();
    let mut awaiting_theme = false;
    for argument in arguments {
        if awaiting_theme {
            // Le nom est validé ici, et non au moment de peindre : une option refusée
            // doit l'être avant que le terminal passe en mode brut, sinon le message
            // s'affiche sur un écran déjà effacé.
            if AirTheme::from_command_line_name(argument).is_none() {
                return Request::UnknownTheme(argument.clone());
            }
            theme.clone_from(argument);
            awaiting_theme = false;
            continue;
        }
        match argument.as_str() {
            "-h" | "--help" => return Request::Usage,
            "-V" | "--version" => return Request::Version,
            "--theme" => awaiting_theme = true,
            "--list-themes" => return Request::ListThemes,
            other if other.starts_with('-') => return Request::Unknown(other.to_owned()),
            other => {
                if path.is_none() {
                    path = Some(PathBuf::from(other));
                }
            }
        }
    }
    if awaiting_theme {
        // `--theme` sans nom : ne rien dire et prendre le défaut serait le pire des deux
        // mondes, puisque l'intention était explicite.
        return Request::UnknownTheme(String::new());
    }
    Request::Edit { path, theme }
}

/// Le mode d'emploi.
fn usage() -> String {
    format!(
        "usage : redit [--theme <nom>] [--list-themes] [FICHIER]\n\
         \n\
         Éditeur de texte en mode terminal, bâti sur air-tui.\n\
         \n\
           -h, --help     affiche ce message\n\
           -V, --version  affiche la version\n\
               --theme    tv (défaut), mc, ou mc:<skin> pour un skin compilé\n\
               --list-themes  énumère les noms acceptés\n\
         \n\
         Touches : F2 enregistrer, F3 ouvrir, F10 quitter, Ctrl+Y supprimer la ligne.\n\
         Version {}\n",
        env!("CARGO_PKG_VERSION")
    )
}

/// La boucle d'édition.
///
/// L'ordre des quatre étapes est celui de l'ADR-007, et il n'est pas interchangeable :
/// réagir **avant** de dessiner, sinon l'écran montre l'état d'avant la réaction.
fn run(mut editor: Editor, theme: AirTheme) -> Result<(), PlatformError> {
    // C'est ici, et nulle part ailleurs dans `redit`, que le choix de la plateforme est
    // fait : le cœur du framework ne connaît que le contrat (ADR-011).
    let device = PosixTerminal::open()?;
    let mut application = AirApplication::new(Box::new(device), theme)?;
    // Sans lui, `Alt`+lettre n'arrive pas de façon fiable (ADR-003). Les événements reçus
    // pendant la négociation sont ignorés : l'écran n'est pas encore dessiné.
    let _ = application.enable_best_keyboard_protocol(PROTOCOL_TIMEOUT);
    editor.root_mut().resize(application.size());

    let mut commands: Vec<AirCommand> = Vec::with_capacity(4);
    loop {
        editor.synchronize(application.commands_mut());
        application.draw(editor.root())?;

        let outcome = match application.pump(editor.root_mut(), Some(TURN_TIMEOUT), &mut commands) {
            Ok(outcome) => outcome,
            // Le périphérique a disparu : c'est une fin de session, pas une panne.
            Err(error) if error.kind() == PlatformErrorKind::InvalidDevice => break,
            Err(error) => return Err(error),
        };

        editor.apply_pending();
        for command in &commands {
            editor.execute(*command);
        }
        if outcome == AirPumpOutcome::Closed || editor.wants_quit() {
            break;
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match parse(&arguments) {
        Request::Usage => {
            print!("{}", usage());
            ExitCode::SUCCESS
        }
        Request::ListThemes => {
            // Les noms exacts à taper, et leur description quand le thème vient d'un
            // skin : c'est cette liste qui fait foi, puisque le nom d'un skin est
            // transposé en identifiant Rust à la compilation.
            for name in AirTheme::command_line_names() {
                match name
                    .strip_prefix("mc:")
                    .and_then(AirTheme::compiled_description)
                {
                    Some(description) => println!("{name:<24} {description}"),
                    None => println!("{name:<24} Turbo Vision"),
                }
            }
            ExitCode::SUCCESS
        }
        Request::Version => {
            println!("redit {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Request::Unknown(option) => {
            eprintln!("option inconnue : {option}\n\n{}", usage());
            ExitCode::FAILURE
        }
        Request::UnknownTheme(name) if name.is_empty() => {
            eprintln!("--theme attend un nom\n\n{}", usage());
            ExitCode::FAILURE
        }
        Request::UnknownTheme(name) => {
            eprintln!("thème inconnu : {name}\n\n{}", usage());
            ExitCode::FAILURE
        }
        Request::Edit { path, theme } => {
            let mut editor = Editor::new(Document::new());
            if let Some(path) = path.as_deref() {
                editor.open(path);
            }
            let Some(resolved) = AirTheme::from_command_line_name(&theme) else {
                // Inatteignable : le nom est validé par `parse`. Refuser plutôt que de
                // retomber sur un défaut, pour que l'écart se voie s'il arrivait.
                eprintln!("redit : thème « {theme} » introuvable");
                return ExitCode::FAILURE;
            };
            match run(editor, resolved) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    // Le terminal a été restauré par la destruction d'`AirApplication`
                    // avant ce message : sans cela, il s'afficherait en mode brut.
                    eprintln!("redit : {error}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{DEFAULT_THEME, Request, parse, usage};

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn sans_argument_on_edite_un_document_vide() {
        assert_eq!(
            parse(&arguments(&[])),
            Request::Edit {
                path: None,
                theme: "tv".to_owned()
            }
        );
    }

    #[test]
    fn un_nom_de_fichier_est_retenu() {
        assert_eq!(
            parse(&arguments(&["essai.txt"])),
            Request::Edit {
                path: Some(PathBuf::from("essai.txt")),
                theme: "tv".to_owned()
            }
        );
    }

    #[test]
    fn seul_le_premier_nom_est_retenu() {
        // `redit` n'ouvre qu'une fenêtre : accepter le second promettrait des onglets.
        assert_eq!(
            parse(&arguments(&["un.txt", "deux.txt"])),
            Request::Edit {
                path: Some(PathBuf::from("un.txt")),
                theme: "tv".to_owned()
            }
        );
    }

    #[test]
    fn le_theme_par_defaut_est_turbo_vision() {
        assert_eq!(DEFAULT_THEME, "tv");
    }

    #[test]
    fn un_skin_compile_se_demande_par_son_nom() {
        // C'est l'apport du jalon : le dépôt porte plusieurs skins compilés, et le choix
        // se fait par un nom, sans que rien soit lu à l'exécution.
        assert_eq!(
            parse(&arguments(&["--theme", "mc:modarcon16_defbg", "essai.txt"])),
            Request::Edit {
                path: Some(PathBuf::from("essai.txt")),
                theme: "mc:modarcon16_defbg".to_owned()
            }
        );
    }

    #[test]
    fn un_skin_qui_n_est_pas_compile_est_refuse_avant_le_mode_brut() {
        // Refusé par `parse`, donc avant que le terminal passe en mode brut : un message
        // d'erreur sur un écran déjà effacé ne se lit pas.
        assert_eq!(
            parse(&arguments(&["--theme", "mc:inexistant"])),
            Request::UnknownTheme("mc:inexistant".to_owned())
        );
        assert_eq!(
            parse(&arguments(&["--theme", "borland"])),
            Request::UnknownTheme("borland".to_owned())
        );
    }

    #[test]
    fn list_themes_est_une_demande_a_part() {
        assert_eq!(parse(&arguments(&["--list-themes"])), Request::ListThemes);
    }

    #[test]
    fn un_theme_demande_est_retenu_avec_le_fichier() {
        assert_eq!(
            parse(&arguments(&["--theme", "mc", "essai.txt"])),
            Request::Edit {
                path: Some(PathBuf::from("essai.txt")),
                theme: "mc".to_owned()
            }
        );
    }

    #[test]
    fn le_theme_peut_suivre_le_fichier() {
        assert_eq!(
            parse(&arguments(&["essai.txt", "--theme", "mc"])),
            Request::Edit {
                path: Some(PathBuf::from("essai.txt")),
                theme: "mc".to_owned()
            }
        );
    }

    #[test]
    fn un_nom_de_theme_n_est_pas_pris_pour_un_fichier() {
        // Le piège d'une option à valeur : sans l'état d'attente, « mc » deviendrait le
        // nom du fichier à ouvrir.
        assert_eq!(
            parse(&arguments(&["--theme", "mc"])),
            Request::Edit {
                path: None,
                theme: "mc".to_owned()
            }
        );
    }

    #[test]
    fn un_theme_inconnu_est_rapporte_comme_tel() {
        assert_eq!(
            parse(&arguments(&["--theme", "borland"])),
            Request::UnknownTheme("borland".to_owned())
        );
    }

    #[test]
    fn un_theme_sans_nom_est_refuse() {
        // Prendre le défaut en silence serait le pire des deux mondes : l'intention
        // était explicite.
        assert_eq!(
            parse(&arguments(&["--theme"])),
            Request::UnknownTheme(String::new())
        );
    }

    #[test]
    fn le_mode_d_emploi_cite_les_deux_themes() {
        let text = usage();
        assert!(text.contains("--theme"));
        assert!(text.contains("tv"));
        assert!(text.contains("mc"));
    }

    #[test]
    fn les_deux_formes_de_l_aide_sont_reconnues() {
        assert_eq!(parse(&arguments(&["-h"])), Request::Usage);
        assert_eq!(parse(&arguments(&["--help"])), Request::Usage);
    }

    #[test]
    fn les_deux_formes_de_la_version_sont_reconnues() {
        assert_eq!(parse(&arguments(&["-V"])), Request::Version);
        assert_eq!(parse(&arguments(&["--version"])), Request::Version);
    }

    #[test]
    fn une_option_inconnue_est_rapportee_telle_quelle() {
        assert_eq!(
            parse(&arguments(&["--inconnue"])),
            Request::Unknown("--inconnue".to_owned())
        );
    }

    #[test]
    fn l_aide_prime_sur_un_nom_de_fichier() {
        assert_eq!(parse(&arguments(&["essai.txt", "--help"])), Request::Usage);
    }

    #[test]
    fn le_mode_d_emploi_cite_les_touches_et_la_version() {
        let text = usage();
        assert!(text.contains("redit [--theme <nom>] [--list-themes] [FICHIER]"));
        assert!(
            text.contains("mc:<skin>"),
            "les skins compilés doivent être cités"
        );
        assert!(text.contains("F10"));
        assert!(text.contains(env!("CARGO_PKG_VERSION")));
    }
}
