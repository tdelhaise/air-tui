# Journal de bord d'air-tui

Archive chronologique **append-only**, en ordre **anté-chronologique** : la plus récente en
tête. On ajoute, on ne réécrit jamais une entrée passée.

Une entrée par campagne, pas par commit. Répartition des rôles : `etat-avancement.md` est
l'état vivant, ce journal est la trace figée, `INDEX.md` est la carte structurelle.

## 2026-10-10 — M2, le protocole clavier Kitty

- `air-tui/src/input/kitty.rs` et l'extension du décodeur : frappes `CSI … u` avec
  modificateurs complets, types d'événement dont le relâchement, et touche de la
  disposition de base — `AirKeyEvent::base_layout` plus `shortcut_key()`, de quoi faire
  fonctionner `Alt+F` sur un clavier cyrillique. Repli `modifyOtherKeys` niveau 2.
- Négociation par interrogation `CSI ? u` **suivie** d'un jalon d'attributs de
  périphérique : voir le jalon sans réponse Kitty avant lui est une conclusion négative
  certaine, pas une expiration de délai. Le délai ne sert qu'au terminal muet. Ce qui est
  tapé pendant l'interrogation est transmis, pas jeté. Le protocole est défait au `Drop`.
- **Deux corrections de fond trouvées en écrivant les tests** : les paramètres de séquence
  étaient en `u16`, ce qui faisait refuser `CSI 128512 u` — un emoji en numéro de touche —
  comme malformé ; et les numéros hors table tombaient dans la zone à usage privé
  d'Unicode, donc devenaient des caractères imprimables fantômes qu'une application aurait
  insérés dans un champ de saisie. Les deux sont couvertes par un test nommé.
- `air-tui` passe en **0.3.0** : `base_layout` est un champ ajouté à une structure
  publique, donc une rupture pour toute construction littérale. En `0.y.z`, la rupture
  prend la case du milieu (ADR-009).
- 212 tests, 100 % de couverture sur les six modules de codec et d'entrée, 371 416
  exécutions de fuzzing sans plantage sur le décodeur étendu.
- **Affirmations périmées corrigées** : `CLAUDE.md`, `AGENTS.md`, `README.md` et
  `CONVENTIONS-fr.md` nommaient encore `air-tui-sys`, crate renommée par ADR-011. Les ADR
  antérieurs et le journal ne sont pas réécrits — ce sont des archives — et une note de
  lecture dans `CONVENTIONS-fr.md` fait le pont.

## 2026-10-10 — vérification sur Linux, dette soldée

- Branche M1 vérifiée sur **hydrogen** (Ubuntu 26.04.1 LTS, noyau 7.0.0-38, `x86_64`) :
  `fmt`, `clippy -D warnings`, 188 tests, `build --locked`, `deny check` propres ;
  couverture globale 95,0 % de régions, codecs et modules d'entrée à 100 % ; exemple
  `frame` rendant une grille identique à celle de macOS ; 371 229 exécutions de fuzzing
  supplémentaires sans plantage.
- Les trois valeurs propres à Linux de la couche POSIX sont confirmées justes :
  `TIOCGWINSZ = 0x5413`, `nfds_t` sur 64 bits, et `poll` rendant `POLLIN` sur `/dev/null`
  — la divergence avec macOS, consignée au jalon M0, est exacte dans les deux sens.
- **Un défaut trouvé, de la classe exacte qu'une seule plateforme de développement laisse
  passer** : un import ne servant qu'à un test gardé par `cfg(target_os = "macos")` faisait
  échouer `clippy -D warnings` sur Linux, contrôle obligatoire du projet. Corrigé dans la
  même branche ; `air-tui-platform-posix` passe en 0.1.1.
- Reste hors vérification : `aarch64` — jamais construit, et c'est là que le piège
  variadique d'`ioctl` sur l'ABI d'Apple se manifesterait — et la console Linux native,
  inaccessible par SSH.

## 2026-10-10 — M1, le décodeur ANSI d'entrée

- `air-tui/src/input/` : modèle d'événements conforme à l'ADR-003 et décodeur de séquences
  ANSI. Caractères UTF-8, commandes C0, touche alternative, CSI et SS3 avec modificateurs,
  souris SGR 1006, collages encadrés, focus. `air-tui` passe en 0.2.1 (ajout d'API pure).
- **100 % de couverture** sur les trois modules d'entrée, comme l'ADR-012 l'exige d'un
  codec. La contrainte a de nouveau dicté la forme du code : deux branches « entrée vide »
  ont été supprimées en passant à l'appelé l'octet de tête que l'appelant connaissait déjà,
  et les trois fermetures `|_| {}` des tests d'erreur ont été fondues en une seule, qu'un
  nouveau test exerce.
- Cible `cargo-fuzz` `ansi_decoder`, dans un espace de travail séparé puisque le fuzzing
  exige nightly et que le dépôt est pinné sur stable. **494 565 exécutions, aucun
  plantage.** Au-delà de l'absence de panique, l'invariant qui compte est vérifié :
  découper l'entrée en deux lectures produit la même suite d'événements qu'un morceau
  unique.
- Limite consignée plutôt que découverte plus tard : sur un pty sans protocole Kitty,
  `Ctrl+Shift+S` reste indiscernable de `Ctrl+S`. C'est le codage historique qui ne sait
  pas l'exprimer, et c'est l'objet du jalon M2.

## 2026-10-10 — publication et protection de `main`

- Dépôt publié : <https://github.com/tdelhaise/air-tui>, public, MPL-2.0.
- Identité du projet : les commits portent `thierry.delhaise@air-desktop.org`, signés par
  la clef `5C6F8A2E20E58061`, déclarée sur le compte GitHub. Les quatre commits d'amorçage
  sont vérifiés et rattachés au compte.
- Protection de `main` activée et **éprouvée par un push réel**, refusé avec
  `GH006 — Changes must be made through a pull request` : pull request obligatoire,
  applicable aux administrateurs, ni force-push ni suppression de branche, historique
  linéaire, résolution des fils requise.
- Deux protections volontairement **absentes**, et pour des raisons distinctes :
  les contrôles de CI requis, parce qu'aucune CI n'existe — en exiger un qui n'existe pas
  bloquerait toute fusion ; et `required_signatures`, qu'Air traite comme une décision du
  BDFL hors du pouvoir d'un agent.

## 2026-10-10 — amorçage du dépôt

- Création du dépôt et reprise des conventions du projet Air (`~/Code/air`), à une
  exception documentée près : la clause Linux-only d'ADR-004 d'Air est remplacée par
  ADR-001 d'air-tui (Linux **et** macOS en tier-1).
- Règle de nommage posée : aucun identifiant abrégé, glossaire contraignant et quatre
  exceptions nommées (`docs/CONVENTIONS-fr.md`).
- ADR-001 à ADR-012 rédigés en français ; traduction anglaise à produire. Quatre principes
  ajoutés en cours de session : versionnement semver par crate incrémenté à chaque PR
  (ADR-009), bordure par défaut sur les vues retirable par `border = false` (ADR-010),
  couture de plateforme par traits en vue du portage sur `linux-air` (ADR-011), et toute
  conversion de flux par un codec tenu à 100 % de couverture (ADR-012).
- Espace de travail recomposé en trois crates : `air-tui-platform` porte le contrat,
  `air-tui-platform-posix` l'implémentation POSIX et tout le `unsafe`, `air-tui` le cœur
  qui ne dépend que du contrat. Effet mesuré : la couverture du `terminal.rs` du cœur passe
  de 0 % à 93,8 %, le rendu devenant testable sans terminal.
- Socle M0 : `air-tui-sys` (six symboles C, enveloppes typées) et `air-tui` (géométrie,
  cellule, surface différentielle, terminal en RAII, thème minimal, exemple `frame`).
- Socle construit et exécuté sur macOS `x86_64` uniquement — Linux reste à vérifier.
  94 tests au vert, clippy propre avec `-D warnings`, 91,2 % de couverture de régions dont
  100 % sur les deux codecs, exemple `frame` vérifié dans un pty.
- Le contrôle du champ `revents` de `poll` a révélé au premier lancement une divergence
  réelle entre macOS et Linux sur `/dev/null` (`POLLNVAL` contre `POLLIN`), désormais
  couverte par des tests sous `cfg`.
