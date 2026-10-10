# Lecture de l'interface en local

La lecture mémoire est complétée par un OCR local quand une information n'est disponible que dans l'interface. Le moteur capture lui-même l'image et transmet les lignes reconnues au script. Les captures restent privées.

L'OCR utilise [ocrs](https://github.com/robertknight/ocrs) et RTen, exécutés sur CPU. Il n'y a aucun appel réseau pendant l'inférence, aucune API payante et aucun modèle de langage dans la boucle. Le modèle reconnaît l'alphabet latin ; les accents français peuvent être mal reconnus. Une observation ambiguë ne doit pas être utilisée pour valider une action irréversible.

## Installation des modèles

Télécharger les modèles fournis par le projet ocrs dans un répertoire privé, hors du dépôt :

- `text-detection.onnx` : https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx
- `text-recognition.onnx` : https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx

Conserver leurs empreintes SHA-256 avec le déploiement. `read-screen FRAME --models DIRECTORY` analyse une image existante ; `run TASK --models DIRECTORY` produit ses propres observations de l'écran à chaque tick.

Empreintes du déploiement initial :

- Détection : `a917b23dbd9524b465df7e922641b3ff2981623df4ded5a0234004ef2fee7cfe`
- Reconnaissance : `86c145c2edb96c8caed5b1ebb8f44d706408922211451c309c625157dd6061c5`

Limiter les threads CPU avec `RTEN_NUM_THREADS=2` sur la Steam Machine. Les commandes sont relâchées pendant les captures et l'OCR. La mémoire est relue après cette analyse pour éviter d'envoyer des commandes à partir de coordonnées périmées.

Le classement de l'interface est provisoire (`world`, `pause`, `journal`, `unknown`). Les scripts doivent exiger les indices correspondant à leur action ; ce classement n'est pas une preuve suffisante pour accepter un dialogue ou prendre un objet.
