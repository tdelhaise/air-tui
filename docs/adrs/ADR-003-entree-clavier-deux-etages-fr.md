# ADR-003 — Entrée clavier à deux étages

**Statut :** Accepté — 2026-10-10

## Contexte

Le cahier des charges demande trois choses simultanément : lire les **vrais scancodes**,
détecter des combinaisons comme `Ctrl+Shift+Alt+X` ou `AltGr+touche`, et permettre de
lancer une application **depuis un shell SSH**, affichée dans un terminal.

Ces trois exigences ne tiennent pas sur un seul chemin d'entrée.

Un pty ne transporte pas de scancode : quand la session est distante, l'émulateur de
terminal côté client a déjà traduit la frappe en séquence d'échappement et jeté le reste.
Lire `/dev/input/event*` depuis une session SSH lirait le clavier physique de la console
**du serveur** — fonctionnellement faux, et équivalent à un enregistreur de frappe sur la
session d'un tiers.

## Décision

Deux étages d'entrée, choisis à l'exécution par sondage, l'étage **terminal en tier-1** et
l'étage natif en complément local.

| Contexte | Source | Scancodes | `Ctrl+Shift+Alt+X` |
|---|---|---|---|
| Console Linux, accès au groupe `input` | evdev `/dev/input/eventN` | oui | oui |
| Console Linux sans evdev | stdin + `ioctl(TIOCLINUX, 6)` | non | oui |
| Terminal local **ou SSH**, protocole Kitty | `CSI > 1 u`, `CSI u` | non, mais keycode de layout de base et relâchement | oui |
| Terminal local ou SSH, xterm ou VTE | `modifyOtherKeys` niveau 2 | non | partiellement |
| Terminal legacy, macOS Terminal.app | séquences ANSI classiques | non | **non** |
| macOS en local, IOKit HID | autorisation TCC requise, pas de session WindowServer en SSH | — | impasse |

Deux conséquences qui modifient l'ordre des travaux :

**Le protocole clavier Kitty est tier-1, au jalon M2.** C'est le seul mécanisme qui livre
les modificateurs complets à travers un pty, donc en SSH et sur macOS. Il n'est pas une
amélioration différable : sans lui, la deuxième exigence du cahier des charges n'est tenue
que sur la console Linux locale.

**La promesse de scancodes se limite à la console Linux.** Elle est écrite comme telle dans
le `README.md`, pas découverte par un rapport de bogue.

## Le modèle de touche

```rust
pub struct AirKeyEvent {
    pub logical:   AirKey,
    pub modifiers: AirModifiers,
    pub state:     AirKeyState,
    pub physical:  Option<AirScanCode>,
}
```

**Aucun widget d'air-tui ne doit dépendre de `physical`.** Un widget qui le ferait
fonctionnerait sur la console de l'auteur et casserait chez tout utilisateur distant. La
règle est vérifiable : `physical` n'apparaît dans aucun fichier de `src/widgets/`.

## Alternatives écartées

**Un seul étage, evdev partout.** Impossible en SSH, et indésirable : ce serait lire le
clavier d'une autre session.

**Un seul étage, terminal partout.** Abandonne la lecture des scancodes là où elle est
réellement possible — la console Linux — alors que c'est une demande explicite.
