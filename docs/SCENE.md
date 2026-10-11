# Découverte des objets du monde

`crimson-bot probe-scene --radius 20 --actors 4` inspecte en lecture seule les associations des acteurs proches. Cet outil sert au reverse engineering des cibles de récolte. Il n'émet aucune commande et ne produit pas de cible utilisable par les scripts.

Il exige le profil du build connu et un personnage contrôlé identifié. Les positions des autres acteurs sont des candidates : registre de composants à +0x78, nombre à +0x80, enregistrements de 16 octets, type `ServerTransformSyncActorComponent`, puis décodeur de transformation. Ces coordonnées serveur et leur relation au monde restent à vérifier sur la machine avant de les utiliser pour guider le bot.

Pour chacun des acteurs retenus, le parcours cherche des objets dont le RTTI mentionne `SceneObject` ou `ResourcePath`, ainsi que des chemins internes de prefab. Il suit au maximum cinq pointeurs successifs et lit au maximum 256 nœuds. Les cycles sont ignorés ; les limites et les résultats partiels sont explicitement indiqués dans le rapport. Les seules chaînes rapportées sont des noms de types validés et des chemins de ressources sous `/object/` ou `/character/`, terminés par `.prefab`.

Une association de pointeurs ne prouve ni qu'une plante est présente, ni qu'elle est récoltable, ni que la position appartient à cette ressource. La prochaine validation en jeu doit établir la chaîne exacte vers le prefab, l'identité de l'objet et sa position, puis confirmer l'interaction et le gain d'inventaire. Les rapports réels restent privés.

Les tests synthétiques couvrent la découverte d'un chemin de plante, le filtrage des autres chaînes, les cycles et les graphes denses. L'outil n'a pas encore été exécuté sur la Steam Machine.
