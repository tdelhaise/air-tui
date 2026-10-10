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

## Alternatives écartées

**`Rc<RefCell<Model>>` partagé entre les vues.** Panic au premier dessin où deux vues
veulent le modèle alors qu'une le tient en écriture — et le défaut ne se manifeste que sous
charge.

**`std::sync::mpsc` pour les événements d'interface.** Tentant car dans `std`, mais ajoute
une file à drainer au bon moment et perd l'ordre relatif au rendu. Réservé aux sources
réellement asynchrones : chargement de fichier en tâche de fond, minuterie.
