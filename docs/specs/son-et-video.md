# Son et vidéo dans grenOS

## Chaîne audio

Dans l'image grenOS, la lecture audio repose sur la pile suivante :

- **pipewire-audio** : serveur audio qui fournit une interface moderne compatible PulseAudio et JACK, gérant le mixage, le routage et le partage des périphériques entre applications.
- **wireplumber** : démon de session et de politique qui configure automatiquement PipeWire, détecte les périphériques et applique les règles de routage (par exemple, rediriger le son vers les écouteurs lorsqu'ils sont branchés).
- **alsa-ucm-conf** : fichiers de configuration Use Case Manager pour ALSA. Sans ces profils, le pilote ALSA ne sait pas comment acheminer le son vers les haut-parleurs sur les cartes Intel SOF ou AMD ACP : le système apparaît fonctionnel mais reste silencieux. Ce paquet est donc essentiel pour que le son sorte réellement sur la plupart des portables récents.
- **alsa-topology-conf** : décrit les topologies des périphériques ALSA (mixers, chemins, widgets) utilisées par les profils UCM.
- **alsa-utils** : fournit les outils en ligne de commande `alsamixer`, `amixer` et `aplay` pour tester et ajuster le niveau sonore directement depuis le terminal.
- **pulseaudio-utils** : comprend `pactl` et `pavucontrol` pour contrôler un serveur PulseAudio ; utile lorsque certaines applications attendent encore cette interface.
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
- **vainfo** : outil en ligne de commande qui liste les profils et points d'entrée VA-API disponibles sur le système ; il permet de vérifier qu'un pilote fonctionne et quels codecs il prend en charge.

### Distinction importante
Le paquet **mesa-vulkan-drivers** concerne le rendu 3D et le calcul GPU via l'API Vulkan. Il ne participe **pas** au décodage vidéo : un système peut avoir des performances 3D élevées mais rester incapable de lire un flux HEVC sans les pilotes VA-API ou VDPAU ci-dessus.

### Que vérifier lorsque la vidéo stutte
1. Lancer `vainfo` et s'assurer qu'au moins un profil d'entrée est listé pour le codec utilisé (ex. : VAProfileH264Main).
2. Vérifier l'utilisation du GPU avec `intel_gpu_top` (Intel) ou `radeontop` (AMD) pendant la lecture : le moteur de décodage doit être actif, pas seulement le moteur 3D.
3. Tester la lecture avec `ffmpeg -hwaccel vaapi -i fichier.mkv -f null -` et observer les messages d'erreur ou de retour.
4. S'assurer que le pilote du noyau (i915, amdgpu, nouveau, etc.) est chargé et sans erreurs dans `dmesg`.
5. Si le logiciel utilise encore VDPAU, confirmer que le pilote NVIDIA propriétaire ou le pilote mesa vdpau est en fonction via `vdpauinfo`.

## En résumé
- Le son dépend de PipeWire et de ses dépendances, avec une attention particulière à **alsa-ucm-conf** pour activer la sortie sur le matériel moderne.
- La vidéo repose sur les pilotes de décodage matériel (VA-API, VDPAU) ; **mesa-vulkan-drivers** n'est pertinent que pour le 3D, pas pour la lecture.
- Les listes de vérification ci-dessus permettent d'isoler rapidement la couche responsable lorsqu'un problème survient.
