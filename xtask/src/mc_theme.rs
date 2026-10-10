// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La compilation d'un skin de Midnight Commander en thème air-tui.
//!
//! # Deux modèles qui ne se recouvrent pas
//!
//! `mc` nomme ses couleurs par **élément d'interface** — `menuhotsel`, `editframeactive`,
//! `dhotnormal` —, air-tui par **rôle sémantique** (ADR-008). Les deux ensembles se
//! recouvrent largement, pas entièrement : `mc` ne connaît ni bouton par défaut, ni
//! ascenseur, ni item de menu à la fois sélectionné et interdit.
//!
//! Les quatre rôles de **liste** vont dans l'autre sens, et c'est le seul endroit où
//! `mc` est plus riche qu'air-tui ne le demande : il nomme les quatre — ordinaire,
//! sous le curseur, marqué, les deux —, là où Turbo Vision n'en nomme que trois et laisse
//! le quatrième à composer. Deux panneaux de fichiers, c'est ce que `mc` sait faire.
//!
//! [`ROLE_SOURCES`] est donc une table explicite, et chaque entrée dit ce qu'elle est :
//! **transcrite** d'une clef du skin, ou **dérivée** d'une règle qui est écrite à côté
//! d'elle. Une dérivation nommée se discute ; une dérivation silencieuse se découvre des
//! mois plus tard devant un écran qui ne ressemble pas au skin.
//!
//! # Les attributs, et la couleur qu'on ne remplace pas
//!
//! Jusqu'au jalon M7g, deux choses faisaient refuser un skin, et c'étaient les deux
//! limites du modèle d'air-tui plutôt que des défauts du skin : un **attribut**
//! (`bold`, `underline`, `italic`) sur un rôle employé, et la couleur `default` — le fond
//! transparent de `mc`. L'ADR-008 les chiffrait depuis M7a : vingt skins pour le premier,
//! neuf pour le second, sur les trente-neuf livrés.
//!
//! Les deux sont levées. `AirColorPair` porte des attributs, `AirColor::TerminalDefault`
//! exprime la couleur du terminal, et la compilation les **transcrit** au lieu de refuser.
//!
//! Une règle a demandé un choix, et elle est écrite ici parce qu'elle ne va pas de soi :
//! pour un rôle **composé** de deux clefs, les attributs retenus sont ceux de la clef qui
//! donne l'**avant-plan**. Les trois attributs que les skins emploient portent sur le
//! dessin du caractère, et c'est l'avant-plan qui le décrit ; prendre ceux du fond mettrait
//! en gras un texte dont le skin ne dit rien.
//!
//! # Ce que la compilation refuse encore
//!
//! Une clef employée **absente** du skin — `gotar.ini` ne définit pas `[core] inputmark`
//! —, un glyphe de cadre qui n'est pas un caractère unique, et un attribut que `mc`
//! accepte sans qu'`AirAttributes` sache l'exprimer. Ce dernier cas ne peut pas se produire
//! aujourd'hui, et ce n'est pas une supposition : un test confronte les cinq noms
//! d'attributs de l'analyseur aux cinq champs que cette table sait écrire.

use crate::mc_skin::{Color, ColorPair, Skin};
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// D'où vient la couleur d'un rôle.
enum Source {
    /// Transcrite telle quelle d'une clef du skin.
    Direct {
        /// La section du skin.
        section: &'static str,
        /// La clef dans cette section.
        key: &'static str,
    },
    /// Composée : l'avant-plan d'une clef, le fond d'une autre.
    Composed {
        /// La clef dont on prend l'avant-plan.
        foreground: (&'static str, &'static str),
        /// La clef dont on prend le fond.
        background: (&'static str, &'static str),
    },
}

/// Les cinq attributs que `mc` sait écrire, et le champ d'`AirAttributes` qui les exprime.
///
/// Les cinq noms coïncident de part et d'autre, et cette table existe pour que la
/// coïncidence soit **vérifiée** plutôt qu'exploitée en silence : un test la confronte à
/// `mc_skin::ATTRIBUTE_NAMES`, de sorte qu'un nom ajouté d'un côté et pas de l'autre
/// échoue à l'essai au lieu de disparaître à la compilation d'un skin.
const ATTRIBUTE_FIELDS: &[(&str, &str)] = &[
    ("bold", "bold"),
    ("italic", "italic"),
    ("underline", "underline"),
    ("reverse", "reverse"),
    ("blink", "blink"),
];

/// Ce qu'un rôle vaut une fois le skin résolu : deux couleurs et des attributs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Presentation {
    /// L'avant-plan.
    foreground: Color,
    /// Le fond.
    background: Color,
    /// Les attributs, dans l'ordre où le skin les écrit.
    attributes: Vec<String>,
}

impl Presentation {
    /// Vrai si le couple s'écrit sous la forme courte `pair(f, b)`.
    ///
    /// C'est-à-dire : deux index et aucun attribut. C'est le cas de l'immense majorité des
    /// rôles, et la forme courte garde l'artefact lisible — trente-huit lignes de
    /// `AirColorPair::new(index(7), index(4))` ne se relisent pas.
    fn is_short_form(&self) -> bool {
        self.attributes.is_empty()
            && matches!(self.foreground, Color::Indexed(_))
            && matches!(self.background, Color::Indexed(_))
    }

    /// Les deux couleurs, pour les parcours qui ne regardent pas les attributs.
    fn colors(&self) -> [Color; 2] {
        [self.foreground, self.background]
    }
}

/// Un rôle d'air-tui, sa provenance, et ce que cette provenance doit au jugement.
struct RoleSource {
    /// Le nom du rôle, tel qu'`AirRole` l'écrit.
    role: &'static str,
    /// La provenance.
    source: Source,
    /// Vide si la correspondance va de soi ; sinon, ce que ce choix doit au jugement.
    ///
    /// Deux choses distinctes y passent, et le compte rendu ne les confond pas : le
    /// **choix de la clef** quand `mc` en offre plusieurs qui pourraient convenir, et la
    /// **règle de composition** quand `mc` ne nomme pas le rôle du tout.
    note: &'static str,
}

/// La table de correspondance, dans l'ordre des champs d'`AirPalette`.
///
/// L'ordre compte : l'émetteur produit les dix groupes de la structure en suivant cette
/// table, et un test vérifie que les trente-neuf rôles y sont, une fois chacun. Deux rôles
/// sont seuls dans leur champ : le fond du bureau en tête, l'ombre en queue.
const ROLE_SOURCES: &[RoleSource] = &[
    RoleSource {
        role: "DesktopBackground",
        source: Source::Direct {
            section: "core",
            key: "_default_",
        },
        note: "mc n'a pas de bureau — ses panneaux couvrent l'écran. \
                     `[core] _default_` est la couleur de fond de l'application",
    },
    RoleSource {
        role: "MenuBar",
        source: Source::Direct {
            section: "menu",
            key: "_default_",
        },
        note: "",
    },
    RoleSource {
        role: "MenuDisabled",
        source: Source::Direct {
            section: "menu",
            key: "menuinactive",
        },
        note: "",
    },
    RoleSource {
        role: "MenuHotkey",
        source: Source::Direct {
            section: "menu",
            key: "menuhot",
        },
        note: "",
    },
    RoleSource {
        role: "MenuSelected",
        source: Source::Direct {
            section: "menu",
            key: "menusel",
        },
        note: "",
    },
    RoleSource {
        role: "MenuSelectedDisabled",
        source: Source::Composed {
            foreground: ("core", "disabled"),
            background: ("menu", "menusel"),
        },
        note: "mc ne nomme pas ce rôle : l'avant-plan éteint de `[core] disabled` \
                     sur le fond de l'item sélectionné",
    },
    RoleSource {
        role: "MenuSelectedHotkey",
        source: Source::Direct {
            section: "menu",
            key: "menuhotsel",
        },
        note: "",
    },
    RoleSource {
        role: "StatusLine",
        source: Source::Direct {
            section: "buttonbar",
            key: "button",
        },
        note: "la barre de statut d'air-tui publie des touches de fonction : c'est \
                     la barre F1-F10 de mc, donc `[buttonbar] et` non [statusbar]",
    },
    RoleSource {
        role: "StatusDisabled",
        source: Source::Composed {
            foreground: ("core", "disabled"),
            background: ("buttonbar", "button"),
        },
        note: "mc ne nomme pas ce rôle : l'avant-plan éteint de `[core] disabled` sur \
                     le fond de la barre",
    },
    RoleSource {
        role: "StatusHotkey",
        source: Source::Direct {
            section: "buttonbar",
            key: "hotkey",
        },
        note: "",
    },
    RoleSource {
        role: "WindowFrameActive",
        source: Source::Direct {
            section: "editor",
            key: "editframeactive",
        },
        note: "une fenêtre air-tui est l'équivalent de la fenêtre d'édition de mc, \
                     pas de son dialogue",
    },
    RoleSource {
        role: "WindowFrameInactive",
        source: Source::Direct {
            section: "editor",
            key: "_default_",
        },
        note: "mc n'a qu'une fenêtre d'édition à la fois : son cadre inactif est \
                     le défaut de la section",
    },
    RoleSource {
        role: "WindowFrameIcon",
        source: Source::Direct {
            section: "editor",
            key: "editframeactive",
        },
        note: "mc donne le caractère de la case de fermeture ([widget-editor] \
                     window-close-char) mais pas sa couleur : celle du cadre actif",
    },
    RoleSource {
        role: "WindowTitle",
        source: Source::Direct {
            section: "editor",
            key: "editframeactive",
        },
        note: "mc écrit le nom du fichier dans le cadre, à la couleur du cadre",
    },
    RoleSource {
        role: "WindowInterior",
        source: Source::Direct {
            section: "editor",
            key: "_default_",
        },
        note: "",
    },
    RoleSource {
        role: "WindowText",
        source: Source::Direct {
            section: "editor",
            key: "_default_",
        },
        note: "",
    },
    RoleSource {
        role: "DialogFrameActive",
        source: Source::Direct {
            section: "dialog",
            key: "_default_",
        },
        note: "",
    },
    RoleSource {
        role: "DialogFrameInactive",
        source: Source::Composed {
            foreground: ("core", "disabled"),
            background: ("dialog", "_default_"),
        },
        note: "mc n'empile pas ses dialogues : l'avant-plan éteint sur le fond du \
                     dialogue",
    },
    RoleSource {
        role: "DialogFrameIcon",
        source: Source::Direct {
            section: "dialog",
            key: "dtitle",
        },
        note: "la couleur que mc réserve au titre d'un dialogue",
    },
    RoleSource {
        role: "DialogInterior",
        source: Source::Direct {
            section: "dialog",
            key: "_default_",
        },
        note: "",
    },
    RoleSource {
        role: "ButtonNormal",
        source: Source::Direct {
            section: "dialog",
            key: "_default_",
        },
        note: "un bouton de mc est un élément de dialogue : il en porte le défaut",
    },
    RoleSource {
        role: "ButtonDefault",
        source: Source::Direct {
            section: "dialog",
            key: "dfocus",
        },
        note: "mc ne distingue pas le bouton par défaut du bouton focalisé",
    },
    RoleSource {
        role: "ButtonFocused",
        source: Source::Direct {
            section: "dialog",
            key: "dfocus",
        },
        note: "",
    },
    RoleSource {
        role: "ButtonDisabled",
        source: Source::Direct {
            section: "core",
            key: "disabled",
        },
        note: "",
    },
    RoleSource {
        role: "ButtonHotkey",
        source: Source::Direct {
            section: "dialog",
            key: "dhotnormal",
        },
        note: "",
    },
    RoleSource {
        role: "ButtonShadow",
        source: Source::Direct {
            section: "core",
            key: "shadow",
        },
        note: "",
    },
    RoleSource {
        role: "ScrollBarPage",
        source: Source::Direct {
            section: "core",
            key: "_default_",
        },
        note: "mc n'a pas d'ascenseur : la zone parcourue prend le fond de \
                     l'application",
    },
    RoleSource {
        role: "ScrollBarControls",
        source: Source::Direct {
            section: "core",
            key: "selected",
        },
        note: "mc n'a pas d'ascenseur : ses flèches prennent la couleur du curseur",
    },
    RoleSource {
        role: "ListNormal",
        source: Source::Direct {
            section: "core",
            key: "_default_",
        },
        note: "un panneau de mc est l'application elle-même : ses lignes portent \
                     `[core] _default_`",
    },
    RoleSource {
        role: "ListSelected",
        source: Source::Direct {
            section: "core",
            key: "selected",
        },
        note: "",
    },
    RoleSource {
        role: "ListMarked",
        source: Source::Direct {
            section: "core",
            key: "marked",
        },
        note: "",
    },
    RoleSource {
        role: "ListMarkedSelected",
        source: Source::Direct {
            section: "core",
            key: "markselect",
        },
        note: "",
    },
    RoleSource {
        role: "EditorSelected",
        source: Source::Direct {
            section: "editor",
            key: "editmarked",
        },
        note: "",
    },
    RoleSource {
        role: "InputLineText",
        source: Source::Direct {
            section: "core",
            key: "input",
        },
        note: "",
    },
    RoleSource {
        role: "InputLineSelected",
        source: Source::Direct {
            section: "core",
            key: "inputmark",
        },
        note: "",
    },
    RoleSource {
        role: "ClusterNormal",
        source: Source::Direct {
            section: "dialog",
            key: "_default_",
        },
        note: "mc ne distingue pas les cases à cocher du fond de leur dialogue",
    },
    RoleSource {
        role: "ClusterSelected",
        source: Source::Direct {
            section: "dialog",
            key: "dfocus",
        },
        note: "",
    },
    RoleSource {
        role: "ClusterHotkey",
        source: Source::Direct {
            section: "dialog",
            key: "dhotnormal",
        },
        note: "",
    },
    RoleSource {
        role: "WindowShadow",
        source: Source::Direct {
            section: "core",
            key: "shadow",
        },
        note: "mc nomme l'ombre, et trente et un des trente-neuf skins livrés la \
                     définissent. Les huit autres l'héritent de [core] _default_, ce qui \
                     la rend invisible — c'est ce que mc fait aussi",
    },
];

/// Les bornes des dix groupes d'`AirPalette` qui vont par assiettes, dans l'ordre de
/// [`ROLE_SOURCES`] — le fond du bureau est seul dans son champ et n'en fait pas partie.
///
/// Le premier rôle est seul dans son champ ; les suivants vont par groupes, et l'émetteur
/// produit un tableau Rust par groupe.
const GROUPS: &[(&str, usize)] = &[
    ("menu", 6),
    ("status", 3),
    ("window", 6),
    ("dialog", 4),
    ("button", 6),
    ("scroll_bar", 2),
    ("list", 4),
    ("editor", 1),
    ("input_line", 2),
    ("cluster", 3),
];

/// Le rôle qui n'appartient à aucune assiette, posé seul après les groupes.
///
/// Comme le fond du bureau, qui est seul en tête : une assiette d'un élément ne nommerait
/// rien de plus que le champ lui-même.
const TRAILING_ROLE: &str = "shadow";

/// Les six clefs de la section `[Lines]` d'un cadre simple, dans l'ordre d'`AirFrameGlyphs`.
const SINGLE_LINE_KEYS: [&str; 6] = [
    "lefttop",
    "righttop",
    "leftbottom",
    "rightbottom",
    "horiz",
    "vert",
];

/// Les mêmes, pour le cadre double — `mc` préfixe ses doubles d'un `d`.
const DOUBLE_LINE_KEYS: [&str; 6] = [
    "dlefttop",
    "drighttop",
    "dleftbottom",
    "drightbottom",
    "dhoriz",
    "dvert",
];

/// Ce que la compilation d'un skin a produit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compilation {
    /// Le source Rust de l'artefact.
    pub source: String,
    /// Le nombre de rôles transcrits d'une seule clef du skin.
    pub transcribed: usize,
    /// Le nombre de rôles composés de deux clefs, faute d'être nommés par `mc`.
    pub composed: usize,
    /// Le nombre de correspondances qui ont demandé un choix, et qui le portent écrit.
    pub noted: usize,
    /// Le nombre de rôles qui portent un attribut du skin.
    pub attributed: usize,
    /// Le nombre de rôles dont une couleur est laissée au terminal.
    pub terminal_colors: usize,
    /// Le nombre de clefs du skin que ce modèle n'emploie pas.
    pub unused_keys: usize,
}

/// Compile un skin en artefact Rust.
///
/// # Errors
///
/// Si une clef employée est absente, si un glyphe de cadre n'est pas un caractère unique,
/// ou si une clef porte un attribut que `mc` accepte et qu'`AirAttributes` n'exprime pas.
pub fn compile(skin: &Skin, origin: &str, fingerprint: u64) -> Result<Compilation, String> {
    let mut pairs = Vec::new();
    let mut transcribed = 0usize;
    let mut composed = 0usize;
    let mut attributed = 0usize;
    let mut terminal_colors = 0usize;
    for entry in ROLE_SOURCES {
        let pair = resolve(skin, &entry.source)?;
        match entry.source {
            Source::Direct { .. } => transcribed = transcribed.saturating_add(1),
            Source::Composed { .. } => composed = composed.saturating_add(1),
        }
        if !pair.attributes.is_empty() {
            attributed = attributed.saturating_add(1);
        }
        if pair.colors().contains(&Color::Terminal) {
            terminal_colors = terminal_colors.saturating_add(1);
        }
        pairs.push((entry, pair));
    }
    let single = frame_glyphs(skin, &SINGLE_LINE_KEYS)?;
    let double = frame_glyphs(skin, &DOUBLE_LINE_KEYS)?;
    let close_mark = skin
        .raw("widget-editor", "window-close-char")
        .and_then(|value| value.chars().next())
        .unwrap_or('■');
    let description = skin.description().unwrap_or("skin sans description");

    let source = emit(&Emission {
        fingerprint,
        pairs: &pairs,
        single,
        double,
        close_mark,
        description,
        origin,
        transcribed,
        composed,
        attributed,
        terminal_colors,
    });
    Ok(Compilation {
        source,
        transcribed,
        composed,
        noted: ROLE_SOURCES
            .iter()
            .filter(|entry| !entry.note.is_empty())
            .count(),
        attributed,
        terminal_colors,
        unused_keys: skin.key_count().saturating_sub(used_key_count()),
    })
}

/// Le nombre de clefs **distinctes** du skin que ce modèle emploie.
///
/// Distinctes, et non le nombre de références : plusieurs rôles partagent une clef —
/// `[editor] _default_` sert à trois rôles, `[dialog] _default_` à trois autres. Compter
/// les références donnait quarante-cinq clefs employées pour un skin qui n'en porte que
/// trente et une, et donc zéro clef inemployée pour tout skin.
fn used_key_count() -> usize {
    let mut used: BTreeSet<(&str, &str)> = BTreeSet::new();
    for entry in ROLE_SOURCES {
        match entry.source {
            Source::Direct { section, key } => {
                used.insert((section, key));
            }
            Source::Composed {
                foreground,
                background,
            } => {
                used.insert(foreground);
                used.insert(background);
            }
        }
    }
    for key in SINGLE_LINE_KEYS.iter().chain(DOUBLE_LINE_KEYS.iter()) {
        used.insert(("Lines", key));
    }
    used.insert(("skin", "description"));
    used.insert(("widget-editor", "window-close-char"));
    used.len()
}

/// Résout la provenance d'un rôle en une présentation.
///
/// Pour un rôle **composé**, les attributs retenus sont ceux de la clef qui donne
/// l'avant-plan : les trois attributs que les skins emploient portent sur le dessin du
/// caractère, et c'est l'avant-plan qui le décrit.
fn resolve(skin: &Skin, source: &Source) -> Result<Presentation, String> {
    match *source {
        Source::Direct { section, key } => {
            let pair = skin.color_pair(section, key)?;
            expressible_attributes(&pair, section, key)?;
            Ok(Presentation {
                foreground: pair.foreground,
                background: pair.background,
                attributes: pair.attributes,
            })
        }
        Source::Composed {
            foreground: (foreground_section, foreground_key),
            background: (background_section, background_key),
        } => {
            let first = skin.color_pair(foreground_section, foreground_key)?;
            expressible_attributes(&first, foreground_section, foreground_key)?;
            let second = skin.color_pair(background_section, background_key)?;
            expressible_attributes(&second, background_section, background_key)?;
            Ok(Presentation {
                foreground: first.foreground,
                background: second.background,
                attributes: first.attributes,
            })
        }
    }
}

/// Vérifie que chaque attribut du couple a un champ dans `AirAttributes`.
///
/// # Errors
///
/// Si `mc` accepte un attribut qu'air-tui ne sait pas exprimer. Le cas ne peut pas se
/// produire tant que les deux tables portent les mêmes cinq noms, ce qu'un test vérifie —
/// mais il **refuse** plutôt que d'ignorer, parce qu'un attribut perdu en silence donne un
/// thème qui ne ressemble pas au skin, et que personne ne va le chercher là.
fn expressible_attributes(pair: &ColorPair, section: &str, key: &str) -> Result<(), String> {
    for attribute in &pair.attributes {
        if !ATTRIBUTE_FIELDS
            .iter()
            .any(|(name, _)| name == &attribute.as_str())
        {
            return Err(format!(
                "[{section}] {key} porte l'attribut « {attribute} », que mc accepte et \
                 qu'AirAttributes n'exprime pas : ce skin n'est pas importable en l'état."
            ));
        }
    }
    Ok(())
}

/// L'expression Rust des attributs d'une présentation, ou `None` s'il n'y en a pas.
fn attributes_expression(attributes: &[String]) -> Option<String> {
    let mut fields = String::new();
    for (name, field) in ATTRIBUTE_FIELDS {
        if attributes.iter().any(|posed| posed == name) {
            let _ = write!(fields, "{field}: true, ");
        }
    }
    if fields.is_empty() {
        return None;
    }
    Some(format!("AirAttributes {{ {fields}..AirAttributes::NONE }}"))
}

/// Les six glyphes d'un cadre.
///
/// Les six clefs sont **déstructurées** plutôt qu'indexées : l'indexation obligeait à un
/// `get_mut` dont la branche d'échec était inatteignable, donc impossible à couvrir.
fn frame_glyphs(skin: &Skin, keys: &[&str; 6]) -> Result<[char; 6], String> {
    let [
        top_left,
        top_right,
        bottom_left,
        bottom_right,
        horizontal,
        vertical,
    ] = *keys;
    Ok([
        skin.line_glyph(top_left)?,
        skin.line_glyph(top_right)?,
        skin.line_glyph(bottom_left)?,
        skin.line_glyph(bottom_right)?,
        skin.line_glyph(horizontal)?,
        skin.line_glyph(vertical)?,
    ])
}

/// La largeur à laquelle les commentaires émis sont repliés.
///
/// `rustfmt` ne replie pas les commentaires — l'option existe et n'est pas stable. C'est
/// donc à l'émetteur de le faire, sinon le fichier généré porte des lignes de deux cents
/// colonnes que personne ne relit.
const COMMENT_WIDTH: usize = 92;

/// Émet la fonction `glyphs()` de l'artefact.
///
/// Extraite d'[`emit`] pour la tenir sous la limite de lignes de clippy, et parce que le
/// jeu de glyphes est une sortie indépendante de la palette : les deux ne partagent que le
/// tampon qu'elles remplissent.
fn emit_glyphs(single: [char; 6], double: [char; 6], close_mark: char) -> String {
    let mut output = String::new();
    let _ = write!(
        output,
        "/// Le jeu de glyphes, compilé depuis la section `[Lines]` du skin.\n\
         ///\n\
         /// **Douze** glyphes ne viennent pas du skin, parce qu'aucun skin ne les décrit :\n\
         /// le remplissage du bureau — `mc` n'a pas de bureau, ses panneaux couvrent\n\
         /// l'écran —, les deux crochets de la case de fermeture, dont `mc` ne donne que la\n\
         /// marque, les deux flèches d'ascenseur et son curseur, que `mc` n'a pas, et les\n\
         /// six caractères des cases à cocher et des boutons radio.\n\
         ///\n\
         /// Ces six derniers restent donc ceux du thème par défaut, et ce n'est **pas** une\n\
         /// transcription : aucune des trente-neuf fournitures de `mc` 4.8.33 ne porte de\n\
         /// clef pour eux, ce qui a été vérifié et non supposé.\n\
         pub(crate) const fn glyphs() -> AirGlyphSet {{\n    \
         AirGlyphSet {{\n"
    );
    let _ = write!(output, "{}", emit_frame("window_frame", single));
    let _ = write!(output, "{}", emit_frame("dialog_frame", double));
    let _ = write!(
        output,
        "        desktop_fill: ' ',\n        \
         close_box_left: '[',\n        \
         close_box_mark: '{close_mark}', // [widget-editor] window-close-char\n        \
         close_box_right: ']',\n        \
         scroll_arrow_up: '▲',\n        \
         scroll_arrow_down: '▼',\n        \
         scroll_thumb: '█',\n        \
         check_box: AirClusterGlyphs::CHECK_BOX,\n        \
         radio_button: AirClusterGlyphs::RADIO_BUTTON,\n    \
         }}\n}}\n"
    );
    output
}

/// Tout ce dont l'émetteur a besoin.
///
/// Une structure plutôt que huit paramètres : au-delà de sept, l'ordre des arguments
/// devient une source de défauts que le compilateur ne voit pas — deux `usize` voisins
/// s'échangent sans un mot.
struct Emission<'compilation> {
    /// L'empreinte du skin d'origine.
    fingerprint: u64,
    /// Les rôles et leurs présentations, dans l'ordre de la table.
    pairs: &'compilation [(&'compilation RoleSource, Presentation)],
    /// Les six glyphes du cadre simple.
    single: [char; 6],
    /// Les six glyphes du cadre double.
    double: [char; 6],
    /// La marque de la case de fermeture.
    close_mark: char,
    /// La description que porte le skin.
    description: &'compilation str,
    /// Le chemin du skin, pour que l'artefact dise d'où il vient.
    origin: &'compilation str,
    /// Le nombre de rôles transcrits d'une clef.
    transcribed: usize,
    /// Le nombre de rôles composés de deux clefs.
    composed: usize,
    /// Le nombre de rôles qui portent un attribut du skin.
    attributed: usize,
    /// Le nombre de rôles dont une couleur est celle du terminal.
    terminal_colors: usize,
}

/// Produit le source Rust de l'artefact.
fn emit(emission: &Emission) -> String {
    let Emission {
        fingerprint,
        pairs,
        single,
        double,
        close_mark,
        description,
        origin,
        transcribed,
        composed,
        attributed,
        terminal_colors,
    } = *emission;
    let mut output = String::new();
    output.push_str(
        "// This Source Code Form is subject to the terms of the Mozilla Public\n\
         // License, v. 2.0. If a copy of the MPL was not distributed with this\n\
         // file, You can obtain one at https://mozilla.org/MPL/2.0/.\n\n",
    );
    let _ = write!(
        output,
        "//! Le thème Midnight Commander — **fichier généré, ne pas éditer**.\n\
         //!\n\
         //! Produit par `cargo xtask import-mc-skin` depuis « {description} », fichier\n\
         //! `{origin}`. {transcribed} rôles transcrits d'une clef, {composed} composés de deux.\n\
         //!\n\
         //! Empreinte FNV-1a du skin d'origine : `{fingerprint:#018x}`. C'est un repère de\n\
         //! dérive, pas une garantie cryptographique : `cargo xtask import-mc-skin --check`\n\
         //! la recalcule et distingue « ce n'est pas le même skin » de « l'artefact ne\n\
         //! correspond plus à ce skin ».\n\
         //!\n\
         //! Les index sont ceux de la palette du **terminal** : `mc` y a toujours tourné,\n\
         //! il n'y a donc aucune conversion à faire — contrairement aux attributs VGA de\n\
         //! Turbo Vision, où oublier l'échange du bleu et du rouge donnait un bureau rouge.\n\
         //!\n\
         //! # Les correspondances qui ont demandé un choix\n\
         //!\n\
         //! `mc` nomme ses couleurs par élément d'interface, air-tui par rôle sémantique :\n\
         //! les deux ensembles ne se recouvrent pas entièrement. Là où le choix de la clef\n\
         //! se discute, ou là où `mc` ne nomme pas le rôle du tout, la règle est écrite\n\
         //! ici — une dérivation nommée se discute, une dérivation silencieuse se\n\
         //! découvre des mois plus tard devant un écran qui ne ressemble pas au skin.\n\
         //!\n"
    );
    for (entry, _) in pairs {
        if entry.note.is_empty() {
            continue;
        }
        let _ = write!(output, "{}", comment_block(entry.role, entry.note));
    }
    if attributed > 0 || terminal_colors > 0 {
        let _ = write!(
            output,
            "//! # Ce que ce skin demande au-delà de deux couleurs\n\
             //!\n\
             //! {attributed} rôles portent un attribut du skin, et {terminal_colors} ont une\n\
             //! couleur laissée au terminal. Les deux étaient des motifs de refus avant le\n\
             //! jalon M7g ; ils sont désormais transcrits. Pour un rôle composé de deux clefs,\n\
             //! les attributs retenus sont ceux de la clef qui donne l'avant-plan.\n\
             //!\n"
        );
    }
    let attributes_import = if attributed > 0 {
        "{AirAttributes, AirColor}"
    } else {
        "AirColor"
    };
    let _ = write!(
        output,
        "\nuse crate::cell::{attributes_import};\n\
         use crate::theme::glyphs::{{AirClusterGlyphs, AirFrameGlyphs, AirGlyphSet}};\n\
         use crate::theme::palette::{{AirColorPair, AirPalette, AirPaletteParts}};\n\n"
    );
    let _ = write!(
        output,
        "/// La description que porte la section `[skin]` du fichier d'origine.\n\
         pub(crate) const DESCRIPTION: &str = \"{description}\";\n\n"
    );
    output.push_str(&emit_helpers(pairs));
    output.push_str(
        "/// La palette, compilée depuis le skin.\n         pub(crate) const fn palette() -> AirPalette {\n             AirPalette::from_parts(&AirPaletteParts {\n",
    );

    let mut index = 0usize;
    if let Some((entry, colors)) = pairs.first() {
        let _ = writeln!(
            output,
            "        desktop_background: {}, // {}",
            expression(colors),
            origin_of(entry)
        );
        index = 1;
    }
    for (group, count) in GROUPS {
        let _ = writeln!(output, "        {group}: [");
        for _ in 0..*count {
            if let Some((entry, colors)) = pairs.get(index) {
                let _ = writeln!(
                    output,
                    "            {}, // {}",
                    expression(colors),
                    origin_of(entry)
                );
            }
            index = index.saturating_add(1);
        }
        output.push_str("        ],\n");
    }
    // Le dernier rôle est seul dans son champ, comme le fond du bureau est seul en tête.
    if let Some((entry, colors)) = pairs.get(index) {
        let _ = writeln!(
            output,
            "        {TRAILING_ROLE}: {}, // {}",
            expression(colors),
            origin_of(entry)
        );
    }
    output.push_str("    })\n}\n\n");

    let _ = write!(output, "{}", emit_glyphs(single, double, close_mark));

    output
}

/// Émet les fonctions d'aide que l'artefact emploie, et **seulement** celles-là.
///
/// Une fonction d'aide inemployée dans un fichier généré est du code mort, que
/// `clippy -D warnings` refuse à juste titre. Les quatre se décident séparément depuis le
/// jalon M7g : un attribut suffit à envoyer un couple d'index sur la forme longue, qui
/// emploie `index` sans qu'aucune vraie couleur paraisse.
fn emit_helpers(pairs: &[(&RoleSource, Presentation)]) -> String {
    let uses = |wanted: fn(&Color) -> bool| {
        pairs
            .iter()
            .any(|(_, pair)| pair.colors().iter().any(wanted))
    };
    let mut output = String::new();
    if pairs.iter().any(|(_, pair)| pair.is_short_form()) {
        output.push_str(
            "/// Un couple de couleurs de la palette du terminal.\n             const fn pair(foreground: u8, background: u8) -> AirColorPair {\n                 AirColorPair::new(AirColor::Indexed(foreground), AirColor::Indexed(background))\n             }\n\n",
        );
    }
    let long_form_index = pairs.iter().any(|(_, pair)| {
        !pair.is_short_form()
            && pair
                .colors()
                .iter()
                .any(|color| matches!(color, Color::Indexed(_)))
    });
    if long_form_index {
        output.push_str(
            "/// Une couleur de la palette du terminal.\n             const fn index(value: u8) -> AirColor {\n                 AirColor::Indexed(value)\n             }\n\n",
        );
    }
    if uses(|color| matches!(color, Color::Rgb { .. })) {
        output.push_str(
            "/// Une vraie couleur, telle que le skin l'écrit en `#rrggbb`.\n             const fn rgb(red: u8, green: u8, blue: u8) -> AirColor {\n                 AirColor::Rgb { red, green, blue }\n             }\n\n",
        );
    }
    if uses(|color| matches!(color, Color::Terminal)) {
        output.push_str(
            "/// La couleur que le terminal emploie déjà — le `default` de mc.\n             const fn terminal() -> AirColor {\n                 AirColor::TerminalDefault\n             }\n\n",
        );
    }
    output
}

/// L'expression Rust d'une présentation.
///
/// La forme courte `pair(7, 4)` sert quand les deux couleurs sont des index et qu'aucun
/// attribut n'est posé — le cas de l'immense majorité des rôles. Une vraie couleur, la
/// couleur du terminal ou un attribut envoient sur la forme longue.
fn expression(pair: &Presentation) -> String {
    if let (Color::Indexed(first), Color::Indexed(second)) = (pair.foreground, pair.background)
        && pair.attributes.is_empty()
    {
        return format!("pair({first}, {second})");
    }
    let colors = format!(
        "AirColorPair::new({}, {})",
        color_expression(pair.foreground),
        color_expression(pair.background)
    );
    match attributes_expression(&pair.attributes) {
        Some(attributes) => format!("{colors}.with_attributes({attributes})"),
        None => colors,
    }
}

/// L'expression Rust d'une couleur.
fn color_expression(color: Color) -> String {
    match color {
        Color::Indexed(value) => format!("index({value})"),
        Color::Rgb { red, green, blue } => {
            format!("rgb(0x{red:02x}, 0x{green:02x}, 0x{blue:02x})")
        }
        Color::Terminal => "terminal()".to_owned(),
    }
}

/// La provenance d'un rôle, en une ligne, pour le commentaire de sa valeur.
///
/// Le détail d'un rôle composé — quelle clef donne l'avant-plan, laquelle donne le fond —
/// est dans l'en-tête du fichier : le mettre ici aussi ferait des lignes de cent dix
/// colonnes pour une information déjà écrite.
fn origin_of(entry: &RoleSource) -> String {
    let location = match entry.source {
        Source::Direct { section, key } => format!("[{section}] {key}"),
        Source::Composed {
            foreground: (foreground_section, foreground_key),
            background: (background_section, background_key),
        } => format!(
            "[{foreground_section}] {foreground_key} + [{background_section}] \
             {background_key}"
        ),
    };
    format!("{} ← {location}", entry.role)
}

/// Un bloc de commentaire de documentation, replié à [`COMMENT_WIDTH`].
fn comment_block(role: &str, derivation: &str) -> String {
    let mut block = String::new();
    let mut line = format!("//! - **`{role}`** —");
    for word in derivation.split_whitespace() {
        if line.chars().count().saturating_add(word.chars().count()) >= COMMENT_WIDTH {
            let _ = writeln!(block, "{line}");
            line.clear();
            line.push_str("//!  ");
        }
        line.push(' ');
        line.push_str(word);
    }
    let _ = writeln!(block, "{line}.");
    block
}

/// Les six champs d'un cadre.
fn emit_frame(field: &str, glyphs: [char; 6]) -> String {
    let names = [
        "top_left",
        "top_right",
        "bottom_left",
        "bottom_right",
        "horizontal",
        "vertical",
    ];
    let mut block = format!("        {field}: AirFrameGlyphs {{\n");
    for (name, glyph) in names.iter().zip(glyphs.iter()) {
        let _ = writeln!(block, "            {name}: '{glyph}',");
    }
    block.push_str("        },\n");
    block
}

#[cfg(test)]
mod tests {
    use super::{ATTRIBUTE_FIELDS, GROUPS, ROLE_SOURCES, Source, compile};
    use crate::mc_skin::{ColorPair, parse};

    /// Un skin complet : toutes les clefs que la table de correspondance emploie.
    fn complete_skin(foreground: &str, background: &str) -> String {
        format!(
            "[skin]
    description = Essai complet
[Lines]
    horiz = ─
    vert = │
    lefttop = ┌
    righttop = ┐
    leftbottom = └
    rightbottom = ┘
    dhoriz = ═
    dvert = ║
    dlefttop = ╔
    drighttop = ╗
    dleftbottom = ╚
    drightbottom = ╝
[core]
    _default_ = {foreground};{background}
    selected = black;cyan
    marked = yellow;blue
    markselect = yellow;cyan
    input = black;cyan
    inputmark = cyan;black
    disabled = gray;lightgray
    shadow = gray;black
[menu]
    _default_ = white;cyan
    menusel = white;black
    menuhot = yellow;cyan
    menuhotsel = yellow;black
    menuinactive = black;cyan
[buttonbar]
    hotkey = white;black
    button = black;cyan
[editor]
    _default_ = lightgray;blue
    editmarked = black;cyan
    editframeactive = white;
[dialog]
    _default_ = black;lightgray
    dfocus = black;cyan
    dhotnormal = blue;lightgray
    dtitle = blue;lightgray
[widget-editor]
    window-close-char = X
"
        )
    }

    #[test]
    fn la_table_couvre_les_trente_neuf_roles_une_fois_chacun() {
        let mut roles: Vec<&str> = ROLE_SOURCES.iter().map(|entry| entry.role).collect();
        let before = roles.len();
        roles.sort_unstable();
        roles.dedup();
        assert_eq!(roles.len(), before, "un rôle apparaît deux fois");
        assert_eq!(before, 39, "AirPalette porte trente-neuf rôles");
    }

    #[test]
    fn les_groupes_couvrent_la_table() {
        // Deux rôles sont seuls dans leur champ — le fond du bureau en tête, l'ombre en
        // queue — et les autres vont par groupes.
        let grouped = GROUPS
            .iter()
            .map(|(_, count)| *count)
            .fold(0usize, usize::saturating_add);
        assert_eq!(grouped.saturating_add(2), ROLE_SOURCES.len());
    }

    #[test]
    fn chaque_role_compose_porte_sa_regle() {
        for entry in ROLE_SOURCES {
            if matches!(entry.source, Source::Composed { .. }) {
                assert!(
                    !entry.note.is_empty(),
                    "{} est composé sans que la règle soit écrite",
                    entry.role
                );
            }
        }
    }

    #[test]
    fn un_skin_complet_se_compile() {
        let text = complete_skin("lightgray", "blue");
        let skin = parse(&text).expect("skin lisible");
        let compiled = compile(&skin, "essai.ini", 0x1234_5678_9abc_def0).expect("compilation");
        assert_eq!(compiled.transcribed, 36);
        assert_eq!(compiled.composed, 3);
        assert_eq!(
            compiled.transcribed.saturating_add(compiled.composed),
            ROLE_SOURCES.len()
        );
    }

    #[test]
    fn l_artefact_porte_l_en_tete_de_licence_et_se_declare_genere() {
        let skin = parse(&complete_skin("lightgray", "blue")).expect("skin lisible");
        let compiled = compile(&skin, "essai.ini", 0x1234_5678_9abc_def0).expect("compilation");
        assert!(compiled.source.starts_with("// This Source Code Form"));
        assert!(compiled.source.contains("fichier généré, ne pas éditer"));
        assert!(compiled.source.contains("essai.ini"), "l'origine est citée");
        assert!(
            compiled.source.contains("Essai complet"),
            "la description est citée"
        );
    }

    #[test]
    fn l_heritage_traverse_la_compilation() {
        // `editframeactive = white;` sur un [editor] en bleu : le cadre actif doit être
        // blanc sur bleu, soit pair(15, 4).
        let skin = parse(&complete_skin("lightgray", "blue")).expect("skin lisible");
        let compiled = compile(&skin, "essai.ini", 0x1234_5678_9abc_def0).expect("compilation");
        assert!(
            compiled
                .source
                .contains("pair(15, 4), // WindowFrameActive"),
            "{}",
            compiled.source
        );
    }

    #[test]
    fn un_role_compose_prend_l_avant_plan_de_l_un_et_le_fond_de_l_autre() {
        // MenuSelectedDisabled : l'avant-plan de [core] disabled (gray, 8) et le fond de
        // [menu] menusel (black, 0).
        let skin = parse(&complete_skin("lightgray", "blue")).expect("skin lisible");
        let compiled = compile(&skin, "essai.ini", 0x1234_5678_9abc_def0).expect("compilation");
        assert!(
            compiled
                .source
                .contains("pair(8, 0), // MenuSelectedDisabled"),
            "{}",
            compiled.source
        );
    }

    #[test]
    fn les_cinq_attributs_de_mc_ont_tous_un_champ_dans_air_attributes() {
        // C'est ce test qui rend le refus d'`expressible_attributes` inatteignable, et
        // c'est pour cela qu'il existe : la correspondance est vérifiée, pas supposée. Un
        // nom ajouté à l'analyseur sans champ ici échoue ici, pas devant un skin.
        for name in crate::mc_skin::ATTRIBUTE_NAMES {
            assert!(
                ATTRIBUTE_FIELDS
                    .iter()
                    .any(|(candidate, _)| candidate == name),
                "l'attribut « {name} » de mc n'a pas de champ dans AirAttributes"
            );
        }
        assert_eq!(
            ATTRIBUTE_FIELDS.len(),
            crate::mc_skin::ATTRIBUTE_NAMES.len(),
            "une des deux tables porte un nom que l'autre ignore"
        );
    }

    #[test]
    fn un_attribut_sur_un_role_employe_est_transcrit() {
        // C'était un refus jusqu'au jalon M7g, et c'est la raison pour laquelle vingt des
        // trente-neuf skins livrés n'étaient pas importables.
        let text = complete_skin("lightgray", "blue").replace(
            "    menusel = white;black",
            "    menusel = white;black;bold+underline",
        );
        let skin = parse(&text).expect("skin lisible");
        let compiled = compile(&skin, "essai.ini", 0x1234_5678_9abc_def0).expect("compilation");
        assert_eq!(compiled.attributed, 1, "seul MenuSelected en porte un");
        assert!(
            compiled.source.contains(
                "AirColorPair::new(index(15), index(0)).with_attributes(AirAttributes { \
                 bold: true, underline: true, ..AirAttributes::NONE })"
            ),
            "{}",
            compiled.source
        );
        assert!(
            compiled
                .source
                .contains("use crate::cell::{AirAttributes, AirColor};"),
            "l'artefact doit importer AirAttributes quand il en pose"
        );
    }

    #[test]
    fn un_attribut_envoie_un_couple_d_index_sur_la_forme_longue_sans_appeler_rgb() {
        // Les quatre fonctions d'aide se décident séparément : un attribut suffit à
        // employer `index` sans qu'aucune vraie couleur paraisse. Les émettre ensemble
        // laisserait `rgb` inemployée, et `clippy -D warnings` refuse le code mort.
        let text = complete_skin("lightgray", "blue").replace(
            "    menusel = white;black",
            "    menusel = white;black;italic",
        );
        let skin = parse(&text).expect("skin lisible");
        let compiled = compile(&skin, "essai.ini", 0x1234_5678_9abc_def0).expect("compilation");
        assert!(
            compiled.source.contains("const fn index("),
            "index attendue"
        );
        assert!(!compiled.source.contains("const fn rgb("), "rgb inemployée");
        assert!(
            !compiled.source.contains("const fn terminal("),
            "terminal inemployée"
        );
    }

    #[test]
    fn un_role_compose_prend_les_attributs_de_la_clef_d_avant_plan() {
        // MenuSelectedDisabled prend l'avant-plan de [core] disabled et le fond de [menu]
        // menusel. Un attribut porte sur le dessin du caractère : c'est l'avant-plan qui
        // le décrit, donc `bold` sur `menusel` ne doit **pas** l'atteindre.
        let text = complete_skin("lightgray", "blue")
            .replace(
                "    menusel = white;black",
                "    menusel = white;black;bold",
            )
            .replace(
                "    disabled = gray;lightgray",
                "    disabled = gray;lightgray;italic",
            );
        let skin = parse(&text).expect("skin lisible");
        let compiled = compile(&skin, "essai.ini", 0x1234_5678_9abc_def0).expect("compilation");
        let line = compiled
            .source
            .lines()
            .find(|line| line.contains("// MenuSelectedDisabled"))
            .unwrap_or_default();
        assert!(line.contains("italic: true"), "{line}");
        assert!(!line.contains("bold: true"), "{line}");
    }

    #[test]
    fn un_attribut_qu_air_attributes_n_exprime_pas_est_refuse() {
        // Inatteignable depuis un skin — l'analyseur refuse les noms inconnus avant — et
        // couvert directement : le refus doit exister, parce qu'un attribut perdu en
        // silence donne un thème qui ne ressemble pas au skin.
        let pair = ColorPair {
            foreground: crate::mc_skin::Color::Indexed(7),
            background: crate::mc_skin::Color::Indexed(4),
            attributes: vec!["clignotant".to_owned()],
        };
        let refusal =
            super::expressible_attributes(&pair, "core", "_default_").expect_err("refus attendu");
        assert!(refusal.contains("clignotant"), "{refusal}");
        assert!(refusal.contains("AirAttributes"), "{refusal}");
    }

    #[test]
    fn un_attribut_sur_un_role_inemploye_ne_derange_pas() {
        // La règle ne porte que sur ce que le modèle emploie : un skin qui met `bold` sur
        // la coloration des fichiers reste importable.
        let text = format!(
            "{}[filehighlight]\n    directory = white;blue;bold\n",
            complete_skin("lightgray", "blue")
        );
        let skin = parse(&text).expect("skin lisible");
        assert!(compile(&skin, "essai.ini", 0x1234_5678_9abc_def0).is_ok());
    }

    #[test]
    fn la_couleur_du_terminal_est_transcrite() {
        // `_default_ = lightgray;default` : le fond de l'application est celui du
        // terminal, et il se propage par héritage à tout ce qui ne nomme pas de fond.
        // C'était un refus jusqu'au jalon M7g — neuf skins sur trente-neuf.
        let text = complete_skin("lightgray", "default");
        let skin = parse(&text).expect("skin lisible");
        let compiled = compile(&skin, "essai.ini", 0x1234_5678_9abc_def0).expect("compilation");
        assert!(compiled.terminal_colors > 0, "au moins le fond du bureau");
        assert!(compiled.source.contains("const fn terminal("), "l'aide");
        assert!(
            compiled
                .source
                .contains("desktop_background: AirColorPair::new(index(7), terminal())"),
            "{}",
            compiled.source
        );
    }

    #[test]
    fn une_clef_employee_absente_herite_au_lieu_d_etre_refusee() {
        // C'était un refus jusqu'au jalon M7g, et c'est ce qui écartait vingt-trois des
        // trente-neuf skins livrés — pas une limite du modèle d'air-tui, un défaut de
        // l'analyseur : `mc` les affiche tous, en appliquant la cascade de son manuel.
        let text = complete_skin("lightgray", "blue").replace("    menusel = white;black\n", "");
        let skin = parse(&text).expect("skin lisible");
        let compiled = compile(&skin, "essai.ini", 0x1234_5678_9abc_def0).expect("compilation");
        // [menu] _default_ = white;cyan, soit pair(15, 6).
        assert!(
            compiled.source.contains("pair(15, 6), // MenuSelected"),
            "{}",
            compiled.source
        );
    }

    #[test]
    fn un_skin_sans_defaut_de_section_ni_de_core_est_refuse() {
        let text = complete_skin("lightgray", "blue")
            .replace("    _default_ = lightgray;blue\n", "")
            .replace("    _default_ = white;cyan\n", "");
        let skin = parse(&text).expect("skin lisible");
        let refusal =
            compile(&skin, "essai.ini", 0x1234_5678_9abc_def0).expect_err("refus attendu");
        assert!(refusal.contains("absent"), "{refusal}");
    }

    #[test]
    fn un_cadre_absent_est_refuse() {
        let text = complete_skin("lightgray", "blue").replace("    dvert = ║\n", "");
        let skin = parse(&text).expect("skin lisible");
        let refusal =
            compile(&skin, "essai.ini", 0x1234_5678_9abc_def0).expect_err("refus attendu");
        assert!(refusal.contains("dvert"), "{refusal}");
    }

    #[test]
    fn les_deux_familles_de_cadre_sont_distinctes() {
        let skin = parse(&complete_skin("lightgray", "blue")).expect("skin lisible");
        let compiled = compile(&skin, "essai.ini", 0x1234_5678_9abc_def0).expect("compilation");
        assert!(compiled.source.contains("top_left: '┌',"), "cadre simple");
        assert!(compiled.source.contains("top_left: '╔',"), "cadre double");
    }

    #[test]
    fn la_marque_de_fermeture_vient_du_skin() {
        let skin = parse(&complete_skin("lightgray", "blue")).expect("skin lisible");
        let compiled = compile(&skin, "essai.ini", 0x1234_5678_9abc_def0).expect("compilation");
        assert!(
            compiled.source.contains("close_box_mark: 'X'"),
            "{}",
            compiled.source
        );
    }

    #[test]
    fn un_skin_indexe_n_emet_que_la_fonction_d_aide_courte() {
        // Une fonction d'aide inemployée dans un fichier généré est du code mort, que
        // `clippy -D warnings` refuse.
        let skin = parse(&complete_skin("lightgray", "blue")).expect("skin lisible");
        let compiled = compile(&skin, "essai.ini", 0x1234_5678_9abc_def0).expect("compilation");
        assert!(compiled.source.contains("const fn pair("));
        assert!(!compiled.source.contains("const fn rgb("));
    }

    #[test]
    fn un_skin_en_vraies_couleurs_emet_les_fonctions_longues() {
        let text = complete_skin("#d3d7cf", "#2e3436");
        let skin = parse(&text).expect("skin lisible");
        let compiled = compile(&skin, "essai.ini", 0x1234_5678_9abc_def0).expect("compilation");
        assert!(compiled.source.contains("const fn rgb("));
        assert!(
            compiled.source.contains("rgb(0xd3, 0xd7, 0xcf)"),
            "{}",
            compiled.source
        );
    }

    #[test]
    fn les_clefs_inemployees_sont_comptees() {
        let text = format!(
            "{}[viewer]\n    viewbold = yellow;blue\n    viewselected = yellow;cyan\n",
            complete_skin("lightgray", "blue")
        );
        let skin = parse(&text).expect("skin lisible");
        let compiled = compile(&skin, "essai.ini", 0x1234_5678_9abc_def0).expect("compilation");
        assert_eq!(compiled.unused_keys, 2, "les deux clefs de [viewer]");
    }

    #[test]
    fn l_artefact_porte_l_empreinte_de_son_skin() {
        let skin = parse(&complete_skin("lightgray", "blue")).expect("skin lisible");
        let compiled = compile(&skin, "essai.ini", 0x1234_5678_9abc_def0).expect("compilation");
        assert!(
            compiled.source.contains("0x123456789abcdef0"),
            "l'empreinte doit être écrite dans l'artefact"
        );
    }

    #[test]
    fn deux_skins_distincts_donnent_deux_artefacts_distincts() {
        let skin = parse(&complete_skin("lightgray", "blue")).expect("skin lisible");
        let first = compile(&skin, "essai.ini", 1).expect("compilation");
        let second = compile(&skin, "essai.ini", 2).expect("compilation");
        assert_ne!(first.source, second.source, "l'empreinte les sépare");
    }

    #[test]
    fn la_compilation_est_deterministe() {
        let skin = parse(&complete_skin("lightgray", "blue")).expect("skin lisible");
        let first = compile(&skin, "essai.ini", 7).expect("compilation");
        let second = compile(&skin, "essai.ini", 7).expect("compilation");
        assert_eq!(first.source, second.source);
    }
}
