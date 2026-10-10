// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La lecture d'un fichier de skin de GNU Midnight Commander.
//!
//! # Pourquoi cette lecture est ici et non dans le cœur
//!
//! L'ADR-008 est explicite : « les thèmes sont compilés, pas analysés au runtime ». Un
//! skin mal formé doit être un refus à la compilation, jamais un comportement dégradé
//! dans une application. Cet analyseur vit donc dans l'outillage, et ce qu'il produit est
//! un artefact Rust committé.
//!
//! # Le format, tel que les fichiers l'écrivent
//!
//! Un fichier `.ini` à sections, commentaires en `#`. Une valeur de couleur est
//! `avant-plan;fond;attributs`, chaque champ facultatif. Un champ vide **hérite** : du
//! `_default_` de la section, puis de celui de `[core]`. C'est la règle du manuel, et
//! c'est elle qui fait que `editframeactive = white;` donne du blanc sur le **bleu** de
//! `[editor] _default_`.
//!
//! Deux écarts entre le manuel de `mc` et les fichiers qu'il livre, constatés sur les 39
//! skins de la version 4.8.33 et tranchés en faveur des fichiers :
//!
//! - le manuel nomme les jonctions `centertop`, `centerbottom` et `centermiddle` ; les 39
//!   skins écrivent `topmiddle`, `bottommiddle` et `cross`. Aucun n'emploie les noms du
//!   manuel ;
//! - le manuel annonce `color16` à `color255` ; les skins emploient aussi `color0` à
//!   `color15`, plus de deux mille fois.
//!
//! # Ce que cet analyseur refuse
//!
//! Une syntaxe invalide, un nom de couleur inconnu, un alias en boucle. Et la couleur
//! `default` — le fond transparent de `mc` —, parce qu'`AirColor` ne sait pas l'exprimer :
//! la refuser nomme la limite, l'accepter en devinant une couleur produirait un thème qui
//! ne ressemble pas au skin. La tolérance est la faille.
//!
//! Les vraies couleurs `#rrggbb`, elles, sont acceptées : `AirColor::Rgb` les exprime, et
//! le quantificateur du jalon M5 les dégrade vers 256 ou 16 couleurs selon ce que le
//! terminal annonce.

use std::collections::BTreeMap;

/// Le nombre maximal de déréférencements d'alias avant de conclure à une boucle.
///
/// Le manuel autorise un alias qui désigne un alias, « tant qu'ils ne forment pas de
/// boucle ». Une profondeur bornée tranche sans tenir la liste des alias déjà vus.
const ALIAS_DEPTH_LIMIT: usize = 16;

/// La section dont tout hérite.
const CORE_SECTION: &str = "core";

/// La clef du couple de couleurs par défaut d'une section.
const DEFAULT_KEY: &str = "_default_";

/// Les seize noms de couleurs de `mc`, et leur index ANSI.
///
/// Ces index sont ceux du **terminal**, pas ceux du mode texte VGA : `mc` a toujours
/// tourné sur un terminal. Il n'y a donc rien à convertir ici — et c'est l'inverse de la
/// transcription de Turbo Vision, où oublier l'échange du bleu et du rouge donnait un
/// bureau rouge. Ajouter une conversion ici serait le défaut symétrique.
const COLOR_NAMES: &[(&str, u8)] = &[
    ("black", 0),
    ("red", 1),
    ("green", 2),
    ("brown", 3),
    ("blue", 4),
    ("magenta", 5),
    ("cyan", 6),
    ("lightgray", 7),
    ("gray", 8),
    ("brightred", 9),
    ("brightgreen", 10),
    ("yellow", 11),
    ("brightblue", 12),
    ("brightmagenta", 13),
    ("brightcyan", 14),
    ("white", 15),
];

/// Les chiffres hexadécimaux et leur valeur.
///
/// Une table plutôt que `char::to_digit` suivi d'un `u8::try_from` : la conversion ne peut
/// pas échouer — `to_digit(16)` rend au plus quinze —, donc son chemin d'erreur était
/// inatteignable, donc impossible à couvrir. L'ADR-012 nomme précisément cet anti-patron,
/// et la table le fait disparaître au lieu de l'excuser.
const HEXADECIMAL_DIGITS: &[(char, u8)] = &[
    ('0', 0),
    ('1', 1),
    ('2', 2),
    ('3', 3),
    ('4', 4),
    ('5', 5),
    ('6', 6),
    ('7', 7),
    ('8', 8),
    ('9', 9),
    ('a', 10),
    ('b', 11),
    ('c', 12),
    ('d', 13),
    ('e', 14),
    ('f', 15),
];

/// Les attributs qu'un couple de couleurs peut porter.
const ATTRIBUTE_NAMES: &[&str] = &["bold", "italic", "underline", "reverse", "blink"];

/// Une couleur de skin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// Un index dans la palette du terminal, de 0 à 255.
    Indexed(u8),
    /// Une vraie couleur, en notation `#rrggbb` ou `#rgb`.
    Rgb {
        /// Composante rouge.
        red: u8,
        /// Composante verte.
        green: u8,
        /// Composante bleue.
        blue: u8,
    },
    /// La couleur par défaut du terminal — le fond transparent de `mc`.
    Terminal,
}

/// Un couple de couleurs résolu, attributs compris.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorPair {
    /// L'avant-plan.
    pub foreground: Color,
    /// Le fond.
    pub background: Color,
    /// Les attributs, dans l'ordre où le skin les écrit.
    pub attributes: Vec<String>,
}

/// Un skin lu, avant toute résolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skin {
    /// Les sections, et leurs clefs.
    sections: BTreeMap<String, BTreeMap<String, String>>,
}

impl Skin {
    /// La description que porte la section `[skin]`, si elle en porte une.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.raw("skin", "description")
    }

    /// La valeur brute d'une clef, sans résolution d'alias ni d'héritage.
    #[must_use]
    pub fn raw(&self, section: &str, key: &str) -> Option<&str> {
        self.sections.get(section)?.get(key).map(String::as_str)
    }

    /// Le nombre de clefs du fichier, toutes sections confondues.
    #[must_use]
    pub fn key_count(&self) -> usize {
        self.sections
            .values()
            .map(BTreeMap::len)
            .fold(0, usize::saturating_add)
    }

    /// Les alias déclarés par la section `[aliases]`.
    fn aliases(&self) -> BTreeMap<String, String> {
        self.sections.get("aliases").cloned().unwrap_or_default()
    }

    /// Le couple de couleurs d'une clef, héritage et alias résolus.
    ///
    /// # Errors
    ///
    /// Si la clef est absente, si un champ reste vide après héritage, si un nom de
    /// couleur est inconnu, ou si un alias boucle.
    pub fn color_pair(&self, section: &str, key: &str) -> Result<ColorPair, String> {
        let aliases = self.aliases();
        let value = self
            .raw(section, key)
            .ok_or_else(|| format!("[{section}] {key} est absent du skin"))?;
        let fields = self.inherited_fields(section, key, value)?;
        let (foreground, background, attributes) = fields;
        Ok(ColorPair {
            foreground: color(&resolve_alias(&foreground, &aliases)?)
                .map_err(|cause| format!("[{section}] {key} : avant-plan — {cause}"))?,
            background: color(&resolve_alias(&background, &aliases)?)
                .map_err(|cause| format!("[{section}] {key} : fond — {cause}"))?,
            attributes: attributes_of(&resolve_alias(&attributes, &aliases)?)
                .map_err(|cause| format!("[{section}] {key} : attributs — {cause}"))?,
        })
    }

    /// Les trois champs d'une valeur, chacun complété par héritage s'il est vide.
    fn inherited_fields(
        &self,
        section: &str,
        key: &str,
        value: &str,
    ) -> Result<(String, String, String), String> {
        let mut fields = split_fields(value);
        for source in [
            self.raw(section, DEFAULT_KEY),
            self.raw(CORE_SECTION, DEFAULT_KEY),
        ] {
            if fields.0.is_empty() || fields.1.is_empty() {
                let Some(fallback) = source else {
                    continue;
                };
                let inherited = split_fields(fallback);
                if fields.0.is_empty() {
                    fields.0 = inherited.0;
                }
                if fields.1.is_empty() {
                    fields.1 = inherited.1;
                }
            }
        }
        if fields.0.is_empty() {
            return Err(format!(
                "[{section}] {key} : avant-plan vide, et ni la section ni [core] n'en \
                 fournissent"
            ));
        }
        if fields.1.is_empty() {
            return Err(format!(
                "[{section}] {key} : fond vide, et ni la section ni [core] n'en fournissent"
            ));
        }
        Ok(fields)
    }

    /// Le caractère unique d'une clef de la section `[Lines]`.
    ///
    /// # Errors
    ///
    /// Si la clef est absente, ou si sa valeur n'est pas exactement une grappe d'un seul
    /// point de code : un cadre se dessine cellule par cellule.
    pub fn line_glyph(&self, key: &str) -> Result<char, String> {
        let value = self
            .raw("Lines", key)
            .ok_or_else(|| format!("[Lines] {key} est absent du skin"))?;
        let mut characters = value.chars();
        let first = characters
            .next()
            .ok_or_else(|| format!("[Lines] {key} est vide"))?;
        if characters.next().is_some() {
            return Err(format!(
                "[Lines] {key} vaut « {value} », qui n'est pas un caractère unique"
            ));
        }
        Ok(first)
    }
}

/// Lit un fichier de skin.
///
/// Un `#` ne commence un commentaire qu'en **début** de ligne. Ailleurs, il introduit une
/// vraie couleur : `_default_ = #d3d7cf;#2e3436`. Les 39 skins de la version 4.8.33
/// portent 192 valeurs contenant un `#`, et les 192 sont des couleurs — aucune n'est un
/// commentaire de fin de ligne. Découper sur `#` vidait donc quatre skins de leurs
/// couleurs, en annonçant « «  » n'est pas une couleur connue ».
///
/// # Errors
///
/// Si une clef apparaît hors de toute section, ou si un en-tête de section n'est pas
/// fermé. Une clef en double n'est pas une erreur : la dernière gagne, comme dans tout
/// `.ini` — et aucun des 39 skins livrés n'en contient.
pub fn parse(text: &str) -> Result<Skin, String> {
    let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut current: Option<String> = None;
    for (index, line) in text.lines().enumerate() {
        let number = index.saturating_add(1);
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('[') {
            let name = rest
                .strip_suffix(']')
                .ok_or_else(|| format!("ligne {number} : en-tête de section non fermé"))?;
            current = Some(name.trim().to_owned());
            sections.entry(name.trim().to_owned()).or_default();
            continue;
        }
        let (key, value) = trimmed
            .split_once('=')
            .ok_or_else(|| format!("ligne {number} : ni section, ni affectation"))?;
        let section = current
            .as_ref()
            .ok_or_else(|| format!("ligne {number} : clef hors de toute section"))?;
        sections
            .entry(section.clone())
            .or_default()
            .insert(key.trim().to_owned(), value.trim().to_owned());
    }
    Ok(Skin { sections })
}

/// L'empreinte d'un skin, pour que l'artefact dise de **quel** fichier il vient.
///
/// # Pourquoi FNV-1a et non une empreinte cryptographique
///
/// Ce qu'il faut ici est une **identité**, pas une résistance à la falsification : savoir
/// si le skin qu'on présente à `--check` est bien celui qui a produit l'artefact. Un skin
/// n'est pas une entrée hostile — c'est un fichier que l'auteur du dépôt choisit et passe
/// en argument. SHA-256 demanderait quatre-vingts lignes de code à couvrir, pour une
/// garantie dont personne n'a besoin ici. ADR-005 interdit par ailleurs d'en prendre une
/// en dépendance.
///
/// L'empreinte est donc un repère de dérive, et l'en-tête de l'artefact le dit.
#[must_use]
pub fn fingerprint(text: &str) -> u64 {
    // Les deux constantes de FNV-1a sur 64 bits.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Découpe une valeur en avant-plan, fond et attributs.
fn split_fields(value: &str) -> (String, String, String) {
    let mut parts = value.split(';');
    let foreground = parts.next().unwrap_or("").trim().to_owned();
    let background = parts.next().unwrap_or("").trim().to_owned();
    let attributes = parts.next().unwrap_or("").trim().to_owned();
    (foreground, background, attributes)
}

/// Remplace un alias par ce qu'il désigne, récursivement.
///
/// # Errors
///
/// Si la chaîne d'alias dépasse [`ALIAS_DEPTH_LIMIT`] — ce qui ne peut arriver que par
/// une boucle, le manuel bornant les alias à des noms simples.
fn resolve_alias(value: &str, aliases: &BTreeMap<String, String>) -> Result<String, String> {
    let mut current = value.to_owned();
    for _ in 0..ALIAS_DEPTH_LIMIT {
        let Some(target) = aliases.get(&current) else {
            return Ok(current);
        };
        current = target.clone();
    }
    Err(format!(
        "l'alias « {value} » ne se résout pas en {ALIAS_DEPTH_LIMIT} déréférencements : \
         il boucle"
    ))
}

/// La couleur que désigne un nom.
///
/// # Errors
///
/// Si le nom n'est ni un des seize, ni `default`, ni `colorN`, ni `rgbRGB`, ni `grayN`.
fn color(name: &str) -> Result<Color, String> {
    if name == "default" {
        return Ok(Color::Terminal);
    }
    for (candidate, index) in COLOR_NAMES {
        if name == *candidate {
            return Ok(Color::Indexed(*index));
        }
    }
    if let Some(digits) = name.strip_prefix('#') {
        return true_color(digits).map_err(|cause| format!("« {name} » : {cause}"));
    }
    if let Some(digits) = name.strip_prefix("color") {
        let index: u16 = digits
            .parse()
            .map_err(|_| format!("« {name} » n'est pas un index de couleur"))?;
        let index = u8::try_from(index)
            .map_err(|_| format!("« {name} » dépasse les 256 couleurs du terminal"))?;
        return Ok(Color::Indexed(index));
    }
    if let Some(digits) = name.strip_prefix("rgb") {
        return cube_color(digits).map_err(|cause| format!("« {name} » : {cause}"));
    }
    if let Some(digits) = name.strip_prefix("gray") {
        return gray_color(digits).map_err(|cause| format!("« {name} » : {cause}"));
    }
    Err(format!("« {name} » n'est pas une couleur connue de mc"))
}

/// La vraie couleur que désignent trois ou six chiffres hexadécimaux.
///
/// La forme courte double chaque chiffre, comme le dit l'en-tête des skins « seasons » :
/// « the short form is interpreted by doubling each hex digit ». `#fff` est donc
/// `#ffffff`, et non `#0f0f0f`.
///
/// Les deux formes sont reconnues par **motif de tranche**, ce qui rend la fonction
/// totale : la longueur n'est plus vérifiée d'un côté et supposée de l'autre, et il ne
/// reste aucune branche « ce chiffre manque » que rien ne peut emprunter. C'est l'effet
/// qu'ADR-012 attend de la règle des 100 %.
fn true_color(digits: &str) -> Result<Color, String> {
    let characters: Vec<char> = digits.chars().collect();
    match *characters.as_slice() {
        [red, green, blue] => Ok(Color::Rgb {
            red: doubled_digit(red)?,
            green: doubled_digit(green)?,
            blue: doubled_digit(blue)?,
        }),
        [
            red_high,
            red_low,
            green_high,
            green_low,
            blue_high,
            blue_low,
        ] => Ok(Color::Rgb {
            red: octet(red_high, red_low)?,
            green: octet(green_high, green_low)?,
            blue: octet(blue_high, blue_low)?,
        }),
        _ => Err("une vraie couleur s'écrit sur trois ou six chiffres".to_owned()),
    }
}

/// Un chiffre hexadécimal, doublé — la forme courte `#rgb`.
///
/// # Errors
///
/// Si le caractère n'est pas un chiffre hexadécimal.
fn doubled_digit(character: char) -> Result<u8, String> {
    let value = hexadecimal(character)?;
    Ok(value.saturating_mul(16).saturating_add(value))
}

/// Un octet, sur deux chiffres hexadécimaux.
///
/// # Errors
///
/// Si l'un des deux caractères n'est pas un chiffre hexadécimal.
fn octet(high: char, low: char) -> Result<u8, String> {
    let high = hexadecimal(high)?;
    let low = hexadecimal(low)?;
    Ok(high.saturating_mul(16).saturating_add(low))
}

/// La valeur d'un chiffre hexadécimal, de 0 à 15.
///
/// # Errors
///
/// Si le caractère n'est pas un chiffre hexadécimal.
fn hexadecimal(character: char) -> Result<u8, String> {
    let lowered = character.to_ascii_lowercase();
    HEXADECIMAL_DIGITS
        .iter()
        .find(|(digit, _)| *digit == lowered)
        .map(|(_, value)| *value)
        .ok_or_else(|| format!("« {character} » n'est pas un chiffre hexadécimal"))
}

/// La couleur du cube 6×6×6 que désignent trois chiffres de 0 à 5.
///
/// Le cube commence à l'index 16 de la palette du terminal, et `rgb000` est donc 16 et
/// non noir : c'est la convention des 256 couleurs, celle qu'emploie `mc`.
///
/// L'arithmétique tient en `u8` par construction — `16 + 5×36 + 5×6 + 5 = 231` —, et les
/// trois composantes sont reconnues par motif de tranche : la fonction n'a donc ni
/// branche d'index hors palette, ni branche de composante manquante.
fn cube_color(digits: &str) -> Result<Color, String> {
    let characters: Vec<char> = digits.chars().collect();
    let [red, green, blue] = *characters.as_slice() else {
        return Err("il faut exactement trois composantes".to_owned());
    };
    let index = 16u8
        .saturating_add(cube_component(red)?.saturating_mul(36))
        .saturating_add(cube_component(green)?.saturating_mul(6))
        .saturating_add(cube_component(blue)?);
    Ok(Color::Indexed(index))
}

/// Une composante du cube : un chiffre de 0 à 5.
///
/// Elle passe par la table hexadécimale, dont les six premières entrées sont justement
/// les chiffres admis : `a` y vaut dix, et se fait refuser par la borne.
///
/// # Errors
///
/// Si le caractère n'est pas un chiffre de 0 à 5.
fn cube_component(character: char) -> Result<u8, String> {
    let bound = "les trois composantes vont de 0 à 5".to_owned();
    let value = hexadecimal(character).map_err(|_| bound.clone())?;
    if value > 5 {
        return Err(bound);
    }
    Ok(value)
}

/// La nuance de gris que désigne un nombre de 0 à 23.
///
/// Les vingt-quatre gris occupent les index 232 à 255 de la palette du terminal. Là
/// encore, l'arithmétique tient en `u8` : `232 + 23 = 255`.
fn gray_color(digits: &str) -> Result<Color, String> {
    let step: u8 = digits
        .parse()
        .map_err(|_| "les gris vont de 0 à 23".to_owned())?;
    if step > 23 {
        return Err("les gris vont de 0 à 23".to_owned());
    }
    Ok(Color::Indexed(232u8.saturating_add(step)))
}

/// Les attributs que désigne un champ, séparés par des `+`.
///
/// # Errors
///
/// Si un nom d'attribut est inconnu. Le mot `none` désigne l'absence d'attribut.
fn attributes_of(field: &str) -> Result<Vec<String>, String> {
    if field.is_empty() || field == "none" {
        return Ok(Vec::new());
    }
    let mut found = Vec::new();
    for name in field.split('+') {
        let name = name.trim();
        if !ATTRIBUTE_NAMES.contains(&name) {
            return Err(format!("« {name} » n'est pas un attribut connu de mc"));
        }
        found.push(name.to_owned());
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::{Color, attributes_of, color, parse};

    /// Un skin minimal, à la forme des fichiers livrés par `mc`.
    const SKIN: &str = "\
# un commentaire en début de ligne
[skin]
    description = Essai

[Lines]
    horiz = ─
    vert = │
    lefttop = ┌

[core]
    _default_ = lightgray;blue
    selected = black;cyan
    disabled = gray;lightgray

[editor]
    _default_ = white;black
    editframeactive = yellow;
";

    #[test]
    fn les_sections_et_les_clefs_se_lisent() {
        let skin = parse(SKIN).expect("skin lisible");
        assert_eq!(skin.description(), Some("Essai"));
        assert_eq!(skin.raw("core", "selected"), Some("black;cyan"));
    }

    #[test]
    fn un_commentaire_en_debut_de_ligne_est_ignore() {
        let skin = parse(SKIN).expect("skin lisible");
        assert_eq!(skin.raw("skin", "# un commentaire en début de ligne"), None);
    }

    #[test]
    fn un_diese_dans_une_valeur_est_une_vraie_couleur_et_non_un_commentaire() {
        // C'est le défaut que la mesure sur les 39 skins livrés a révélé : découper sur
        // `#` vidait les quatre skins « seasons » de leurs couleurs.
        let skin = parse("[core]\n    _default_ = #d3d7cf;#2e3436\n").expect("skin lisible");
        assert_eq!(skin.raw("core", "_default_"), Some("#d3d7cf;#2e3436"));
        let pair = skin
            .color_pair("core", "_default_")
            .expect("couple lisible");
        assert_eq!(
            pair.foreground,
            Color::Rgb {
                red: 0xd3,
                green: 0xd7,
                blue: 0xcf
            }
        );
    }

    #[test]
    fn une_clef_hors_section_est_refusee() {
        assert!(parse("horiz = ─\n").is_err());
    }

    #[test]
    fn un_en_tete_non_ferme_est_refuse() {
        assert!(parse("[core\n    _default_ = white;black\n").is_err());
    }

    #[test]
    fn une_ligne_sans_affectation_est_refusee() {
        assert!(parse("[core]\n    ceci n'est pas une affectation\n").is_err());
    }

    #[test]
    fn la_derniere_valeur_d_une_clef_en_double_gagne() {
        let skin = parse("[core]\n    _default_ = white;black\n    _default_ = black;white\n")
            .expect("skin lisible");
        assert_eq!(skin.raw("core", "_default_"), Some("black;white"));
    }

    #[test]
    fn un_champ_vide_herite_de_la_section() {
        // `editframeactive = yellow;` : le fond vient du `_default_` de [editor], donc
        // noir, et non du bleu de [core].
        let skin = parse(SKIN).expect("skin lisible");
        let pair = skin
            .color_pair("editor", "editframeactive")
            .expect("couple lisible");
        assert_eq!(pair.foreground, Color::Indexed(11));
        assert_eq!(pair.background, Color::Indexed(0));
    }

    #[test]
    fn un_champ_vide_herite_de_core_a_defaut_de_section() {
        let skin = parse("[core]\n    _default_ = white;blue\n[menu]\n    menusel = ;cyan\n")
            .expect("skin lisible");
        let pair = skin.color_pair("menu", "menusel").expect("couple lisible");
        assert_eq!(
            pair.foreground,
            Color::Indexed(15),
            "l'avant-plan vient de [core]"
        );
        assert_eq!(pair.background, Color::Indexed(6));
    }

    #[test]
    fn un_champ_que_rien_ne_fournit_est_refuse() {
        let skin = parse("[menu]\n    menusel = ;cyan\n").expect("skin lisible");
        assert!(skin.color_pair("menu", "menusel").is_err());
    }

    #[test]
    fn une_clef_absente_est_refusee() {
        let skin = parse(SKIN).expect("skin lisible");
        assert!(skin.color_pair("core", "inexistante").is_err());
    }

    #[test]
    fn un_alias_est_resolu() {
        let text = "[aliases]\n    fond = blue\n[core]\n    _default_ = white;fond\n";
        let skin = parse(text).expect("skin lisible");
        let pair = skin
            .color_pair("core", "_default_")
            .expect("couple lisible");
        assert_eq!(pair.background, Color::Indexed(4));
    }

    #[test]
    fn un_alias_qui_designe_un_alias_est_resolu() {
        let text = "[aliases]\n    un = deux\n    deux = cyan\n[core]\n    _default_ = un;black\n";
        let skin = parse(text).expect("skin lisible");
        let pair = skin
            .color_pair("core", "_default_")
            .expect("couple lisible");
        assert_eq!(pair.foreground, Color::Indexed(6));
    }

    #[test]
    fn un_alias_en_boucle_est_refuse() {
        let text = "[aliases]\n    un = deux\n    deux = un\n[core]\n    _default_ = un;black\n";
        let skin = parse(text).expect("skin lisible");
        let refusal = skin
            .color_pair("core", "_default_")
            .expect_err("boucle attendue");
        assert!(refusal.contains("boucle"), "{refusal}");
    }

    #[test]
    fn les_seize_noms_sont_des_index_ansi_et_non_vga() {
        // Le piège symétrique de la transcription de Turbo Vision : là-bas il fallait
        // échanger le bleu et le rouge, ici il ne faut surtout pas.
        assert_eq!(color("blue"), Ok(Color::Indexed(4)));
        assert_eq!(color("red"), Ok(Color::Indexed(1)));
        assert_eq!(color("cyan"), Ok(Color::Indexed(6)));
        assert_eq!(color("brown"), Ok(Color::Indexed(3)));
        assert_eq!(color("lightgray"), Ok(Color::Indexed(7)));
        assert_eq!(color("gray"), Ok(Color::Indexed(8)));
        assert_eq!(color("white"), Ok(Color::Indexed(15)));
    }

    #[test]
    fn default_est_la_couleur_du_terminal() {
        assert_eq!(color("default"), Ok(Color::Terminal));
    }

    #[test]
    fn un_index_explicite_se_lit_de_zero_a_deux_cent_cinquante_cinq() {
        // Le manuel annonce color16 à color255 ; les skins livrés emploient color0, et
        // plus de deux mille fois des colorN. Ce sont les fichiers qui font foi.
        assert_eq!(color("color0"), Ok(Color::Indexed(0)));
        assert_eq!(color("color255"), Ok(Color::Indexed(255)));
        assert!(color("color256").is_err());
        assert!(color("colorX").is_err());
    }

    #[test]
    fn le_cube_de_couleurs_commence_a_seize() {
        assert_eq!(color("rgb000"), Ok(Color::Indexed(16)));
        assert_eq!(color("rgb555"), Ok(Color::Indexed(231)));
        assert_eq!(color("rgb100"), Ok(Color::Indexed(52)));
        assert!(color("rgb666").is_err(), "les composantes vont de 0 à 5");
        assert!(color("rgb11").is_err(), "il en faut trois");
        assert!(color("rgb1111").is_err(), "il n'en faut pas quatre");
    }

    #[test]
    fn les_gris_occupent_la_fin_de_la_palette() {
        assert_eq!(color("gray0"), Ok(Color::Indexed(232)));
        assert_eq!(color("gray23"), Ok(Color::Indexed(255)));
        assert!(color("gray24").is_err());
        assert!(color("grayX").is_err());
    }

    #[test]
    fn une_vraie_couleur_courte_double_chaque_chiffre() {
        // « the short form is interpreted by doubling each hex digit », en-tête des skins
        // « seasons ». `#fff` est donc blanc, et non un gris sombre.
        assert_eq!(
            color("#fff"),
            Ok(Color::Rgb {
                red: 0xff,
                green: 0xff,
                blue: 0xff
            })
        );
        assert_eq!(
            color("#123"),
            Ok(Color::Rgb {
                red: 0x11,
                green: 0x22,
                blue: 0x33
            })
        );
    }

    #[test]
    fn une_vraie_couleur_longue_se_lit_par_octets() {
        assert_eq!(
            color("#2e3436"),
            Ok(Color::Rgb {
                red: 0x2e,
                green: 0x34,
                blue: 0x36
            })
        );
    }

    #[test]
    fn une_vraie_couleur_mal_formee_est_refusee() {
        assert!(color("#ff").is_err());
        assert!(color("#abcdefa").is_err());
        assert!(color("#xyz").is_err());
        assert!(color("#xyzxyz").is_err());
    }

    #[test]
    fn un_nom_inconnu_est_refuse() {
        let refusal = color("chartreuse").expect_err("refus attendu");
        assert!(refusal.contains("chartreuse"), "{refusal}");
    }

    #[test]
    fn les_attributs_se_lisent_et_se_composent() {
        assert_eq!(attributes_of(""), Ok(Vec::new()));
        assert_eq!(attributes_of("none"), Ok(Vec::new()));
        assert_eq!(attributes_of("bold"), Ok(vec!["bold".to_owned()]));
        assert_eq!(
            attributes_of("bold+underline"),
            Ok(vec!["bold".to_owned(), "underline".to_owned()])
        );
        assert!(attributes_of("clignotant").is_err());
    }

    #[test]
    fn un_attribut_se_lit_en_troisieme_champ() {
        let skin = parse("[core]\n    _default_ = white;black;bold+italic\n").expect("lisible");
        let pair = skin
            .color_pair("core", "_default_")
            .expect("couple lisible");
        assert_eq!(
            pair.attributes,
            vec!["bold".to_owned(), "italic".to_owned()]
        );
    }

    #[test]
    fn un_glyphe_de_cadre_est_un_caractere_unique() {
        let skin = parse(SKIN).expect("skin lisible");
        assert_eq!(skin.line_glyph("horiz"), Ok('─'));
        assert!(skin.line_glyph("inexistant").is_err());
    }

    #[test]
    fn un_glyphe_de_plusieurs_caracteres_est_refuse() {
        let skin = parse("[Lines]\n    horiz = ──\n").expect("skin lisible");
        let refusal = skin.line_glyph("horiz").expect_err("refus attendu");
        assert!(refusal.contains("caractère unique"), "{refusal}");
    }

    #[test]
    fn un_glyphe_vide_est_refuse() {
        let skin = parse("[Lines]\n    horiz =\n").expect("skin lisible");
        assert!(skin.line_glyph("horiz").is_err());
    }

    #[test]
    fn le_compte_de_clefs_couvre_toutes_les_sections() {
        let skin = parse(SKIN).expect("skin lisible");
        // [skin] 1, [Lines] 3, [core] 3, [editor] 2.
        assert_eq!(skin.key_count(), 9);
    }

    #[test]
    fn un_nom_de_couleur_invalide_remonte_avec_sa_clef() {
        // La fermeture d'erreur de `color_pair` nomme la section et la clef : sans elle,
        // le message dirait « chartreuse n'est pas une couleur » sans dire où.
        let skin = parse("[core]\n    _default_ = chartreuse;black\n").expect("lisible");
        let refusal = skin
            .color_pair("core", "_default_")
            .expect_err("refus attendu");
        assert!(refusal.contains("[core] _default_"), "{refusal}");
        assert!(refusal.contains("avant-plan"), "{refusal}");
    }

    #[test]
    fn un_fond_invalide_est_nomme_comme_tel() {
        let skin = parse("[core]\n    _default_ = white;chartreuse\n").expect("lisible");
        let refusal = skin
            .color_pair("core", "_default_")
            .expect_err("refus attendu");
        assert!(refusal.contains("fond"), "{refusal}");
    }

    #[test]
    fn un_attribut_invalide_est_nomme_comme_tel() {
        let skin = parse("[core]\n    _default_ = white;black;clignotant\n").expect("lisible");
        let refusal = skin
            .color_pair("core", "_default_")
            .expect_err("refus attendu");
        assert!(refusal.contains("attributs"), "{refusal}");
    }

    #[test]
    fn une_composante_de_cube_manquante_est_refusee() {
        // `rgb1` : une seule composante, là où il en faut trois.
        assert!(color("rgb1").is_err());
        assert!(color("rgb").is_err());
    }

    #[test]
    fn une_vraie_couleur_tronquee_est_refusee() {
        // Six caractères attendus, dont le dernier manque après découpage par octets.
        assert!(color("#abcde").is_err());
        assert!(color("#").is_err());
    }

    #[test]
    fn une_composante_tardive_invalide_est_refusee() {
        // Le premier chiffre est bon, le deuxième non : c'est le chemin que seule une
        // entrée de ce genre emprunte.
        assert!(color("#axc").is_err(), "deuxième chiffre");
        assert!(color("#aax").is_err(), "troisième chiffre");
        assert!(color("#abxdef").is_err(), "deuxième octet, quartet haut");
        assert!(color("#aabbxc").is_err(), "troisième octet, quartet haut");
        assert!(color("#aabbcx").is_err(), "troisième octet, quartet bas");
        assert!(color("rgb1x1").is_err(), "deuxième composante");
        assert!(color("rgb11x").is_err(), "troisième composante");
    }

    #[test]
    fn la_table_hexadecimale_est_complete_et_ordonnee() {
        assert_eq!(super::HEXADECIMAL_DIGITS.len(), 16);
        for (index, (_, value)) in super::HEXADECIMAL_DIGITS.iter().enumerate() {
            assert_eq!(usize::from(*value), index, "la table doit être ordonnée");
        }
    }

    #[test]
    fn une_majuscule_hexadecimale_est_admise() {
        assert_eq!(color("#ABCDEF"), color("#abcdef"));
    }

    #[test]
    fn une_lettre_n_est_pas_une_composante_de_cube() {
        // `a` vaut dix dans la table : c'est la borne qui le refuse, pas la table.
        let refusal = color("rgb0a0").expect_err("refus attendu");
        assert!(refusal.contains("0 à 5"), "{refusal}");
    }

    #[test]
    fn un_alias_en_boucle_sur_le_fond_est_refuse() {
        let text = "[aliases]\n    un = deux\n    deux = un\n[core]\n    _default_ = white;un\n";
        let skin = parse(text).expect("skin lisible");
        assert!(skin.color_pair("core", "_default_").is_err());
    }

    #[test]
    fn un_alias_en_boucle_sur_les_attributs_est_refuse() {
        let text = "[aliases]\n    un = deux\n    deux = un\n                    [core]\n    _default_ = white;black;un\n";
        let skin = parse(text).expect("skin lisible");
        assert!(skin.color_pair("core", "_default_").is_err());
    }

    #[test]
    fn un_fond_que_rien_ne_fournit_est_refuse() {
        // `_default_` est lui-même le recours : un fond vide n'a donc rien d'où hériter.
        let skin = parse("[core]\n    _default_ = white;\n").expect("skin lisible");
        let refusal = skin
            .color_pair("core", "_default_")
            .expect_err("refus attendu");
        assert!(refusal.contains("fond vide"), "{refusal}");
    }

    #[test]
    fn l_empreinte_distingue_deux_skins_et_se_reproduit() {
        let first = super::fingerprint("[core]\n    _default_ = white;black\n");
        let again = super::fingerprint("[core]\n    _default_ = white;black\n");
        let other = super::fingerprint("[core]\n    _default_ = black;white\n");
        assert_eq!(first, again, "la même entrée donne la même empreinte");
        assert_ne!(first, other, "deux skins différents se distinguent");
    }

    #[test]
    fn l_empreinte_d_un_texte_vide_est_la_graine() {
        assert_eq!(super::fingerprint(""), 0xcbf2_9ce4_8422_2325);
    }

    #[test]
    fn une_section_sans_clef_existe_quand_meme() {
        let skin = parse("[vide]\n[core]\n    _default_ = white;black\n").expect("lisible");
        assert_eq!(skin.key_count(), 1);
        assert_eq!(skin.raw("vide", "quoi"), None);
    }
}
