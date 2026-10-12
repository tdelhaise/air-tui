# ADR-016 — La recherche : une question au modèle, une politique au contrôleur

**Statut :** Accepté — 2026-10-12

## Contexte

`redit` édite un fichier et ne sait pas y chercher un motif. Sur un fichier de plus d'un
écran, c'est l'absence qui se remarque en premier — avant l'annulation, qui existe depuis
le jalon M7h.

L'ADR-007 fait passer **toute mutation et tout déplacement** par une `AirTextAction` : le
jeu de ce qui peut arriver au texte se lit d'un coup d'œil sur une énumération. La question
de cet ADR est de savoir si la recherche en est une.

Elle ne se pose pas qu'en théorie, parce que trois décisions se cachent derrière le mot
« chercher », et qu'elles n'appartiennent pas au même endroit :

1. **où est l'occurrence** — un fait sur le texte, que seul le tampon peut établir ;
2. **que faire quand il n'y en a plus** — reprendre au début, ou s'arrêter en le disant ;
3. **quel était le dernier motif** — pour qu'une recherche puisse se continuer.

## Décision

### La recherche est une question, pas une action

`AirTextBuffer::find_from(position, &search)` rend le couple des positions de la première
occurrence à partir d'une position, ou `None`. Elle ne déplace rien, ne sélectionne rien,
ne consigne rien et ne fait avancer **aucune** génération.

Ce que le contrôleur en fait passe par les actions qui existent déjà, et il n'en a fallu
aucune de nouvelle :

```rust
AirTextAction::move_by(AirTextMotion::To(first)),   // efface la sélection, pose l'ancre
AirTextAction::select_by(AirTextMotion::To(last)),  // l'étend jusqu'au bout
```

Les deux sont appliquées au même tour, donc la vue ne voit que l'état final. Le tampon n'a
gagné qu'une **méthode de lecture**, et l'énumération des actions — la table qui dit tout
ce qui peut arriver au texte — n'a pas bougé d'une ligne.

Un `AirTextAction::Find` aurait été plus court à écrire au site d'appel. Il aurait aussi
obligé le tampon à répondre « je n'ai rien trouvé » par le canal des **changements
structurels**, c'est-à-dire à inventer un `AirTextChange::SearchFailed` — un fait qui n'en
est pas un : rien n'a changé. L'échec d'une recherche est une **valeur de retour**, et c'est
le seul endroit où il ne demande rien à personne.

### La politique est au contrôleur, et elle est nommée

Le modèle cherche **vers l'avant et ne reprend pas au début**. La reprise est une décision,
pas un fait : `redit` cherche du curseur à la fin, puis une fois depuis le début du
document, et **annonce** la reprise dans le titre de la fenêtre. Une occurrence qui paraît
avant le curseur sans un mot d'explication ressemble à un défaut.

Le **dernier motif** est au contrôleur pour la même raison : ce n'est pas une propriété du
document. Un tampon qui le retiendrait le porterait d'un fichier à l'autre, et deux vues
sur le même tampon partageraient une recherche qu'une seule a demandée.

Le motif est retenu **avant** d'être cherché, même quand il ne donne rien : corriger une
faute de frappe et continuer est le geste suivant le plus probable.

### L'unité de comparaison est la grappe de graphème

Pas l'octet, et pas le caractère. Un curseur d'air-tui se déplace par grappes depuis le
jalon M3 ; une occurrence qui commencerait ou finirait **au milieu** d'une grappe donnerait
une sélection que le curseur ne sait pas décrire.

Chercher `e` dans un `é` **décomposé** — `e` suivi de `U+0301` — ne trouve donc rien, et
c'est juste : ce qui est affiché est un `é`. Deux essais le vérifient, l'un pour le début
de l'occurrence et l'autre pour sa fin, et le second n'est pas redondant — le motif y
commence bien sur une frontière, et c'est sa fin qui tomberait dans la grappe suivante.

Un effet secondaire agréable : la comparaison grappe à grappe **est** la vérification des
frontières. Il n'y a pas de passe de validation à oublier.

### Ce que la recherche ne fait pas, et pourquoi

- **pas de bouclage dans le modèle** : c'est la politique ci-dessus ;
- **pas de recherche en arrière.** Le mécanisme existerait en vingt lignes, et personne ne
  les lirait : `redit` n'a pas de geste pour la demander. Un réglage sans lecteur est ce
  que ce dépôt refuse d'écrire (ADR-015) ;
- **pas de remplacement.** C'est une mutation, donc une action — et une action qui demande
  de décider ce qu'un « remplacer tout » fait à l'historique d'annulation. Un pas, ou un
  par occurrence ? La question mérite son propre jalon ;
- **pas d'expression régulière.** Elle demanderait un moteur, donc un sous-système, dans un
  dépôt qui n'a que `std` ;
- **le motif ne traverse pas les lignes.** Il vient d'un champ de saisie d'une seule ligne,
  qui ne peut pas produire de saut de ligne ; un motif qui en contiendrait un ne trouverait
  rien, puisqu'aucune ligne du tampon n'en porte. C'est une limite de la **question posée**
  et non de la recherche.

### Le repliement de casse est simple, et c'est écrit

`char::to_lowercase` de la bibliothèque standard, appliqué grappe à grappe. `Ê` et `ê` se
trouvent l'un l'autre ; `ß` et `SS` **non**, parce que le repliement complet d'Unicode
change le *nombre* de grappes et qu'aucune table de ce dépôt ne le décrit.

Mieux vaut une limite nommée qu'un repliement qui marche sur l'alphabet latin et surprend
ailleurs. La comparaison d'octets est tentée **d'abord** : c'est le cas courant, et il ne
demande aucun repliement.

### Les deux gestes, et le canal par lequel ils sortent

`Ctrl`+`F` ouvre la question, `Ctrl`+`L` continue la recherche, `F7` ouvre la question
depuis la barre de statut.

Les deux premiers sortent de la vue d'édition par le canal des **commandes** et non par
celui des intentions, pour la même raison que le copier-coller : ce qu'il faut pour chercher
— poser une question à l'utilisateur — n'appartient pas à une vue d'édition. `FIND` et
`FIND_NEXT` rejoignent donc les commandes que le framework nomme lui-même.

`Ctrl`+`L` est le geste de Borland, qui répète la recherche dans son IDE. `F3`, que le même
IDE emploie, est pris par « Ouvrir » dans `redit`. `F7` est celui de l'éditeur de `mc` :
Turbo Vision n'a pas de touche de fonction pour chercher, et une barre de statut qui ne
montrerait pas la recherche la rendrait introuvable sans lire la documentation.

## Conséquences

- le tampon gagne **une méthode de lecture** et aucune action ; l'énumération de l'ADR-007
  n'a pas bougé ;
- un module `model/search.rs` porte le motif, la règle de casse et la comparaison, et
  s'éprouve **sans monter de contrôleur** : c'est la conséquence pratique d'avoir refusé
  d'en faire une action ;
- `redit` retient le dernier motif, grise « Continuer la recherche » tant qu'il n'y en a
  pas — le *pull* de l'ADR-007, donc sans notification —, et pré-remplit la question
  suivante avec lui ;
- un motif **n'est pas coupé de ses espaces**, là où un nom de fichier l'est : chercher
  « le » entre deux espaces est une demande légitime, et la couper en ferait une autre. Les
  deux questions passent par le même dialogue, donc la distinction est écrite dans la
  réponse et non dans le dialogue ;
- le dialogue de saisie ne porte **pas** de case « respecter la casse ». Le modèle sait les
  deux et `redit` ne sait poser que l'insensible : `AirPromptDialog` tient un champ et aucun
  parcours de focus interne, ce qui est une absence voulue depuis M5b. L'offrir demande de
  trancher cette question-là, pas celle-ci.

## Alternatives écartées

**Une action `Find` dans l'énumération.** Plus court au site d'appel, et deux coûts : un
`AirTextChange` pour dire l'échec, alors que rien n'a changé ; et une énumération d'actions
qui ne décrit plus seulement ce qui **arrive au texte**.

**Le dernier motif dans le tampon.** Il suivrait le document d'un fichier à l'autre, et
deux vues sur le même tampon partageraient une recherche qu'une seule a demandée.

**Le bouclage dans le modèle.** Il retirerait au contrôleur la seule décision qui se voit à
l'écran — reprendre, ou dire que c'est fini —, et rendrait impossible une application qui
voudrait s'arrêter à la fin du fichier.

**La comparaison par octets, avec `str::find`.** Deux fois plus courte, et elle couperait
les grappes : chercher `e` sélectionnerait la première moitié d'un `é` décomposé, et le
curseur ne saurait pas décrire la position obtenue.
