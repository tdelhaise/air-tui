# ADR-004 — Conventions de nommage

**Statut :** Accepté — 2026-10-10

## Contexte

Deux décisions de nommage ont été prises par l'auteur, et toutes deux méritent d'être
consignées parce qu'elles vont contre un usage courant.

## Décision 1 — aucun identifiant abrégé

`command`, pas `cmd`. `foreground`, pas `fg`. La règle vaut pour les types, les champs,
les fonctions, les paramètres et les variables locales. Le glossaire contraignant et les
quatre exceptions admises sont dans `docs/CONVENTIONS-fr.md`.

**Constat honnête** : le code d'Air ne suit pas cette règle aujourd'hui — relevé sur
`crates/`, hors tests, le 2026-10-10 : `buf` 118 fois, `res` 44, `cmd` 30, `ctx` 12,
`idx` 11, `ev` 10. C'est donc une règle **neuve**, posée pour air-tui, et non une
convention d'Air reprise. Une base vierge la tient sans effort là où une base de cent
crates demanderait une campagne.

Les abréviations qui comptent le plus ici sont celles vers lesquelles un projet d'interface
texte dérive spontanément : `fg`, `bg` et `attr` dans un moteur de thèmes ; `seq` et `esc`
dans un parseur ANSI ; `col`, `pos` et `win` partout ailleurs.

## Décision 2 — préfixe `Air` sur le modèle d'objets

Les types du modèle d'objets public portent le préfixe : `AirScreen`, `AirWindow`,
`AirView`, `AirInputLine`, `AirRectangle`.

Rust a des modules, donc le préfixe est techniquement redondant — c'est d'ailleurs pour
cette raison que `tvision-rs` a supprimé le `T` de Borland, lequel n'existait que parce
que le C++ de 1990 n'avait pas d'espaces de noms. Le choix inverse est retenu ici pour
trois raisons : c'est celui de Qt (`QWidget`), de Cocoa (`NSWindow`) et de GTK
(`GtkWindow`) ; il rend le `grep` sans ambiguïté sur une base de code mêlant plusieurs
bibliothèques ; et il translittère le `T` de Turbo Vision pour qui vient de là.

**La couche 0 ne le porte pas** : `air-tui-sys` expose `Errno`, `WindowSize`,
`TerminalMode`. Précédent dans Air : `air_sys_types::Errno`.

## Conséquences

- `AirApplication` et non `AirApp`, `AirRectangle` et non `AirRect`, `AirColumn` et non
  `AirCol`, `AirChangeDirectoryDialog` et non `AirChDirDialog`, `command::SAVE` et non
  `cmd::SAVE`.
- `AirViewId` conserve `Id` : précédent direct dans Air (`AirInputId`, `AirMonotonicId`).
- Le gate `check-naming` **existe** depuis le jalon M6b : `cargo xtask check-naming` balaye
  les sites de déclaration du dépôt, refuse le glossaire, et tient les quatre exceptions en
  liste blanche — les noms imposés par `std`, et les corps de bloc `extern "C"`, exemptés
  entiers. L'intégration continue l'exécute sur chaque PR.
- Son **angle mort** est nommé, parce qu'un contrôle dont on ignore la portée donne une
  fausse assurance : la liaison d'un bras de `match` — `Some(index) => …` — demanderait de
  distinguer un motif d'une expression, donc une analyse syntaxique que le balayage ne
  porte pas. La règle y reste tenue à la main.
- Le contrôle porte sur ce que le dépôt **nomme**, pas sur ce qu'il **appelle** :
  `value.len()` est l'emploi d'un nom imposé par `std`, `fn len()` serait un nom choisi
  ici. Sans cette distinction, `u32::MAX` ferait échouer chaque fichier.
