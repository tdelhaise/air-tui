// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `rmc` — la coquille à deux panneaux d'air-tui, à la manière de Midnight Commander.
//!
//! # Ce qu'il fait
//!
//! ```sh
//! rmc                       # le répertoire courant des deux côtés
//! rmc /usr /etc             # un répertoire par panneau
//! rmc --theme tv            # aux couleurs de Turbo Vision
//! ```
//!
//! - deux panneaux côte à côte, chacun une fenêtre bordée titrée de son chemin ;
//! - `Tab` change de panneau — le cadre actif et le curseur de liste disent lequel
//!   répond ;
//! - les flèches, `Page haut`, `Page bas`, `Début` et `Fin` parcourent la liste ;
//! - `Entrée` descend dans un répertoire, `..` remonte, et `Re~m~onter` est **grisée** à
//!   la racine : une seule source, deux chemins (ADR-007) ;
//! - `Inser` marque et démarque ; le titre du panneau porte le compte et la taille de ce
//!   qui est marqué ;
//! - `F5` relit le panneau actif, en gardant les marques des fichiers encore là ;
//! - `F10` quitte, et le menu « Fichier » aussi ;
//! - la molette et l'ascenseur défilent sans déplacer le curseur.
//!
//! Le thème par défaut est celui de **Midnight Commander** : c'est la coquille dont il
//! porte le nom. `--theme tv` donne la même coquille aux couleurs de Turbo Vision, et
//! c'est la démonstration la plus courte de ce qu'ADR-008 affirme — la coquille et le
//! thème sont deux choses distinctes.
//!
//! # Ce qu'il ne fait pas
//!
//! **`rmc` n'exécute aucun programme.** Il n'a ni ligne de commande, ni `F3` pour voir, ni
//! `F4` pour éditer, ni `F5` pour copier, ni `F6` pour déplacer, ni `F8` pour supprimer :
//! rien de ce que `mc` fait au contenu du disque. `Entrée` sur un fichier ne fait rien.
//!
//! Il ne lit pas non plus les fichiers de configuration de `mc`, ne connaît pas les
//! systèmes de fichiers virtuels, et n'affiche ni droits, ni propriétaire, ni date.
//!
//! Ces absences sont écrites ici et dans `docs/etat-avancement.md` plutôt que découvertes
//! à l'usage.

#![forbid(unsafe_code)]
// `expect` et `unwrap` sont interdits en production (lints de l'espace de travail) et
// légitimes dans un test, où une précondition non tenue doit faire échouer le test
// bruyamment. Même choix que dans `air-tui` et `redit`.
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

mod directory;
mod shell;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use air_tui::application::{AirApplication, AirPumpOutcome};
use air_tui::command::AirCommand;
use air_tui::theme::AirTheme;
use air_tui::view::AirView;
use air_tui::{PlatformError, PlatformErrorKind};
use air_tui_platform_posix::PosixTerminal;

use crate::shell::{Shell, default_directory};

/// Le délai d'attente d'un tour de boucle.
///
/// Un quart de seconde : assez court pour que le redimensionnement du terminal soit vu
/// sans gestionnaire de signal, assez long pour que la boucle dorme la plupart du temps.
const TURN_TIMEOUT: Duration = Duration::from_millis(250);

/// Le délai laissé au terminal pour répondre à la négociation du protocole clavier.
const PROTOCOL_TIMEOUT: Duration = Duration::from_millis(100);

/// Le thème par défaut, quand la ligne de commande n'en nomme pas.
///
/// Il n'est **pas** celui de `redit` : une coquille qui s'appelle `rmc` s'ouvre aux
/// couleurs de `mc`. La résolution d'un nom en thème vit dans `air-tui`, parce que les
/// deux applications la demandent — écrite deux fois, elle finirait par ne plus l'être
/// qu'une.
const DEFAULT_THEME: &str = "mc";

/// Ce que la ligne de commande demande.
#[derive(Debug, PartialEq, Eq)]
enum Request {
    /// Parcourir, avec un répertoire par panneau et un thème.
    Browse {
        /// Le répertoire du panneau de gauche.
        left: Option<PathBuf>,
        /// Celui du panneau de droite.
        right: Option<PathBuf>,
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
    /// Plus de deux répertoires : il n'y a que deux panneaux.
    TooManyDirectories(String),
}

/// Interprète la ligne de commande.
///
/// Deux répertoires au plus : il y a deux panneaux, et en accepter un troisième en
/// silence promettrait un panneau qui n'existe pas.
fn parse(arguments: &[String]) -> Request {
    let mut directories: Vec<PathBuf> = Vec::new();
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
                if directories.len() >= 2 {
                    return Request::TooManyDirectories(other.to_owned());
                }
                directories.push(PathBuf::from(other));
            }
        }
    }
    if awaiting_theme {
        // `--theme` sans nom : ne rien dire et prendre le défaut serait le pire des deux
        // mondes, puisque l'intention était explicite.
        return Request::UnknownTheme(String::new());
    }
    let mut found = directories.into_iter();
    Request::Browse {
        left: found.next(),
        right: found.next(),
        theme,
    }
}

/// Le mode d'emploi.
fn usage() -> String {
    format!(
        "usage : rmc [--theme <nom>] [--list-themes] [RÉPERTOIRE [RÉPERTOIRE]]\n\
         \n\
         Coquille à deux panneaux en mode terminal, bâtie sur air-tui.\n\
         \n\
           -h, --help     affiche ce message\n\
           -V, --version  affiche la version\n\
               --theme    mc (défaut), tv, ou mc:<skin> pour un skin compilé\n\
               --list-themes  énumère les noms acceptés\n\
         \n\
         Touches : Tab change de panneau, Entrée descend, Inser marque,\n\
         F5 relit, F10 quitte.\n\
         rmc n'exécute aucun programme et ne modifie aucun fichier.\n\
         Version {}\n",
        env!("CARGO_PKG_VERSION")
    )
}

/// La boucle de parcours.
///
/// L'ordre des quatre étapes est celui de l'ADR-007, et il n'est pas interchangeable :
/// réagir **avant** de dessiner, sinon l'écran montre l'état d'avant la réaction.
fn run(mut shell: Shell, theme: AirTheme) -> Result<(), PlatformError> {
    // C'est ici, et nulle part ailleurs dans `rmc`, que le choix de la plateforme est
    // fait : le cœur du framework ne connaît que le contrat (ADR-011).
    let device = PosixTerminal::open()?;
    let mut application = AirApplication::new(Box::new(device), theme)?;
    // Sans lui, `Alt`+lettre n'arrive pas de façon fiable (ADR-003). Les événements reçus
    // pendant la négociation sont ignorés : l'écran n'est pas encore dessiné.
    let _ = application.enable_best_keyboard_protocol(PROTOCOL_TIMEOUT);
    shell.root_mut().resize(application.size());
    shell.reload_all();

    let mut commands: Vec<AirCommand> = Vec::with_capacity(4);
    loop {
        shell.synchronize(application.commands_mut());
        application.draw(shell.root())?;

        let outcome = match application.pump(shell.root_mut(), Some(TURN_TIMEOUT), &mut commands) {
            Ok(outcome) => outcome,
            // Le périphérique a disparu : c'est une fin de session, pas une panne.
            Err(error) if error.kind() == PlatformErrorKind::InvalidDevice => break,
            Err(error) => return Err(error),
        };

        shell.apply_pending();
        for command in &commands {
            shell.execute(*command);
        }
        if outcome == AirPumpOutcome::Closed || shell.wants_quit() {
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
            println!("rmc {}", env!("CARGO_PKG_VERSION"));
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
        Request::TooManyDirectories(extra) => {
            eprintln!(
                "il n'y a que deux panneaux : {extra} est en trop\n\n{}",
                usage()
            );
            ExitCode::FAILURE
        }
        Request::Browse { left, right, theme } => {
            let left = left.unwrap_or_else(default_directory);
            // Sans second répertoire, les deux panneaux s'ouvrent au même endroit : c'est
            // ce que fait `mc` au premier lancement.
            let right = right.unwrap_or_else(|| left.clone());
            let shell = Shell::new(left, right);
            let Some(resolved) = AirTheme::from_command_line_name(&theme) else {
                // Inatteignable : le nom est validé par `parse`. Refuser plutôt que de
                // retomber sur un défaut, pour que l'écart se voie s'il arrivait.
                eprintln!("rmc : thème « {theme} » introuvable");
                return ExitCode::FAILURE;
            };
            match run(shell, resolved) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("rmc : {error}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_THEME, Request, parse, usage};
    use std::path::PathBuf;

    /// Une ligne de commande, telle que `std::env::args` la rendrait.
    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn sans_argument_les_deux_panneaux_sont_implicites() {
        assert_eq!(
            parse(&arguments(&[])),
            Request::Browse {
                left: None,
                right: None,
                theme: "mc".to_owned(),
            }
        );
    }

    #[test]
    fn le_theme_par_defaut_est_celui_dont_la_coquille_porte_le_nom() {
        assert_eq!(DEFAULT_THEME, "mc");
    }

    #[test]
    fn un_skin_compile_se_demande_par_son_nom() {
        assert_eq!(
            parse(&arguments(&["--theme", "mc:seasons_autumn16m"])),
            Request::Browse {
                left: None,
                right: None,
                theme: "mc:seasons_autumn16m".to_owned(),
            }
        );
    }

    #[test]
    fn list_themes_est_une_demande_a_part() {
        assert_eq!(parse(&arguments(&["--list-themes"])), Request::ListThemes);
    }

    #[test]
    fn un_repertoire_garnit_le_panneau_de_gauche() {
        assert_eq!(
            parse(&arguments(&["/usr"])),
            Request::Browse {
                left: Some(PathBuf::from("/usr")),
                right: None,
                theme: "mc".to_owned(),
            }
        );
    }

    #[test]
    fn deux_repertoires_garnissent_les_deux_panneaux() {
        assert_eq!(
            parse(&arguments(&["/usr", "/etc"])),
            Request::Browse {
                left: Some(PathBuf::from("/usr")),
                right: Some(PathBuf::from("/etc")),
                theme: "mc".to_owned(),
            }
        );
    }

    #[test]
    fn un_troisieme_repertoire_est_refuse() {
        // Il n'y a que deux panneaux : l'accepter en silence promettrait un panneau qui
        // n'existe pas.
        assert_eq!(
            parse(&arguments(&["/a", "/b", "/c"])),
            Request::TooManyDirectories("/c".to_owned())
        );
    }

    #[test]
    fn le_theme_se_choisit_par_son_nom() {
        // Le nom est retenu **tel qu'il est écrit**, alias compris : le résoudre ici
        // obligerait `Request` à porter un `AirTheme`, qui n'est pas comparable.
        for name in ["tv", "turbo-vision", "mc", "midnight-commander"] {
            assert_eq!(
                parse(&arguments(&["--theme", name])),
                Request::Browse {
                    left: None,
                    right: None,
                    theme: name.to_owned(),
                }
            );
        }
    }

    #[test]
    fn un_nom_de_theme_n_est_pas_pris_pour_un_repertoire() {
        assert_eq!(
            parse(&arguments(&["--theme", "tv", "/usr"])),
            Request::Browse {
                left: Some(PathBuf::from("/usr")),
                right: None,
                theme: "tv".to_owned(),
            }
        );
    }

    #[test]
    fn un_theme_inconnu_est_rapporte_tel_quel() {
        assert_eq!(
            parse(&arguments(&["--theme", "borland"])),
            Request::UnknownTheme("borland".to_owned())
        );
    }

    #[test]
    fn le_theme_sans_nom_est_refuse() {
        // Prendre le défaut en silence serait le pire des deux mondes, puisque
        // l'intention était explicite.
        assert_eq!(
            parse(&arguments(&["--theme"])),
            Request::UnknownTheme(String::new())
        );
    }

    #[test]
    fn l_aide_et_la_version_se_demandent_des_deux_facons() {
        assert_eq!(parse(&arguments(&["-h"])), Request::Usage);
        assert_eq!(parse(&arguments(&["--help"])), Request::Usage);
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
    fn l_aide_prime_sur_un_repertoire() {
        assert_eq!(parse(&arguments(&["/usr", "--help"])), Request::Usage);
    }

    #[test]
    fn le_mode_d_emploi_cite_les_touches_la_version_et_ce_qui_n_est_pas_fait() {
        let text = usage();
        assert!(text.contains("rmc [--theme <nom>] [--list-themes]"));
        assert!(
            text.contains("mc:<skin>"),
            "les skins compilés doivent être cités"
        );
        assert!(text.contains("Tab"));
        assert!(text.contains("F10"));
        // Un mode d'emploi qui taira ce que l'outil ne fait pas le fera découvrir à
        // l'usage : `rmc` n'exécute rien, et il le dit.
        assert!(text.contains("n'exécute aucun programme"));
        assert!(text.contains(env!("CARGO_PKG_VERSION")));
    }
}
