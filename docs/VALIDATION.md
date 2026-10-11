# Validation

## Vérifié sur machine

- Jeu local sous Proton Experimental ; processus du jeu présent.
- Accès en lecture seule à `/proc/<pid>/mem` possible depuis le compte utilisateur.
- `/dev/uinput` accessible ; son coupé.
- Binaire Rust statique musl exécuté sur SteamOS, sans installation système.
- Profil 2.03.02 protégé par l'empreinte SHA-256 du binaire du jeu ; RTTI des gestionnaires client/serveur vérifiée.
- Personnage contrôlé : possession aller-retour et même identifiant dans les deux domaines.
- Coordonnées et santé lues en direct. Les maxima des ressources restent inconnus.
- Script Rhai de mesure : commande de recul de 150 ms, déplacement observé d'environ 0,27 unité, santé stable ; commandes relâchées. La commande de retour n'a pas ramené le personnage au point initial, donc le retour exact n'est pas validé.
- Copie indépendante des sauvegardes avec contrôle SHA-256 et vérification que les sources n'ont pas changé pendant la copie.
- Script passif terminé de manière autonome ; tests synthétiques et Clippy passent.
- Inventaire serveur lu avec le stride 2.03.02 de 200 octets : l'ancien stride de 192 octets produit des données invalides et n'est pas utilisé pour ce profil.
- OCR local : texte du HUD et du journal reconnu. L'ouverture du journal est asynchrone ; le script doit attendre l'état attendu au lieu de supposer une transition immédiate.
- Sélection autonome de l'index des factions, de la vie quotidienne de Hernand et d'une entrée de quête ; titre de détail vérifié par le script.
- Catalogue d'objets lu depuis `ItemInfoManager`, définitions à +0x58. La lavande et ses variantes ont des clés stables différentes et des index distincts résolus en direct.
- Régression de sélection : certains derniers caractères OCR absorbent les espaces vers la carte voisine. Centre médian des caractères et régions explicites testés.
- Capacité de récolte testée sur observations synthétiques : aucune réussite avant la hausse effective du bon objet dans le bon stockage.
- Reprise de la partie depuis le menu principal français « Jouer », avec sauvegarde indépendante préalable et attente de l'interface de jeu reconnue. Aucun choix de sauvegarde ancienne effectué.
- Navigation locale autonome : calibration d'un recul, cible calculée à une unité du point initial dans cette direction, arrivée observée dans un rayon de 0,2 unité, puis relâchement des commandes. L'essai a terminé avec le statut `succeeded` en environ 22 secondes.
- Nouvelle lecture de la transformation validée avec secteurs cohérente avec la position observée. Changements de secteur et parent testés sur données synthétiques.
- Lecture de scènes de collecte chargées : prefab et transformation sectorisée, avec revalidation du type. Les buissons de pivoine proches sont maintenant accessibles à l'API des scripts.
- Premier trajet de récolte arrêté sur commandes latérales sans mouvement, sans perte de santé. Ce résultat est un blocage constaté, pas une récolte réussie.
- Cause identifiée pour les directions non réactives : profil de touches incorrect. Le profil ZQSD a été vérifié sur les quatre directions, avec contrôle des axes opposés, santé et inventaire inchangés. Les réglages du jeu ont seulement été consultés.
- Première récolte autonome de pivoine : découverte de la cible par la mémoire, trajet d'environ 50 unités en dix segments, recherche de l'interaction par la caméra, prise, puis inventaire personnel passé de zéro à une pivoine. Santé stable et monnaie inchangée. Durée d'environ 9 min 32 s ; aucune quête n'est déclarée terminée par cet essai.
- Deuxième exécution réussie à proximité : inventaire personnel passé d'une à trois pivoines, santé stable, en environ 24 secondes. Une exécution indépendante a confirmé le gain de deux objets.

## À valider

- Sémantique des ressources autres que la santé, maxima et calibration de la caméra.
- Reconnaissance de l'interface au-delà des écrans testés, autres ressources et répétabilité de la récolte sur plusieurs cibles.
- Navigation sur longs trajets, obstacles, falaises, montures, combat et quête complète.

Les signatures publiques anciennes ne correspondent pas à ce build. Le profil actuel vient d'une exploration structurelle et d'une vérification en jeu ; il ne doit pas être appliqué à une autre empreinte.

Les preuves de terrain restent privées. Ce document rapporte seulement des résultats agrégés et leurs limites.
