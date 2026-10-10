// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le contrôle des commentaires `// SAFETY:` et de la frontière `unsafe` (ADR-011).
//!
//! # Deux règles, et la seconde n'était pas demandée
//!
//! `docs/CONVENTIONS-fr.md` demande « un `// SAFETY:` par bloc `unsafe` ». `CLAUDE.md`
//! demande autre chose, que rien n'outillait : « `unsafe` et les appels C vivent dans
//! `air-tui-platform-posix`, et nulle part ailleurs ». Les deux crates du cœur portent
//! `#![forbid(unsafe_code)]`, donc le compilateur tient la frontière **pour elles** — mais
//! pas pour `redit`, `rmc`, `xtask` ni les cibles de fuzzing, qui ne la portent pas.
//!
//! Ce contrôle tient donc les deux, et la seconde est le seul apport réel : la première
//! ne trouve rien aujourd'hui, et c'est mesuré.
//!
//! # Ce qu'est « un bloc `unsafe` », et ce qui n'en est pas un
//!
//! La question n'a pas une seule réponse syntaxique, et c'est ce qui fait l'intérêt de ce
//! contrôle. Cinq formes portent le mot-clé :
//!
//! - `unsafe { … }` — une **promesse** faite au compilateur sur du code précis. C'est
//!   celle qui exige un `// SAFETY:`, parce que c'est la seule qui affirme quelque chose
//!   que le compilateur ne vérifie pas ;
//! - `unsafe extern "C" { … }` — une **déclaration** de symboles étrangers. Depuis
//!   l'édition 2024, le mot-clé y est obligatoire : il n'affirme rien sur un appel, il
//!   introduit des signatures. L'engagement se prend à l'appel, donc dans un `unsafe { }`,
//!   et c'est là que le commentaire a un sens ;
//! - `unsafe fn` — l'engagement est demandé à l'**appelant**, et il se documente par une
//!   section rustdoc `# Safety`, ce que `clippy::missing_safety_doc` exige déjà ;
//! - `unsafe impl` et `unsafe trait` — mêmes raisons.
//!
//! Les quatre dernières sont **comptées et annoncées** plutôt qu'exemptées en silence : un
//! contrôle qui taît ce qu'il ne regarde pas donne la même assurance qu'un contrôle absent.
//!
//! # Pourquoi un balayage lexical plutôt qu'un `grep`
//!
//! Parce qu'un `grep` se trompe, et on peut le montrer : `xtask/src/naming.rs` porte le
//! mot `unsafe` deux fois — une fois dans sa liste de mots-clés, une fois dans un source
//! d'essai. Les deux sont des **chaînes de caractères**. Un contrôle de frontière fondé
//! sur `grep` déclarerait donc `xtask` en infraction, et on apprendrait à l'ignorer.
//!
//! Le balayage est celui de `check-naming`, partagé plutôt que réécrit.

use std::path::Path;

use crate::naming::{Lexeme, Token, tokenize};

/// La crate où `unsafe` est admis (ADR-011).
const UNSAFE_CRATE: &str = "crates/air-tui-platform-posix";

/// Le marqueur qu'un bloc `unsafe` doit porter au-dessus de lui.
const SAFETY_MARKER: &str = "// SAFETY:";

/// Le nombre de lignes au-dessus d'un bloc où le marqueur est cherché.
///
/// Trois, et non une : une justification tient rarement sur une ligne, et le marqueur est
/// alors suivi de sa suite — ou précédé d'une ligne vide quand le bloc est au milieu d'une
/// fonction. Chercher sur une seule ligne obligerait à écrire court là où il faut écrire
/// juste.
const LOOKBACK: usize = 3;

/// Ce qu'une forme `unsafe` est.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// `unsafe { … }` — une promesse sur du code, qui exige un `// SAFETY:`.
    Block,
    /// `unsafe extern` — une déclaration de symboles étrangers.
    Extern,
    /// `unsafe fn` — l'engagement est demandé à l'appelant.
    Function,
    /// `unsafe impl` ou `unsafe trait`.
    Implementation,
    /// Le mot-clé suivi d'autre chose — inattendu, donc signalé.
    Unknown,
}

impl Form {
    /// Le nom de la forme, pour le compte rendu.
    const fn label(self) -> &'static str {
        match self {
            Self::Block => "unsafe { }",
            Self::Extern => "unsafe extern",
            Self::Function => "unsafe fn",
            Self::Implementation => "unsafe impl / unsafe trait",
            Self::Unknown => "unsafe suivi d'autre chose",
        }
    }
}

/// Un manquement relevé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Le fichier et la ligne en cause.
    pub subject: String,
    /// Ce qui manque, ou ce qui est en trop.
    pub detail: String,
}

/// Le compte rendu d'un contrôle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Le nombre de fichiers examinés.
    pub examined: usize,
    /// Le nombre de blocs `unsafe { }` trouvés.
    pub blocks: usize,
    /// Les autres formes trouvées, comptées par forme.
    pub other_forms: Vec<(&'static str, usize)>,
    /// Les manquements.
    pub violations: Vec<Violation>,
}

/// Contrôle les commentaires `// SAFETY:` et la frontière `unsafe`.
///
/// # Errors
///
/// Si un répertoire ou un fichier est illisible.
pub fn check(repository: &Path) -> Result<Report, String> {
    let mut files = Vec::new();
    for root in crate::naming::ROOTS {
        let directory = repository.join(root);
        if !directory.is_dir() {
            continue;
        }
        crate::naming::collect_rust_files(&directory, &mut files)?;
    }
    files.sort();

    let mut report = Report {
        examined: files.len(),
        blocks: 0,
        other_forms: Vec::new(),
        violations: Vec::new(),
    };
    for file in &files {
        let source = std::fs::read_to_string(file)
            .map_err(|cause| format!("{} illisible : {cause}", file.display()))?;
        let relative = file
            .strip_prefix(repository)
            .unwrap_or(file)
            .display()
            .to_string();
        let lines: Vec<&str> = source.lines().collect();
        let inside_frontier = relative.replace('\\', "/").starts_with(UNSAFE_CRATE);
        for (line, form) in unsafe_forms(&tokenize(&source)) {
            let at = format!("{relative}:{line}");
            if !inside_frontier {
                report.violations.push(Violation {
                    subject: at.clone(),
                    detail: format!(
                        "« {} » hors de {UNSAFE_CRATE} : l'ADR-011 y enferme tout unsafe, \
                         et les deux crates du cœur ne le tiennent que par leur \
                         #![forbid(unsafe_code)]",
                        form.label()
                    ),
                });
                continue;
            }
            match form {
                Form::Block => {
                    report.blocks = report.blocks.saturating_add(1);
                    if !has_safety_comment(&lines, line) {
                        report.violations.push(Violation {
                            subject: at,
                            detail: format!(
                                "bloc unsafe sans « {SAFETY_MARKER} » dans les {LOOKBACK} \
                                 lignes au-dessus"
                            ),
                        });
                    }
                }
                other => count(&mut report.other_forms, other.label()),
            }
        }
    }
    Ok(report)
}

/// Incrémente le compte d'une forme.
fn count(forms: &mut Vec<(&'static str, usize)>, label: &'static str) {
    if let Some(entry) = forms.iter_mut().find(|(name, _)| *name == label) {
        entry.1 = entry.1.saturating_add(1);
        return;
    }
    forms.push((label, 1));
}

/// Les formes `unsafe` d'un source, avec leur ligne.
fn unsafe_forms(tokens: &[Token]) -> Vec<(usize, Form)> {
    let mut found = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.lexeme != Lexeme::Identifier("unsafe".to_owned()) {
            continue;
        }
        let next = tokens
            .get(index.saturating_add(1))
            .map(|token| &token.lexeme);
        let form = match next {
            Some(Lexeme::Symbol(symbol)) if symbol == "{" => Form::Block,
            Some(Lexeme::Identifier(word)) if word == "extern" => Form::Extern,
            Some(Lexeme::Identifier(word)) if word == "fn" => Form::Function,
            Some(Lexeme::Identifier(word)) if word == "impl" || word == "trait" => {
                Form::Implementation
            }
            _ => Form::Unknown,
        };
        found.push((token.line, form));
    }
    found
}

/// Vrai si le marqueur est dans les lignes qui précèdent celle du bloc.
fn has_safety_comment(lines: &[&str], line: usize) -> bool {
    let Some(position) = line.checked_sub(1) else {
        // Inatteignable : les lignes du balayage commencent à un.
        return false;
    };
    let first = position.saturating_sub(LOOKBACK);
    lines
        .get(first..position)
        .unwrap_or_default()
        .iter()
        .any(|candidate| candidate.trim_start().starts_with(SAFETY_MARKER))
}

#[cfg(test)]
mod tests {
    use super::{Form, has_safety_comment, unsafe_forms};
    use crate::naming::tokenize;

    /// Les formes trouvées dans un source, sans leurs lignes.
    fn forms(source: &str) -> Vec<Form> {
        unsafe_forms(&tokenize(source))
            .into_iter()
            .map(|(_, form)| form)
            .collect()
    }

    #[test]
    fn un_bloc_est_reconnu() {
        assert_eq!(forms("fn a() { unsafe { b() } }"), vec![Form::Block]);
    }

    #[test]
    fn les_quatre_autres_formes_sont_distinguees() {
        assert_eq!(forms("unsafe extern \"C\" { fn b(); }"), vec![Form::Extern]);
        assert_eq!(forms("unsafe fn b() {}"), vec![Form::Function]);
        assert_eq!(
            forms("unsafe impl Send for A {}"),
            vec![Form::Implementation]
        );
        assert_eq!(forms("unsafe trait A {}"), vec![Form::Implementation]);
    }

    #[test]
    fn une_forme_inattendue_est_signalee_et_non_ignoree() {
        // Si une forme nouvelle du langage apparaissait, la taire serait pire que la
        // signaler : on ne saurait pas qu'elle n'est pas contrôlée.
        assert_eq!(forms("unsafe"), vec![Form::Unknown]);
    }

    #[test]
    fn le_mot_dans_une_chaine_n_est_pas_du_code() {
        // C'est la raison d'être du balayage : `xtask/src/naming.rs` porte deux fois le
        // mot `unsafe` dans des chaînes, et un `grep` déclarerait `xtask` en infraction.
        assert_eq!(forms(r#"const K: &[&str] = &["unsafe", "use"];"#), vec![]);
        assert_eq!(forms(r#"let s = "unsafe { }";"#), vec![]);
    }

    #[test]
    fn le_mot_dans_un_commentaire_n_est_pas_du_code() {
        assert_eq!(
            forms("// un bloc unsafe { } en commentaire\nfn a() {}"),
            vec![]
        );
        assert_eq!(forms("/* unsafe { } */ fn a() {}"), vec![]);
        assert_eq!(forms("//! Un module où `unsafe` est interdit.\n"), vec![]);
    }

    #[test]
    fn un_identifiant_qui_contient_le_mot_n_en_est_pas_un() {
        assert_eq!(forms("fn unsafely() {}"), vec![]);
        assert_eq!(forms("let unsafe_count = 0;"), vec![]);
    }

    #[test]
    fn le_marqueur_est_trouve_juste_au_dessus() {
        let lines = vec![
            "    // SAFETY: la précondition tient.",
            "    unsafe { b() }",
        ];
        assert!(has_safety_comment(&lines, 2));
    }

    #[test]
    fn le_marqueur_est_trouve_trois_lignes_au_dessus() {
        // Une justification tient rarement sur une ligne.
        let lines = vec![
            "    // SAFETY: le pointeur vient de `as_mut_ptr`, donc il est aligné et",
            "    // valide pour la durée de l'appel.",
            "",
            "    unsafe { b() }",
        ];
        assert!(has_safety_comment(&lines, 4));
    }

    #[test]
    fn le_marqueur_quatre_lignes_au_dessus_ne_compte_pas() {
        // La borne existe pour qu'un marqueur d'un autre bloc ne couvre pas celui-ci.
        let lines = vec!["// SAFETY: autre chose.", "", "", "", "unsafe { b() }"];
        assert!(!has_safety_comment(&lines, 5));
    }

    #[test]
    fn un_bloc_sans_marqueur_est_refuse() {
        let lines = vec!["    let value = 1;", "    unsafe { b() }"];
        assert!(!has_safety_comment(&lines, 2));
    }

    #[test]
    fn un_marqueur_en_fin_de_ligne_ne_compte_pas() {
        // Le marqueur doit **commencer** la ligne : en queue d'une ligne de code, il se
        // perdrait à la première reformulation.
        let lines = vec!["    let value = 1; // SAFETY: non", "    unsafe { b() }"];
        assert!(!has_safety_comment(&lines, 2));
    }

    #[test]
    fn un_bloc_en_premiere_ligne_n_a_rien_au_dessus() {
        let lines = vec!["unsafe { b() }"];
        assert!(!has_safety_comment(&lines, 1));
    }

    #[test]
    fn les_lignes_sont_rendues_dans_l_ordre_du_source() {
        let source = "fn a() {\n    unsafe { b() }\n}\nunsafe fn c() {}\n";
        let found = unsafe_forms(&tokenize(source));
        assert_eq!(found, vec![(2, Form::Block), (4, Form::Function)]);
    }
}
