# Sources du reverse engineering

Ces sources décrivent d'autres versions. Leurs affirmations ne valent pas validation sur la version installée. Nous ne chargeons aucun de leurs mods dans le jeu.

- [Trinity](https://github.com/XeTrinityz/Trinity), MIT : `src/game/offsets.h`, `player.cpp`, `inventory.cpp`. Signatures du gestionnaire de personnages, test de possession et structures d'inventaire. Les constantes et commentaires structurels qui en sont dérivés sont attribués dans `NOTICE`.
- [CrimsonDesertCoop](https://github.com/blizz3010/CrimsonDesertCoop) : `docs/REVERSE_ENGINEERING.md`, `include/cdcoop/core/game_structures.h`. Chaînes de position et suggestions de validation ; les indications anciennes doivent être revalidées.
- [CD Companion](https://github.com/andreikapica-dot/CD_Companion) : résolution des coordonnées statiques et translation de l'origine du monde. Leurs mécanismes d'injection et de téléportation ne font pas partie de notre approche.

Stratégie : retrouver les signatures en mémoire, exiger des résolutions non ambiguës, vérifier les structures puis confirmer les changements par les commandes normales. L'accès mémoire est ouvert exclusivement en lecture seule.
