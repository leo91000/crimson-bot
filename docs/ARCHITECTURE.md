# Architecture

## Couches

1. **Adaptateur de jeu** : accès Linux en lecture seule à `/proc/<pid>/mem`, résolution de signatures du module PE, validation des pointeurs et production d'un état typé. Aucune adresse absolue n'est conservée d'une session à l'autre.
2. **Commandes** : périphérique virtuel via `/dev/uinput`, touches et mouvement relatif. Une commande possède une durée maximale. Toute interruption relâche les commandes.
3. **Capacités** : navigation, interaction, récolte et combat. Chaque capacité observe son résultat et distingue réussite, échec et information insuffisante.
4. **Objectifs Rhai** : composition des capacités ; état du script explicite entre les ticks et conditions de réussite. Les appels système restent dans le moteur Rust. La reprise d'un objectif après redémarrage du moteur reste à développer.
5. **Supervision** : limite de temps et d'opérations, vérification du focus, fraîcheur des observations, arrêt sur perte du processus et événements structurés.

## Boucle d'exécution

Observer → valider → appeler le script → valider sa décision → agir → mesurer le résultat.

Le moteur ne déclenche aucune commande quand les données obligatoires sont absentes. L'adaptateur indique séparément les capacités disponibles : une position valide ne prouve pas que l'inventaire ou la caméra sont accessibles.

## Évolution

Un script de quête dépend d'API stables, pas de pointeurs ni d'offsets. Les particularités d'une version du jeu restent dans l'adaptateur. Les essais produisent des traces privées qui alimentent des cas synthétiques publics de régression. Les politiques de navigation/combat sont partagées entre les quêtes.

## Sûreté des essais

Les copies de sauvegarde sont vérifiées avant toute émission de commandes. Elles sont conservées à part et ne sont jamais restaurées automatiquement. Une seule instance peut contrôler le jeu. Un arrêt demandé, une erreur de script ou une perte de focus entraîne le relâchement des touches.
