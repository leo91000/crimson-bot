# API de scripting

Chaque script Rhai définit `fn tick(ctx)`. Le moteur appelle cette fonction avec une observation fraîche ; elle renvoie une décision, jamais un appel système.

```rhai
fn tick(ctx) {
    if !ctx.world.player_resolved {
        return #{status: "blocked", reason: "Personnage indisponible"};
    }
    if ctx.elapsed_ms >= 2000 {
        return #{status: "succeeded", reason: "Observation terminée"};
    }
    #{status: "running", memory: #{last_pid: ctx.world.pid}}
}
```

## Entrées

- `ctx.world` : PID, horodatage, personnage résolu, position optionnelle, ressources, inventaire optionnel, capacités et diagnostics.
- `ctx.elapsed_ms` : durée monotone de l'exécution.
- `ctx.memory` : état JSON explicite renvoyé par le tick précédent. Initialement un objet vide.
- `ctx.goal` : paramètres JSON de l'objectif, fournis par `--params FILE`, limités à 64 Kio.

Une valeur optionnelle absente devient `()` dans Rhai. Un inventaire absent est inconnu, pas vide. Les positions candidates doivent être validées par un essai de mouvement avant de servir à la navigation.

## Décisions

- `status` : `running`, `succeeded`, `blocked` ou `failed`.
- `reason` : explication exploitable pour les traces.
- `memory` : état du prochain tick, limité à 64 Kio.
- `action` : facultatif, uniquement pour `running`.

```rhai
#{status: "running", memory: #{phase: "measure"},
  action: #{keys: ["forward"], duration_ms: 150}}
```

Les actions acceptent `keys`, `mouse_x`, `mouse_y`, `duration_ms`. Durée : 1 à 500 ms ; caméra : ±500 en X et ±200 en Y. Le moteur relâche les touches entre deux actions et à l'arrêt. Les noms de commandes sont centralisés dans `control.rs` ; leur correspondance avec les réglages du jeu doit être validée sur machine.

Fonctions : `distance_xz(ax, az, bx, bz)` et `bearing_xz(ax, az, bx, bz)`. L'import de modules, les fichiers, le réseau et le shell ne sont pas accessibles depuis les scripts. Chaque tick possède un budget de 50 000 opérations et une profondeur d'appel limitée.

`find_ui(ctx.world.ui, "Factions")` renvoie le centre d'un mot reconnu, ou `()` si la cible est absente ou ambiguë. Les accents sont normalisés pour les sélecteurs. Le résultat est une proposition de sélection, pas une preuve de réussite.

Une action d'interface peut utiliser `cursor: [x, y]`, `keys: ["click"]`, `ui_mode: "journal"`. Le moteur vérifie le mode observé, le focus et les dimensions actuelles. Le déplacement et le clic d'interface passent ensemble par XTest ; les commandes de jeu passent par uinput. Les touches et boutons sont relâchés sur erreur, arrêt demandé, SIGINT et SIGTERM.

## Exécution sur SteamOS

Après compilation statique sur la machine de développement, copier le binaire et `tasks/` sur la Steam Machine :

```sh
cargo build --release --target x86_64-unknown-linux-musl
# Installer le target au préalable : rustup target add x86_64-unknown-linux-musl
```

Sur la Steam Machine :

```sh
crimson-bot observe
crimson-bot backup
crimson-bot run tasks/observe.rhai --dry-run --seconds 5
crimson-bot run tasks/probe_movement.rhai --seconds 5 --frames
```

Le dernier exemple émet des commandes et exige un jeu en cours, un personnage vivant identifié et le focus du jeu. Une sauvegarde indépendante est créée et vérifiée avant la création du périphérique virtuel.

Les sessions et sauvegardes sont privées dans `~/.local/state/crimson-bot`. Arrêt : Ctrl-C ou création du fichier `~/.local/state/crimson-bot/STOP`. Supprimer ce fichier explicitement avant une nouvelle exécution. Une seule instance par utilisateur peut contrôler le jeu, même avec des répertoires de session différents.

`find_ui_in(ui, text, left, top, right, bottom)` limite la recherche à une région normalisée entre 0 et 1. Les coordonnées sont calculées à partir du centre médian des caractères pour éviter qu'un espace de fin de ligne ne déplace le clic vers une carte voisine. Les scripts vérifient ensuite le titre et le type de page attendu.

`item_type_id(item_key)` résout une clé stable vers l'index utilisé par le processus actuel. `inventory_count(inventory, storage, type_id)` somme les piles dans un stockage explicite. Les index changent entre les builds ; une clé stable ne doit jamais être utilisée directement comme index d'inventaire.

`catalog Lavender` recherche dans les définitions chargées par le jeu. Le catalogue reste dans l'adaptateur Rust, hors du contexte complet envoyé à chaque tick.

`navigation_step(world, memory, goal)` est décrit dans [NAVIGATION.md](NAVIGATION.md). Les coordonnées de l'API sont des f64 ; les données du jeu sont lues en f32 puis converties, pour conserver une interface cohérente avec Rhai.

Les coordonnées des paramètres JSON peuvent être des entiers ou des décimales. Un état de calibration incomplet ou invalide bloque la navigation sans commande.

`launch` demande à Steam d'ouvrir le jeu sans charger une sauvegarde. Le script `resume_latest.rhai` exige `--allow-title` pour pouvoir choisir l'option Continuer, Reprendre ou Jouer du menu principal après sauvegarde de secours ; il ne choisit jamais Nouvelle partie, une sauvegarde arbitraire ou une résolution de conflit cloud. Sans ce drapeau, les commandes exigent toujours un personnage vivant identifié.

`tasks/gather_nearby.rhai` est une capacité de récolte à proximité : elle exige le nom de la ressource et l'indication de prise dans la zone d'interaction, puis confirme une hausse d'inventaire. Elle ne fournit pas encore la navigation jusqu'à une plante. La validation de ce script est actuellement synthétique.

Une exécution à blanc ne simule pas les effets : un script qui attend un mouvement réel peut se bloquer à cette étape. Les tests synthétiques vérifient séparément les transitions du moteur.
