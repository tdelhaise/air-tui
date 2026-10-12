# Architecture d'air-tui

## Les trois crates, et pourquoi la frontière est là

```
             air-tui                  ← le cœur : ne connaît que le contrat
                │
       air-tui-platform               ← le contrat : des traits, zéro unsafe, zéro C
                ▲
      ┌─────────┴──────────┐
air-tui-platform-posix     air-tui-platform-air     ← à écrire
(Linux glibc, macOS)       (Rust intégral, linux-air)
```

La règle vient d'Air : « au-dessus de la couche 0, Rust pur et idiomatique — pas
d'assembleur, pas d'appel à du C, pas d'`unsafe` ». Mais la raison du découpage en trois
plutôt qu'en deux est le **portage à venir sur `linux-air`** (ADR-011), cible intégralement
en Rust dont la couche 0 n'est pas appelable par un framework. Sur cette cible, air-tui
sera en couche 2 ou 3.

D'où la conséquence qui structure tout : **`air-tui` n'a aucune dépendance vers une
implémentation de plateforme.** Elle ne peut pas atteindre le système, même par accident —
ce n'est pas une discipline, c'est le graphe de dépendances. C'est le binaire final qui
choisit une implémentation et l'injecte :

```rust
let device = air_tui_platform_posix::PosixTerminal::open()?;
let terminal = AirTerminal::new(Box::new(device))?;
```

Bénéfice immédiat, qui n'était pas la motivation : le rendu se teste **sans terminal**. Un
périphérique en mémoire implémente le contrat en trente lignes, et la couverture du module
`terminal.rs` du cœur est passée de 0 % à 93,8 % le jour où la couture a été posée.

### La surface C, exhaustive

Six symboles, tous dans la bibliothèque C que `std` lie déjà — glibc sur `*-linux-gnu`,
libSystem sur `*-apple-darwin`. Aucune entrée dans `Cargo.toml`. Ils vivent uniquement
dans `air-tui-platform-posix`.

| Symbole | Emploi |
|---|---|
| `tcgetattr` | lire l'état du terminal avant de le modifier |
| `tcsetattr` | passer en mode brut, et le restaurer |
| `cfmakeraw` | composer le mode brut sans réécrire les masques de drapeaux à la main |
| `ioctl` | `TIOCGWINSZ` (taille), plus tard `TIOCLINUX` (état des modificateurs sur VT) |
| `poll` | attendre une entrée avec délai, sans occuper le processeur |
| `isatty` | refuser de passer en mode brut sur ce qui n'est pas un terminal |

Trois choses sont volontairement **absentes** de cette liste, parce que `std` les couvre :
l'ouverture de `/dev/tty` (`std::fs::File`), la lecture et l'écriture
(`std::io::Read`/`Write`), et la récupération d'`errno`
(`std::io::Error::last_os_error()`).

### Trois pièges de plateforme, traités explicitement

**`ioctl` est variadique.** Sur l'ABI arm64 d'Apple, les arguments variadiques passent par
la pile, là où les arguments fixes passent par registre. Déclarer `ioctl` en non-variadique
« marche » sur `x86_64` et casse silencieusement sur Apple Silicon. La déclaration est donc
variadique, et chaque opération reçoit son enveloppe dédiée typée (ADR-021 §3 d'Air).

**`struct termios` n'a pas la même forme sur les deux plateformes** : `tcflag_t` est 32 bits
et `NCCS` vaut 32 sur Linux ; 64 bits et 20 sur macOS. Plutôt que de recopier deux
définitions de champs — et deux jeux de constantes de drapeaux — depuis une mémoire
faillible, le type est un **tampon opaque sur-dimensionné** que seul le noyau remplit. Le
code ne lit aucun champ : il enchaîne `tcgetattr` → `cfmakeraw` → `tcsetattr`, et garde une
copie intacte pour la restauration. Aucune constante de drapeau n'apparaît dans le projet,
donc aucune ne peut être fausse.

**`nfds_t` diffère** : `unsigned long` dans la glibc, `unsigned int` chez Apple. Et
`TIOCGWINSZ` n'a pas la même valeur, Linux employant un numéro plat là où les BSD encodent
la direction et la taille. C'est exactement ce que la crate d'implémentation existe pour
absorber.

### Les codecs

Toute conversion d'un flux passe par un codec — `Decoder`, `Encoder`, ou les deux — et tout
codec est tenu à **100 % de couverture** (ADR-012). Deux existent : l'encodeur SGR, et
l'écriture décimale qu'il emploie. Les décodeurs à venir — séquences ANSI d'entrée,
protocole Kitty, graphèmes Unicode — porteront chacun une cible `cargo-fuzz`.

## Unicode

Écrit au jalon M3 (`air-tui/src/unicode/`), avec des tables **générées depuis le
référentiel et committées** (ADR-006), édition 18.0.0 — `cargo xtask
generate-unicode-tables`.

Deux propriétés que `std` ne donne pas, et dont une interface texte ne peut pas se passer :

- la **largeur d'affichage**. Sans elle, un `漢` compté pour une colonne décale la bordure
  droite de sa fenêtre. `AirSurface::draw_text` écrit donc une **cellule de continuation**
  après une grappe large, et le rendu la saute : l'émettre écraserait la moitié droite du
  glyphe que le terminal vient de dessiner ;
- la **segmentation en graphèmes** (annexe 29). Sans elle, `Backspace` démembre un
  `👨‍👩‍👧‍👦` en quatre personnages. Les règles `GB3` à `GB13` sont implémentées, **y
  compris `GB9c`** — la coupure conjointe indienne, sans laquelle le curseur coupe au
  milieu d'un `क्ष` devanagari ;
- la **segmentation en mots** (annexe 29 encore, jalon M6), pour `Ctrl`+flèche. Les règles
  `WB1` à `WB999`, avec le repliement de `WB4` et les regards en arrière de `WB7`, `WB7c`
  et `WB11`. C'est ce qui garde `3,14` et `can't` entiers là où une coupure sur la
  ponctuation les casserait, et ce qui découpe `日本語` en trois unités de déplacement sans
  un seul espace.

Un choix à connaître : la largeur d'une grappe est le **maximum** des largeurs de ses
caractères, pas leur somme. Une famille d'emoji soudée par des jointures occupe deux
colonnes, pas huit — c'est ce que font les terminaux modernes, et la seule définition qui
garde le curseur et le rendu d'accord entre eux.

### Les deux segmentations sont vérifiées contre les essais officiels

`cargo xtask check-segmentation` confronte les deux à `GraphemeBreakTest.txt` et
`WordBreakTest.txt` du Consortium : 853 et 1944 cas, **aucun écart**. Les fichiers d'essais
ne sont pas committés — ce sont des données du référentiel —, donc la vérification est une
commande et non un `#[test]` : un test qui se tairait faute de données donnerait exactement
l'assurance fausse que les deux NO GO de ce dépôt interdisent.

Ce contrôle a payé immédiatement. Il a trouvé **dix écarts** sur la segmentation en
grappes du jalon M3, annoncée comme faite : la révision 49 de l'annexe, livrée avec Unicode
18.0.0, a réécrit `GB9c` en `\p{InCB=Linker} \p{InCB=Extend}* × \p{InCB=Consonant}` et
retiré l'exigence d'une consonne **avant** le lieur. Les tables venaient déjà de 18.0.0, les
règles non — elles n'étaient pas de la même édition.

## La hiérarchie d'objets

```
AirApplication                 boucle, terminal, thème, jeu de commandes
                               — l'arbre lui est PASSÉ, elle ne le possède pas
AirScreen                      surface plein écran : chrome + contenu, empilable
 ├─ AirDesktop                 zone des fenêtres : ordre de profondeur, cascade, mosaïque
 │   └─ AirWindow              cadre, titre, case de fermeture
 │       └─ AirView            tout ce qui se dessine et reçoit des événements
 ├─ AirStatusLine              post-traitement, imposé par l'écran
 ├─ AirMenuBar                 pré-traitement, imposé par l'écran, et au premier plan
 └─ modal                      quand il y en a un : prend tous les événements
```

Cette hiérarchie **existe** depuis le jalon M6 : les trois pièces qui manquaient —
`AirApplication`, `AirScreen` et `AirDesktop` — sont écrites, et `redit` les emploie.

`AirScreen` existe pour rendre une erreur impossible. Au jalon M5b, la barre de menu devait
être insérée **en dernier**, sous peine d'être recouverte au dessin comme au routage, et
cette contrainte ne tenait que par la documentation. L'écran range désormais ses quatre
pièces lui-même — bureau, barre de statut, barre de menu, modal —, dans le seul ordre qui
marche, quel que soit l'ordre des appels. Il impose aussi le pré-traitement à la barre de
menu, le post-traitement à la barre de statut, et il porte la disposition.

La **modalité** est sa quatrième pièce, et elle n'est pas une propriété de la vue posée :
c'est `handle_event` de l'écran qui donne tous les événements au modal et empêche la
descente. Un clic hors du modal est consommé plutôt que laissé à ce qui est derrière — un
modal qui laisse passer un clic n'est pas modal.

Et la couche basse, nommée pour ne pas entrer en collision avec `AirScreen` :

| Type | Rôle |
|---|---|
| `AirTerminal` | le périphérique : `/dev/tty`, mode brut, source d'entrée, puits de sortie |
| `AirSurface` | la grille de cellules, le différentiel, le suivi des zones salies |
| `AirCell` | un caractère, ses couleurs, ses attributs |

`AirScreen` est un **ajout** par rapport à Turbo Vision, qui n'a qu'un seul chrome
d'application figé. En faire un objet empilable et commutable donne ce que Turbo Vision ne
savait pas faire : un écran d'accueil, un assistant multi-étapes, et surtout la
cohabitation de deux chromes radicalement différents — `redit` en style Turbo Vision,
`rmc` en style Midnight Commander — sur le même moteur. Les deux existent depuis le jalon
M7b, et c'est ce qui vérifie l'affirmation : la même boucle, le même bureau et les mêmes
barres servent un éditeur à une fenêtre et un gestionnaire à deux panneaux, et aucune des
deux applications ne monte sa propre disposition.

## L'arbre de vues

Écrit au jalon M4 (`air-tui/src/view/`). Trois pièces, et c'est la deuxième qui fait tout
tenir :

**Le modèle.** `AirView` est un **trait** et `AirViewState` une structure que chaque vue
**contient** — composition, pas héritage (ADR-011). Une vue expose son état et dit comment
se dessiner ; elle n'hérite de rien.

**Le peintre.** `AirPainter` est la pièce maîtresse. Une vue dessine à partir de `(0, 0)`,
**le sien** : elle ne sait pas où elle se trouve à l'écran, et c'est voulu — un bouton dans
une fenêtre déplacée n'a aucune raison d'être réécrit. C'est le modèle relatif au
propriétaire de Turbo Vision (`TView::origin`), et il apporte ce qui manquait depuis M0 :
**un découpage réel**. Toute écriture hors du rectangle de découpage est abandonnée, et un
peintre d'enfant intersecte son découpage avec celui de son parent. Une vue ne peut pas
déborder de son propriétaire, même en essayant. Le texte trop long qui mangeait le cadre
au jalon M0 est devenu impossible, et l'exemple `frame` en fait la démonstration en
insérant volontairement une étiquette plus large que sa fenêtre.

**Le routage.** Les trois phases de Turbo Vision, conservées telles quelles :
`PreProcess`, `Focused`, `PostProcess`. C'est ce qui permet à une barre de menu ouverte de
voir `Échap` avant le champ de saisie qui a le focus, et à un raccourci global de ne pas
voler une frappe qui lui était destinée. Les événements de souris, eux, se routent par la
**position** : du premier plan vers l'arrière, et les coordonnées sont traduites à la
descente puis restaurées à la remontée — une vue reçoit toujours des coordonnées qui sont
les siennes.

Le focus est **exclusif entre sœurs**, tenu par le groupe : le garder à un seul endroit est
la façon la plus sûre de ne pas le voir se dédoubler.

**Le contexte de dessin.** `AirDrawContext` porte ce qu'une vue a besoin de savoir pour se
dessiner et qui ne lui appartient pas : le **thème** et le **jeu de commandes**. Le second
a été ajouté au jalon M5b pour corriger un défaut de fond — ce document affirmait déjà que
chaque vue interroge le jeu au moment où elle se dessine, mais le chemin de dessin ne
recevait que le thème, et le grisage d'un item de menu reposait sur une fonction qui
rendait toujours `true`. Le grisage et le refus d'activer lisent désormais la même source.

### Pas de surimpression, et pourquoi

Un menu déroulant devait demander un mécanisme de surimpression : se dessiner par-dessus
ce qui est déjà là, hors des limites de son propriétaire. Il n'en faut aucun.

Le dessin parcourt les enfants de **l'arrière vers l'avant** — le dernier inséré écrit par
dessus — et le routage positionnel les parcourt de **l'avant vers l'arrière** — le premier
plan reçoit le clic d'abord. Une vue au premier plan recouvre donc déjà ses sœurs, au
dessin comme au routage, sans rien savoir d'elles. `AirMenuBar` **grandit** quand elle
s'ouvre : ses limites couvrent la zone du menu, et tout reste à l'intérieur. Le découpage
par vue ne souffre aucune exception.

La contrainte qui en découle est réelle : la barre de menu doit être insérée **en dernier**
dans son groupe. Insérée ailleurs, elle serait recouverte par ce qui vient après. Depuis le
jalon M6, `AirScreen` la range lui-même : la contrainte ne tient plus par la documentation.

### Le curseur

Une vue répond à `AirView::cursor` en coordonnées de sa zone de contenu. `draw_view` rend
la position **absolue** du curseur, et trois règles sont appliquées par la traversée et non
par la vue :

- seule une vue **focalisée** voit sa réponse retenue — deux champs ne peuvent donc pas
  revendiquer le curseur en même temps, et un champ peut répondre sans savoir s'il a le
  focus ;
- c'est le curseur du descendant **le plus profond** qui gagne ;
- un curseur qui ne passe pas le **découpage** n'en est pas un : il irait clignoter sur la
  vue qui recouvre.

`AirTerminal::set_cursor` le pose, et le rendu l'émet **après** les cellules : l'écriture
de la dernière cellule a laissé le curseur matériel là où elle s'est terminée. Aucun
curseur réclamé signifie un curseur caché.

## Les commandes

`AirCommandSet` est le mécanisme 3 de l'ADR-007, et il mérite d'être signalé parce qu'il
résout en *pull* ce pour quoi on écrirait un observateur. Griser « Enregistrer » grise d'un
coup l'item de menu, le bouton et l'item de barre de statut — **sans que personne soit
notifié** : chaque vue interroge le jeu au moment où elle se dessine. Il n'y a aucune arête
à tenir à jour, donc aucune à oublier, et les widgets créés après la désactivation sont
grisés comme les autres.

## La bordure des vues

Contrairement à Turbo Vision, où le cadre est un `TFrame` qu'un `TWindow` insère comme
sous-vue, **toute `AirView` a une bordure par défaut** et la retire par `border = false`
(ADR-010). La zone de contenu est donc distincte des limites de la vue :
`content_bounds()` rend les limites moins une cellule de chaque côté quand la bordure est
présente, et c'est elle que le dessin d'un widget doit viser.

Les petits contrôles posent `border = false` dans leur constructeur : le défaut profite
aux conteneurs. Au jalon M5b, cinq le font — `AirStaticText`, `AirButton`, `AirInputLine`,
`AirMenuBar` et `AirStatusLine` — et deux gardent la bordure : `AirPanel` et `AirWindow`.

## L'entrée, à deux étages

Détaillé dans [`adrs/ADR-003-entree-clavier-deux-etages-fr.md`](adrs/ADR-003-entree-clavier-deux-etages-fr.md).
Le point essentiel : **un pty ne transporte pas de scancode**. En SSH, l'émulateur côté
client a déjà traduit la frappe en séquence d'échappement. Le modèle de touche le dit :

```rust
pub struct AirKeyEvent {
    pub logical:   AirKey,              // toujours présent
    pub modifiers: AirModifiers,
    pub state:     AirKeyState,         // Press | Repeat | Release
    pub physical:  Option<AirScanCode>, // None sur un pty
}
```

Règle d'API : **aucun widget d'air-tui ne doit dépendre de `physical`**. Un widget qui le
ferait casserait en SSH.

### Ce qui est écrit

L'étage **terminal** l'est : `air-tui/src/input/` décode les caractères UTF-8, les
commandes C0, les combinaisons avec la touche alternative, les séquences CSI et SS3 avec
leurs modificateurs, la souris au format SGR 1006, les collages encadrés et les événements
de focus. Il est à 100 % de couverture et porte une cible `cargo-fuzz`, comme l'ADR-012
l'exige d'un décodeur.

Trois propriétés y sont tenues, et c'est ce qui le rend utilisable sur un flux réel :

- un **préfixe incomplet** ne consomme rien, donc une séquence coupée entre deux lectures
  n'est pas perdue — le fuzzing vérifie qu'un découpage arbitraire de l'entrée produit la
  même suite d'événements qu'un morceau unique ;
- une séquence **malformée** est une erreur, jamais une devinette ;
- une séquence **bien formée mais non reconnue** est consommée sans produire d'événement.
  Ce n'est pas de la tolérance : un terminal émet des réponses qu'on ne lui a pas
  demandées, et les refuser bloquerait le flux sur la première.

L'ambiguïté de l'échappement est traitée là où elle peut l'être : un `ESC` isolé est
indiscernable du début d'une séquence, seul le temps les sépare. Le décodeur, qui est pur,
rend « incomplet » ; c'est la boucle d'événements, qui sait qu'un délai s'est écoulé, qui
appelle `AirAnsiDecoder::isolated_escape_event`.

### Ce que le décodeur refuse de faire — jalon M7e

Un double clic n'est pas une séquence d'octets : c'est **deux séquences et un délai**. Le
décodeur est pur — mêmes octets, mêmes événements, toujours —, et c'est cette pureté qui
permet au fuzzing de vérifier qu'un découpage arbitraire de l'entrée produit la même suite
d'événements. Y mettre une horloge la détruirait : le même flux rejoué donnerait deux
résultats.

La synthèse vit donc dans la **boucle d'application**, seul endroit qui voit tous les
événements avant l'arbre et qui connaît déjà le temps. Pas dans les widgets : ce serait la
même règle écrite douze fois, et le jalon M7c a payé cette leçon sur le filtrage des
relâchements de touche. Pas dans `dispatch_event` non plus, qui est une fonction pure sur
l'arbre et doit le rester pour être testable sans horloge.

Le suiveur ne tient d'ailleurs **pas** d'horloge lui-même : l'instant lui est donné, et la
mesure d'écart aussi. C'est ce qui rend la règle vérifiable par des essais déterministes,
là où un `Instant::now()` à l'intérieur demanderait de faire dormir un test — donc un test
lent et instable. Le seuil est celui de Borland : `TEventQueue::doubleDelay`, 8 ticks à
18,2 Hz, soit 440 ms, avec ses trois conditions reprises telles quelles — même bouton,
exactement la même cellule, et le délai.

### Le protocole Kitty, et pourquoi il était indispensable

Écrit au jalon M2 (`air-tui/src/input/kitty.rs`). C'est le seul mécanisme qui livre les
modificateurs complets à travers un pty : sans lui, `Ctrl+Shift+S` et `Ctrl+S` produisent
tous deux l'octet `0x13` et sont indiscernables. Il apporte aussi deux choses qu'aucun
autre étage terminal ne donne :

- les **relâchements de touche**, et la distinction enfoncement / répétition. Le
  framework les **écarte à la distribution** (`view::dispatch_event`) : une vue qui
  traiterait un relâchement agirait deux fois par frappe. Une répétition, elle, passe —
  maintenir une flèche doit répéter le déplacement. Les deux moitiés de cette règle sont
  éprouvées, parce qu'un filtre trop large passerait la première et casserait la seconde ;
- la **touche de la disposition de base**, exposée par `AirKeyEvent::base_layout` et
  `shortcut_key()`. C'est ce qui fait fonctionner `Alt+F` pour « Fichier » sur un clavier
  cyrillique comme sur un QWERTY — exactement le problème qu'un framework à menus doit
  résoudre, et que le codage historique ne permet pas de résoudre du tout.

**La négociation ne repose pas sur un délai.** On envoie l'interrogation `CSI ? u`
*suivie* d'une demande d'attributs de périphérique. Les terminaux répondent dans l'ordre :
voir les attributs sans avoir vu de réponse Kitty avant eux est donc une conclusion
négative **certaine**. Le délai ne sert qu'au terminal qui ne répond à rien, pour ne jamais
geler le lancement. Et ce qui est tapé pendant l'interrogation est transmis à l'appelant,
pas jeté.

Le repli est `modifyOtherKeys` niveau 2, demandé **sans interrogation** : il n'existe pas
de sondage universel pour lui, mais la demande est inoffensive — un terminal qui ne la
comprend pas l'ignore.

Le protocole demandé est défait à la destruction du terminal. Laisser un terminal en
protocole Kitty après la sortie rendrait le shell inutilisable.

Deux limites consignées : la table des touches fonctionnelles ne couvre que les numéros
**certains** — tabulation, entrée, échappement, retour arrière, caractères imprimables,
`F13` à `F35` —, le pavé numérique et les touches multimédia étant hors table et donc
consommés sans événement ; et cette table a été écrite d'après la spécification, **pas
vérifiée contre un terminal réel**.

L'étage **natif** (M8) reste à écrire.

## Le rendu

`AirSurface` tient deux grilles — la courante et la précédente — et produit un
différentiel. `AirTerminal` traduit ce différentiel en séquences ANSI, encadrées par les
séquences de mise à jour synchronisée (`2026h` / `2026l`) quand le terminal les annonce,
ce qui supprime le scintillement sur les redessins larges.

Aucune allocation sur le chemin chaud : les deux grilles et le tampon de sortie sont
alloués à la construction et réutilisés (ADR-021 §4).

## MVC

Détaillé dans [`adrs/ADR-007-mvc-journal-de-changements-fr.md`](adrs/ADR-007-mvc-journal-de-changements-fr.md).
L'arête Observer du MVC canonique est remplacée par une **donnée rendue à l'appelant** :
le modèle consigne ce qui a changé et ne notifie personne. Conséquence directe : aucun
`Rc`, aucun `RefCell`, et la réentrance est impossible par construction.

Écrit au jalon M6 (`air-tui/src/model/`), et `redit` emploie les quatre mécanismes :

| Mécanisme | Ce qu'il porte, dans `redit` |
|---|---|
| `AirChangeSet` | le fichier chargé, le nombre de lignes, le drapeau de modification |
| `AirGeneration` | le texte, le curseur et la sélection — trois compteurs, comparés en O(1) à chaque tour |
| `AirCommandSet` au dessin | « Enregistrer » grisé sur un document intact, menu **et** barre de statut |
| `AirViewId` | le contrôleur retrouve sa vue d'édition et ses deux ascenseurs (ADR-013) |

Le partage entre le journal et les compteurs n'est pas une question de goût : un journal de
ce qui est fréquent devient une liste que personne ne relit, et un compteur sur ce qui est
structurel ne dit pas **quoi** a changé. Une frappe ne consigne donc rien ; elle fait
avancer deux entiers.

### La vue d'édition ne tient pas son modèle

`AirEditorView` ne porte aucune référence vers le tampon : ni `&`, ni `Rc`, ni pointeur.
Elle en garde un **reflet** — les lignes visibles et la position du curseur — que le
contrôleur lui pousse à l'étape 3, et elle accumule des `AirTextAction` que le contrôleur
relève à l'étape 2. Une frappe ne peut donc pas muter le modèle depuis la vue.

Deux conséquences qui valent le détour : le **défilement appartient à la vue**, donc deux
vues du même tampon défilent séparément ; et ce que la vue reflète est borné par la
dimension de la fenêtre, pas par la longueur des lignes — une ligne d'un mégaoctet ne coûte
que ses colonnes visibles.

### La sélection éprouve ce découpage — jalon M7d

La sélection est **dans le tampon**, parce que le texte y est. Mais ce que la vue doit
peindre, c'est un intervalle de **colonnes**, et les bornes sont des décalages en octets :
les convertir demande le début de la ligne, que le reflet ne garde pas. La conversion se
fait donc au moment du reflet, seul instant où la vue voit le tampon — exactement la place
que l'ADR-007 lui réserve. Une vue qui aurait gardé une référence vers le tampon n'aurait
pas eu à se poser la question, et aurait reconverti à chaque dessin.

Le presse-papiers, lui, n'est **ni dans le modèle ni dans la vue** : il est partagé entre
les deux widgets éditables, donc il appartient au contrôleur. Les vues le **demandent** par
trois commandes — `COPY`, `CUT`, `PASTE` — sans jamais le toucher, comme un ascenseur
demande une position sans rien défiler. La **recherche** sort par le même canal, et pour la
même raison : `FIND` et `FIND_NEXT` demandent de poser une question à l'utilisateur, ce
qu'une vue d'édition ne sait pas faire. Un widget qui copierait lui-même aurait besoin d'une
référence mutable vers un état qu'il ne possède pas.

Le champ de saisie fait exception, et c'est assumé : il tient sa sélection lui-même, parce
qu'il tient déjà son texte. Lui donner un modèle pour une ligne serait payer l'ADR-007 sans
rien en retirer.

### La recherche est une question, pas une action — jalon M7l

L'ADR-007 fait passer toute mutation et tout déplacement par une `AirTextAction`. La
recherche n'en est pas une : elle ne change rien, elle **répond**. `find_from` est une
méthode de **lecture** du tampon, et l'énumération des actions — la table qui dit tout ce
qui peut arriver au texte — n'a pas bougé d'une ligne (ADR-016).

Ce que le contrôleur en fait passe par deux actions qui existaient déjà : un déplacement
qui efface la sélection vers le début de l'occurrence, un déplacement qui l'étend vers sa
fin. Appliqués au même tour, donc la vue ne voit que l'état final.

Le gain n'est pas théorique. Une action `Find` aurait obligé le tampon à dire « je n'ai
rien trouvé » par le canal des **changements structurels**, c'est-à-dire à inventer un fait
qui n'en est pas un : rien n'a changé. L'échec d'une recherche est une valeur de retour.

Et ce qui reste au contrôleur y reste pour une raison nommée : le **bouclage** — reprendre
au début du document — est une politique qui se voit à l'écran, et le **dernier motif** est
au contrôleur parce qu'un motif n'est pas une propriété du document.

L'unité de comparaison est la **grappe de graphème** et non l'octet : une occurrence qui
commencerait au milieu d'une grappe donnerait une sélection que le curseur, qui se déplace
par grappes depuis M3, ne sait pas décrire. Chercher `e` dans un `é` décomposé ne trouve
donc rien, et la comparaison grappe à grappe **est** la vérification des frontières — il
n'y a pas de passe de validation à oublier.

### L'annulation est une propriété de l'unique mutation — jalon M7h

L'ADR-014 tranche entre deux choses qui portent le même nom. Un **instantané** du tampon
par pas d'annulation est simple et incapable de se tromper, au prix de la taille du
document à chaque frappe. Un **journal de remplacements** coûte la longueur du texte qui
change, donc un caractère pour une frappe — mais il n'est juste que si chaque mutation
produit son inverse exact.

C'est cette contrainte qui a payé. Le tampon portait **sept** chirurgies de lignes écrites
à la main — frappe, collage, saut de ligne, deux suppressions de grappe, suppression de
ligne, suppression de sélection. Chacune découpait et joignait les lignes à sa façon, donc
chacune aurait demandé son inverse. Il n'y en a plus qu'une, `splice`, à laquelle les sept
se ramènent par un triplet `(début, fin, texte)`.

L'annulation n'est donc pas une couche posée à côté du tampon : c'est une propriété de son
unique point de mutation. Un huitième chemin d'édition qui la contournerait ne compilerait
pas sans toucher aux champs privés.

Deux effets de bord que la refonte a rendus visibles, et qui n'étaient pas prévus :

- **taper sur une sélection était deux gestes** — une suppression, puis une insertion —, et
  l'utilisateur aurait dû annuler deux fois ce qu'il avait fait une. Les trois insertions
  remplacent désormais la sélection **dans le même remplacement**, ce qui a aussi retiré un
  appel à la suppression de sélection sur trois chemins ;
- **la plage d'une dernière ligne supprimée** part de la fin de la **précédente**, parce que
  le saut de ligne à emporter est devant elle et non derrière. L'écriture d'origine retirait
  la ligne du vecteur, ce qui masquait la question ; la refonte l'a posée, et un essai
  existant a attrapé l'écart.

Le drapeau de modification est **dérivé** de l'historique et non posé à la main : annuler
jusqu'au point d'enregistrement éteint l'étoile du titre. Un drapeau que seule une édition
allume y resterait allumé sur un texte revenu à son état enregistré.

Le grisage des deux commandes reste en *pull*, et c'est ce qui fait que ce jalon n'a ajouté
**aucun** changement structurel à l'ADR-007 : `can_undo` et `can_redo` sont lues au dessin.

### L'application n'en tient pas plus

`AirApplication::pump` fait **un** tour — attendre, lire, décoder, router — et rend les
commandes que l'arbre n'a pas traitées ; la racine lui est **passée en paramètre**. C'est le
même argument deux fois : ce dont le contrôleur a besoin, l'application ne le détient pas.
Une première écriture faisait l'inverse, et le contrôleur de `redit` ne pouvait plus
atteindre ses propres vues.

### Une liste à plusieurs colonnes reste une liste à un indice — jalon M7m

La mise en page est l'affaire de la **vue**, et c'est tout ce que les colonnes multiples
ont demandé : `AirListBox` a gagné un nombre de colonnes, et ni l'item, ni la demande
qu'une liste produit, ni le contrôleur qui lui pousse son reflet n'ont changé de forme. Les
items restent une suite à un seul indice ; la grille est une lecture de cette suite, de haut
en bas puis de gauche à droite.

Ce qui en découle n'est pas un réglage de plus mais une **unité** qui change : avec
plusieurs colonnes, le défilement se compte en colonnes et une page vaut la surface
visible. C'est pourquoi `visible_items` existe à côté de `visible_rows`, et pourquoi c'est
la première que l'ascenseur reçoit — il mesure ce que la fenêtre montre, non sa hauteur.
Les quatre règles (ordre de remplissage, alignement du défilement, taille d'une page,
`Gauche`/`Droite` prises seulement s'il y a plusieurs colonnes) sont transcrites de
`TListViewer`, et le pas de flèche que son constructeur donne à son ascenseur — `size.y`
quand il y a plusieurs colonnes, 1 sinon — est ce qui confirme l'alignement plutôt que de
le laisser deviner.

## Les thèmes

Écrit au jalon M5. Un thème est le **triplet** que l'ADR-008 décrit — palette, glyphes,
décorations — parce que ce qui fait reconnaître un look tient autant aux caractères qu'aux
couleurs. Il a été un **couple** pendant quatre jalons : la seule décoration que la
troisième assiette aurait portée est l'ombre portée, et elle tombe hors des limites de la
vue qui la projette. L'ADR-015 a tranché la question au jalon M7j, et le jalon M7k a donné
à l'assiette son second champ — l'ombre des boutons, que Turbo Vision dessine et que
Midnight Commander ne dessine pas.

### L'ombre portée, et l'invariant qui n'a pas bougé

Le peintre découpe toute écriture aux limites de la vue, sans exception — c'est ce qui fait
qu'une étiquette trop large ne mange plus la bordure qui l'entoure, ce qu'elle faisait au
jalon M0. L'ombre d'une fenêtre tombe pourtant **dehors**.

La réponse de l'ADR-015 : ce qui dépasse d'une vue est peint par le **propriétaire de la
surface où cela dépasse**. L'ombre appartient au bureau, pas à la fenêtre, et c'est donc le
bureau qui l'écrit — dans ses propres limites, juste avant de dessiner la fenêtre.

Le dépôt employait déjà cette famille de solutions sans l'avoir nommée : la barre de menu
**agrandit ses propres limites** à l'ouverture d'un menu, depuis le jalon M5b. Agrandir ne
convenait pas à l'ombre, pour une raison précise — la zone d'ombre entrerait dans la
détection de clic de la fenêtre et dans le calcul de son intérieur.

L'ombre d'une fenêtre **recolore** ce qui est dessous en gardant le caractère, ce qui lui
donne l'aspect d'une ombre sur le motif `░` du bureau plutôt que d'un rectangle collé à la
fenêtre. Elle est découpée comme tout le reste : une fenêtre contre le bord du bureau n'en
projette qu'un morceau.

Celle d'un **bouton** n'a pas la même apparence, et c'est pourquoi la vue déclare un
**genre** — `AirShadowStyle` — et non un booléen : un trait fin qui épouse le bouton, peint
en demi-blocs `▄` et `▀`, les caractères `0xDC` et `0xDF` que Turbo Vision emploie pour
qu'une ombre de bouton paraisse deux fois plus fine que celle d'une fenêtre. Un demi-bloc
ne couvrant que la moitié de sa cellule, elle garde le **fond** de ce qu'elle assombrit et
n'en change que l'avant-plan — règle qui donne exactement l'attribut `0x70` de Borland dans
un dialogue gris, et qui tient encore sur la fenêtre bleue de l'exemple `frame`.

Deux propriétaires peignent des ombres, et pas un seul : la traversée de l'arbre, pour les
enfants d'un groupe, et `AirPromptDialog`, qui compose ses contrôles lui-même. Les deux
passent par la même fonction, `view::draw_view_shadow`.

### Un rôle décrit une présentation, pas un couple de couleurs

Depuis le jalon M7g, `AirColorPair` porte des **attributs** — graisse, italique, souligné,
clignotant, inverse, barré — en plus de ses deux couleurs. Le thème Turbo Vision n'en pose
aucun : les seize couleurs du VGA portent déjà la distinction vive/sombre dans l'index
lui-même. Les skins de `mc`, eux, en posent, et c'était le premier motif de refus de
l'importeur.

Deux chemins mènent un attribut à la grille, et aucun autre :
`AirColorPair::cell(character)` et `AirPainter::draw_text_with_colors`. Un widget qui
construirait sa cellule à partir des deux champs de couleur perdrait l'attribut **en
silence** — et comme aucun des deux thèmes livrés n'en pose, rien ne le montrerait. Un test
peint donc les douze widgets avec une palette dont les quarante rôles sont gras, et
refuse la moindre cellule qui ne l'est pas.

`AirColor` compte une troisième forme, `TerminalDefault` : non pas une couleur, mais
l'absence de choix — « laisser au terminal celle qu'il emploie déjà ». Elle s'émet par les
paramètres SGR `39` et `49`, qui *annulent* une couleur au lieu d'en poser une, et c'est la
seule que le quantificateur laisse passer inchangée : une couleur qu'on ne connaît pas n'a
pas de plus proche voisine.

### La palette est transcrite, pas approchée

Les valeurs viennent de `cpAppColor` et `cpGrayDialog` de `magiblot/tvision`, en suivant
les chaînes de `cpBackground`, `cpMenuView`, `cpStatusLine`, `cpFrame` et `cpButton`.
Chaque entrée porte en commentaire l'octet d'origine et sa chaîne : la transcription se
vérifie ligne à ligne contre le source C++.

Deux pièges s'y cachaient, et ils ont chacun produit une correction :

- **les indices VGA ne sont pas les indices ANSI.** Dans un attribut VGA, 1 est le bleu et
  4 le rouge ; en ANSI c'est l'inverse. Transcrire sans conversion donnait un bureau rouge
  et des lettres d'accès rapide bleues. La conversion est faite par `ansi_from_vga`, et un
  test vérifie qu'elle est une bijection qui échange bien ces paires ;
- **la barre de statut n'est pas cyan.** `cpStatusLine` et `cpMenuView` partagent la même
  chaîne : elle est noir sur gris clair, comme la barre de menu. Les premières valeurs de
  ce projet la donnaient sur cyan, par souvenir plutôt que par lecture.

### Le quantificateur

Requis et non optionnel (ADR-008), pour une raison concrète : un thème s'écrit en RGB,
mais une console Linux n'a que seize couleurs. Sans lui, un thème soigné s'affiche en
couleurs arbitraires sur la moitié des terminaux visés.

Trois profondeurs — vraies couleurs, deux cent cinquante-six, seize —, déduites de
`COLORTERM` et de `TERM` et forçables par l'application. La quantification a lieu **au
dernier moment**, dans le rendu : un thème est écrit une fois, et chaque terminal reçoit ce
qu'il sait afficher. L'encodeur SGR, lui, reste pur.

Une couleur déjà représentable traverse sans être touchée, et la quantification est
**idempotente** — un test le vérifie, parce que sans cela un rendu partiel et un rendu
complet donneraient deux images différentes.

La métrique est euclidienne dans l'espace RGB. Ce n'est pas perceptuel, et c'est dit : une
distance CIE choisirait mieux sur les teintes voisines, au prix de tables et d'une
arithmétique en flottant qu'ADR-005 ne permet pas.

### Thème et coquille restent deux choses

Ce qui fait reconnaître Midnight Commander est sa **coquille** — deux panneaux, ligne de
commande, barre F1-F10 — pas sa palette. `AirTheme` porte palette, glyphes et décorations ;
la composition du chrome relève de l'`AirScreen` que l'application choisit. D'où deux
applications `redit` et `rmc`, et non une application à deux thèmes.

Les thèmes sont **compilés, pas analysés au runtime** : l'importeur de skins Midnight
Commander est un `xtask` qui compile un `.ini` en artefact Rust committé (Principe 11
d'Air), écrit au jalon M7a.

Les **trente-neuf** skins livrés avec `mc` 4.8.33 sont importables depuis M7g. Ils étaient
neuf, et le chemin pour y arriver est la leçon du jalon : l'ADR-008 chiffrait les refus par
cause, mais l'importeur s'arrête à la **première** cause qu'il rencontre — le décompte
comptait donc une cause par skin là où la plupart en avaient deux. Lever les deux limites
du modèle n'a fait passer le compte qu'à seize ; les vingt-trois autres étaient refusés
pour une clef absente, que `mc` fait simplement **hériter** du `_default_` de sa section
puis de celui de `[core]`. Ce n'était pas une limite du modèle d'air-tui, c'était un défaut
de l'analyseur, rangé du mauvais côté pendant six jalons.
