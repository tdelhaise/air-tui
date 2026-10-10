// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le contrôle du DCO et de la paternité : `Signed-off-by` sur chaque commit, aucune
//! trace d'agent parmi les auteurs.
//!
//! # Les deux règles, et d'où elles viennent
//!
//! `docs/CONVENTIONS-fr.md` et `CLAUDE.md` les posent toutes les deux :
//!
//! 1. **`git commit -s` obligatoire.** Un commit sans `Signed-off-by` n'a pas de
//!    certificat d'origine, et c'est ce que le DCO exige ;
//! 2. **aucun trailer qui fasse d'un agent un auteur.** « Claude Code — ou tout autre
//!    agent — n'est pas un auteur. C'est un outil utilisé pour automatiser des tâches,
//!    comme l'est un compilateur. Seule la signature de l'auteur humain fait foi. »
//!
//! # Ce que ce contrôle ne regarde **pas**, et pourquoi
//!
//! Il ne vérifie pas que l'adresse du `Signed-off-by` est celle de l'auteur du commit, et
//! ce n'est pas un oubli : sur ce dépôt, dix-neuf des vingt-trois commits de `main` sont
//! des **fusions squash**, que GitHub signe de son adresse `…@users.noreply.github.com`
//! alors que la signature porte celle de l'auteur. Exiger l'accord ferait échouer presque
//! tout l'historique pour une raison qui n'est pas celle du DCO. La règle écrite demande
//! une signature, pas une concordance d'adresses.
//!
//! # Ce que ce contrôle ne peut pas empêcher
//!
//! Le commit qui a motivé son écriture — `806f7ae`, la fusion du jalon M7g — **n'aurait
//! pas été attrapé sur sa propre pull request**, parce qu'il n'existait pas encore : une
//! fusion squash est fabriquée au moment de la fusion, après le dernier passage de la CI
//! sur la branche. C'est pour cela que ce contrôle a deux portées : la **plage** sur une
//! pull request, où il garde les commits de la branche, et l'**historique entier** sur une
//! poussée vers `main`, où il est le seul moment qui voit le commit de fusion. Sur `main`,
//! il ne prévient donc pas : il **constate**, et c'est tout ce qu'un dépôt qui interdit le
//! force-push peut obtenir.

use std::collections::BTreeSet;
use std::path::Path;

/// Les séparateurs employés pour lire plusieurs commits d'un seul appel à `git`.
///
/// Deux caractères de contrôle qu'aucun message ne contient — et si l'un y figurait, le
/// découpage ne rendrait pas deux champs, ce que [`parse_commits`] **refuse** au lieu de
/// mal lire. Une ambiguïté détectée vaut mieux qu'une ambiguïté supposée absente.
const FIELD_SEPARATOR: char = '\u{1e}';
/// Le séparateur d'enregistrements.
const RECORD_SEPARATOR: char = '\u{1f}';

/// La clef du trailer de certification d'origine.
const SIGN_OFF_KEY: &str = "signed-off-by:";

/// Les clefs de trailer qui feraient d'un agent un auteur, et sont donc refusées.
///
/// Les deux sont celles que le client de Claude Code tente d'injecter de lui-même —
/// `Co-Authored-By: Claude …` et `Claude-Session: …`. `CLAUDE.md` nomme explicitement la
/// première ; la seconde est refusée pour la même raison, et le document le dit depuis que
/// ce contrôle existe. Une règle outillée qui dépasse sa règle écrite est le défaut
/// symétrique de celui que ce contrôle répare.
const AGENT_TRAILER_KEYS: &[&str] = &["claude-session:"];

/// Les marques qui, dans une valeur de `Co-Authored-By`, désignent un agent.
///
/// La liste est courte et explicite plutôt que large et devinée : elle nomme ce qui a
/// réellement été observé dans ce dépôt et dans les messages que son client propose. Un
/// co-auteur **humain** reste légitime, et aucun document ne l'interdit.
const AGENT_MARKS: &[&str] = &["claude", "anthropic"];

/// La clef du trailer de co-paternité.
const CO_AUTHOR_KEY: &str = "co-authored-by:";

/// Les commits que ce contrôle **pardonne**, avec la raison écrite à côté.
///
/// Un pardon nommé se discute ; un contrôle dont on a restreint la portée pour qu'il passe
/// ne se discute plus, parce que plus personne ne sait ce qu'il ne regarde pas. Le compte
/// rendu annonce donc toujours combien de pardons il a appliqués, à chaque exécution.
const PARDONED: &[(&str, &str)] = &[(
    "806f7ae009edc9b95e7a11389e46034e8da71070",
    "fusion squash du jalon M7g : ses quatre commits de branche portaient tous le \
     Signed-off-by, et un corps de fusion écrit à la main a remplacé celui que GitHub \
     aurait construit à partir d'eux. Irréparable sans force-push sur `main`, que ce dépôt \
     interdit sans exception",
)];

/// L'étendue du contrôle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Les commits propres à la branche, depuis la base de comparaison.
    Range {
        /// La référence de comparaison.
        base: String,
    },
    /// Tout l'historique joignable depuis `HEAD`.
    All,
}

/// Un commit, réduit à ce que le contrôle regarde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    /// L'empreinte complète.
    pub hash: String,
    /// Le message entier, sujet et corps.
    pub body: String,
}

/// Un manquement relevé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Le commit en cause, en forme courte.
    pub subject: String,
    /// Ce qui manque, ou ce qui est en trop.
    pub detail: String,
}

/// Le compte rendu d'un contrôle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// L'étendue, telle qu'elle a été résolue, pour que le compte rendu dise ce qu'il a vu.
    pub scope: String,
    /// Le nombre de commits examinés.
    pub examined: usize,
    /// Le nombre de commits pardonnés, et donc **non** examinés.
    pub pardoned: usize,
    /// Les manquements.
    pub violations: Vec<Violation>,
}

/// Contrôle les commits d'une étendue.
///
/// # Errors
///
/// Si `git` échoue, si la référence de comparaison est introuvable, ou si la sortie de
/// `git` ne se découpe pas — une sortie qu'on ne comprend pas ne doit pas passer en
/// silence.
pub fn check(repository: &Path, scope: &Scope) -> Result<Report, String> {
    let (range, described) = match *scope {
        Scope::All => (None, "tout l'historique joignable depuis HEAD".to_owned()),
        Scope::Range { ref base } => {
            let merge_base = git(repository, &["merge-base", base, "HEAD"]).map_err(|cause| {
                format!(
                    "impossible de situer {base} par rapport à HEAD : {cause}\n\
                     Sur un clone superficiel, récupérer la base d'abord :\n  \
                     git fetch --depth=1 origin main"
                )
            })?;
            let merge_base = merge_base.trim().to_owned();
            (Some(format!("{merge_base}..HEAD")), format!("{base}..HEAD"))
        }
    };

    let format = format!("--format=%H{FIELD_SEPARATOR}%B{RECORD_SEPARATOR}");
    let mut arguments = vec!["log", format.as_str()];
    if let Some(ref value) = range {
        arguments.push(value.as_str());
    }
    let output = git(repository, &arguments)?;
    let commits = parse_commits(&output)?;

    let pardoned: BTreeSet<&str> = PARDONED.iter().map(|(hash, _)| *hash).collect();
    let mut violations = Vec::new();
    let mut examined = 0usize;
    let mut forgiven = 0usize;
    for commit in &commits {
        if pardoned.contains(commit.hash.as_str()) {
            forgiven = forgiven.saturating_add(1);
            continue;
        }
        examined = examined.saturating_add(1);
        violations.extend(violations_of(commit));
    }

    Ok(Report {
        scope: described,
        examined,
        pardoned: forgiven,
        violations,
    })
}

/// Découpe la sortie de `git log` en commits.
///
/// # Errors
///
/// Si un enregistrement ne porte pas exactement deux champs — ce qui arriverait si un
/// message contenait l'un des deux séparateurs. Le cas est refusé plutôt que mal lu.
fn parse_commits(output: &str) -> Result<Vec<Commit>, String> {
    let mut commits = Vec::new();
    for record in output.split(RECORD_SEPARATOR) {
        let record = record.trim_start_matches('\n');
        if record.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = record.split(FIELD_SEPARATOR).collect();
        let [hash, body] = *fields.as_slice() else {
            return Err(format!(
                "la sortie de git ne se découpe pas : {} champ(s) dans un enregistrement \
                 qui doit en porter deux. Un message de commit contient probablement un \
                 caractère de contrôle U+001E ou U+001F.",
                fields.len()
            ));
        };
        commits.push(Commit {
            hash: hash.trim().to_owned(),
            body: body.to_owned(),
        });
    }
    Ok(commits)
}

/// Les manquements d'un seul commit.
fn violations_of(commit: &Commit) -> Vec<Violation> {
    let short = short_hash(&commit.hash);
    let subject = commit.body.lines().next().unwrap_or("").trim().to_owned();
    let named = format!("{short} « {} »", truncate(&subject, 48));
    let mut violations = Vec::new();

    if !commit.body.lines().any(is_sign_off) {
        violations.push(Violation {
            subject: named.clone(),
            detail: "aucun Signed-off-by : le DCO exige `git commit -s`".to_owned(),
        });
    }
    for line in commit.body.lines() {
        if let Some(detail) = agent_trace(line) {
            violations.push(Violation {
                subject: named.clone(),
                detail,
            });
        }
    }
    violations
}

/// Vrai si la ligne est un `Signed-off-by` suivi d'une valeur.
///
/// La clef est comparée sans tenir compte de la casse, comme `git interpret-trailers` le
/// fait : `Signed-Off-By:` est le même trailer, et le refuser serait refuser un commit
/// conforme.
fn is_sign_off(line: &str) -> bool {
    let trimmed = line.trim();
    let Some(value) = key_value(trimmed, SIGN_OFF_KEY) else {
        return false;
    };
    !value.is_empty()
}

/// La trace d'agent que porte une ligne, s'il y en a une.
fn agent_trace(line: &str) -> Option<String> {
    let trimmed = line.trim();
    for key in AGENT_TRAILER_KEYS {
        if key_value(trimmed, key).is_some() {
            return Some(format!(
                "le trailer « {} » fait d'un agent un auteur : seule la signature de \
                 l'auteur humain fait foi",
                trimmed_key(trimmed)
            ));
        }
    }
    let value = key_value(trimmed, CO_AUTHOR_KEY)?;
    let lowered = value.to_lowercase();
    let mark = AGENT_MARKS.iter().find(|mark| lowered.contains(*mark))?;
    Some(format!(
        "Co-Authored-By nomme « {value} », où « {mark} » désigne un agent : un agent n'est \
         pas un auteur, c'est un outil"
    ))
}

/// La valeur d'un trailer, si la ligne porte cette clef.
fn key_value<'line>(line: &'line str, key: &str) -> Option<&'line str> {
    let (candidate, value) = line.split_at_checked(key.len())?;
    if candidate.to_lowercase() == key {
        return Some(value.trim());
    }
    None
}

/// La clef d'une ligne de trailer, pour la citer telle qu'elle est écrite.
fn trimmed_key(line: &str) -> &str {
    match line.split_once(':') {
        Some((key, _)) => key,
        // Inatteignable : seule une ligne reconnue comme trailer arrive ici, et une clef
        // reconnue se termine par un deux-points.
        None => line,
    }
}

/// L'empreinte courte, sur sept caractères comme `git log --oneline`.
fn short_hash(hash: &str) -> &str {
    match hash.get(..7) {
        Some(short) => short,
        None => hash,
    }
}

/// Un texte tronqué à une largeur, avec des points de suspension s'il a été coupé.
fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let kept: String = text.chars().take(width).collect();
    format!("{kept}…")
}

/// Lance `git` dans le dépôt et rend sa sortie standard.
///
/// # Errors
///
/// Si le processus ne peut pas être lancé, ou s'il rend un code d'échec.
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

/// Les pardons, pour que le compte rendu les nomme.
#[must_use]
pub fn pardons() -> Vec<(String, String)> {
    PARDONED
        .iter()
        .map(|(hash, reason)| (short_hash(hash).to_owned(), (*reason).to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        Commit, FIELD_SEPARATOR, PARDONED, RECORD_SEPARATOR, agent_trace, is_sign_off,
        parse_commits, short_hash, truncate, violations_of,
    };

    /// Un commit conforme : un sujet, un corps, une signature.
    fn conforme() -> Commit {
        Commit {
            hash: "0123456789abcdef0123456789abcdef01234567".to_owned(),
            body: "feat: une chose\n\nLe corps.\n\n\
                   Signed-off-by: Thierry DELHAISE <thierry.delhaise@air-desktop.org>\n"
                .to_owned(),
        }
    }

    #[test]
    fn un_commit_signe_ne_manque_de_rien() {
        assert_eq!(violations_of(&conforme()), Vec::new());
    }

    #[test]
    fn un_commit_sans_signature_est_refuse() {
        let commit = Commit {
            body: "feat: une chose\n\nLe corps.\n".to_owned(),
            ..conforme()
        };
        let violations = violations_of(&commit);
        assert_eq!(violations.len(), 1);
        let first = violations.first().expect("un manquement");
        assert!(first.detail.contains("Signed-off-by"), "{first:?}");
        assert!(
            first.subject.contains("0123456") && first.subject.contains("une chose"),
            "le manquement doit nommer le commit : {first:?}"
        );
    }

    #[test]
    fn la_casse_de_la_clef_ne_decide_pas() {
        // `git interpret-trailers` compare les clefs sans tenir compte de la casse :
        // refuser `Signed-Off-By` serait refuser un commit conforme.
        for line in [
            "Signed-off-by: A <a@b.c>",
            "Signed-Off-By: A <a@b.c>",
            "SIGNED-OFF-BY: A <a@b.c>",
        ] {
            assert!(is_sign_off(line), "pour « {line} »");
        }
    }

    #[test]
    fn une_clef_sans_valeur_ne_signe_rien() {
        // Un `Signed-off-by:` nu certifie quoi, et de qui ? Rien.
        assert!(!is_sign_off("Signed-off-by:"));
        assert!(!is_sign_off("Signed-off-by:    "));
    }

    #[test]
    fn une_ligne_qui_commence_autrement_n_est_pas_un_trailer() {
        assert!(!is_sign_off("Pas de Signed-off-by: ici"));
        assert!(!is_sign_off("# Signed-off-by: A <a@b.c>"));
    }

    #[test]
    fn une_ligne_plus_courte_que_la_clef_ne_panique_pas() {
        // Le découpage se fait sur la longueur de la clef : sans borne, une ligne plus
        // courte paniquerait. C'est le cas qu'un corps de commit produit à chaque ligne
        // vide.
        assert!(!is_sign_off(""));
        assert!(!is_sign_off("S"));
        assert!(agent_trace("Co").is_none());
    }

    #[test]
    fn un_co_auteur_agent_est_refuse() {
        let detail = agent_trace("Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>")
            .expect("refus attendu");
        assert!(detail.contains("Claude"), "{detail}");
        assert!(detail.contains("agent"), "{detail}");
    }

    #[test]
    fn un_co_auteur_humain_reste_legitime() {
        // Aucun document du dépôt n'interdit un co-auteur humain : seul un **agent** est
        // refusé. Un contrôle plus large que sa règle écrite est le défaut symétrique.
        assert!(agent_trace("Co-Authored-By: Ada Lovelace <ada@example.org>").is_none());
    }

    #[test]
    fn un_message_qui_cite_le_trailer_n_en_porte_pas_un() {
        // Le cas que les messages de commit de ce contrôle produisent eux-mêmes : ils
        // citent `Co-Authored-By` et `Claude-Session` pour dire qu'ils les refusent. Une
        // clef n'est un trailer qu'en **début** de ligne et suivie d'un deux-points.
        for line in [
            "`Co-Authored-By` nommant Claude ou Anthropic, et `Claude-Session`.",
            "il refuse Co-Authored-By quand la valeur nomme Claude",
            "voir Claude-Session dans la liste",
        ] {
            assert!(agent_trace(line).is_none(), "pour « {line} »");
        }
    }

    #[test]
    fn le_trailer_de_session_est_refuse() {
        let detail =
            agent_trace("Claude-Session: https://claude.ai/code/session_x").expect("refus attendu");
        assert!(detail.contains("Claude-Session"), "{detail}");
    }

    #[test]
    fn un_commit_peut_cumuler_les_manquements() {
        let commit = Commit {
            body: "feat: deux fois\n\n\
                   Co-Authored-By: Claude <noreply@anthropic.com>\n\
                   Claude-Session: https://claude.ai/code/session_x\n"
                .to_owned(),
            ..conforme()
        };
        let violations = violations_of(&commit);
        assert_eq!(violations.len(), 3, "signature absente plus deux trailers");
    }

    #[test]
    fn la_sortie_de_git_se_decoupe_en_commits() {
        let output = format!(
            "abc{FIELD_SEPARATOR}feat: un\n\nSigned-off-by: A <a@b.c>\n{RECORD_SEPARATOR}\
             def{FIELD_SEPARATOR}feat: deux\n{RECORD_SEPARATOR}"
        );
        let commits = parse_commits(&output).expect("découpage");
        assert_eq!(commits.len(), 2);
        assert_eq!(
            commits.first().map(|commit| commit.hash.as_str()),
            Some("abc")
        );
        assert_eq!(
            commits.get(1).map(|commit| commit.hash.as_str()),
            Some("def")
        );
    }

    #[test]
    fn une_sortie_vide_ne_rend_aucun_commit() {
        // Le cas d'une plage vide : une poussée vers `main` où la base **est** HEAD.
        assert_eq!(parse_commits("").expect("découpage"), Vec::new());
        assert_eq!(parse_commits("\n\n").expect("découpage"), Vec::new());
    }

    #[test]
    fn un_enregistrement_ambigu_est_refuse_et_non_mal_lu() {
        // Si un message portait l'un des deux séparateurs, le découpage rendrait trois
        // champs. Le refuser nomme l'ambiguïté ; la deviner la cacherait.
        let output = format!("abc{FIELD_SEPARATOR}feat{FIELD_SEPARATOR}un{RECORD_SEPARATOR}");
        let refusal = parse_commits(&output).expect_err("refus attendu");
        assert!(refusal.contains("3 champ"), "{refusal}");
        assert!(refusal.contains("U+001E"), "{refusal}");
    }

    #[test]
    fn les_pardons_portent_une_empreinte_complete_et_une_raison() {
        // Un pardon sans raison écrite est une portée restreinte en douce, c'est-à-dire
        // exactement ce que ce contrôle existe pour empêcher.
        for (hash, reason) in PARDONED {
            assert_eq!(
                hash.len(),
                40,
                "« {hash} » n'est pas une empreinte complète"
            );
            assert!(
                hash.chars().all(|digit| digit.is_ascii_hexdigit()),
                "« {hash} » n'est pas hexadécimale"
            );
            assert!(reason.len() > 40, "la raison de {hash} est trop courte");
        }
    }

    #[test]
    fn l_empreinte_courte_tient_sur_sept_caracteres() {
        assert_eq!(short_hash("0123456789abcdef"), "0123456");
        // Plus courte que sept : rendue telle quelle plutôt que tronquée au hasard.
        assert_eq!(short_hash("abc"), "abc");
    }

    #[test]
    fn un_sujet_trop_long_est_tronque() {
        assert_eq!(truncate("court", 10), "court");
        assert_eq!(truncate("abcdefghijkl", 4), "abcd…");
        // Les accents comptent pour un caractère, pas pour deux octets.
        assert_eq!(truncate("éééé", 2), "éé…");
    }
}
