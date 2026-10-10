# ADR-011 — Couture de plateforme par traits

**Statut :** Accepté — 2026-10-10
**Précise et complète :** [ADR-002](ADR-002-architecture-en-couches-fr.md), dont le
découpage à deux crates est remplacé par celui-ci.

## Contexte

ADR-002 posait deux crates : une couche 0 qui isole `unsafe` et les appels C, et un cœur
en Rust pur. Cela suffit pour Linux et macOS, qui partagent POSIX.

Mais air-tui sera porté sur **`linux-air`**, la cible du projet Air. Cette cible est
intégralement en Rust : pas de C, hormis une couche 0 isolée qu'**un framework n'a pas le
droit d'appeler directement**. Sur `linux-air`, air-tui sera au minimum en couche 2, voire
en couche 3.

Les deux mondes n'ont donc rien en commun au niveau des appels système, et le portage ne
peut pas se faire par `#[cfg(target_os)]` saupoudré dans le cœur : ce serait exactement ce
que la macro-architecture d'Air refuse.

## Décision

**Une couture par traits, et une crate d'implémentation par plateforme.**

```text
             air-tui                  ← le cœur : ne connaît que le contrat
                │
       air-tui-platform               ← le contrat : des traits, zéro unsafe, zéro C
                ▲
      ┌─────────┴──────────┐
air-tui-platform-posix     air-tui-platform-air     ← à écrire
(Linux glibc, macOS)       (Rust intégral, linux-air)
```

| Crate | Rôle | Régime |
|---|---|---|
| `air-tui-platform` | les traits et les types portables | `#![forbid(unsafe_code)]`, zéro dépendance |
| `air-tui-platform-posix` | l'implémentation POSIX | le **seul** `unsafe`, le **seul** appel C |
| `air-tui` | le cœur du framework | `#![forbid(unsafe_code)]`, dépend **uniquement** du contrat |

Le point essentiel : **`air-tui` n'a aucune dépendance vers une implémentation.** Elle ne
peut donc pas atteindre le système, même par accident — ce n'est pas une règle à respecter,
c'est le graphe de dépendances. C'est le binaire final qui choisit une implémentation et
l'injecte :

```rust
let device = air_tui_platform_posix::PosixTerminal::open()?;
let terminal = AirTerminal::new(Box::new(device))?;
```

Le trait [`TerminalDevice`] est **object-safe** : le cœur tient un `Box<dyn TerminalDevice>`
et ne porte aucun paramètre de type jusque dans l'arbre de vues. C'est ce qui permettra
d'ajouter l'implémentation `linux-air` sans toucher une ligne du cœur.

Le contrat ne parle ni de descripteur de fichier, ni de `termios`, ni de `poll` : ces
notions n'existent pas sur toutes les cibles. Il parle de mode brut, de taille, d'attente,
de lecture et d'écriture — ce dont une interface a besoin, et rien de plus.

Corollaire sur les erreurs : la couture expose un `PlatformError` **portable**, dont la
nature a le même sens partout. Le code natif est conservé pour le diagnostic, jamais pour
décider. Un `errno` est un détail de POSIX ; `Interrupted` n'en est pas un.

## Conséquences

- Aucun `#[cfg(target_os)]` au-dessus de la couche de contrat. La divergence est absorbée
  par la crate d'implémentation — et il y en a : `nfds_t` est un `unsigned long` dans la
  glibc et un `unsigned int` chez Apple, `TIOCGWINSZ` n'a pas la même valeur, `struct
  termios` n'a pas la même forme.
- **Bénéfice immédiat et non anticipé** : le rendu du framework se teste sans terminal. Un
  périphérique en mémoire implémente le contrat en trente lignes, et la couverture d'
  `air-tui/src/terminal.rs` est passée de 0 % à 93,8 % le jour où la couture a été posée.
  Un test déterministe du rendu, sur une machine sans `/dev/tty`, était impossible avec le
  découpage d'ADR-002.
- Le portage `linux-air` consistera à écrire `air-tui-platform-air` et à changer une ligne
  dans le binaire. Le cœur ne saura pas que quelque chose a changé.
- Le préfixe `Air` ne s'applique pas aux crates de plateforme : `PlatformError`,
  `WindowSize`, `TerminalDevice`, `PosixTerminal` ne font pas partie du modèle d'objets de
  l'interface (ADR-004).

## Alternatives écartées

**`#[cfg(target_os)]` dans le cœur.** Ce que fait l'écosystème, et ce qui rend un portage
coûteux : chaque ajout de cible retouche des dizaines de fichiers, et le cœur finit par
connaître tous les systèmes.

**Un paramètre de type générique `AirTerminal<D: TerminalDevice>`.** Évite la répartition
dynamique, mais contamine toute signature qui touche au terminal — jusque dans l'arbre de
vues, qui n'a aucune raison de savoir sur quelle plateforme il tourne. Le coût d'un appel
indirect par cellule modifiée est négligeable devant celui de l'écriture sur le terminal.
