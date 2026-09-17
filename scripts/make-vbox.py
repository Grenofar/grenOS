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

Depuis 0.9.0 (D-042), la machine a aussi un disque dur : l'image .vdi de
kernel/scripts/make-disk.sh, sur un contrôleur SATA (AHCI), et elle démarre
dessus. C'est ce disque que grenOS met à jour lui-même ; l'ISO reste dans le
lecteur, en second. VirtualBox refuse un .vdi dont l'UUID d'en-tête n'est pas
celui du .vbox : `vdi` en écrit un connu dans l'image (offset 0x188, ordre
d'octets mixte, block/vdi.c de QEMU).

    python3 scripts/make-vbox.py <nom de l'ISO> <sortie.vbox>
    python3 scripts/make-vbox.py <nom de l'ISO> <sortie.vbox> --disk <image.vdi> <nom du .vdi>
"""

import struct
import sys
import uuid
import xml.etree.ElementTree as ET

SERIAL = r"C:\Users\Public\Documents\grenos-serie.txt"


VDI_SIGNATURE = 0xBEDA107F
VDI_UUID_IMAGE = 0x188
VDI_UUID_MODIFY = 0x198


def stamp_vdi(path: str) -> str:
    """Donne à l'image .vdi un UUID neuf et le renvoie, tel que VirtualBox l'écrit."""
    with open(path, "r+b") as f:
        header = f.read(0x200)
        if len(header) < 0x200 or struct.unpack_from("<I", header, 0x40)[0] != VDI_SIGNATURE:
            raise ValueError(f"{path} is not a VDI image")
        image = uuid.uuid4()
        f.seek(VDI_UUID_IMAGE)
        f.write(image.bytes_le)
        f.seek(VDI_UUID_MODIFY)
        f.write(uuid.uuid4().bytes_le)
    return str(image)


def safe_name(name: str, what: str) -> str:
    if not name or any(ch in name for ch in '<>&"/\\'):
        raise ValueError(f"unexpected {what} name: {name!r}")
    return name


def vbox(iso_name: str, machine: str = "", disc: str = "", disk_name: str = "", disk_uuid: str = "") -> str:
    """Le .vbox d'une machine grenOS : sur `disk_name` quand il est donné, sinon sur `iso_name`."""
    safe_name(iso_name, "ISO")
    if disk_name:
        safe_name(disk_name, "disk")
        uuid.UUID(disk_uuid)
    machine = machine or str(uuid.uuid4())
    disc = disc or str(uuid.uuid4())
    hard_disks = (
        f"""      <HardDisks>
        <HardDisk uuid="{{{disk_uuid}}}" location="{disk_name}" format="VDI" type="Normal"/>
      </HardDisks>
"""
        if disk_name
        else ""
    )
    boot = (
        """        <Order position="1" device="HardDisk"/>
        <Order position="2" device="DVD"/>
        <Order position="3" device="None"/>
        <Order position="4" device="None"/>"""
        if disk_name
        else """        <Order position="1" device="DVD"/>
        <Order position="2" device="None"/>
        <Order position="3" device="None"/>
        <Order position="4" device="None"/>"""
    )
    sata = (
        f"""      <StorageController name="SATA" type="AHCI" PortCount="1" useHostIOCache="false" Bootable="true">
        <AttachedDevice type="HardDisk" hotpluggable="false" port="0" device="0">
          <Image uuid="{{{disk_uuid}}}"/>
        </AttachedDevice>
      </StorageController>
"""
        if disk_name
        else ""
    )
    text = f"""<?xml version="1.0"?>
<VirtualBox xmlns="http://www.virtualbox.org/" version="1.16-windows">
  <Machine uuid="{{{machine}}}" name="grenOS" OSType="Other_64" snapshotFolder="Snapshots">
    <MediaRegistry>
{hard_disks}      <DVDImages>
        <Image uuid="{{{disc}}}" location="{iso_name}"/>
      </DVDImages>
    </MediaRegistry>
    <Hardware>
      <CPU count="1">
        <PAE enabled="true"/>
        <LongMode enabled="true"/>
      </CPU>
      <Memory RAMSize="256"/>
      <!-- Souris et clavier en PS/2, écrit noir sur blanc (D-035). VirtualBox
           donne à plusieurs types d'OS une tablette USB en pointeur, qu'un
           noyau sans pile USB ne voit pas du tout : le pointeur ne bougerait
           jamais, et la fenêtre garderait la souris capturée pour rien. En
           PS/2, le pilote du noyau la reçoit ; Ctrl droite la relâche. -->
      <HID Pointing="PS2Mouse" Keyboard="PS2Keyboard"/>
      <Boot>
{boot}
      </Boot>
      <Display controller="VBoxVGA" VRAMSize="32"/>
      <BIOS>
        <IOAPIC enabled="true"/>
      </BIOS>
      <!-- Une carte réseau que le noyau sait piloter : la 82540EM est la même
           puce que le e1000 de QEMU (8086:100e), donc un seul pilote pour les
           deux. En NAT, VirtualBox fournit DHCP, DNS et la sortie vers
           l'extérieur sans rien demander à l'humain. -->
      <Network>
        <Adapter slot="0" enabled="true" MACAddress="080027A1B2C3" type="82540EM" cable="true">
          <NAT/>
        </Adapter>
      </Network>
      <UART>
        <Port slot="0" enabled="true" IOBase="0x3f8" IRQ="4" hostMode="RawFile" path="{SERIAL}"/>
      </UART>
    </Hardware>
    <StorageControllers>
{sata}      <StorageController name="IDE" type="PIIX4" PortCount="2" useHostIOCache="true" Bootable="true">
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
    if len(argv) == 3:
        with open(argv[2], "w", encoding="utf-8", newline="\n") as f:
            f.write(vbox(argv[1]))
        print(f"{argv[2]} : machine grenOS sur {argv[1]}")
        return 0
    if len(argv) == 6 and argv[3] == "--disk":
        disk_uuid = stamp_vdi(argv[4])
        with open(argv[2], "w", encoding="utf-8", newline="\n") as f:
            f.write(vbox(argv[1], disk_name=argv[5], disk_uuid=disk_uuid))
        print(f"{argv[2]} : machine grenOS sur {argv[5]} ({disk_uuid}), {argv[1]} en second")
        return 0
    print(__doc__, file=sys.stderr)
    return 64


if __name__ == "__main__":
    sys.exit(main(sys.argv))
