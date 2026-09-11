#!/usr/bin/env python3
"""La machine VirtualBox de grenOS, publiée à côté de l'ISO (D-033).

VirtualBox ouvre un fichier .vbox (double-clic, ou Machine → Ajouter…) et
l'enregistre tel quel : 64 bits « Other », 256 Mo, l'ISO dans le lecteur
optique, COM1 écrit dans un fichier. Le chemin de l'ISO est relatif, et
VirtualBox résout les médias depuis le dossier du .vbox : il suffit de garder
les deux fichiers ensemble. Celui du port série doit être absolu, VirtualBox
l'ouvrant tel quel : C:\\Users\\Public\\Documents existe, et s'écrit, sur tout
Windows. L'humain a VirtualBox sur son PC principal, sous Windows.

Format des réglages 1.16 (VirtualBox 6.0 et 6.1), que VirtualBox 7 lit et met
à jour à l'enregistrement.

    python3 scripts/make-vbox.py <nom de l'ISO> <sortie.vbox>
"""

import sys
import uuid
import xml.etree.ElementTree as ET

SERIAL = r"C:\Users\Public\Documents\grenos-serie.txt"


def vbox(iso_name: str, machine: str = "", disc: str = "") -> str:
    """Le .vbox d'une machine grenOS qui démarre sur `iso_name`."""
    if not iso_name or any(ch in iso_name for ch in '<>&"/\\'):
        raise ValueError(f"unexpected ISO name: {iso_name!r}")
    machine = machine or str(uuid.uuid4())
    disc = disc or str(uuid.uuid4())
    text = f"""<?xml version="1.0"?>
<VirtualBox xmlns="http://www.virtualbox.org/" version="1.16-windows">
  <Machine uuid="{{{machine}}}" name="grenOS" OSType="Other_64" snapshotFolder="Snapshots">
    <MediaRegistry>
      <DVDImages>
        <Image uuid="{{{disc}}}" location="{iso_name}"/>
      </DVDImages>
    </MediaRegistry>
    <Hardware>
      <CPU count="1">
        <PAE enabled="true"/>
        <LongMode enabled="true"/>
      </CPU>
      <Memory RAMSize="256"/>
      <Boot>
        <Order position="1" device="DVD"/>
        <Order position="2" device="None"/>
        <Order position="3" device="None"/>
        <Order position="4" device="None"/>
      </Boot>
      <Display controller="VBoxVGA" VRAMSize="32"/>
      <BIOS>
        <IOAPIC enabled="true"/>
      </BIOS>
      <UART>
        <Port slot="0" enabled="true" IOBase="0x3f8" IRQ="4" hostMode="RawFile" path="{SERIAL}"/>
      </UART>
    </Hardware>
    <StorageControllers>
      <StorageController name="IDE" type="PIIX4" PortCount="2" useHostIOCache="true" Bootable="true">
        <AttachedDevice passthrough="false" type="DVD" hotpluggable="false" port="1" device="0">
          <Image uuid="{{{disc}}}"/>
        </AttachedDevice>
      </StorageController>
    </StorageControllers>
  </Machine>
</VirtualBox>
"""
    ET.fromstring(text.encode("utf-8"))  # bien formé, ou une exception ici plutôt que chez l'humain
    return text


def main(argv) -> int:
    if len(argv) != 3:
        print(__doc__, file=sys.stderr)
        return 64
    with open(argv[2], "w", encoding="utf-8", newline="\n") as f:
        f.write(vbox(argv[1]))
    print(f"{argv[2]} : machine grenOS sur {argv[1]}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
