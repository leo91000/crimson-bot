# Découverte des objets du monde

`crimson-bot probe-scene --radius 20 --actors 4` inspecte en lecture seule les associations des acteurs proches. Cet outil sert au reverse engineering des cibles de récolte. Il n'émet aucune commande et ne produit pas de cible utilisable par les scripts.

Il exige le profil du build connu et un personnage contrôlé identifié. Les positions des autres acteurs sont des candidates : registre de composants à +0x78, nombre à +0x80, enregistrements de 16 octets, type `ServerTransformSyncActorComponent`, puis décodeur de transformation. Ces coordonnées serveur et leur relation au monde restent à vérifier sur la machine avant de les utiliser pour guider le bot.

Pour chacun des acteurs retenus, le parcours cherche des objets dont le RTTI mentionne `SceneObject` ou `ResourcePath`, ainsi que des chemins internes de prefab. Il suit au maximum cinq pointeurs successifs et lit au maximum 256 nœuds. Les cycles sont ignorés ; les limites et les résultats partiels sont explicitement indiqués dans le rapport. Les seules chaînes rapportées sont des noms de types validés et des chemins de ressources sous `/object/` ou `/character/`, terminés par `.prefab`.

Une association de pointeurs ne prouve ni qu'une plante est présente, ni qu'elle est récoltable, ni que la position appartient à cette ressource. La prochaine validation en jeu doit établir la chaîne exacte vers le prefab, l'identité de l'objet et sa position, puis confirmer l'interaction et le gain d'inventaire. Les rapports réels restent privés.

Les tests synthétiques couvrent la découverte d'un chemin de plante, le filtrage des autres chaînes, les cycles et les graphes denses. L'outil a été exécuté sur la Steam Machine : il retrouve des objets de scène, mais un parcours générique peut suivre des voisins d'allocation ou des équipements. Ces associations ne sont donc pas utilisées pour guider le bot.

## Candidats de récolte

L'adaptateur `resources.rs` utilise une lecture plus précise : objets `SceneObjectServer` validés par RTTI, chemin de prefab à +0x38, transformation de monde à +0x1A4 avec secteurs x/z. Les zones explorées doivent contenir un objet de scène validé ; aucun balayage arbitraire de toute la mémoire n'est effectué. Lecture limitée à 256 Mio par découverte, huit zones et 8 192 enregistrements proches. Les petites zones passent en premier pour éviter qu'une grande zone de ressources en cache n'épuise le budget.

L'index est renouvelé toutes les dix secondes. Le type, le chemin et la position sont relus à chaque observation. L'API `world.resource_candidates` contient au maximum 64 candidats dans un rayon de 150 unités ; `None` indique une lecture indisponible. `diagnostics.resource_scan` indique les scans partiels. `crimson-bot resources peony` filtre les candidats chargés, sans commande de jeu.

Les pivoines, arbres fruitiers et variantes de romarin ont été retrouvés en jeu. Les identifiants sont opaques et valables seulement dans le processus actuel. Un enregistrement peut correspondre à un objet désactivé ou déjà récolté : la présence en mémoire reste une proposition de cible. Le script exige encore une interaction nommée et un gain d'inventaire.
