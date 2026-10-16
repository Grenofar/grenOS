# grenHub

grenHub est une application qui présente deux moitiés : la partie supérieure lit l’état de la machine, la partie inférieure exécute un diagnostic automatisé.

## État lu (lecture seule)

Les pavés suivants affichent des informations obtenues sans toucher au matériel :

- **La machine** : nom, version, noyau Linux, temps depuis le démarrage, et si la machine est réelle ou virtuelle.  
  Données fournies par `sante.demarrage()`.

- **Processeur et mémoire** : modèle du processeur, nombre de cœurs, mémoire vive totale et libre.  
  Données fournies par `materiel.resume()`.

- **Affichage** : carte graphique et pilote utilisé.  
  Données fournies par `materiel.resume()`.

- **Disques** : pour chaque disque détecté, son nom et une description du type (SSD, amovible, etc.) suivie du modèle si connu.  
  La description du type provient de `materiel.genre_du_disque(d)` où `d` est chaque élément de la liste retournée par `materiel.resume()["disques"]`.

- **Réseau** : pour chaque carte réseau, son nom, son genre (filaire, sans-fil, virtuelle), son état opérationnel et son adresse IPv4 si disponible.  
  Données fournies par `sante.cartes_reseau()`.

- **USB** : jusqu’à huit périphériques USB, chacun avec son nom et la liste de ses pilotes ; s’il y en a plus de huit, une ligne supplémentaire indique le nombre restant.  
  Données fournies par `sante.peripheriques_usb()`.

- **Température et énergie** : jusqu’à six sondes de température (en °C) et jusqu’à quatre ventilateurs (en tr/min) ; si une batterie est présente, son niveau de charge (%) et son état.  
  Les températures et ventilateurs proviennent de `sante.temperatures()` et `sante.ventilateurs()` ; la batterie de `sante.batterie()`.

- **Services** : liste des unités systemd en échec, chacune avec la raison de l’échec fournie par `sante.pourquoi(unite)`.  
  Données fournies par `sante.unites_en_echec()`.

## Diagnostic (moitié inférieure)

Après l’affichage de la fenêtre, un fil de travail exécute la série de tests suivante, fournie par `sante.diagnostic()` :

- **Processeur** : fait travailler chaque cœur pendant un court intervalle et mesure le nombre d’opérations par seconde.  
  Correspond à `sante.essai_processeur()`.

- **Mémoire** : écrit un motif en mémoire (limité à 64 Mio ou au quart de la mémoire libre) et le relit à l’identique.  
  Correspond à `sante.essai_memoire()`.

- **Stockage** : écrit un fichier aléatoire dans le répertoire personnel, le relit, le compare et le supprime.  
  Correspond à `sante.essai_stockage()`.

- **Réseau** : résout un nom DNS puis ouvre une connexion TCP vers le port 80.  
  Correspond à `sante.essai_reseau()`.

- **Carte graphique** : liste les cartes d’affichage détectées et leur pilote.  
  Correspond à `sante.essai_graphique()`.

- **USB** : compte les périphériques USB branchés et signale ceux sans pilote.  
  Correspond à `sante.essai_usb()`.

- **Services** : vérifie si des unités systemd ont échoué au démarrage et, le cas échéant, indique la raison de l’échec pour chacune.  
  Correspond à `sante.essai_services()`.

Chaque test retourne un triplet `(nom, verdict, détail)` où le verdict vaut `"bon"`, `"doute"` ou `"rate"`. Le texte affiché à côté de chaque ligne du diagnostic provient directement du champ `detail` du triplet correspondant.
