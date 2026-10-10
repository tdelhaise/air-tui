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

### `state`, et où il se lit — précisé au jalon M7c

Le protocole Kitty place le type d'événement en **sous-paramètre du champ des
modificateurs**, et cette règle vaut pour *toutes* les formes, y compris l'encodage
historique des touches spéciales : `kitty` écrit `CSI ; 1 : 3 B` pour un relâchement de la
flèche bas — en omettant le numéro de touche, que la spécification autorise à taire quand
il vaut un. Vérifié dans `kitty/key_encoding.py`, qui n'écrit le numéro que s'il diffère
de un ; la page du protocole ne donne que la règle générale et aucun exemple de flèche.

Le décodeur ne lisait ce sous-champ que sur la forme `CSI … u` : le type était donc décodé
là où il ne sert qu'aux caractères, et **ignoré là où il désigne les touches dont les
widgets vivent** — flèches, `Début`, `Fin`, `Inser`, `Suppr`, `F1` à `F20`. Un relâchement
ressortait en enfoncement, et aucun filtre en aval ne pouvait le rattraper, puisque
l'information avait déjà été perdue au décodage.

**Qui écarte les relâchements :** la distribution, en un seul endroit
(`view::dispatch_event`), et non chaque widget — douze widgets qui répètent la règle sont
douze endroits où elle peut manquer. Une **répétition** passe, et c'est voulu : maintenir
une flèche doit répéter le déplacement. L'information n'est pas détruite pour autant : une
boucle d'application tient l'événement d'entrée avant de le distribuer.

## Alternatives écartées

**Un seul étage, evdev partout.** Impossible en SSH, et indésirable : ce serait lire le
clavier d'une autre session.

**Un seul étage, terminal partout.** Abandonne la lecture des scancodes là où elle est
réellement possible — la console Linux — alors que c'est une demande explicite.
