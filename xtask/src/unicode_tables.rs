// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La génération des tables Unicode d'air-tui.
//!
//! Le produit est `crates/air-tui/src/unicode/tables.rs`, **committé** et jamais édité à
//! la main (ADR-006). Le relancer sur la même édition de l'UCD produit le même fichier,
//! octet pour octet : c'est ce qui rend la mise à jour d'une version d'Unicode lisible
//! comme un diff.

use std::fmt::Write as _;
use std::path::Path;

use crate::ucd::{self, PropertyRange, UcdError};

/// Les noms des cinq fichiers du référentiel dont les tables dérivent.
const EAST_ASIAN_WIDTH_FILE: &str = "EastAsianWidth.txt";
/// Le fichier des propriétés dérivées, d'où vient `Grapheme_Extend`.
const DERIVED_CORE_PROPERTIES_FILE: &str = "DerivedCoreProperties.txt";
/// Le fichier de la propriété de coupure de graphème.
const GRAPHEME_BREAK_FILE: &str = "GraphemeBreakProperty.txt";
/// Le fichier de la propriété de coupure de mot.
const WORD_BREAK_FILE: &str = "WordBreakProperty.txt";
/// Le fichier des propriétés d'emoji.
const EMOJI_DATA_FILE: &str = "emoji-data.txt";

/// La correspondance entre les noms de l'UCD et les variantes d'air-tui.
///
/// Les noms d'air-tui sont écrits en entier : la règle de nommage n'admet pas `L`, `LV`
/// ou `ZWJ` (ADR-004), même quand la norme les abrège.
const GRAPHEME_BREAK_NAMES: &[(&str, &str)] = &[
    ("CR", "CarriageReturn"),
    ("LF", "LineFeed"),
    ("Control", "Control"),
    ("Extend", "Extend"),
    ("ZWJ", "ZeroWidthJoiner"),
    ("Regional_Indicator", "RegionalIndicator"),
    ("Prepend", "Prepend"),
    ("SpacingMark", "SpacingMark"),
    ("L", "HangulLeading"),
    ("V", "HangulVowel"),
    ("T", "HangulTrailing"),
    ("LV", "HangulLeadingVowel"),
    ("LVT", "HangulLeadingVowelTrailing"),
];

/// La correspondance entre les noms de coupure de mot de l'UCD et ceux d'air-tui.
///
/// Même règle que ci-dessus : la norme écrit `ALetter`, `MidNumLet` et `WSegSpace`, qui
/// sont des abréviations ; air-tui les écrit en entier (ADR-004). `Other` n'y figure pas —
/// c'est la valeur par défaut, et le référentiel ne l'énumère pas.
const WORD_BREAK_NAMES: &[(&str, &str)] = &[
    ("CR", "CarriageReturn"),
    ("LF", "LineFeed"),
    ("Newline", "Newline"),
    ("Extend", "Extend"),
    ("ZWJ", "ZeroWidthJoiner"),
    ("Regional_Indicator", "RegionalIndicator"),
    ("Format", "Format"),
    ("Katakana", "Katakana"),
    ("Hebrew_Letter", "HebrewLetter"),
    ("ALetter", "AlphabeticLetter"),
    ("Single_Quote", "SingleQuote"),
    ("Double_Quote", "DoubleQuote"),
    ("MidNumLet", "MiddleNumericOrLetter"),
    ("MidLetter", "MiddleLetter"),
    ("MidNum", "MiddleNumeric"),
    ("Numeric", "Numeric"),
    ("ExtendNumLet", "ExtendNumericOrLetter"),
    ("WSegSpace", "WordSegmentationSpace"),
];

/// Produit le contenu du fichier de tables depuis un répertoire du référentiel.
///
/// # Errors
///
/// Si un fichier manque, est illisible, porte une ligne malformée, ou emploie une valeur
/// de propriété de coupure que la table de correspondance ne connaît pas.
pub fn generate(ucd_directory: &Path) -> Result<String, UcdError> {
    let east_asian_path = ucd_directory.join(EAST_ASIAN_WIDTH_FILE);
    let version = ucd::read_version(&east_asian_path)?;

    let east_asian = ucd::read_property_file(&east_asian_path)?;
    let derived = ucd::read_property_file(&ucd_directory.join(DERIVED_CORE_PROPERTIES_FILE))?;
    let grapheme = ucd::read_property_file(&ucd_directory.join(GRAPHEME_BREAK_FILE))?;
    let word = ucd::read_property_file(&ucd_directory.join(WORD_BREAK_FILE))?;
    let emoji = ucd::read_property_file(&ucd_directory.join(EMOJI_DATA_FILE))?;

    // Largeur deux : les caractères est-asiatiques larges ou pleine chasse, et les emoji
    // dont la présentation par défaut est l'emoji. `Extended_Pictographic` serait trop
    // large : il contient des symboles qui s'affichent sur une seule colonne.
    let mut wide = ucd::select(&east_asian, &["W", "F"]);
    wide.extend(ucd::select(&emoji, &["Emoji_Presentation"]));
    wide.sort_unstable();
    let wide = ucd::merge_adjacent(wide);

    let zero_width = ucd::select(&derived, &["Grapheme_Extend"]);
    let extended_pictographic = ucd::select(&emoji, &["Extended_Pictographic"]);
    let grapheme_break = select_named(&grapheme, GRAPHEME_BREAK_NAMES, GRAPHEME_BREAK_FILE)?;
    let word_break = select_named(&word, WORD_BREAK_NAMES, WORD_BREAK_FILE)?;

    // La coupure conjointe indienne porte la règle GB9c de l'annexe 29 : sans elle, le
    // curseur coupe au milieu d'une conjointe en devanagari.
    let indic_consonant = ucd::select_qualified(&derived, "InCB", &["Consonant"]);
    let indic_extend = ucd::select_qualified(&derived, "InCB", &["Extend"]);
    let indic_linker = ucd::select_qualified(&derived, "InCB", &["Linker"]);

    let mut output = String::with_capacity(256 * 1024);
    write_header(&mut output, &version);
    write_range_table(
        &mut output,
        "WIDE",
        "Les plages dont les caractères occupent deux colonnes.",
        &wide,
    );
    write_range_table(
        &mut output,
        "ZERO_WIDTH",
        "Les plages dont les caractères n'occupent aucune colonne — les marques combinantes.",
        &zero_width,
    );
    write_range_table(
        &mut output,
        "EXTENDED_PICTOGRAPHIC",
        "Les plages pictographiques, pour la règle `GB11` de l'annexe 29.",
        &extended_pictographic,
    );
    write_range_table(
        &mut output,
        "INDIC_CONSONANT",
        "Les consonnes au sens de la coupure conjointe indienne, pour la règle `GB9c`.",
        &indic_consonant,
    );
    write_range_table(
        &mut output,
        "INDIC_EXTEND",
        "Les marques de prolongement de la coupure conjointe indienne, règle `GB9c`.",
        &indic_extend,
    );
    write_range_table(
        &mut output,
        "INDIC_LINKER",
        "Les lieurs de la coupure conjointe indienne, pour la règle `GB9c`.",
        &indic_linker,
    );
    write_named_table(
        &mut output,
        "GRAPHEME_BREAK",
        "AirGraphemeBreak",
        "La propriété de coupure de graphème, par plage.",
        &grapheme_break,
    );
    write_named_table(
        &mut output,
        "WORD_BREAK",
        "AirWordBreak",
        "La propriété de coupure de mot, par plage — l'annexe 29 encore, autre segmentation.",
        &word_break,
    );
    Ok(output)
}

/// Les plages d'une propriété énumérée, traduites par une table de noms et fusionnées.
///
/// Une valeur absente de la table est une **erreur** et non une plage ignorée : une
/// nouvelle édition d'Unicode qui ajouterait une valeur de coupure doit arrêter la
/// génération, pas produire une table silencieusement incomplète.
fn select_named<'names>(
    ranges: &[PropertyRange],
    names: &'names [(&str, &str)],
    file: &str,
) -> Result<Vec<(u32, u32, &'names str)>, UcdError> {
    let mut translated: Vec<(u32, u32, &str)> = Vec::with_capacity(ranges.len());
    for range in ranges {
        let Some((_, air_name)) = names.iter().find(|(ucd_name, _)| *ucd_name == range.value)
        else {
            return Err(UcdError::MalformedLine {
                path: file.to_owned(),
                line_number: 0,
                content: format!("valeur de coupure inconnue : {}", range.value),
            });
        };
        translated.push((range.first, range.last, *air_name));
    }
    translated.sort_unstable();
    Ok(merge_named(translated))
}

/// Fusionne les plages contiguës qui portent la même valeur nommée.
fn merge_named(sorted: Vec<(u32, u32, &str)>) -> Vec<(u32, u32, &str)> {
    let mut merged: Vec<(u32, u32, &str)> = Vec::with_capacity(sorted.len());
    for (first, last, name) in sorted {
        match merged.last_mut() {
            Some(previous) if previous.2 == name && first <= previous.1.saturating_add(1) => {
                previous.1 = previous.1.max(last);
            }
            _ => merged.push((first, last, name)),
        }
    }
    merged
}

/// Écrit l'en-tête du fichier généré.
fn write_header(output: &mut String, version: &str) {
    output.push_str(
        "// This Source Code Form is subject to the terms of the Mozilla Public\n\
         // License, v. 2.0. If a copy of the MPL was not distributed with this\n\
         // file, You can obtain one at https://mozilla.org/MPL/2.0/.\n\
         \n\
         //! Tables dérivées du référentiel de caractères Unicode.\n\
         //!\n\
         //! **FICHIER GÉNÉRÉ — ne pas éditer à la main.** Il est produit par\n\
         //! `cargo xtask generate-unicode-tables` et committé (ADR-006). Une modification\n\
         //! manuelle serait écrasée à la prochaine génération, et le contrôle\n\
         //! `cargo xtask generate-unicode-tables --check` la refuserait.\n\
         //!\n\
         //! Les données appartiennent au Consortium Unicode et sont distribuées sous la\n\
         //! licence `Unicode-3.0`, admise par `deny.toml`.\n\
         \n\
         use crate::unicode::{AirGraphemeBreak, AirWordBreak};\n\
         \n",
    );
    let _ = writeln!(
        output,
        "/// L'édition du référentiel dont ces tables proviennent.\npub const UNICODE_VERSION: &str = {version:?};\n"
    );
}

/// Écrit une table de plages booléenne.
fn write_range_table(output: &mut String, name: &str, description: &str, ranges: &[(u32, u32)]) {
    let _ = writeln!(output, "/// {description}");
    let _ = writeln!(
        output,
        "///\n/// {} plages, triées et fusionnées.",
        ranges.len()
    );
    let _ = writeln!(output, "pub(crate) static {name}: &[(u32, u32)] = &[");
    for (first, last) in ranges {
        let _ = writeln!(output, "    ({first:#07X}, {last:#07X}),");
    }
    output.push_str("];\n\n");
}

/// Écrit une table de propriété énumérée, par plage.
fn write_named_table(
    output: &mut String,
    table: &str,
    type_name: &str,
    description: &str,
    ranges: &[(u32, u32, &str)],
) {
    let _ = writeln!(output, "/// {description}");
    let _ = writeln!(
        output,
        "///\n/// {} plages, triées et fusionnées. Les points de code absents valent\n\
         /// [`{type_name}::Other`].",
        ranges.len()
    );
    let _ = writeln!(
        output,
        "pub(crate) static {table}: &[(u32, u32, {type_name})] = &["
    );
    for (first, last, name) in ranges {
        let _ = writeln!(
            output,
            "    ({first:#07X}, {last:#07X}, {type_name}::{name}),"
        );
    }
    output.push_str("];\n\n");
}

#[cfg(test)]
mod tests {
    use super::{GRAPHEME_BREAK_NAMES, WORD_BREAK_NAMES, merge_named, write_range_table};

    #[test]
    fn les_treize_valeurs_de_coupure_sont_nommees_sans_abreviation() {
        // La norme abrège `L`, `LV`, `ZWJ` ; la règle de nommage d'air-tui ne l'admet pas.
        assert_eq!(GRAPHEME_BREAK_NAMES.len(), 13);
        for (_, air_name) in GRAPHEME_BREAK_NAMES {
            assert!(
                air_name.len() > 2,
                "{air_name} est trop court pour ne pas être une abréviation"
            );
        }
    }

    #[test]
    fn les_dix_huit_valeurs_de_coupure_de_mot_sont_nommees_sans_abreviation() {
        // Le référentiel énumère dix-huit valeurs ; `Other` est le défaut et n'y figure
        // pas. En manquer une arrêterait la génération, ce qui est voulu.
        assert_eq!(WORD_BREAK_NAMES.len(), 18);
        for (_, air_name) in WORD_BREAK_NAMES {
            assert!(
                air_name.len() > 2,
                "{air_name} est trop court pour ne pas être une abréviation"
            );
        }
    }

    #[test]
    fn les_plages_contigues_de_meme_valeur_sont_fusionnees() {
        assert_eq!(
            merge_named(vec![(0, 10, "Extend"), (11, 20, "Extend")]),
            vec![(0, 20, "Extend")]
        );
    }

    #[test]
    fn les_plages_contigues_de_valeurs_differentes_restent_separees() {
        assert_eq!(
            merge_named(vec![(0, 10, "Extend"), (11, 20, "Control")]),
            vec![(0, 10, "Extend"), (11, 20, "Control")]
        );
    }

    #[test]
    fn une_table_vide_est_ecrite_sans_entree() {
        let mut output = String::new();
        write_range_table(&mut output, "VIDE", "Rien.", &[]);
        assert!(output.contains("static VIDE: &[(u32, u32)] = &[\n];"));
        assert!(output.contains("0 plages"));
    }

    #[test]
    fn une_table_ecrit_ses_plages_en_hexadecimal() {
        let mut output = String::new();
        write_range_table(&mut output, "TEST", "Essai.", &[(0x41, 0x5A)]);
        assert!(output.contains("(0x00041, 0x0005A)"), "rendu : {output}");
    }
}
