# Cahier des charges confirmé

## Utilisation

Le bot tourne sur une Steam Machine, dans la version locale du jeu sous Proton. L'utilisateur donne des objectifs au développeur, qui les traduit en scripts, suit leur exécution et corrige les causes des problèmes constatés. L'utilisateur ne souhaite pas administrer la logique interne.

Les scripts doivent pouvoir grandir : récolte, déplacements, interactions, combats et objectifs composés. Ils ne doivent pas dépendre d'une séquence aveugle de touches ni de l'intervention du développeur à chaque observation.

## Contraintes

- Runtime entièrement local. Pas d'appel à une IA payante.
- Reverse engineering autorisé pour lire l'état du jeu. Pas de triche : aucune écriture mémoire, téléportation ou validation artificielle de quête.
- Le son est coupé pendant le travail sur la machine.
- Essais avec sauvegardes indépendantes, tentatives bornées et dépenses encadrées.
- Arrêt avant les décisions irréversibles de quête et les dépenses exceptionnelles.
- Le premier objectif est une quête secondaire de récolte peu intéressante, sélectionnée par le développeur parmi les quêtes disponibles.
- Documentation et publication régulière des étapes vérifiées dans un dépôt GitHub public.

## Critères de réussite

1. Identifier de manière fiable le personnage contrôlé, sa position et ses ressources utiles.
2. Confirmer que les commandes normales produisent le changement attendu, puis relâcher toutes les commandes.
3. Identifier une cible de récolte et naviguer jusqu'à elle avec détection des blocages.
4. Récolter et vérifier l'augmentation d'inventaire ou la progression de l'objectif.
5. Répéter la tâche sans intervention dans la boucle d'action.
6. Étendre aux combats et aux quêtes composées avec les mêmes garanties.

Ces critères décrivent le résultat attendu, pas des capacités déjà acquises.
