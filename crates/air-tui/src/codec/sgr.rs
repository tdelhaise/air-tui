// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! L'encodeur SGR — la présentation d'une cellule vers les octets du terminal.

use air_tui_platform::Encoder;

use crate::cell::{AirAttributes, AirCell, AirColor};
use crate::codec::decimal::push_decimal;

/// La présentation d'une cellule : tout sauf son caractère.
///
/// Séparée de [`AirCell`] parce que c'est exactement ce que l'encodeur traduit : deux
/// cellules de caractères différents mais de même présentation n'ont besoin que d'une
/// séquence SGR, et c'est ce qui divise par dix le volume émis sur un écran plein.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirCellPresentation {
    /// La couleur du caractère.
    pub foreground: AirColor,
    /// La couleur du fond.
    pub background: AirColor,
    /// Les attributs de rendu.
    pub attributes: AirAttributes,
}

impl From<AirCell> for AirCellPresentation {
    fn from(cell: AirCell) -> Self {
        Self {
            foreground: cell.foreground,
            background: cell.background,
            attributes: cell.attributes,
        }
    }
}

/// Encode une présentation de cellule en séquence SGR.
///
/// L'encodeur **retient la dernière présentation émise** et n'écrit rien quand elle ne
/// change pas. [`Self::forget`] le remet à zéro : à appeler au début de chaque rendu,
/// puisque le terminal, lui, ne se souvient de rien entre deux images.
#[derive(Debug, Default)]
pub struct AirSgrEncoder {
    /// La dernière présentation écrite, pour ne pas la réémettre.
    last_written: Option<AirCellPresentation>,
}

impl AirSgrEncoder {
    /// Un encodeur neuf, qui n'a encore rien écrit.
    #[must_use]
    pub const fn new() -> Self {
        Self { last_written: None }
    }

    /// Oublie ce qui a été écrit, pour que la prochaine présentation soit émise en entier.
    pub const fn forget(&mut self) {
        self.last_written = None;
    }

    /// Ajoute le paramètre SGR d'une couleur.
    ///
    /// `is_foreground` choisit entre les paramètres du caractère et ceux du fond. Une
    /// couleur nommée s'émet en notation étendue — 38 ou 48, puis `5;index` pour la
    /// palette du terminal ou `2;r;g;b` pour les vraies couleurs.
    ///
    /// [`AirColor::TerminalDefault`] emploie 39 et 49, qui ne sont pas de la même nature :
    /// ils **annulent** une couleur au lieu d'en poser une, et rendent donc au terminal
    /// celle qu'il employait. C'est ce qui rend un thème transparent sur le fond que
    /// l'utilisateur a choisi — et c'est pour cela qu'il n'y a pas d'index « par défaut »
    /// à écrire à la place.
    fn push_color(output: &mut Vec<u8>, color: AirColor, is_foreground: bool) {
        let extended = if is_foreground { 38 } else { 48 };
        let restored = if is_foreground { 39 } else { 49 };
        output.push(b';');
        match color {
            AirColor::Indexed(index) => {
                push_decimal(output, extended);
                output.extend_from_slice(b";5;");
                push_decimal(output, u16::from(index));
            }
            AirColor::Rgb { red, green, blue } => {
                push_decimal(output, extended);
                output.extend_from_slice(b";2;");
                push_decimal(output, u16::from(red));
                output.push(b';');
                push_decimal(output, u16::from(green));
                output.push(b';');
                push_decimal(output, u16::from(blue));
            }
            AirColor::TerminalDefault => push_decimal(output, restored),
        }
    }
}

impl Encoder for AirSgrEncoder {
    type Item = AirCellPresentation;
    /// Toute présentation a une représentation : l'encodage ne peut pas échouer.
    type Error = core::convert::Infallible;

    fn encode(&mut self, item: &Self::Item, output: &mut Vec<u8>) -> Result<(), Self::Error> {
        if self.last_written == Some(*item) {
            return Ok(());
        }
        // Chaque séquence repart de zéro plutôt que de composer un delta : un terminal
        // dont on ne connaît pas l'état exact rendrait le delta faux, et le coût de deux
        // octets supplémentaires est sans commune mesure avec celui d'un attribut qui
        // traîne sur le reste de l'écran.
        output.extend_from_slice(b"\x1b[0");
        for (enabled, parameter) in [
            (item.attributes.bold, b"1"),
            (item.attributes.dim, b"2"),
            (item.attributes.italic, b"3"),
            (item.attributes.underline, b"4"),
            (item.attributes.blink, b"5"),
            (item.attributes.reverse, b"7"),
            (item.attributes.strikethrough, b"9"),
        ] {
            if enabled {
                output.push(b';');
                output.extend_from_slice(parameter);
            }
        }
        Self::push_color(output, item.foreground, true);
        Self::push_color(output, item.background, false);
        output.push(b'm');
        self.last_written = Some(*item);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use air_tui_platform::Encoder;

    use super::{AirCellPresentation, AirSgrEncoder};
    use crate::cell::{AirAttributes, AirCell, AirColor};

    fn presentation(foreground: AirColor, background: AirColor) -> AirCellPresentation {
        AirCellPresentation {
            foreground,
            background,
            attributes: AirAttributes::NONE,
        }
    }

    fn encoded(encoder: &mut AirSgrEncoder, item: &AirCellPresentation) -> String {
        let mut output = Vec::new();
        encoder
            .encode(item, &mut output)
            .expect("l'encodage SGR ne peut pas échouer");
        String::from_utf8(output).expect("une séquence SGR est de l'UTF-8 valide")
    }

    #[test]
    fn une_couleur_indexee_est_emise_en_notation_etendue() {
        let mut encoder = AirSgrEncoder::new();
        let rendered = encoded(
            &mut encoder,
            &presentation(AirColor::Indexed(7), AirColor::Indexed(4)),
        );
        assert_eq!(rendered, "\u{1b}[0;38;5;7;48;5;4m");
    }

    #[test]
    fn une_couleur_rgb_est_emise_composante_par_composante() {
        let mut encoder = AirSgrEncoder::new();
        let rendered = encoded(
            &mut encoder,
            &presentation(
                AirColor::Rgb {
                    red: 255,
                    green: 128,
                    blue: 0,
                },
                AirColor::Rgb {
                    red: 1,
                    green: 2,
                    blue: 3,
                },
            ),
        );
        assert_eq!(rendered, "\u{1b}[0;38;2;255;128;0;48;2;1;2;3m");
    }

    #[test]
    fn chaque_attribut_a_son_parametre() {
        let cases = [
            (AirAttributes::NONE.with_bold(), ";1"),
            (
                AirAttributes {
                    dim: true,
                    ..AirAttributes::NONE
                },
                ";2",
            ),
            (
                AirAttributes {
                    italic: true,
                    ..AirAttributes::NONE
                },
                ";3",
            ),
            (
                AirAttributes {
                    underline: true,
                    ..AirAttributes::NONE
                },
                ";4",
            ),
            (
                AirAttributes {
                    blink: true,
                    ..AirAttributes::NONE
                },
                ";5",
            ),
            (
                AirAttributes {
                    reverse: true,
                    ..AirAttributes::NONE
                },
                ";7",
            ),
            (
                AirAttributes {
                    strikethrough: true,
                    ..AirAttributes::NONE
                },
                ";9",
            ),
        ];
        for (attributes, expected) in cases {
            let mut encoder = AirSgrEncoder::new();
            let item = AirCellPresentation {
                foreground: AirColor::Indexed(0),
                background: AirColor::Indexed(0),
                attributes,
            };
            let rendered = encoded(&mut encoder, &item);
            assert!(
                rendered.contains(expected),
                "{expected} absent de {rendered} pour {attributes:?}"
            );
        }
    }

    #[test]
    fn tous_les_attributs_a_la_fois_sont_emis_dans_l_ordre() {
        let mut encoder = AirSgrEncoder::new();
        let item = AirCellPresentation {
            foreground: AirColor::Indexed(1),
            background: AirColor::Indexed(2),
            attributes: AirAttributes {
                bold: true,
                dim: true,
                italic: true,
                underline: true,
                blink: true,
                reverse: true,
                strikethrough: true,
            },
        };
        let rendered = encoded(&mut encoder, &item);
        assert_eq!(rendered, "\u{1b}[0;1;2;3;4;5;7;9;38;5;1;48;5;2m");
    }

    #[test]
    fn la_couleur_du_terminal_annule_au_lieu_de_poser() {
        // 39 et 49, et non 38;5;n : ce sont les deux paramètres qui rendent au terminal
        // la couleur qu'il employait. C'est ce qui rend un thème transparent.
        let mut encoder = AirSgrEncoder::new();
        let rendered = encoded(
            &mut encoder,
            &presentation(AirColor::TerminalDefault, AirColor::TerminalDefault),
        );
        assert_eq!(rendered, "\u{1b}[0;39;49m");
    }

    #[test]
    fn la_couleur_du_terminal_se_mele_a_une_couleur_nommee() {
        // Le cas des skins « defbg » de mc : un avant-plan choisi sur le fond du terminal.
        let mut encoder = AirSgrEncoder::new();
        let rendered = encoded(
            &mut encoder,
            &presentation(AirColor::Indexed(11), AirColor::TerminalDefault),
        );
        assert_eq!(rendered, "\u{1b}[0;38;5;11;49m");
        encoder.forget();
        let inverted = encoded(
            &mut encoder,
            &presentation(AirColor::TerminalDefault, AirColor::Indexed(4)),
        );
        assert_eq!(inverted, "\u{1b}[0;39;48;5;4m");
    }

    #[test]
    fn une_presentation_inchangee_n_emet_rien() {
        let mut encoder = AirSgrEncoder::new();
        let item = presentation(AirColor::Indexed(7), AirColor::Indexed(4));
        assert_ne!(encoded(&mut encoder, &item), "");
        assert_eq!(encoded(&mut encoder, &item), "");
    }

    #[test]
    fn forget_force_la_reemission() {
        let mut encoder = AirSgrEncoder::new();
        let item = presentation(AirColor::Indexed(7), AirColor::Indexed(4));
        let first = encoded(&mut encoder, &item);
        encoder.forget();
        assert_eq!(encoded(&mut encoder, &item), first);
    }

    #[test]
    fn un_changement_de_presentation_est_emis() {
        let mut encoder = AirSgrEncoder::new();
        let first = presentation(AirColor::Indexed(7), AirColor::Indexed(4));
        let second = presentation(AirColor::Indexed(15), AirColor::Indexed(4));
        assert_ne!(encoded(&mut encoder, &first), "");
        assert_ne!(encoded(&mut encoder, &second), "");
    }

    #[test]
    fn l_encodeur_ajoute_sans_vider_le_tampon() {
        let mut encoder = AirSgrEncoder::new();
        let mut output = b"avant".to_vec();
        encoder
            .encode(
                &presentation(AirColor::Indexed(0), AirColor::Indexed(0)),
                &mut output,
            )
            .expect("encodage");
        assert!(output.starts_with(b"avant"));
    }

    #[test]
    fn une_cellule_se_convertit_en_presentation() {
        let cell = AirCell::new('X', AirColor::WHITE, AirColor::BLUE)
            .with_attributes(AirAttributes::NONE.with_bold());
        let converted = AirCellPresentation::from(cell);
        assert_eq!(converted.foreground, AirColor::WHITE);
        assert_eq!(converted.background, AirColor::BLUE);
        assert!(converted.attributes.bold);
    }

    #[test]
    fn l_encodeur_par_defaut_est_vierge() {
        let mut default_encoder = AirSgrEncoder::default();
        let item = presentation(AirColor::Indexed(3), AirColor::Indexed(5));
        assert_ne!(encoded(&mut default_encoder, &item), "");
    }
}
