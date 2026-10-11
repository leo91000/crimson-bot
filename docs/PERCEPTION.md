# Perception par les données du jeu

Contrainte explicite : aucun OCR. Le moteur ne contient ni bibliothèque OCR, ni modèle ONNX, ni analyse de pixels. La boucle de contrôle lit les données natives du processus en lecture seule et agit par les entrées ordinaires.

## Fournisseurs validés

Personnage contrôlé, position sectorisée, santé, inventaire et catalogue d'objets sont lus depuis des structures typées avec validation RTTI. Les scènes de collecte chargées sont revalidées à chaque observation ; leur présence ne suffit pas à connaître leur disponibilité.

## Fournisseurs à résoudre

- État natif d'interface : jeu, menus, dialogue, transition.
- Interactions : cible actuelle, type, ressource et disponibilité.
- Quêtes : identifiant stable, étape, conditions et achèvement.
- PNJ : définition de personnage, instance et position.

Ces champs restent indisponibles tant que leur structure n'est pas vérifiée. Le runtime refuse alors les entrées concernées. Une instance vivante et focalisée ne prouve pas que ses commandes s'adressent au monde plutôt qu'à un menu.

Le reverse engineering a identifié les composants serveur d'interaction (+0x98 du conteneur à +0x68), dialogue de quête (+0xF8) et quête (+0x138). Leur pointeur propriétaire à +0x08 a été recoupé avec l'acteur contrôlé. Cela valide leur association, pas encore le format de leurs états.

Le catalogue `CharacterInfoManager` fournit la définition `NHM_Unique_Theoric`, clé 1001865 sur le build exploré. L'index de définition et un nom de catalogue ne prouvent pas l'identité d'une instance ; le lien à un acteur reste à établir.

Les captures éventuelles servent au diagnostic de développement. Elles ne sont pas lues par le runtime et ne déclenchent aucune décision.
