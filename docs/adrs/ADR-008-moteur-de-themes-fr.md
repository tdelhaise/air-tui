# ADR-008 — Moteur de thèmes

**Statut :** Accepté — 2026-10-10

## Contexte

Le look and feel doit être un thème, le thème par défaut reproduisant Turbo Vision et un
second reproduisant Midnight Commander.

Le piège est de croire qu'un thème est une palette de couleurs. Ce qui fait reconnaître
Turbo Vision tient autant à ses **glyphes** — cadres doubles `╔═╗` pour les dialogues,
simples pour les fenêtres, case de fermeture `[■]`, flèches `▲▼`, bureau en `░`, ombre
portée de deux colonnes — qu'à ses couleurs.

## Décision

Un thème est un **triplet**, pas une palette :

```rust
pub struct AirTheme {
    palette:     AirPalette,     // rôles sémantiques → couleurs
    glyphs:      AirGlyphSet,    // cadres, cases, flèches, remplissages, curseurs
    decorations: AirDecorations, // ombres, simple ou double trait, style des hotkeys
}
```

**La palette est indexée par rôles sémantiques** — `window.frame.active`,
`menu.item.hotkey`, `dialog.button.default`, `scrollbar.thumb` — et non par la chaîne de
remappage à 63 octets de Borland. La fidélité du thème par défaut est obtenue en
**instanciant** les valeurs exactes de Borland dans ce modèle, pas en reproduisant son
mécanisme.

**Un quantificateur est requis, pas optionnel.** Un thème écrit en RGB 24 bits doit
dégrader proprement vers 256 couleurs puis vers les 16 couleurs ANSI d'une console Linux.
Sans lui, le thème par défaut est illisible sur une VT.

**Thème et coquille sont deux choses distinctes.** Ce qui fait reconnaître Midnight
Commander est sa coquille — deux panneaux, ligne de commande, barre F1-F10 étiquetée — pas
sa palette. Un « thème MC » appliqué au chrome de Turbo Vision ne ressemblerait pas à MC.
D'où : `AirTheme` porte palette, glyphes et décorations ; la composition du chrome relève
de l'`AirScreen` que l'application choisit. Deux applications d'exemple en découlent,
`redit` et `airmc`, et non une application à deux thèmes.

**Les thèmes sont compilés, pas analysés au runtime.** Le Principe 11 d'Air — « quand un
texte doit entrer, il ne passe pas par la porte d'exécution : un outil dédié le compile en
artefact vérifié » — s'applique directement. L'importeur de skins Midnight Commander est
donc un `xtask` qui compile un `.ini` en artefact Rust committé : un skin mal formé est un
refus à la compilation, jamais un comportement dégradé à l'exécution.

## Conséquences

- Le thème Turbo Vision est le défaut et sert de référence de non-régression visuelle : la
  comparaison se fait contre `tvdemo` de `magiblot/tvision`.
- L'importeur de skins MC donne accès à des dizaines de thèmes existants pour le coût d'un
  analyseur `.ini` d'environ cent cinquante lignes — mais côté `xtask`, pas côté runtime.
- Jalons : M5 pour le moteur et le thème Turbo Vision, M7 pour MC et l'importeur.

## Alternatives écartées

**Thème = palette seule.** Ne reproduit ni l'un ni l'autre des deux looks visés.

**Analyse des fichiers de thème à l'exécution.** Contraire au Principe 11, et introduit un
analyseur tolérant sur un chemin d'entrée de décision.
