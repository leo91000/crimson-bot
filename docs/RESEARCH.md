# Sources du reverse engineering

Ces sources décrivent d'autres versions. Leurs affirmations ne valent pas validation sur la version installée. Nous ne chargeons aucun de leurs mods dans le jeu.

- [Trinity](https://github.com/XeTrinityz/Trinity), MIT : `src/game/offsets.h`, `player.cpp`, `inventory.cpp`. Signatures du gestionnaire de personnages, test de possession et structures d'inventaire. Les constantes et commentaires structurels qui en sont dérivés sont attribués dans `NOTICE`.
- [CrimsonDesertCoop](https://github.com/blizz3010/CrimsonDesertCoop) : `docs/REVERSE_ENGINEERING.md`, `include/cdcoop/core/game_structures.h`. Chaînes de position et suggestions de validation ; les indications anciennes doivent être revalidées.
- [CD Companion](https://github.com/andreikapica-dot/CD_Companion) : résolution des coordonnées statiques et translation de l'origine du monde. Leurs mécanismes d'injection et de téléportation ne font pas partie de notre approche.
- [Crimson World Builder](https://github.com/Moon-yungg/crimson-desert-world-builder), MIT : `asi/cdmodkit/cdmodkit.cpp`, fonction `ReadTransformPos`. Structure de la transformation validée, secteurs et composition avec un parent ; attribution dans `NOTICE`.

Stratégie : retrouver les signatures en mémoire, exiger des résolutions non ambiguës, vérifier les structures puis confirmer les changements par les commandes normales. L'accès mémoire est ouvert exclusivement en lecture seule.

## Résultats du build 2.03.02

Les anciens motifs ne correspondent pas au binaire installé. Une exploration des pointeurs statiques et la lecture du RTTI MSVC ont identifié les gestionnaires client et serveur. Le gestionnaire serveur expose une liste à +0xB8 ; le personnage contrôlé est validé par la possession aller-retour. Le gestionnaire client expose le corps actif à +0x50. Les deux identifiants sont comparés avant de lire le composant de transformation client à +0x1A0.

La position est décodée depuis la transformation validée de 0x48 octets à +0xB4 dans ce composant : coordonnées locales, secteurs de 1 000 unités et transformation du parent éventuel. La première lecture à +0x29C correspondait aux coordonnées observées à pied, mais ne traitait pas ces relations. Le décodeur actuel a été vérifié en lecture sur la partie ; les changements de secteur et la composition avec un parent sont testés sur données synthétiques, sans essai à cheval.

Le profil contient seulement des RVA et des offsets, jamais les adresses privées d'une session. Il exige l'empreinte exacte du fichier exécutable et revalide les types et identités à chaque observation.

`ItemInfoManager` : racine +0x6CF30D0, compte à +0x08, définitions à +0x58. La liste à +0x50 contient les structures de recherche, pas les définitions complètes de ce build. Une définition expose la clé u32 à +0x00 et l'objet chaîne du nom interne à +0x08. Les clés de romarin et lavande ont été recoupées avec la [base communautaire](https://github.com/NattKh/CrimsonDesertCommunityItemMapping), puis les index ont été obtenus dans le processus, sans importer cette base dans le runtime.

Les tentatives de récolte doivent utiliser ces clés stables, une indication d'interaction reconnue et un résultat d'inventaire mesuré. Un petit déplacement vers une cible calculée a été validé en jeu. La découverte des plantes, les trajets dans le monde et la récolte réelle restent à implémenter ou à vérifier.
