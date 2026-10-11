# API de scripting

Chaque script Rhai définit `fn tick(ctx)` et renvoie une décision à partir d'une observation mémoire fraîche. Le runtime n'utilise aucun OCR, modèle de vision ou service d'IA.

## Contexte

- `ctx.world` : PID, position, santé, inventaire, scènes de collecte, interface et interactions natives optionnelles.
- `ctx.elapsed_ms` : durée active monotone de l'objectif.
- `ctx.memory` : état JSON explicite du tick précédent, initialement vide.
- `ctx.goal` : objet JSON fourni par `--params FILE`, limité à 64 Kio.

Une information absente devient `()` dans Rhai. `world.ui == ()` signifie que le mode d'interface n'a pas été établi par le fournisseur mémoire. `world.interactions == ()` signifie que les interactions sont inconnues ; une liste vide indique une absence confirmée. Les fournisseurs d'interface et d'interaction sont encore à résoudre sur le build installé.

## Décisions et commandes

Statuts : `running`, `succeeded`, `blocked`, `failed`. Une décision terminale ne peut pas contenir de commande. `memory` est limitée à 64 Kio ; chaque tick dispose de 50 000 opérations et d'une profondeur d'appel limitée. Fichiers, réseau, shell et imports sont inaccessibles depuis Rhai.

```rhai
fn tick(ctx) {
    if ctx.world.ui == () || ctx.world.ui.mode != "world" {
        return #{status:"blocked", reason:"Mode de jeu natif requis"};
    }
    #{status:"running", memory:ctx.memory,
      action:#{keys:["forward"], ui_mode:"world", duration_ms:100}}
}
```

Le runtime contrôle mode natif, personnage vivant, focus et fraîcheur avant toute commande. Une action sans `ui_mode` exige aussi le mode `world`. Les touches sont relâchées entre deux actions et sur interruption ou erreur. `duration_ms` est bornée à 1–500 ms ; `mouse_x` à ±500 et `mouse_y` à ±200.

Une interaction exige `keys:["interact"]` et `interaction_target:"ID"`. Cet identifiant doit correspondre à une unique interaction native actuellement disponible. Aucune autre touche, caméra ou action souris ne peut être combinée avec la prise.

Les interactions natives exposent `target_id`, `kind` (`pickup`, `talk`, `turn_in`), `item_key` optionnelle et `available`. Pour une plante, le fournisseur devra relier la cible d'interaction à l'identifiant de la scène sélectionnée. Ce contrat est testé sur observations synthétiques ; le fournisseur n'est pas encore validé en jeu.

Les actions de menu utilisent un mode natif vérifié. `cursor:[x,y]` est borné aux dimensions natives ; `keys:["click"]` sélectionne une entrée. `wheel_y` accepte au plus trois crans dans `journal` ou `settings`, avec un curseur et aucune autre commande. Aucun script de lecture de journal par images n'est distribué.

## Fonctions et capacités

- `item_type_id(item_key)` résout une clé stable vers l'index du catalogue courant.
- `inventory_count(items, storage, type_id)` somme les piles du stockage demandé.
- `distance_xz` et `bearing_xz` utilisent les positions typées.
- `navigation_step` et `navigation_retarget` sont décrits dans [NAVIGATION.md](NAVIGATION.md).

`gather_nearby.rhai` reçoit `item_key`, `storage`, `amount` et exige une interaction native unique. Il confirme un gain réel dans le bon inventaire.

`route_gather.rhai` ajoute `prefab_suffix`, choisit une scène proche et compose des segments bornés. Le résultat est confirmé par l'inventaire ; une scène seule ne prouve pas sa disponibilité. Les cibles consommées ou indisponibles sont exclues. Origine, santé, segments, nombre de cibles et tentatives de prise bornent l'exécution. La validation historique utilisait un fournisseur d'interface désormais retiré ; une nouvelle validation native est nécessaire.

## Exécution sur SteamOS

```sh
cargo build --locked --release --target x86_64-unknown-linux-musl
crimson-bot observe
crimson-bot catalog Peony
crimson-bot resources peony
crimson-bot run tasks/observe.rhai --dry-run --seconds 5
```

`--movement-layout wasd|zqsd` adapte le clavier virtuel sans changer les réglages du jeu. ZQSD a été mesuré sur la machine de développement. Les commandes de déplacement restent bloquées jusqu'à la disponibilité d'un mode natif vérifié.

`--frames` conserve uniquement des images de diagnostic ; aucune image n'entre dans la décision du script. `capture` produit aussi une image de diagnostic sans commande de jeu. `launch` demande à Steam d'ouvrir le jeu sans charger une sauvegarde. Les options de modèles et la commande de lecture d'écran ont été supprimées.

Avant chaque exécution avec commandes, une copie indépendante des sauvegardes est vérifiée. Les sessions sont privées dans `~/.local/state/crimson-bot`. Arrêt : Ctrl-C ou création du fichier `STOP` dans ce répertoire. Une seule instance peut contrôler le jeu.

## Reprise

Un script doté de `fn resume(memory)` obtient `runs/<session>/checkpoint.json`, écrit après relâchement des commandes. `--resume FILE` exige le même script, objectif et profil de touches. Le moteur compare démarrage Linux, PID et création du processus, empreinte du jeu, identifiant opaque du corps, santé, inventaire et position. Une exécution réussie, échouée ou en erreur ne se reprend pas.

La récolte conserve les exclusions et les compteurs, puis efface sa calibration de déplacement. Aucune ancienne commande n'est rejouée ; les budgets continuent depuis l'objectif initial. Une nouvelle copie des sauvegardes précède chaque reprise. Le checkpoint est un état du script, jamais une sauvegarde restaurée dans le jeu.
