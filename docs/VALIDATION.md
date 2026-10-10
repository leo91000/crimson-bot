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
- Script passif terminé de manière autonome ; cinq tests synthétiques et Clippy passent.

## À valider

- Inventaire, caméra, état de l'interface et cibles de récolte.
- Navigation avec observations, récolte confirmée, combat et quête complète.

Les signatures publiques anciennes ne correspondent pas à ce build. Le profil actuel vient d'une exploration structurelle et d'une vérification en jeu ; il ne doit pas être appliqué à une autre empreinte.

Les preuves de terrain restent privées. Ce document rapporte seulement des résultats agrégés et leurs limites.
