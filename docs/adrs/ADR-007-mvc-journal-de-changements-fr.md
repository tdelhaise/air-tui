# ADR-007 — MVC par journal de changements

**Statut :** Accepté — 2026-10-10

## Contexte

L'auteur veut du MVC, avec une nette préférence sur MVVM. Or le MVC canonique repose sur
l'Observer, et l'Observer est précisément ce que Rust rend coûteux : l'arête de
notification est une **référence arrière** du modèle vers les vues, alors que le contrôleur
tient déjà les deux.

Les deux contournements classiques se paient :

- `Rc<RefCell<dyn Observer>>` : cycles qui fuient sans `Weak`, et surtout **réentrance** —
  une vue qui modifie le modèle pendant qu'elle est notifiée déclenche un panic
  `already mutably borrowed`. On a converti une erreur de compilation en panne de
  production ;
- callbacks `Box<dyn FnMut(..)>` stockés dans le modèle : modèle `!Send`, intestable
  isolément, et ordre de notification devenu contractuel sans être documenté.

## Décision

**L'arête Observer devient une donnée rendue à l'appelant.** Le modèle ne connaît aucune
vue et n'appelle personne.

```rust
pub trait AirModel {
    type Action;
    type Change;

    /// Applique `action` et consigne les changements dans `changes`.
    ///
    /// `changes` est fourni par l'appelant et réutilisé d'un tour de boucle à l'autre :
    /// aucune allocation sur le chemin chaud (ADR-021 §4).
    fn apply(&mut self, action: Self::Action, changes: &mut AirChangeSet<Self::Change>);
}
```

Boucle du contrôleur, un seul emprunt mutable vivant à la fois :

1. entrée → intention : les vues ne voient le modèle qu'en lecture et retournent une
   `Action` ;
2. intention → mutation : `model.apply(action, &mut changes)`, les vues inaccessibles ;
3. faits → réaction : `screen.react(changes.as_slice(), &model)` ;
4. rendu différentiel.

Quatre mécanismes complémentaires, aucun n'employant `Rc` ni `RefCell` :

| Mécanisme | Emploi |
|---|---|
| journal de changements | événements **structurels** rares : fichier ouvert, fenêtre fermée |
| compteurs de génération monotones | état **continu** fréquent : texte, curseur, défilement — comparaison en O(1), zéro allocation |
| `AirCommandSet` interrogé au dessin | griser menus, boutons et items de statut — c'est du *pull*, pas de l'Observer |
| abonnement par `AirViewId` | quand une vue doit réagir à autrui : table d'identifiants chez le contrôleur, jamais de pointeur arrière |

**MVVM garde une place, bornée aux formulaires** : `AirBinding<T>` est un ViewModel local à
un `AirDialog`, qui possède les valeurs de champs et leur état de validation. Les widgets
s'y lient par `&mut` sur un objet que le dialogue possède ; aucun des problèmes ci-dessus
ne se pose. L'application reste en MVC, la saisie est en MVVM, la frontière est le
dialogue.

## Conséquences

- La réentrance est **impossible par construction** : une vue ne peut pas muter le modèle,
  elle ne peut qu'émettre une `Action`.
- L'ensemble de ce qui peut se propager est auditable par un `grep` sur l'énumération
  `Change`.
- Les trois phases de routage de Turbo Vision (`PreProcess`, `Focused`, `PostProcess`) sont
  conservées : elles sont orthogonales au MVC et ce sont elles qui font fonctionner menus,
  raccourcis et barre de statut.

## Éprouvé au jalon M7d : la sélection et le presse-papiers

Trois questions que la sélection a posées, et que ce découpage a tranchées sans amendement.

**Où vit la sélection ?** Dans le modèle, parce que le texte y est. Mais ce que la vue
dessine est un intervalle de **colonnes**, quand les bornes sont des décalages en octets :
la conversion demande le début de la ligne, que le reflet ne garde pas. Elle se fait donc à
l'étape 3, au moment où le contrôleur pousse le reflet — seul instant où la vue voit le
modèle. La contrainte a désigné sa place ; il n'a pas fallu la choisir.

**Où vit le presse-papiers ?** Ni dans le modèle, ni dans la vue. Il est partagé entre les
widgets éditables, donc il appartient au **contrôleur**. Les vues le demandent par trois
commandes — `COPY`, `CUT`, `PASTE` — sans jamais le toucher, exactement comme un ascenseur
demande une position sans rien défiler. Un widget qui copierait lui-même aurait besoin
d'une référence mutable vers un état qu'il ne possède pas, ce que cet ADR interdit.

**Un troisième compteur était-il nécessaire ?** Oui, et la raison est instructive :
abandonner une sélection ne déplace pas le curseur, et la vue doit pourtant repeindre.
Réutiliser la génération du curseur aurait laissé une surbrillance orpheline à l'écran. Un
compteur par chose qui peut changer **seule** — c'est la règle, et elle se vérifie en
essayant de la violer.

Le grisage en *pull* a servi une seconde fois sans rien ajouter : « Copier » et « Couper »
n'ont pas de sens sans sélection, « Coller » n'en a pas sur un presse-papiers vide. L'état
est lu à chaque tour, personne n'est notifié, et l'item de menu se grise partout à la fois.

**Une exception, assumée :** le champ de saisie tient sa sélection lui-même. Il tient déjà
son texte, et lui donner un modèle pour une ligne serait payer cet ADR sans rien en
retirer. La frontière n'est pas « tout widget a un modèle », elle est « ce qui est partagé
ou persistant a un modèle ».

## Éprouvé au jalon M7h : l'annulation n'a demandé aucun mécanisme de plus

C'est le résultat qui compte ici, et il est négatif au bon sens du terme : l'annulation
(ADR-014) n'a ajouté **ni** canal, **ni** changement structurel, **ni** mécanisme.

- l'historique vit dans le **modèle**, parce que seul lui connaît le texte qu'une opération
  retire, donc seul lui peut produire l'inverse. C'est l'exact inverse du presse-papiers,
  qui est au contrôleur parce qu'il est **partagé** entre deux widgets. Les deux décisions
  suivent la même règle : une chose appartient à qui la connaît ;
- `Ctrl`+`Z` sort par le canal des **intentions** et non par celui des commandes, puisque la
  vue d'édition adresse déjà le modèle. Les commandes `UNDO` et `REDO` n'existent que pour
  la barre de menu, qui ne sait émettre que des commandes ;
- le grisage des deux items est en **pull** — mécanisme 3 — : `can_undo` et `can_redo` sont
  lues au moment du dessin. Rien n'est consigné quand le journal se remplit ou se vide, et
  il n'en faut rien : une vue qui demande au moment où elle peint ne peut pas afficher un
  état périmé ;
- le drapeau de modification, lui, **est** un changement structurel — il l'était déjà —, et
  il bascule désormais dans les deux sens. Il est dérivé de l'historique, de sorte
  qu'annuler jusqu'au point d'enregistrement l'éteigne.

Le seul endroit où l'annulation a touché le découpage de cet ADR est le compteur de
génération du texte : annuler l'avance, comme une édition, parce que la vue doit repeindre
pour la même raison.

## Alternatives écartées

**`Rc<RefCell<Model>>` partagé entre les vues.** Panic au premier dessin où deux vues
veulent le modèle alors qu'une le tient en écriture — et le défaut ne se manifeste que sous
charge.

**`std::sync::mpsc` pour les événements d'interface.** Tentant car dans `std`, mais ajoute
une file à drainer au bon moment et perd l'ordre relatif au rendu. Réservé aux sources
réellement asynchrones : chargement de fichier en tâche de fond, minuterie.
