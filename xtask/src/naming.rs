// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le contrôle de nommage de l'ADR-004 : aucune abréviation dans un nom **déclaré**.
//!
//! # Ce qu'il inspecte
//!
//! Les **sites de déclaration**, et eux seuls : ce que le dépôt nomme, pas ce qu'il
//! appelle. `value.len()` est l'emploi d'un nom imposé par `std` ; `fn len()` serait un
//! nom choisi ici. La distinction est la seule qui rende le contrôle utilisable : sans
//! elle, `u32::MAX` et `.min(other)` feraient échouer chaque fichier.
//!
//! Sont considérés comme déclarations : `fn`, `struct`, `enum`, `union`, `trait`, `type`,
//! `mod`, `const`, `static`, les motifs de `let` et de `for`, les paramètres de fermeture,
//! et tout identifiant suivi de `:` en position de champ ou de paramètre.
//!
//! # Ce qu'il n'inspecte pas, et pourquoi
//!
//! - Les **liaisons de bras de `match`** — `Some(index) => …`. Les reconnaître demanderait
//!   de distinguer un motif d'une expression, donc une analyse syntaxique que ce contrôle
//!   ne porte pas. C'est son angle mort connu ; la règle reste tenue à la main là.
//! - Les corps de bloc `extern "C"` : c'est l'**exception 2** des conventions. Un symbole
//!   C recopié verbatim ne se traduit pas, sinon on ne peut plus le confronter à sa page
//!   de manuel.
//! - Les noms imposés par `std` ou rustdoc — [`IMPOSED_NAMES`] : c'est l'**exception 1**.
//!
//! # Pourquoi un balayage lexical et non `syn`
//!
//! ADR-005 : aucune dépendance tierce, ici non plus. Un balayage qui écarte commentaires,
//! chaînes et littéraux suffit pour des sites de déclaration, et son angle mort est
//! nommable — ce qui vaut mieux qu'un contrôle qu'on croit complet.

use std::path::{Path, PathBuf};

/// Le glossaire contraignant de `docs/CONVENTIONS-fr.md`, transcrit ligne pour ligne.
///
/// La première valeur est l'abréviation interdite, la seconde le nom attendu. Toute
/// modification de ce tableau doit suivre celle du glossaire, jamais l'inverse : le
/// document fait foi.
const FORBIDDEN_WORDS: &[(&str, &str)] = &[
    ("fg", "foreground"),
    ("bg", "background"),
    ("attr", "attribute"),
    ("seq", "sequence"),
    ("esc", "escape"),
    ("col", "column"),
    ("pos", "position"),
    ("win", "window"),
    ("btn", "button"),
    ("msg", "message"),
    ("err", "error"),
    ("buf", "buffer"),
    ("len", "length"),
    ("src", "source"),
    ("dst", "destination"),
    ("prev", "previous"),
    ("min", "minimum"),
    ("max", "maximum"),
    ("cnt", "count"),
    ("num", "number"),
    ("idx", "index"),
    ("elem", "element"),
    ("cfg", "configuration"),
    ("init", "initialize"),
    ("dir", "directory"),
    ("gen", "generation"),
    ("ctx", "context"),
    ("cur", "current ou cursor"),
    ("mods", "modifiers"),
    ("res", "result"),
    ("val", "value"),
    ("tmp", "temporary"),
    ("ptr", "le nom du pointé"),
];

/// Les noms déclarés que `std` ou rustdoc imposent — exception 1 des conventions.
///
/// `len` vient du trait `ExactSizeIterator`, `fmt` de `Display::fmt` : les renommer
/// briserait l'implémentation du trait. La liste est volontairement courte ; l'allonger
/// demande de pouvoir citer le trait qui impose le nom.
const IMPOSED_NAMES: &[&str] = &["len", "fmt"];

/// Les racines parcourues, relatives à la racine du dépôt.
pub(crate) const ROOTS: &[&str] = &["crates", "xtask/src", "fuzz/fuzz_targets"];

/// Les mots-clés du langage, qu'un nom déclaré ne peut pas être.
const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "type", "union",
    "unsafe", "use", "where", "while",
];

/// Un manquement relevé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Le fichier, relatif à la racine du dépôt.
    pub file: String,
    /// La ligne, à partir de 1.
    pub line: usize,
    /// Le nom déclaré qui porte l'abréviation.
    pub name: String,
    /// L'abréviation interdite.
    pub word: &'static str,
    /// Le nom attendu à sa place.
    pub expected: &'static str,
}

/// Le compte rendu d'un parcours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Le nombre de fichiers inspectés.
    pub files: usize,
    /// Le nombre de sites de déclaration examinés.
    pub declarations: usize,
    /// Les manquements, dans l'ordre du parcours.
    pub violations: Vec<Violation>,
}

/// Inspecte le dépôt et rend ce qu'il a trouvé.
///
/// # Errors
///
/// Rend une erreur si une racine est illisible — un répertoire absent n'en est pas une :
/// `fuzz/` n'existe pas sur une machine qui n'a pas de toolchain nightly.
pub fn check(repository: &Path) -> Result<Report, String> {
    let mut files = Vec::new();
    for root in ROOTS {
        let directory = repository.join(root);
        if !directory.is_dir() {
            continue;
        }
        collect_rust_files(&directory, &mut files)?;
    }
    files.sort();

    let mut report = Report {
        files: files.len(),
        declarations: 0,
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
        for declaration in declarations(&tokenize(&source)) {
            report.declarations = report.declarations.saturating_add(1);
            if let Some((word, expected)) = forbidden_word_in(&declaration.name) {
                report.violations.push(Violation {
                    file: relative.clone(),
                    line: declaration.line,
                    name: declaration.name,
                    word,
                    expected,
                });
            }
        }
    }
    Ok(report)
}

/// Ajoute récursivement les fichiers `.rs` d'un répertoire.
pub(crate) fn collect_rust_files(directory: &Path, found: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(directory)
        .map_err(|cause| format!("{} illisible : {cause}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|cause| format!("entrée illisible : {cause}"))?;
        let path = entry.path();
        if path.is_dir() {
            // `target/` peut apparaître dans une crate : du code généré par des tiers,
            // qui n'est pas soumis aux conventions de ce dépôt.
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            collect_rust_files(&path, found)?;
            continue;
        }
        if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

/// L'abréviation interdite que porte un nom, s'il en porte une.
fn forbidden_word_in(name: &str) -> Option<(&'static str, &'static str)> {
    if IMPOSED_NAMES.contains(&name) {
        return None;
    }
    for word in words_of(name) {
        for (forbidden, expected) in FORBIDDEN_WORDS {
            if word == *forbidden {
                return Some((*forbidden, *expected));
            }
        }
    }
    None
}

/// Découpe un nom en mots, en minuscules.
///
/// `desired_column` donne `["desired", "column"]`, `AirSgrEncoder` donne
/// `["air", "sgr", "encoder"]`, et `HTMLParser` donne `["html", "parser"]` — la coupure
/// d'un acronyme se fait avant sa dernière majuscule quand une minuscule suit.
fn words_of(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let characters: Vec<char> = name.chars().collect();
    for (index, character) in characters.iter().enumerate() {
        if *character == '_' {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            continue;
        }
        let previous = index
            .checked_sub(1)
            .and_then(|before| characters.get(before));
        let next = characters.get(index.saturating_add(1));
        let after_lower_or_digit =
            previous.is_some_and(|before| before.is_lowercase() || before.is_ascii_digit());
        let acronym_end = previous.is_some_and(|before| before.is_uppercase())
            && next.is_some_and(|following| following.is_lowercase());
        if character.is_uppercase() && (after_lower_or_digit || acronym_end) && !current.is_empty()
        {
            words.push(std::mem::take(&mut current));
        }
        current.extend(character.to_lowercase());
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// Un lexème retenu par le balayage.
///
/// Le balayage est **partagé** : `check-naming` y cherche des sites de déclaration,
/// `check-safety-comments` des mots-clés `unsafe`. Les deux ont le même besoin — du code
/// débarrassé de ses commentaires, de ses chaînes et de ses littéraux —, et l'écrire deux
/// fois donnerait deux scanners dont un seul serait éprouvé. Celui-ci l'est sur près de
/// dix mille noms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Lexeme {
    /// Un identifiant, mots-clés compris.
    Identifier(String),
    /// Un signe, éventuellement composé — `::`.
    Symbol(String),
}

/// Un lexème et la ligne où il commence.
#[derive(Debug, Clone)]
pub(crate) struct Token {
    /// Le lexème.
    pub lexeme: Lexeme,
    /// La ligne, à partir de 1.
    pub line: usize,
}

/// Un nom déclaré et sa ligne.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Declaration {
    /// Le nom tel qu'il est écrit.
    name: String,
    /// La ligne, à partir de 1.
    line: usize,
}

/// Le balayeur lexical : il garde les identifiants et les signes, et jette le reste.
struct Scanner {
    /// Le texte source, en points de code.
    characters: Vec<char>,
    /// La position de lecture.
    index: usize,
    /// La ligne courante, à partir de 1.
    line: usize,
}

impl Scanner {
    /// Le caractère à `ahead` positions devant, s'il existe.
    fn peek(&self, ahead: usize) -> Option<char> {
        self.characters
            .get(self.index.saturating_add(ahead))
            .copied()
    }

    /// Avance d'un caractère et compte les lignes.
    fn bump(&mut self) -> Option<char> {
        let character = self.peek(0);
        if let Some(value) = character {
            self.index = self.index.saturating_add(1);
            if value == '\n' {
                self.line = self.line.saturating_add(1);
            }
        }
        character
    }

    /// Jette un commentaire de ligne.
    fn skip_line_comment(&mut self) {
        while let Some(character) = self.peek(0) {
            if character == '\n' {
                return;
            }
            self.bump();
        }
    }

    /// Jette un commentaire de bloc, imbrications comprises.
    fn skip_block_comment(&mut self) {
        let mut depth = 0usize;
        while let Some(character) = self.peek(0) {
            if character == '/' && self.peek(1) == Some('*') {
                self.bump();
                self.bump();
                depth = depth.saturating_add(1);
                continue;
            }
            if character == '*' && self.peek(1) == Some('/') {
                self.bump();
                self.bump();
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return;
                }
                continue;
            }
            self.bump();
        }
    }

    /// Jette une chaîne délimitée par des guillemets, échappements compris.
    fn skip_quoted_string(&mut self) {
        self.bump();
        while let Some(character) = self.peek(0) {
            if character == '\\' {
                self.bump();
                self.bump();
                continue;
            }
            self.bump();
            if character == '"' {
                return;
            }
        }
    }

    /// Jette une chaîne brute `r#"…"#`, pour le nombre de dièses ouvert.
    fn skip_raw_string(&mut self, hashes: usize) {
        while let Some(character) = self.peek(0) {
            self.bump();
            if character != '"' {
                continue;
            }
            let mut seen = 0usize;
            while seen < hashes && self.peek(0) == Some('#') {
                self.bump();
                seen = seen.saturating_add(1);
            }
            if seen == hashes {
                return;
            }
        }
    }

    /// Le nombre de dièses d'un préfixe de chaîne brute, si c'en est une.
    fn raw_string_hashes(&self, from: usize) -> Option<usize> {
        let mut hashes = 0usize;
        while self.peek(from.saturating_add(hashes)) == Some('#') {
            hashes = hashes.saturating_add(1);
        }
        if self.peek(from.saturating_add(hashes)) == Some('"') {
            return Some(hashes);
        }
        None
    }

    /// Jette un littéral de caractère, ou garde le signe d'une durée de vie.
    fn scan_quote(&mut self, tokens: &mut Vec<Token>) {
        let escaped = self.peek(1) == Some('\\');
        let single = self.peek(2) == Some('\'');
        if escaped || single {
            self.bump();
            while let Some(character) = self.peek(0) {
                if character == '\\' {
                    self.bump();
                    self.bump();
                    continue;
                }
                self.bump();
                if character == '\'' {
                    return;
                }
            }
            return;
        }
        let line = self.line;
        self.bump();
        tokens.push(Token {
            lexeme: Lexeme::Symbol("'".to_owned()),
            line,
        });
    }

    /// Lit un identifiant complet.
    fn take_identifier(&mut self) -> String {
        let mut name = String::new();
        while let Some(character) = self.peek(0) {
            if character.is_alphanumeric() || character == '_' {
                name.push(character);
                self.bump();
                continue;
            }
            break;
        }
        name
    }

    /// Jette un littéral numérique et son suffixe.
    fn skip_number(&mut self) {
        while let Some(character) = self.peek(0) {
            if character.is_alphanumeric() || character == '_' {
                self.bump();
                continue;
            }
            break;
        }
    }

    /// Balaye tout le source.
    fn run(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while let Some(character) = self.peek(0) {
            if character == '/' && self.peek(1) == Some('/') {
                self.skip_line_comment();
                continue;
            }
            if character == '/' && self.peek(1) == Some('*') {
                self.skip_block_comment();
                continue;
            }
            if character == '"' {
                self.skip_quoted_string();
                continue;
            }
            if character == 'r'
                && let Some(hashes) = self.raw_string_hashes(1)
            {
                self.bump();
                for _ in 0..hashes {
                    self.bump();
                }
                self.skip_raw_string(hashes);
                continue;
            }
            if character == 'b' {
                if self.peek(1) == Some('"') {
                    self.bump();
                    self.skip_quoted_string();
                    continue;
                }
                if self.peek(1) == Some('\'') {
                    self.bump();
                    self.scan_quote(&mut tokens);
                    continue;
                }
                if self.peek(1) == Some('r')
                    && let Some(hashes) = self.raw_string_hashes(2)
                {
                    self.bump();
                    self.bump();
                    for _ in 0..hashes {
                        self.bump();
                    }
                    self.skip_raw_string(hashes);
                    continue;
                }
            }
            if character == '\'' {
                self.scan_quote(&mut tokens);
                continue;
            }
            if character.is_alphabetic() || character == '_' {
                let line = self.line;
                let name = self.take_identifier();
                tokens.push(Token {
                    lexeme: Lexeme::Identifier(name),
                    line,
                });
                continue;
            }
            if character.is_ascii_digit() {
                self.skip_number();
                continue;
            }
            if character.is_whitespace() {
                self.bump();
                continue;
            }
            let line = self.line;
            self.bump();
            if character == ':' && self.peek(0) == Some(':') {
                self.bump();
                tokens.push(Token {
                    lexeme: Lexeme::Symbol("::".to_owned()),
                    line,
                });
                continue;
            }
            tokens.push(Token {
                lexeme: Lexeme::Symbol(character.to_string()),
                line,
            });
        }
        tokens
    }
}

/// Balaye un source Rust et rend ses lexèmes significatifs.
pub(crate) fn tokenize(source: &str) -> Vec<Token> {
    Scanner {
        characters: source.chars().collect(),
        index: 0,
        line: 1,
    }
    .run()
}

/// L'identifiant à une position, si c'en est un.
fn identifier_at(tokens: &[Token], index: usize) -> Option<&str> {
    match tokens.get(index).map(|token| &token.lexeme) {
        Some(Lexeme::Identifier(name)) => Some(name.as_str()),
        _ => None,
    }
}

/// Le signe à une position, si c'en est un.
fn symbol_at(tokens: &[Token], index: usize) -> Option<&str> {
    match tokens.get(index).map(|token| &token.lexeme) {
        Some(Lexeme::Symbol(symbol)) => Some(symbol.as_str()),
        _ => None,
    }
}

/// La ligne d'une position, ou zéro hors des lexèmes.
fn line_at(tokens: &[Token], index: usize) -> usize {
    tokens.get(index).map_or(0, |token| token.line)
}

/// Les mots-clés qu'un site de déclaration simple peut porter avant le nom.
const DECORATIONS: &[&str] = &["mut", "ref"];

/// Les mots-clés qui introduisent un nom déclaré, le nom suivant immédiatement.
const INTRODUCERS: &[&str] = &[
    "fn", "struct", "enum", "union", "trait", "type", "mod", "const", "static",
];

/// Relève les noms déclarés d'une suite de lexèmes.
fn declarations(tokens: &[Token]) -> Vec<Declaration> {
    let mut found = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        // Exception 2 : un bloc `extern "C"` recopie des symboles C verbatim.
        if identifier_at(tokens, index) == Some("extern")
            && symbol_at(tokens, index.saturating_add(1)) == Some("{")
        {
            index = skip_braced_block(tokens, index.saturating_add(1));
            continue;
        }
        if let Some(keyword) = identifier_at(tokens, index) {
            if INTRODUCERS.contains(&keyword) && !is_lifetime(tokens, index) {
                let mut cursor = index.saturating_add(1);
                while identifier_at(tokens, cursor).is_some_and(|name| DECORATIONS.contains(&name))
                {
                    cursor = cursor.saturating_add(1);
                }
                if let Some(name) = identifier_at(tokens, cursor)
                    && !KEYWORDS.contains(&name)
                {
                    found.push(Declaration {
                        name: name.to_owned(),
                        line: line_at(tokens, cursor),
                    });
                }
                index = index.saturating_add(1);
                continue;
            }
            if keyword == "let" {
                index = collect_pattern(
                    tokens,
                    index.saturating_add(1),
                    &["=", ";"],
                    &[],
                    &mut found,
                );
                continue;
            }
            if keyword == "for" {
                index = collect_pattern(
                    tokens,
                    index.saturating_add(1),
                    &["{", ";"],
                    &["in"],
                    &mut found,
                );
                continue;
            }
        }
        if symbol_at(tokens, index) == Some("|") {
            index = collect_closure_parameters(tokens, index, &mut found);
            continue;
        }
        if let Some(name) = identifier_at(tokens, index)
            && symbol_at(tokens, index.saturating_add(1)) == Some(":")
            && is_field_position(tokens, index)
            && !KEYWORDS.contains(&name)
        {
            found.push(Declaration {
                name: name.to_owned(),
                line: line_at(tokens, index),
            });
        }
        index = index.saturating_add(1);
    }
    found
}

/// Vrai si le lexème précédent est l'apostrophe d'une durée de vie — `&'static str`.
fn is_lifetime(tokens: &[Token], index: usize) -> bool {
    index
        .checked_sub(1)
        .is_some_and(|before| symbol_at(tokens, before) == Some("'"))
}

/// Vrai si l'identifiant occupe une position de champ ou de paramètre.
///
/// Le lexème précédent tranche : une parenthèse ou une accolade ouvrante, une virgule,
/// `pub` ou `mut`. Un identifiant précédé d'un point ou de `::` est un **emploi**, jamais
/// une déclaration.
fn is_field_position(tokens: &[Token], index: usize) -> bool {
    let Some(before) = index.checked_sub(1) else {
        return false;
    };
    if let Some(symbol) = symbol_at(tokens, before) {
        return matches!(symbol, "(" | "{" | "," | ")");
    }
    identifier_at(tokens, before).is_some_and(|name| matches!(name, "pub" | "mut"))
}

/// Saute un bloc entre accolades et rend la position qui suit l'accolade fermante.
fn skip_braced_block(tokens: &[Token], opening: usize) -> usize {
    let mut depth = 0usize;
    let mut index = opening;
    while index < tokens.len() {
        match symbol_at(tokens, index) {
            Some("{") => depth = depth.saturating_add(1),
            Some("}") => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return index.saturating_add(1);
                }
            }
            _ => {}
        }
        index = index.saturating_add(1);
    }
    index
}

/// Relève les liaisons d'un motif et rend la position où le motif s'arrête.
///
/// Un identifiant suivi de `::` ou de `(` est un chemin ou un constructeur, pas une
/// liaison. Après le premier `:` de premier niveau, on est en position de **type** : plus
/// aucune liaison n'y est déclarée.
fn collect_pattern(
    tokens: &[Token],
    start: usize,
    stops: &[&str],
    keyword_stops: &[&str],
    found: &mut Vec<Declaration>,
) -> usize {
    let mut index = start;
    let mut in_type = false;
    let mut depth = 0usize;
    while index < tokens.len() {
        if let Some(symbol) = symbol_at(tokens, index) {
            if depth == 0 && stops.contains(&symbol) {
                return index;
            }
            match symbol {
                "(" | "[" | "<" => depth = depth.saturating_add(1),
                ")" | "]" | ">" => depth = depth.saturating_sub(1),
                ":" if depth == 0 => in_type = true,
                _ => {}
            }
            index = index.saturating_add(1);
            continue;
        }
        if let Some(name) = identifier_at(tokens, index) {
            if keyword_stops.contains(&name) {
                return index.saturating_add(1);
            }
            let followed_by_path = matches!(
                symbol_at(tokens, index.saturating_add(1)),
                Some("::" | "(" | "{")
            );
            let preceded_by_path = index
                .checked_sub(1)
                .is_some_and(|before| symbol_at(tokens, before) == Some("::"));
            if !in_type
                && !followed_by_path
                && !preceded_by_path
                && !KEYWORDS.contains(&name)
                && !DECORATIONS.contains(&name)
            {
                found.push(Declaration {
                    name: name.to_owned(),
                    line: line_at(tokens, index),
                });
            }
        }
        index = index.saturating_add(1);
    }
    index
}

/// Le nombre maximal de lexèmes admis entre les deux barres d'une fermeture.
///
/// Au-delà, ce n'est pas une liste de paramètres : c'est un `|` d'alternative ou de
/// disjonction binaire, et le relevé s'abstient plutôt que de deviner.
const CLOSURE_PARAMETER_BUDGET: usize = 12;

/// Relève les paramètres d'une fermeture `|…|` et rend la position qui suit.
///
/// Le relevé n'accepte que ce qu'une liste de paramètres peut contenir : des
/// identifiants, des virgules, `mut`, `&` et un `:` de type. Tout autre signe fait
/// abandonner — `Pattern::A | Pattern::B` et `left | right` ne sont pas des fermetures.
fn collect_closure_parameters(
    tokens: &[Token],
    opening: usize,
    found: &mut Vec<Declaration>,
) -> usize {
    let mut candidates = Vec::new();
    let mut index = opening.saturating_add(1);
    let mut in_type = false;
    let mut examined = 0usize;
    while examined < CLOSURE_PARAMETER_BUDGET {
        if let Some(symbol) = symbol_at(tokens, index) {
            match symbol {
                "|" => {
                    found.append(&mut candidates);
                    return index.saturating_add(1);
                }
                "," => in_type = false,
                ":" => in_type = true,
                "&" => {}
                _ => return opening.saturating_add(1),
            }
        } else if let Some(name) = identifier_at(tokens, index) {
            if !in_type && !DECORATIONS.contains(&name) && !KEYWORDS.contains(&name) {
                candidates.push(Declaration {
                    name: name.to_owned(),
                    line: line_at(tokens, index),
                });
            }
        } else {
            return opening.saturating_add(1);
        }
        index = index.saturating_add(1);
        examined = examined.saturating_add(1);
    }
    opening.saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::{
        Declaration, FORBIDDEN_WORDS, check, declarations, forbidden_word_in, tokenize, words_of,
    };
    use std::path::PathBuf;

    fn names(source: &str) -> Vec<String> {
        declarations(&tokenize(source))
            .into_iter()
            .map(|declaration| declaration.name)
            .collect()
    }

    fn contains(source: &str, name: &str) -> bool {
        names(source).iter().any(|found| found == name)
    }

    #[test]
    fn un_nom_en_serpent_se_decoupe_en_mots() {
        assert_eq!(words_of("desired_column"), vec!["desired", "column"]);
    }

    #[test]
    fn un_nom_en_chameau_se_decoupe_en_mots() {
        assert_eq!(words_of("AirSgrEncoder"), vec!["air", "sgr", "encoder"]);
    }

    #[test]
    fn un_acronyme_se_coupe_avant_sa_derniere_majuscule() {
        assert_eq!(words_of("HTMLParser"), vec!["html", "parser"]);
        assert_eq!(words_of("AirUTF8Codec"), vec!["air", "utf8", "codec"]);
    }

    #[test]
    fn un_mot_complet_n_est_pas_son_abreviation() {
        // `position` contient `pos` comme sous-chaîne : le découpage en mots est ce qui
        // empêche le contrôle de le refuser.
        assert_eq!(forbidden_word_in("position"), None);
        assert_eq!(forbidden_word_in("current_column"), None);
        assert_eq!(forbidden_word_in("generation"), None);
    }

    #[test]
    fn une_abreviation_est_relevee_avec_le_nom_attendu() {
        assert_eq!(
            forbidden_word_in("cmd_set"),
            None,
            "cmd n'est pas au glossaire"
        );
        assert_eq!(forbidden_word_in("fg_color"), Some(("fg", "foreground")));
        assert_eq!(forbidden_word_in("AirCtxBuilder"), Some(("ctx", "context")));
        assert_eq!(forbidden_word_in("idx"), Some(("idx", "index")));
    }

    #[test]
    fn un_nom_impose_par_std_est_admis() {
        // Exception 1 : `len` vient d'ExactSizeIterator, et il est au glossaire.
        assert_eq!(forbidden_word_in("len"), None);
        assert_eq!(forbidden_word_in("fmt"), None);
        // Mais seul le nom nu est admis : `len_of` reste un nom choisi ici.
        assert_eq!(forbidden_word_in("len_of"), Some(("len", "length")));
    }

    #[test]
    fn le_glossaire_couvre_les_lignes_du_document() {
        // Le tableau de `docs/CONVENTIONS-fr.md` porte trente-trois entrées.
        assert_eq!(FORBIDDEN_WORDS.len(), 33);
    }

    #[test]
    fn les_declarations_simples_sont_relevees() {
        assert!(contains("fn draw_buffer() {}", "draw_buffer"));
        assert!(contains("struct AirWindow;", "AirWindow"));
        assert!(contains("enum AirSlot {}", "AirSlot"));
        assert!(contains("trait AirView {}", "AirView"));
        assert!(contains("type Outcome = u8;", "Outcome"));
        assert!(contains("mod editor;", "editor"));
        assert!(contains("const MAXIMUM_WIDTH: usize = 1;", "MAXIMUM_WIDTH"));
        assert!(contains("static TABLE: u8 = 0;", "TABLE"));
    }

    #[test]
    fn un_parametre_et_un_champ_sont_releves() {
        assert!(contains("fn draw(surface: &mut Surface) {}", "surface"));
        assert!(contains("struct Air { pub title: String }", "title"));
        assert!(contains("struct Air { hidden: bool }", "hidden"));
    }

    #[test]
    fn une_liaison_de_let_est_relevee_sans_son_type() {
        assert!(contains("let value: Vec<Painter> = give();", "value"));
        assert!(!contains("let value: Vec<Painter> = give();", "Painter"));
        assert!(contains("let mut buffer_of_text = 0;", "buffer_of_text"));
        assert!(contains("let (first, second) = pair();", "second"));
        assert!(contains(
            "let Some(found) = option else { return };",
            "found"
        ));
        assert!(!contains(
            "let Some(found) = option else { return };",
            "Some"
        ));
    }

    #[test]
    fn une_liaison_de_for_est_relevee_sans_l_iterable() {
        assert!(contains(
            "for element_of_list in children {}",
            "element_of_list"
        ));
        assert!(!contains("for element in children {}", "children"));
    }

    #[test]
    fn un_parametre_de_fermeture_est_releve() {
        assert!(contains("list.map(|value| value);", "value"));
        assert!(contains("list.map(|left, right| left);", "right"));
    }

    #[test]
    fn une_disjonction_n_est_pas_une_fermeture() {
        // `Pattern::A | Pattern::B` contient `::`, donc le relevé s'abstient.
        let source = "match key { Key::Up | Key::Down => {} }";
        assert!(!contains(source, "Down"));
    }

    #[test]
    fn un_emploi_n_est_pas_une_declaration() {
        // C'est ce qui rend le contrôle utilisable : `u32::MAX` et `.len()` sont des noms
        // imposés par std, employés et non déclarés.
        let source = "let limit = u32::MAX.min(other.len());";
        assert_eq!(forbidden_word_in("limit"), None);
        assert!(!contains(source, "MAX"));
        assert!(!contains(source, "min"));
        assert!(!contains(source, "len"));
    }

    #[test]
    fn un_commentaire_et_une_chaine_sont_ignores() {
        assert!(!contains("// let idx = 0;\nfn clean() {}", "idx"));
        assert!(!contains("/* let idx = 0; */ fn clean() {}", "idx"));
        assert!(!contains("let text = \"let idx = 0;\";", "idx"));
        assert!(!contains("let text = r#\"let idx = 0;\"#;", "idx"));
    }

    #[test]
    fn une_duree_de_vie_statique_n_est_pas_une_declaration() {
        assert!(!contains("const NAMES: &[&'static str] = &[];", "str"));
    }

    #[test]
    fn un_litteral_de_caractere_ne_derange_pas_le_balayage() {
        assert!(contains(
            "let separator = '\\'';\nfn after_it() {}",
            "after_it"
        ));
    }

    #[test]
    fn un_bloc_extern_c_est_exempte() {
        // Exception 2 : un symbole C recopié verbatim ne se traduit pas.
        let source = "unsafe extern \"C\" {\n    fn tcgetattr(fd: i32) -> i32;\n}\nfn after() {}";
        assert!(!contains(source, "fd"));
        assert!(!contains(source, "tcgetattr"));
        assert!(contains(source, "after"), "le relevé reprend après le bloc");
    }

    /// Un dépôt jetable, pour éprouver le parcours complet.
    fn temporary_repository(name: &str, source: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("air-tui-naming-{name}"));
        let crates = root.join("crates").join("sample").join("src");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&crates).expect("création du dépôt d'essai");
        std::fs::write(crates.join("lib.rs"), source).expect("écriture du source d'essai");
        root
    }

    #[test]
    fn un_depot_propre_passe() {
        let root = temporary_repository("propre", "fn draw_the_window() {}\n");
        let report = check(&root).expect("parcours");
        assert_eq!(report.files, 1);
        assert_eq!(report.violations, Vec::new());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn un_depot_fautif_est_refuse_en_citant_le_fichier_et_la_ligne() {
        let root = temporary_repository("fautif", "fn draw() {}\nfn paint(bg: u8) {}\n");
        let report = check(&root).expect("parcours");
        let first = report.violations.first().expect("un manquement attendu");
        assert_eq!(first.name, "bg");
        assert_eq!(first.word, "bg");
        assert_eq!(first.expected, "background");
        assert_eq!(first.line, 2);
        assert!(first.file.ends_with("lib.rs"), "{} inattendu", first.file);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn une_racine_absente_n_est_pas_une_erreur() {
        let root = std::env::temp_dir().join("air-tui-naming-vide");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("création du dépôt vide");
        let report = check(&root).expect("parcours");
        assert_eq!(report.files, 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn les_declarations_relevees_portent_leur_ligne() {
        let found = declarations(&tokenize("fn first() {}\n\nfn second() {}\n"));
        assert_eq!(
            found,
            vec![
                Declaration {
                    name: "first".to_owned(),
                    line: 1
                },
                Declaration {
                    name: "second".to_owned(),
                    line: 3
                },
            ]
        );
    }
}
