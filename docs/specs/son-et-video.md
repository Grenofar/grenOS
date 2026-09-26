# Son et vidéo dans grenOS

## Chaîne audio

Dans l'image grenOS, la lecture audio repose sur la pile suivante :

- **pipewire-audio** : serveur audio qui fournit une interface moderne compatible PulseAudio et JACK, gérant le mixage, le routage et le partage des périphériques entre applications.
- **wireplumber** : démon de session et de politique qui configure automatiquement PipeWire, détecte les périphériques et applique les règles de routage (par exemple, rediriger le son vers les écouteurs lorsqu'ils sont branchés).
- **alsa-ucm-conf** : fichiers de configuration Use Case Manager pour ALSA. Sans ces profils, le pilote ALSA ne sait pas comment acheminer le son vers les haut-parleurs sur les cartes Intel SOF ou AMD ACP : le système apparaît fonctionnel mais reste silencieux. Ce paquet est donc essentiel pour que le son sorte réellement sur la plupart des portables récents.
- **alsa-topology-conf** : décrit les topologies des périphériques ALSA (mixers, chemins, widgets) utilisées par les profils UCM.
- **alsa-utils** : fournit les outils en ligne de commande `alsamixer`, `amixer` et `aplay` pour tester et ajuster le niveau sonore directement depuis le terminal.
- **pulseaudio-utils** : fournit `pactl`, pour interroger et piloter PipeWire par son interface compatible PulseAudio. `pavucontrol` est un **autre** paquet, également présent, mais volontairement caché du menu : son rôle est tenu par le panneau de son de grenOS.
- **firmware-sof-signed** : firmware signé pour les contrôleurs son Intel Sound Open Firmware (SOF), nécessaire à l'initialisation du DSP sur le matériel moderne.

### Que vérifier lorsqu'il n'y a pas de son
1. Le volume n'est pas muet dans `alsamixer` (canaux Master, Headphone, Speaker).
2. Un périphérique est détecté par `wpctl status` ou `pactl list short sinks`.
3. Le profil actif affiché par `wpctl inspect <id>` correspond bien au matériel (ex. : `HiFi` ou `Pro Audio`).
4. Un test simple avec `aplay /usr/share/sounds/alsa/Front_Center.wav` produit du son.
5. Redémarrer les services : `systemctl --user restart pipewire wireplumber`.

## Chaîne vidéo

Pour l'affichage et la lecture vidéo, grenOS utilise :

- **va-driver-all** : regroupe les pilotes d'accélération vidéo (VA-API) pour Intel, AMD et autres GPU, permettant le décodage matériel des flux H.264, HEVC, VP9, etc.
- **mesa-vdpau-drivers** : implémentation VDPAU (Video Decode and Presentation API for Unix) basée sur Mesa, principalement pour les GPU NVIDIA anciens et certaines cartes AMD.
- **vainfo** et **vdpauinfo** : les deux outils qui listent ce que chaque API sait décoder sur cette machine. Ce sont eux qui répondent à la question « le décodage matériel fonctionne-t-il ? » ; tout le reste est une supposition.
- **libavcodec-extra** : le décodeur lui-même, et le maillon le plus facile à oublier. grenOS n'embarque **aucun lecteur vidéo** : toute vidéo regardée ici est une vidéo web. Or Firefox ne décode pas lui-même sous Linux — il appelle libavcodec, qu'il se contente de *recommander*. grenOS se construisant avec `--apt-recommends false`, il était absent, avec deux conséquences dont la seconde est la pire : pas de H.264, et **pas de décodage matériel du tout**, puisque le chemin VA-API de Firefox passe par ffmpeg. Installer `va-driver-all` sans lui revient à poser un bon moteur sans le relier au réservoir.

### Distinction importante
Le paquet **mesa-vulkan-drivers** concerne le rendu 3D et le calcul GPU via l'API Vulkan. Il ne participe **pas** au décodage vidéo : un système peut avoir des performances 3D élevées mais rester incapable de lire un flux HEVC sans les pilotes VA-API ou VDPAU ci-dessus.

### Que vérifier lorsque la vidéo saccade

Ces commandes existent toutes sur l'image : c'est vérifié à la construction.
Les taper dans l'ordre isole la couche fautive en quelques minutes.

1. **Le décodeur est-il là ?** `ls /usr/lib/x86_64-linux-gnu/libavcodec.so.*`
   doit répondre. Sans lui, rien de ce qui suit ne peut marcher, et le
   navigateur décode tout au processeur — c'est la panne la plus fréquente, et
   la plus silencieuse.
2. **Le matériel répond-il ?** `vainfo` doit lister au moins un profil pour le
   codec en cause (`VAProfileH264Main` pour le H.264, `VAProfileVP9Profile0`
   pour YouTube récent). `vdpauinfo` fait de même pour l'autre API.
3. **Le noyau a-t-il chargé son pilote ?** `dmesg | grep -iE 'i915|amdgpu|nouveau'`
   doit montrer une carte initialisée, et aucune erreur de micrologiciel.
4. **Firefox s'en sert-il vraiment ?** Ouvrir `about:support` et lire la
   section « Décodage vidéo accéléré matériellement ». C'est le seul endroit
   qui dise ce que fait le navigateur, plutôt que ce dont la machine serait
   capable.
5. **Lire la page Son des Réglages**, qui affiche les mêmes réponses sans
   terminal.

Ce qui n'est **pas** en cause : le débit. Une vidéo qui saccade à 200 Mbit/s
dans les deux sens ne saccade pas pour une raison de réseau. Le symptôme a été
signalé ainsi, et la cause était entièrement locale.

## En résumé
- Le son dépend de PipeWire et de ses dépendances, avec une attention particulière à **alsa-ucm-conf** pour activer la sortie sur le matériel moderne.
- La vidéo demande **deux** choses, et oublier l'une annule l'autre : un décodeur (`libavcodec`) et des pilotes matériels (VA-API, VDPAU). **mesa-vulkan-drivers** ne fait ni l'un ni l'autre : c'est de la 3D.
- Les listes de vérification ci-dessus permettent d'isoler rapidement la couche responsable lorsqu'un problème survient.
