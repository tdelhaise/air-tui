# Conventions de code d'air-tui

Ces conventions sont **reprises du projet Air** (`~/Code/air`, `CLAUDE.md`) et appliquées
à air-tui. Ce document note ce qui est repris tel quel, ce qui est adapté, et la seule
règle qui ne l'est pas.

## Ce qui est repris tel quel

| Règle | Origine |
|---|---|
| MPL-2.0, en-tête de licence sur **chaque** fichier source | Air, « Licence » |
| `unsafe` confiné à la couche 0, `// SAFETY:` sur chaque bloc | Air, Principe 1 / ADR-149 D2 |
| `Option<T>` au lieu des sentinelles kernel (`0`, `-1`, `NULL`) | ADR-021 §1 |
| EINTR remonté à l'appelant, **jamais** de retry automatique en couche 0 | ADR-021 §2 |
| Pas de wrapper générique pour syscall multiplexé — une fonction dédiée typée par opération | ADR-021 §3 |
| Pas d'allocation sur le happy path, sauf nécessité documentée | ADR-021 §4 |
| `as` **interdit** pour une conversion lossy → `TryFrom` / `try_into` | Principe 2 |
| Arithmétique explicite : `checked_*` / `saturating_*` / `wrapping_*`, jamais l'opération nue | Principe 2 |
| Octets et UTF-8 distingués par le typage (`&[u8]` vs `&str`), `from_utf8` et jamais `_unchecked` | Principe 3 |
| Slicing par `get()`, jamais l'indexation qui panique | Principe 3 |
| Validation en tête de fonction publique, « parse, don't validate », newtypes validés | Principe 4 |
| Verbosité au service de la clarté, pas de magie, pas d'abstraction empilée | Principe 7 |
| Fuzzing systématique sur toute API qui accepte des données externes | Principe 1 |
| Entrée stricte, sortie lisible | Principe 11 |
| Builds reproductibles, toolchain pinnée et datée | ADR-025 |
| Trunk-based, DCO (`git commit -s`), signature GPG, Conventional Commits | Air, « Workflow Git » |
| Aucun trailer `Co-Authored-By` | Air, « Paternité » |

## Ce qui est adapté

**Toolchain stable au lieu de nightly.** Air est contraint au nightly par sa cible JSON
custom et `-Z build-std` ; air-tui n'a aucune de ces exigences. Le pin est donc sur
stable, daté (`rust-toolchain.toml`).

**ADR en français seulement, pour l'instant.** Le gate d'Air exige les paires
`ADR-NNN-*-fr.md` et `ADR-NNN-*-en.md`. Ici la documentation est française d'abord, la
traduction anglaise viendra ensuite — décision de l'auteur. Le registre généré et le gate
correspondant ne sont pas repris tant que la traduction n'est pas faite.

**Pas de règle des 80 % sur les dépendances.** Elle est sans objet : air-tui n'a aucune
dépendance (ADR-005). `deny.toml` garde néanmoins les bornes d'Air.

## La règle qui n'est PAS reprise

ADR-004 d'Air pose **Linux exclusivement** et interdit tout repli macOS dans le code.
C'est une décision de *cible*, pas une convention de code, et elle contredit le cahier
des charges d'air-tui. Elle est remplacée par
[`ADR-001-cibles-tier-1-fr.md`](adrs/ADR-001-cibles-tier-1-fr.md) : Linux **et** macOS en
tier-1.

Cette substitution est explicite pour une raison précise : recopier le `CLAUDE.md` d'Air
sans la retirer installerait dans ce dépôt une règle fausse pour ce projet — exactement le
défaut que le NO GO n° 1 existe pour empêcher.

---

# Nommage

## Les identifiants du code sont en anglais, les commentaires en français

```rust
/// Rend le cadre de la fenêtre avec les glyphes du thème actif.
pub fn draw_frame(&self, surface: &mut AirSurface, theme: &AirTheme) { /* … */ }
```

Les titres de section rustdoc restent anglais (`# Safety`, `# Errors`, `# Panics`) :
`clippy::missing_safety_doc` les reconnaît par leur nom.

## Aucun nom abrégé — glossaire contraignant

`command`, pas `cmd`. La règle vaut pour les types, les champs, les fonctions, les
paramètres **et** les variables locales.

| Interdit | Attendu | Interdit | Attendu |
|---|---|---|---|
| `fg`, `bg` | `foreground`, `background` | `attr` | `attribute` |
| `seq` | `sequence` | `esc` | `escape` |
| `col` | `column` | `pos` | `position` |
| `win` | `window` | `btn` | `button` |
| `msg` | `message` | `err` | `error` |
| `buf` | `buffer` | `len` | `length` |
| `src`, `dst` | `source`, `destination` | `prev` | `previous` |
| `min`, `max` | `minimum`, `maximum` | `cnt`, `num` | `count`, `number` |
| `idx` | `index` | `elem` | `element` |
| `cfg` | `configuration` | `init` | `initialize` |
| `dir` | `directory` | `gen` | `generation` |
| `ctx` | `context` | `cur` | `current` ou `cursor` |
| `mods` | `modifiers` | `res`, `val` | `result`, `value` |
| `tmp` | `temporary` | `ptr` | le nom du pointé |

Les trois premières lignes sont celles qui comptent le plus ici : `fg` / `bg` / `attribute`
dans un moteur de thèmes et `sequence` / `escape` dans un parseur ANSI sont les
abréviations qu'on écrit sans y penser à la quarantième occurrence.

## Les quatre seules exceptions

1. **Noms imposés par `std` ou rustdoc** : `len()` (trait `ExactSizeIterator`), `fmt`
   (`Display::fmt`), les titres `# Safety` / `# Errors` / `# Panics`.
2. **Symboles C et kernel recopiés verbatim** dans une déclaration `extern "C"` :
   `tcgetattr`, `cfmakeraw`, `TIOCGWINSZ`, et le paramètre `fd` d'un prototype C. Un nom
   d'emprunt ne se traduit pas, sinon on ne peut plus le confronter à sa page de manuel.
   La frontière est nette : verbatim **dans** la déclaration, nom complet dès l'enveloppe
   Rust (`terminal_window_size()`, pas `tiocgwinsz()`).
3. **Le suffixe `-sys`** de `air-tui-sys` : convention de l'écosystème Rust, et précédent
   dans Air avec `air-sys-types` et `air-sys-syscall`.
4. **`Id`** : précédent direct dans Air (`AirInputId`, `AirMonotonicId`). Donc `AirViewId`,
   pas `AirViewIdentifier`.

## Préfixe `Air` sur le modèle d'objets

Les types du modèle d'objets public d'`air-tui` portent le préfixe `Air` :
`AirScreen`, `AirWindow`, `AirView`, `AirInputLine`, `AirRectangle`…

Le choix est assumé : Rust a des modules, donc le préfixe est techniquement redondant —
mais c'est celui de Qt (`QWidget`), de Cocoa (`NSWindow`) et de GTK (`GtkWindow`), il rend
le `grep` sans ambiguïté, et il translittère le `T` de Turbo Vision pour qui vient de là.

**La couche 0 ne le porte pas.** `air-tui-sys` expose `Errno`, `WindowSize`, `TerminalMode`
sans préfixe, comme Air expose `air_sys_types::Errno` : le chemin de module suffit, et ces
types ne font pas partie du modèle d'objets de l'interface.

---

# Codecs

Toute conversion d'un flux de données passe par un codec — un `Decoder`, un `Encoder`, ou
les deux — et **tout codec est tenu à 100 % de couverture de code** (ADR-012).

La contrainte a un effet voulu sur le style : une étape partielle écrite en ligne laisse
une branche inatteignable, donc impossible à couvrir. Un `unwrap_or` sur une conversion
infaillible, un `if let` sur un index toujours valide — chacun crée un angle mort. On
extrait donc chaque étape partielle dans une fonction **totale**, dont le cas dégénéré est
inatteignable depuis l'appelant mais directement testable.

Tout **décodeur** porte une cible `cargo-fuzz` : il reçoit des octets hostiles par
définition.

---

# Portage et plateformes

Le code dépendant du système vit dans une crate d'implémentation dédiée, derrière les
traits d'`air-tui-platform` (ADR-011). Deux règles en découlent :

- **aucun `#[cfg(target_os)]` au-dessus de la couche de contrat.** La divergence entre
  plateformes est absorbée par la crate d'implémentation, pas saupoudrée dans le cœur ;
- `air-tui` ne dépend **jamais** d'une crate d'implémentation. C'est le binaire final qui
  en choisit une et l'injecte.

Le portage à venir sur `linux-air` — où air-tui sera en couche 2 ou 3, sans aucun accès
direct à la couche 0 — consistera à écrire `air-tui-platform-air` sans toucher au cœur.
Préférer massivement les traits là où une notion peut différer d'une cible à l'autre.

---

# Versionnement

Semver strict, **une version par crate**, incrémentée par toute PR qui touche la crate
(ADR-009). `version` n'est donc **pas** déclarée dans `[workspace.package]`.

Rappel qui se perd souvent : en `0.y.z`, cargo traite `y` comme la case de rupture.
`0.1.0 → 0.2.0` est incompatible, `0.1.0 → 0.1.1` est compatible, et il n'y a pas de place
distincte pour « ajout sans rupture » — ajouts et correctifs vont tous deux en `z`.

Une PR qui ne touche que `docs/` n'incrémente rien.

---

# Dessin des vues

Toute `AirView` a une **bordure par défaut** ; on la retire par `border = false`
(ADR-010). Deux corollaires à respecter en écrivant un widget :

- viser `content_bounds()` et non `bounds()` — une vue bordée perd une cellule de chaque
  côté ;
- les petits contrôles (`AirButton`, `AirLabel`, `AirStaticText`, `AirCheckBoxes`) posent
  `border = false` dans leur constructeur. Un widget qui laisse le défaut doit avoir une
  raison, et c'est un point de revue.

---

# À outiller (rien n'existe à ce jour)

Aucun de ces contrôles n'est écrit. Ils sont à exécuter à la main, et **ne doivent pas
être annoncés comme automatisés** avant de l'être (NO GO n° 1).

- `check-naming` — balayer les identifiants déclarés et refuser le glossaire ci-dessus,
  avec les quatre exceptions en liste blanche.
- `check-licence-headers` — en-tête MPL présent sur chaque `.rs`.
- `check-safety-comments` — un `// SAFETY:` par bloc `unsafe` dans `air-tui-sys`.
- `check-dco` — `Signed-off-by` sur chaque commit, aucun `Co-Authored-By: Claude`.
- `check-versions` — refuser une PR qui modifie le code d'une crate sans incrémenter sa
  version (ADR-009).
- `check-codec-coverage` — mesurer par fichier et refuser qu'un module de `codec/`
  descende sous 100 % (ADR-012).
