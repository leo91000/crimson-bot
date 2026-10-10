# Navigation locale

`navigation_step(world, memory, goal)` produit une décision de déplacement à partir de coordonnées observées. `tasks/navigate_local.rhai` l'expose directement.

Paramètres : `x`, `z`, `radius` (0,25 par défaut), `max_distance` (5 unités), `max_height_change` (0,5 unité).

Le contrôleur mesure un court recul pour apprendre l'axe avant/arrière. Si le point n'est pas sur cet axe, il mesure un déplacement latéral. Il choisit ensuite une commande à partir de la projection du déplacement souhaité sur les axes mesurés et actualise ceux-ci à partir des déplacements réellement observés.

Il exige une interface de jeu reconnue et un personnage identifié, borne le déplacement et la variation d'altitude, puis bloque sur calibration sans mouvement ou tentatives répétées sans progrès. Une arrivée est confirmée par la distance mesurée, jamais par la durée de touche.

**Limite actuelle :** contrôleur de petits déplacements dans une zone connue. Il ne connaît pas la géométrie, les routes du monde, les falaises ni les obstacles avant contact. Les tests synthétiques couvrent arrivée, calibration bloquée, point trop éloigné et changement d'altitude. La validation sur machine reste à réaliser.

Pour la navigation dans le monde complexe, il faudra ajouter un fournisseur de géométrie/chemins et des stratégies de récupération validées. Le pilotage local reste une couche réutilisable sous ce futur planificateur.
