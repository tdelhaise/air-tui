// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le quantificateur de couleurs — d'un thème en vraies couleurs vers ce que le terminal
//! sait afficher.
//!
//! # Pourquoi il est requis et non optionnel
//!
//! L'ADR-008 le dit, et la raison est concrète : un thème s'écrit en RGB, mais une console
//! Linux n'a que seize couleurs. Sans quantification, un thème soigné s'affiche en couleurs
//! arbitraires — ou pas du tout — sur la moitié des terminaux visés. Ce n'est pas une
//! finition, c'est la condition pour qu'un thème soit lisible ailleurs que sur la machine
//! de son auteur.
//!
//! # La métrique employée, et ses limites
//!
//! La distance est euclidienne dans l'espace RGB, au carré. Ce n'est **pas** perceptuel :
//! une distance CIE donnerait de meilleurs choix sur les teintes voisines, au prix de
//! tables de conversion et d'une arithmétique en flottant. Pour ramener seize à
//! deux cent cinquante-six valeurs vers une palette fixe, l'écart entre les deux métriques
//! ne change le résultat que sur des cas limites, et l'ADR-005 interdit la dépendance qui
//! rendrait l'alternative raisonnable.

use crate::cell::AirColor;

/// Les niveaux de chaque composante du cube six par six par six d'xterm.
const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
/// Le premier indice du cube dans la palette de deux cent cinquante-six couleurs.
const CUBE_FIRST_INDEX: u16 = 16;
/// Le premier indice de la rampe de gris.
const GRAYSCALE_FIRST_INDEX: u16 = 232;
/// Le nombre de niveaux de la rampe de gris.
const GRAYSCALE_LEVEL_COUNT: u16 = 24;

/// Les seize couleurs ANSI historiques, telles qu'un terminal les rend par défaut.
///
/// Ce sont les valeurs d'xterm, et non celles du VGA d'origine : c'est contre elles que la
/// quantification doit choisir, puisque c'est elles que l'utilisateur verra.
const ANSI_COLORS: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (128, 0, 0),
    (0, 128, 0),
    (128, 128, 0),
    (0, 0, 128),
    (128, 0, 128),
    (0, 128, 128),
    (192, 192, 192),
    (128, 128, 128),
    (255, 0, 0),
    (0, 255, 0),
    (255, 255, 0),
    (0, 0, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
];

/// Ce que le terminal sait afficher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AirColorDepth {
    /// Vingt-quatre bits par cellule.
    TrueColor,
    /// La palette de deux cent cinquante-six couleurs.
    #[default]
    Indexed256,
    /// Les seize couleurs ANSI — une console Linux sans plus.
    Indexed16,
}

impl AirColorDepth {
    /// Déduit la profondeur des variables d'environnement.
    ///
    /// `COLORTERM` est la seule indication fiable des vraies couleurs, et `TERM` celle des
    /// deux cent cinquante-six. En l'absence des deux, on suppose **seize** : se tromper
    /// par excès donne des couleurs fausses, se tromper par défaut donne des couleurs
    /// ternes. Le second est réparable par l'utilisateur, le premier ne se voit pas.
    #[must_use]
    pub fn from_environment() -> Self {
        let color_term = std::env::var("COLORTERM").ok();
        let term = std::env::var("TERM").ok();
        Self::from_variables(color_term.as_deref(), term.as_deref())
    }

    /// La même déduction, à partir de valeurs explicites.
    ///
    /// Séparée pour être testable sans toucher à l'environnement du processus — un test
    /// qui modifie `TERM` modifie l'état global et se met à dépendre de l'ordre
    /// d'exécution.
    #[must_use]
    pub fn from_variables(color_term: Option<&str>, term: Option<&str>) -> Self {
        if let Some(value) = color_term
            && (value.eq_ignore_ascii_case("truecolor") || value.eq_ignore_ascii_case("24bit"))
        {
            return Self::TrueColor;
        }
        if let Some(value) = term
            && (value.contains("256color") || value.contains("direct"))
        {
            return Self::Indexed256;
        }
        Self::Indexed16
    }
}

/// La distance au carré entre deux couleurs, dans l'espace RGB.
///
/// Les composantes passent en `i32` avant la soustraction : la faire sur des `u8`
/// donnerait une différence saturée, donc une distance fausse dès que la seconde couleur
/// est plus claire que la première.
fn squared_distance(first: (u8, u8, u8), second: (u8, u8, u8)) -> i32 {
    let (first_red, first_green, first_blue) = first;
    let (second_red, second_green, second_blue) = second;
    let red = i32::from(first_red).saturating_sub(i32::from(second_red));
    let green = i32::from(first_green).saturating_sub(i32::from(second_green));
    let blue = i32::from(first_blue).saturating_sub(i32::from(second_blue));
    red.saturating_mul(red)
        .saturating_add(green.saturating_mul(green))
        .saturating_add(blue.saturating_mul(blue))
}

/// L'indice du niveau du cube le plus proche d'une composante.
fn nearest_cube_level(component: u8) -> u16 {
    let mut best_index = 0_u16;
    let mut best_distance = i32::MAX;
    for (index, level) in CUBE_LEVELS.iter().enumerate() {
        let difference = i32::from(component).saturating_sub(i32::from(*level));
        let distance = difference.saturating_mul(difference);
        if distance < best_distance {
            best_distance = distance;
            best_index = u16::try_from(index).unwrap_or(0);
        }
    }
    best_index
}

/// La couleur d'un niveau du cube, par son indice.
fn cube_level_value(index: u16) -> u8 {
    let position = usize::from(index);
    match CUBE_LEVELS.get(position) {
        Some(level) => *level,
        // Inatteignable : les indices viennent de `nearest_cube_level`, borné à six.
        None => 0,
    }
}

/// La couleur d'un indice de la palette de deux cent cinquante-six.
///
/// Les seize premiers viennent de la table ANSI, le cube et la rampe de gris sont
/// calculés. Exposée parce qu'un thème peut avoir besoin de savoir à quoi ressemble un
/// indice avant de le choisir.
#[must_use]
pub fn indexed_to_rgb(index: u8) -> (u8, u8, u8) {
    let value = u16::from(index);
    if value < CUBE_FIRST_INDEX {
        return match ANSI_COLORS.get(usize::from(value)) {
            Some(color) => *color,
            // Inatteignable : la table a seize entrées et `value` est inférieur à seize.
            None => (0, 0, 0),
        };
    }
    if value >= GRAYSCALE_FIRST_INDEX {
        let step = value.saturating_sub(GRAYSCALE_FIRST_INDEX);
        let level = 8_u16.saturating_add(step.saturating_mul(10));
        let clamped = u8::try_from(level).unwrap_or(u8::MAX);
        return (clamped, clamped, clamped);
    }
    let offset = value.saturating_sub(CUBE_FIRST_INDEX);
    let red = offset / 36;
    let green = (offset % 36) / 6;
    let blue = offset % 6;
    (
        cube_level_value(red),
        cube_level_value(green),
        cube_level_value(blue),
    )
}

/// L'indice de la palette de deux cent cinquante-six le plus proche d'une couleur RGB.
fn nearest_indexed_256(color: (u8, u8, u8)) -> u8 {
    let (red, green, blue) = color;
    // Le candidat du cube.
    let cube_index = CUBE_FIRST_INDEX
        .saturating_add(nearest_cube_level(red).saturating_mul(36))
        .saturating_add(nearest_cube_level(green).saturating_mul(6))
        .saturating_add(nearest_cube_level(blue));
    let cube_distance = squared_distance(color, indexed_to_rgb(narrow(cube_index)));

    // Le candidat de la rampe de gris : la moyenne des composantes, ramenée au palier le
    // plus proche. Une couleur franchement colorée y perdra, et c'est voulu.
    let average = (u16::from(red)
        .saturating_add(u16::from(green))
        .saturating_add(u16::from(blue)))
        / 3;
    let step = average.saturating_sub(8) / 10;
    let bounded = step.min(GRAYSCALE_LEVEL_COUNT.saturating_sub(1));
    let gray_index = GRAYSCALE_FIRST_INDEX.saturating_add(bounded);
    let gray_distance = squared_distance(color, indexed_to_rgb(narrow(gray_index)));

    if gray_distance < cube_distance {
        narrow(gray_index)
    } else {
        narrow(cube_index)
    }
}

/// L'indice ANSI le plus proche d'une couleur RGB.
fn nearest_indexed_16(color: (u8, u8, u8)) -> u8 {
    let mut best_index = 0_u8;
    let mut best_distance = i32::MAX;
    for (index, candidate) in ANSI_COLORS.iter().enumerate() {
        let distance = squared_distance(color, *candidate);
        if distance < best_distance {
            best_distance = distance;
            best_index = u8::try_from(index).unwrap_or(0);
        }
    }
    best_index
}

/// Ramène un indice de palette dans un octet.
///
/// Les indices calculés ici ne dépassent jamais deux cent cinquante-cinq ; la borne est là
/// pour ne pas écrire un indice tronqué si une table changeait sans que cette ligne soit
/// relue.
fn narrow(index: u16) -> u8 {
    u8::try_from(index).unwrap_or(u8::MAX)
}

/// Ramène une couleur à ce que le terminal sait afficher.
///
/// Une couleur déjà représentable est rendue telle quelle : la quantification ne dégrade
/// jamais ce qui n'en a pas besoin.
#[must_use]
pub fn quantize(color: AirColor, depth: AirColorDepth) -> AirColor {
    match depth {
        AirColorDepth::TrueColor => color,
        AirColorDepth::Indexed256 => match color {
            AirColor::Indexed(_) => color,
            AirColor::Rgb { red, green, blue } => {
                AirColor::Indexed(nearest_indexed_256((red, green, blue)))
            }
        },
        AirColorDepth::Indexed16 => match color {
            AirColor::Indexed(index) if u16::from(index) < CUBE_FIRST_INDEX => color,
            AirColor::Indexed(index) => {
                AirColor::Indexed(nearest_indexed_16(indexed_to_rgb(index)))
            }
            AirColor::Rgb { red, green, blue } => {
                AirColor::Indexed(nearest_indexed_16((red, green, blue)))
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ANSI_COLORS, AirColorDepth, CUBE_LEVELS, cube_level_value, indexed_to_rgb,
        nearest_cube_level, nearest_indexed_16, nearest_indexed_256, quantize, squared_distance,
    };
    use crate::cell::AirColor;

    fn rgb(red: u8, green: u8, blue: u8) -> AirColor {
        AirColor::Rgb { red, green, blue }
    }

    // ------------------------------------------------- profondeur

    #[test]
    fn colorterm_annonce_les_vraies_couleurs() {
        assert_eq!(
            AirColorDepth::from_variables(Some("truecolor"), None),
            AirColorDepth::TrueColor
        );
        assert_eq!(
            AirColorDepth::from_variables(Some("24bit"), None),
            AirColorDepth::TrueColor
        );
        // La casse ne doit pas décider de la profondeur.
        assert_eq!(
            AirColorDepth::from_variables(Some("TrueColor"), None),
            AirColorDepth::TrueColor
        );
    }

    #[test]
    fn term_annonce_les_deux_cent_cinquante_six() {
        assert_eq!(
            AirColorDepth::from_variables(None, Some("xterm-256color")),
            AirColorDepth::Indexed256
        );
        assert_eq!(
            AirColorDepth::from_variables(None, Some("screen-256color")),
            AirColorDepth::Indexed256
        );
    }

    #[test]
    fn sans_indication_on_suppose_seize_couleurs() {
        // Se tromper par excès donne des couleurs fausses ; se tromper par défaut donne
        // des couleurs ternes, et l'utilisateur peut le corriger.
        assert_eq!(
            AirColorDepth::from_variables(None, None),
            AirColorDepth::Indexed16
        );
        assert_eq!(
            AirColorDepth::from_variables(None, Some("linux")),
            AirColorDepth::Indexed16
        );
        assert_eq!(
            AirColorDepth::from_variables(Some("8bit"), Some("vt100")),
            AirColorDepth::Indexed16
        );
    }

    #[test]
    fn la_profondeur_par_defaut_est_la_palette_de_deux_cent_cinquante_six() {
        assert_eq!(AirColorDepth::default(), AirColorDepth::Indexed256);
    }

    #[test]
    fn la_deduction_depuis_l_environnement_ne_panique_pas() {
        // On ne peut pas asserter le résultat sans modifier l'environnement du processus,
        // ce qui rendrait le test dépendant de l'ordre d'exécution. On vérifie qu'elle
        // rend une des trois valeurs.
        let deduced = AirColorDepth::from_environment();
        assert!(matches!(
            deduced,
            AirColorDepth::TrueColor | AirColorDepth::Indexed256 | AirColorDepth::Indexed16
        ));
    }

    // ------------------------------------------------- distance

    #[test]
    fn la_distance_est_symetrique_et_nulle_sur_l_identite() {
        assert_eq!(squared_distance((1, 2, 3), (1, 2, 3)), 0);
        assert_eq!(
            squared_distance((0, 0, 0), (255, 255, 255)),
            squared_distance((255, 255, 255), (0, 0, 0))
        );
    }

    #[test]
    fn la_distance_ne_sature_pas_quand_la_seconde_couleur_est_plus_claire() {
        // Le piège d'une soustraction sur des `u8` : elle saturerait à zéro et la distance
        // serait fausse dans un sens sur deux.
        assert_eq!(squared_distance((0, 0, 0), (10, 0, 0)), 100);
        assert_eq!(squared_distance((10, 0, 0), (0, 0, 0)), 100);
    }

    // ------------------------------------------------- cube

    #[test]
    fn chaque_niveau_du_cube_est_son_propre_plus_proche() {
        for (index, level) in CUBE_LEVELS.iter().enumerate() {
            assert_eq!(
                usize::from(nearest_cube_level(*level)),
                index,
                "pour le niveau {level}"
            );
        }
    }

    #[test]
    fn un_niveau_intermediaire_choisit_le_plus_proche() {
        // 120 est entre 95 et 135, plus près de 135.
        assert_eq!(cube_level_value(nearest_cube_level(120)), 135);
        // 100 est plus près de 95.
        assert_eq!(cube_level_value(nearest_cube_level(100)), 95);
    }

    #[test]
    fn un_indice_de_niveau_hors_domaine_rend_zero() {
        // Inatteignable depuis `nearest_cube_level` ; couvert directement.
        assert_eq!(cube_level_value(99), 0);
    }

    // ------------------------------------------------- palette 256

    #[test]
    fn les_seize_premiers_indices_viennent_de_la_table_ansi() {
        for (index, expected) in ANSI_COLORS.iter().enumerate() {
            let narrowed = u8::try_from(index).expect("seize entrées tiennent dans un octet");
            assert_eq!(indexed_to_rgb(narrowed), *expected, "pour l'indice {index}");
        }
    }

    #[test]
    fn le_cube_se_decode_composante_par_composante() {
        // 16 est le coin noir du cube, 231 le coin blanc.
        assert_eq!(indexed_to_rgb(16), (0, 0, 0));
        assert_eq!(indexed_to_rgb(231), (255, 255, 255));
        // 196 = 16 + 36*5 : rouge pur du cube.
        assert_eq!(indexed_to_rgb(196), (255, 0, 0));
        // 46 = 16 + 6*5 : vert pur.
        assert_eq!(indexed_to_rgb(46), (0, 255, 0));
    }

    #[test]
    fn la_rampe_de_gris_se_decode_par_paliers_de_dix() {
        assert_eq!(indexed_to_rgb(232), (8, 8, 8));
        assert_eq!(indexed_to_rgb(233), (18, 18, 18));
        assert_eq!(indexed_to_rgb(255), (238, 238, 238));
    }

    #[test]
    fn le_rouge_pur_se_quantifie_sur_le_rouge_du_cube() {
        assert_eq!(nearest_indexed_256((255, 0, 0)), 196);
        assert_eq!(nearest_indexed_256((255, 255, 255)), 231);
        assert_eq!(nearest_indexed_256((0, 0, 0)), 16);
    }

    #[test]
    fn un_gris_prefere_la_rampe_au_cube() {
        // C'est tout l'intérêt de la rampe : le cube n'a que six paliers par composante et
        // rendrait les gris par bonds visibles.
        assert_eq!(
            indexed_to_rgb(nearest_indexed_256((128, 128, 128))),
            (128, 128, 128)
        );
        assert_eq!(
            indexed_to_rgb(nearest_indexed_256((18, 18, 18))),
            (18, 18, 18)
        );
    }

    #[test]
    fn une_couleur_franche_prefere_le_cube_a_la_rampe() {
        let chosen = nearest_indexed_256((200, 20, 20));
        assert!(
            chosen < 232,
            "la rampe de gris a été choisie à tort : {chosen}"
        );
    }

    // ------------------------------------------------- palette 16

    #[test]
    fn chaque_couleur_ansi_est_son_propre_plus_proche() {
        for (index, color) in ANSI_COLORS.iter().enumerate() {
            let expected = u8::try_from(index).expect("seize entrées tiennent dans un octet");
            assert_eq!(
                nearest_indexed_16(*color),
                expected,
                "pour l'indice {index}"
            );
        }
    }

    #[test]
    fn un_rouge_sombre_choisit_le_rouge_sombre_et_non_le_vif() {
        // 170 est le rouge du VGA : plus près de 128 que de 255.
        assert_eq!(nearest_indexed_16((170, 0, 0)), 1);
        assert_eq!(nearest_indexed_16((255, 10, 10)), 9);
    }

    // ------------------------------------------------- quantification

    #[test]
    fn en_vraies_couleurs_rien_n_est_quantifie() {
        let color = rgb(1, 2, 3);
        assert_eq!(quantize(color, AirColorDepth::TrueColor), color);
        let indexed = AirColor::Indexed(200);
        assert_eq!(quantize(indexed, AirColorDepth::TrueColor), indexed);
    }

    #[test]
    fn un_indice_est_laisse_tel_quel_en_palette_de_deux_cent_cinquante_six() {
        // La quantification ne dégrade jamais ce qui n'en a pas besoin.
        let indexed = AirColor::Indexed(200);
        assert_eq!(quantize(indexed, AirColorDepth::Indexed256), indexed);
    }

    #[test]
    fn une_couleur_rgb_devient_un_indice_en_palette_de_deux_cent_cinquante_six() {
        assert_eq!(
            quantize(rgb(255, 0, 0), AirColorDepth::Indexed256),
            AirColor::Indexed(196)
        );
    }

    #[test]
    fn les_seize_premiers_indices_traversent_la_quantification_en_seize() {
        for index in 0..16_u8 {
            let color = AirColor::Indexed(index);
            assert_eq!(quantize(color, AirColorDepth::Indexed16), color);
        }
    }

    #[test]
    fn un_indice_eleve_est_ramene_dans_les_seize() {
        // 196 est le rouge pur du cube ; il doit atterrir sur le rouge vif ANSI.
        assert_eq!(
            quantize(AirColor::Indexed(196), AirColorDepth::Indexed16),
            AirColor::Indexed(9)
        );
        // 232 est presque noir.
        assert_eq!(
            quantize(AirColor::Indexed(232), AirColorDepth::Indexed16),
            AirColor::Indexed(0)
        );
    }

    #[test]
    fn une_couleur_rgb_est_ramenee_dans_les_seize() {
        assert_eq!(
            quantize(rgb(0, 0, 255), AirColorDepth::Indexed16),
            AirColor::Indexed(12)
        );
    }

    #[test]
    fn la_quantification_est_idempotente() {
        // Quantifier deux fois ne doit pas dériver : c'est ce qui garantit qu'un rendu
        // partiel et un rendu complet donnent la même image.
        for depth in [
            AirColorDepth::TrueColor,
            AirColorDepth::Indexed256,
            AirColorDepth::Indexed16,
        ] {
            for color in [rgb(200, 20, 20), rgb(128, 128, 128), AirColor::Indexed(196)] {
                let once = quantize(color, depth);
                assert_eq!(quantize(once, depth), once, "pour {color:?} en {depth:?}");
            }
        }
    }

    #[test]
    fn toutes_les_couleurs_rgb_d_un_echantillon_se_quantifient_sans_paniquer() {
        // Balayage grossier des seize millions de couleurs : la quantification doit être
        // totale, et ne jamais rendre un indice qui n'existe pas.
        for red in (0..=255_u16).step_by(17) {
            for green in (0..=255_u16).step_by(51) {
                for blue in (0..=255_u16).step_by(51) {
                    let color = rgb(
                        u8::try_from(red).unwrap_or(0),
                        u8::try_from(green).unwrap_or(0),
                        u8::try_from(blue).unwrap_or(0),
                    );
                    assert!(matches!(
                        quantize(color, AirColorDepth::Indexed256),
                        AirColor::Indexed(_)
                    ));
                    let reduced = quantize(color, AirColorDepth::Indexed16);
                    assert!(
                        matches!(reduced, AirColor::Indexed(index) if index < 16),
                        "une couleur des seize était attendue : {reduced:?}"
                    );
                }
            }
        }
    }
}
