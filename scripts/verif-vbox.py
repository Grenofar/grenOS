#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Le `.vbox` livré dit-il des choses que VirtualBox comprend ?

POURQUOI CE CONTRÔLE EXISTE

La CI vérifiait que le fichier est du **XML valide**. Il l'était — et il portait
depuis des semaines, pour la carte son :

    <AudioAdapter controller="HDA" driver="Default" …/>

Lu dans `VirtualBox-settings.xsd` (schéma 1.20, VirtualBox 7.1), type
`TAudioAdapter` : l'attribut `driver` est `use="required"` et n'accepte que

    Null | OSS | ALSA | Pulse | CoreAudio | MMPM | SolAudio | WinMM | DirectSound

**`Default` n'y figure pas.** Il est valide ailleurs dans le même schéma — pour
la priorité de processus, le moteur d'exécution, un `provider` — et c'est
exactement ce qui rendait la faute crédible à la relecture.

*Du XML valide n'est pas du XML conforme à son schéma.* Même famille que
`partition.conf` offrant btrfs sans `mkfs.btrfs`, et que la règle polkit qui
nommait une action inexistante : une promesse à l'écran sans rien derrière.

CE QU'IL NE FAIT PAS

Il ne valide pas tout le schéma — cela demanderait de le télécharger à chaque
construction, donc une dépendance réseau dans une étape qui n'en a pas besoin.
Il vérifie les quelques énumérations que **nous** écrivons, c'est-à-dire
exactement celles que nous pouvons nous tromper. Les valeurs sont recopiées du
schéma, avec sa version, et le jour où elles changent c'est ce fichier qu'on
relit.

    python3 verif-vbox.py machine.vbox

Rend 0 si tout ce qu'on écrit est reconnu, 1 sinon, en nommant l'attribut.
"""
import sys
import xml.etree.ElementTree as ET

# Recopié de VirtualBox-settings.xsd, schéma 1.20 (VirtualBox 7.1).
ATTENDU = {
    "AudioAdapter": {
        "driver": {"Null", "OSS", "ALSA", "Pulse", "CoreAudio",
                   "MMPM", "SolAudio", "WinMM", "DirectSound"},
        "controller": {"AC97", "SB16", "HDA"},
    },
    "Display": {
        "controller": {"VBoxVGA", "VMSVGA", "VBoxSVGA", "None"},
    },
    "HID": {
        "Pointing": {"None", "PS2Mouse", "USBMouse", "USBTablet",
                     "ComboMouse", "USBMultiTouch",
                     "USBMultiTouchScreenPlusPad"},
        "Keyboard": {"None", "PS2Keyboard", "USBKeyboard", "ComboKeyboard"},
    },
}


def main():
    if len(sys.argv) < 2:
        print("verif-vbox : aucun fichier donne, rien a dire")
        return 0
    chemin = sys.argv[1]
    try:
        arbre = ET.parse(chemin)
    except (OSError, ET.ParseError) as souci:
        print("verif-vbox : %s n est pas lisible : %s" % (chemin, souci))
        return 1

    fautes = []
    vus = 0
    for element in arbre.iter():
        # Les noms portent l'espace de noms de VirtualBox : on ne garde que la
        # fin, sans quoi aucune comparaison ne mordrait jamais.
        nom = element.tag.rsplit("}", 1)[-1]
        regles = ATTENDU.get(nom)
        if not regles:
            continue
        for attribut, permis in regles.items():
            valeur = element.get(attribut)
            if valeur is None:
                continue
            vus += 1
            if valeur not in permis:
                fautes.append(
                    "%s/@%s = %r — VirtualBox n accepte que %s"
                    % (nom, attribut, valeur, ", ".join(sorted(permis))))

    for faute in fautes:
        print("verif-vbox : " + faute)
    if fautes:
        print("verif-vbox : le .vbox porte une valeur que VirtualBox refusera")
        return 1
    if vus == 0:
        # Zero attribut regarde n est PAS une bonne nouvelle : cela voudrait
        # dire que le fichier ne declare ni carte son, ni ecran, ni pointeur.
        print("verif-vbox : aucun attribut connu trouve — le .vbox est-il vide ?")
        return 1
    print("verif-vbox : %d valeur(s) verifiee(s), toutes reconnues par VirtualBox"
          % vus)
    return 0


if __name__ == "__main__":
    sys.exit(main())
