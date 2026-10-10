# Crimson Bot

Un moteur de scripting local pour automatiser les tâches de Crimson Desert sur SteamOS/Proton. Rust porte les interfaces système et les contrôles ; Rhai décrit les objectifs et les politiques. Aucun service d'IA n'est nécessaire pendant l'exécution.

**En développement :** la récolte, la navigation dans le monde et le combat ne sont pas encore validés. Les étapes réellement vérifiées sont consignées dans [VALIDATION.md](docs/VALIDATION.md).

## Objectif

L'opérateur donne un objectif (« faire les quêtes X, Y, Z »). Le développeur étudie les quêtes, écrit leurs scripts, surveille les essais et améliore les capacités communes à partir des échecs. Le programme assure sa propre boucle perception–décision–action. Il doit fonctionner sans qu'un humain interprète chaque capture ou envoie chaque commande.

La première cible est une quête secondaire de récolte à faible conséquence, choisie à partir de la partie disponible. L'objectif est d'obtenir une exécution complète et répétable avant d'élargir les capacités.

## Principes

- Lire l'état du jeu sans modifier sa mémoire, ses règles ou ses sauvegardes.
- Agir par les commandes ordinaires, avec un clavier/souris virtuel Linux.
- Séparer l'adaptateur du jeu, les actions, la navigation, le combat et les objectifs.
- Vérifier les résultats ; une touche envoyée ne prouve pas une action réussie.
- Arrêter sur état inconnu, perte de contrôle, échec répété ou décision irréversible.
- Préserver une copie indépendante des sauvegardes avant les essais.
- Exécuter entièrement en local, sans API d'IA payante.

Voir le [cahier des charges](docs/REQUIREMENTS.md), l'[architecture](docs/ARCHITECTURE.md) et les [sources du reverse engineering](docs/RESEARCH.md).

Le [guide de scripting](docs/SCRIPTING.md) détaille les scripts Rhai, les commandes et l'installation sur SteamOS.

## Développement

```sh
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
```

Le dépôt public contient le code, les scripts et les cas synthétiques. Sauvegardes, captures, journaux de partie et extraits mémoire sont exclus.
