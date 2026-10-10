// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La lettre d'accès rapide d'un libellé.
//!
//! La notation est celle de Turbo Vision : `~O~K` désigne un libellé « OK » dont le `O`
//! est la lettre d'accès rapide. Elle est conservée telle quelle parce qu'elle est
//! compacte, qu'elle se lit dans le source, et qu'un développeur venant de Turbo Vision la
//! reconnaît sans explication.

/// Un libellé découpé autour de sa lettre d'accès rapide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirHotkeyLabel<'label> {
    /// Ce qui précède la lettre.
    pub before: &'label str,
    /// La lettre d'accès rapide, si le libellé en désigne une.
    pub hotkey: Option<&'label str>,
    /// Ce qui suit la lettre.
    pub after: &'label str,
}

impl AirHotkeyLabel<'_> {
    /// Le caractère d'accès rapide, replié en minuscule pour la comparaison.
    ///
    /// Une comparaison sensible à la casse ferait que `Alt+F` n'ouvrirait pas un menu
    /// marqué `~F~ichier`, ce qui est exactement l'inverse du but.
    #[must_use]
    pub fn hotkey_character(&self) -> Option<char> {
        self.hotkey?
            .chars()
            .next()
            .map(|character| character.to_ascii_lowercase())
    }
}

/// Découpe un libellé autour de sa lettre d'accès rapide.
///
/// Un libellé sans paire de `~` est rendu entier, sans lettre : c'est le cas normal d'une
/// étiquette, et ce n'est pas une erreur. Un `~` orphelin l'est tout autant — rien ne
/// justifie de refuser un libellé pour un caractère de ponctuation.
#[must_use]
pub fn split_hotkey(label: &str) -> AirHotkeyLabel<'_> {
    let whole = AirHotkeyLabel {
        before: label,
        hotkey: None,
        after: "",
    };
    let Some(opening) = label.find('~') else {
        return whole;
    };
    let after_opening = opening.saturating_add(1);
    let Some(rest) = label.get(after_opening..) else {
        return whole;
    };
    let Some(relative_closing) = rest.find('~') else {
        return whole;
    };
    if relative_closing == 0 {
        // `~~` ne désigne aucune lettre.
        return whole;
    }
    let closing = after_opening.saturating_add(relative_closing);
    let Some(before) = label.get(..opening) else {
        return whole;
    };
    let Some(hotkey) = label.get(after_opening..closing) else {
        return whole;
    };
    let Some(after) = label.get(closing.saturating_add(1)..) else {
        return whole;
    };
    AirHotkeyLabel {
        before,
        hotkey: Some(hotkey),
        after,
    }
}

#[cfg(test)]
mod tests {
    use super::{AirHotkeyLabel, split_hotkey};

    #[test]
    fn une_lettre_au_debut_est_detachee() {
        let label = split_hotkey("~O~K");
        assert_eq!(label.before, "");
        assert_eq!(label.hotkey, Some("O"));
        assert_eq!(label.after, "K");
    }

    #[test]
    fn une_lettre_au_milieu_est_detachee() {
        let label = split_hotkey("Enregi~s~trer");
        assert_eq!(label.before, "Enregi");
        assert_eq!(label.hotkey, Some("s"));
        assert_eq!(label.after, "trer");
    }

    #[test]
    fn un_libelle_sans_lettre_est_rendu_entier() {
        // Le cas normal d'une étiquette : ce n'est pas une erreur.
        let label = split_hotkey("Bonjour");
        assert_eq!(label.before, "Bonjour");
        assert_eq!(label.hotkey, None);
        assert_eq!(label.after, "");
    }

    #[test]
    fn un_tilde_orphelin_ne_designe_rien() {
        // Rien ne justifie de refuser un libellé pour un caractère de ponctuation.
        let label = split_hotkey("3 ~ 4");
        assert_eq!(label.before, "3 ~ 4");
        assert_eq!(label.hotkey, None);
    }

    #[test]
    fn deux_tildes_accoles_ne_designent_rien() {
        let label = split_hotkey("a~~b");
        assert_eq!(label.before, "a~~b");
        assert_eq!(label.hotkey, None);
    }

    #[test]
    fn un_libelle_vide_est_rendu_vide() {
        let label = split_hotkey("");
        assert_eq!(label.before, "");
        assert_eq!(label.hotkey, None);
        assert_eq!(label.after, "");
    }

    #[test]
    fn une_lettre_accentuee_est_acceptee() {
        let label = split_hotkey("~É~diter");
        assert_eq!(label.hotkey, Some("É"));
        assert_eq!(label.after, "diter");
    }

    #[test]
    fn la_lettre_se_compare_sans_tenir_compte_de_la_casse() {
        // Sans ce repli, `Alt+F` n'ouvrirait pas un menu marqué `~F~ichier`.
        assert_eq!(split_hotkey("~F~ichier").hotkey_character(), Some('f'));
        assert_eq!(split_hotkey("~f~ichier").hotkey_character(), Some('f'));
    }

    #[test]
    fn un_libelle_sans_lettre_n_a_pas_de_caractere_d_acces() {
        assert_eq!(split_hotkey("Fichier").hotkey_character(), None);
    }

    #[test]
    fn une_lettre_vide_n_a_pas_de_caractere_d_acces() {
        // Inatteignable depuis `split_hotkey`, qui refuse `~~` ; couvert directement.
        let label = AirHotkeyLabel {
            before: "",
            hotkey: Some(""),
            after: "",
        };
        assert_eq!(label.hotkey_character(), None);
    }
}
