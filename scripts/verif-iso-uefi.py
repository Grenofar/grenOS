#!/usr/bin/env python3
"""L'image démarrera-t-elle sur un PC récent ?

Presque aucune machine vendue depuis dix ans ne démarre encore en BIOS. Une
image sans partition EFI ne s'allumerait tout simplement pas : l'écran du
firmware dirait « aucun support amorçable », et personne ne saurait pourquoi.

Rien ne le vérifiait. La question a été posée pour la première fois le
2026-09-25, sur l'image déjà publiée, et la réponse était heureusement oui —
mais elle tenait à une valeur par défaut de `live-build` que rien ne protège.
Un changement dans `linux/auto/config` pourrait la faire disparaître sans
qu'aucune étape ne rougisse.

Une ISO hybride porte un MBR de compatibilité. Quand `grub-efi` a été inclus,
ce MBR contient une entrée de type **0xEF** — « partition système EFI » — qui
pointe vers l'image FAT que le firmware va chercher. C'est cette entrée qu'on
demande.

    python3 scripts/verif-iso-uefi.py <image.iso>

Sort en 0 si l'image peut démarrer en UEFI, en 1 sinon.
"""
import struct
import sys

TYPE_EFI = 0xEF


def partitions(mbr):
    """Les quatre entrées de la table de partitions d'un MBR."""
    return [mbr[446 + rang * 16: 462 + rang * 16] for rang in range(4)]


def verifier(chemin):
    """(peut démarrer en UEFI ?, ce qu'on a trouvé) pour cette image."""
    with open(chemin, "rb") as image:
        mbr = image.read(512)
    if len(mbr) < 512:
        return False, "l'image fait moins d'un secteur"
    if mbr[510:512] != b"\x55\xaa":
        return False, "aucune signature de MBR : ce n'est pas une image hybride"

    for entree in partitions(mbr):
        if entree[4] == TYPE_EFI:
            debut, taille = struct.unpack("<II", entree[8:16])
            return True, f"partition EFI au secteur {debut}, {taille} secteurs"

    presentes = ", ".join(f"0x{e[4]:02x}" for e in partitions(mbr) if e[4])
    return False, f"aucune partition de type 0xEF (trouvees : {presentes or 'aucune'})"


def main(argv):
    if len(argv) != 2:
        print(__doc__, file=sys.stderr)
        return 64
    try:
        bon, detail = verifier(argv[1])
    except OSError as souci:
        print(f"::error::Impossible de lire l'image : {souci}")
        return 1
    if not bon:
        print(f"::error::L'image ne demarrera pas sur un PC recent : {detail}")
        return 1
    print(f"demarrage UEFI : {detail}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
