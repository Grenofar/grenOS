"use client";

import { useLang } from "@/lib/i18n";

/**
 * La page de téléchargement de grenOS, édition Linux.
 *
 * Une seule chose à télécharger, en deux formes : l'image ISO, et la machine
 * VirtualBox déjà réglée. L'édition à noyau maison a été retirée de cette page
 * le 2026-09-18 à la demande de Grenofar : elle reste dans le dépôt et dans
 * l'historique des Releases, mais elle n'est plus proposée ici, où deux
 * systèmes côte à côte ne faisaient que semer le doute.
 */
export interface LinuxRelease {
  tag: string;
  publishedAt: string;
  isoUrl: string;
  isoSize: number;
  zipUrl?: string;
  zipSize?: number;
  pageUrl: string;
}

const go = (bytes: number) => (bytes / 1024 / 1024 / 1024).toFixed(2);

/*
 * Les textes vivent ici et non dans le dictionnaire partagé : ils sont lus par
 * des gens qui ne sont pas opérateurs, et ils changent avec ce que le système
 * sait faire. Rester honnête sur ce qu'il y a dedans : c'est la première chose
 * que lit quelqu'un qui découvre grenOS.
 */
const FR = {
  title: "Télécharger grenOS",
  lead: "grenOS est un système d'exploitation complet, construit sur Debian, assemblé et vérifié par une équipe d'agents. Chaque image publiée ici a d'abord été construite, puis démarrée pour de vrai, par la CI.",
  published: "publiée le",
  go: "Go",
  isoTitle: "PC, clé USB ou QEMU",
  isoText: "L'image seule : à écrire sur une clé USB pour démarrer un vrai PC, ou à lancer dans une machine virtuelle.",
  isoButton: "Télécharger l'ISO",
  zipTitle: "VirtualBox",
  zipText:
    "La machine déjà réglée, son disque de 25 Go et l'image, dans un seul zip : l'extraire, puis double-cliquer sur le fichier .vbox.",
  zipButton: "Télécharger pour VirtualBox",
  none: "La première image Linux est en cours de construction. Elle arrive.",
  checked: "Construite et démarrée par la CI avant publication, captures d'écran à l'appui.",
  vboxTitle: "Dans VirtualBox",
  vbox: [
    "Télécharger le zip « pour VirtualBox » et l'extraire (clic droit → Extraire tout) : le fichier .vbox, le disque .vdi et l'image .iso sont côte à côte, et doivent le rester.",
    "Double-cliquer sur le fichier .vbox, ou dans VirtualBox : Machine → Ajouter…, puis le choisir. La machine grenOS apparaît, déjà réglée : 64 bits, 8 Go de mémoire, quatre cœurs, 256 Mo de mémoire vidéo, un disque de 25 Go.",
    "La démarrer. Le premier écran demande ton nom, puis le bureau s'ouvre — sans mot de passe — et tout ce que tu fais est gardé sur le disque de la machine.",
    "Il n'y a rien à installer : le disque de la machine garde déjà tes fichiers, tes réglages et tes applications. grenOS ne propose donc pas de s'installer dans une machine virtuelle — l'icône « Installer grenOS » n'apparaît que sur un vrai PC, où elle est utile.",
  ],
  vboxNote:
    "Une machine 64 bits demande la virtualisation matérielle (VT-x ou AMD-V) activée dans le BIOS du PC. Sans le fichier .vbox : nouvelle machine Debian (64-bit), 4 Go de mémoire, un disque de 25 Go, l'image dans le lecteur optique.",
  usbTitle: "Un vrai PC, sur clé USB",
  usb: [
    "Écrire l'image sur une clé avec Rufus (mode image DD), balenaEtcher, ou sous Linux la commande ci-dessous. La clé est entièrement effacée.",
    "Démarrer le PC sur la clé. L'image démarre aussi bien en BIOS qu'en UEFI ; Secure Boot doit être désactivé.",
    "Le bureau s'affiche sans rien installer. L'icône « Installer grenOS » pose le système sur le disque — l'installateur demande avant de toucher à quoi que ce soit.",
  ],
  qemuTitle: "QEMU",
  qemu: "Pour essayer sans rien écrire sur un disque :",
  source: "Code source",
};

const EN: typeof FR = {
  title: "Download grenOS",
  lead: "grenOS is a complete operating system, built on Debian, assembled and checked by a team of agents. Every image published here was first built, then really booted, by CI.",
  published: "published",
  go: "GB",
  isoTitle: "PC, USB stick or QEMU",
  isoText: "The image alone: write it to a USB stick to boot a real PC, or run it in a virtual machine.",
  isoButton: "Download the ISO",
  zipTitle: "VirtualBox",
  zipText:
    "The ready-made machine, its 25 GB disk and the image, in one zip: extract it, then double-click the .vbox file.",
  zipButton: "Download for VirtualBox",
  none: "The first Linux image is being built. It is on its way.",
  checked: "Built and booted by CI before publication, with screenshots to show for it.",
  vboxTitle: "In VirtualBox",
  vbox: [
    "Download the zip “for VirtualBox” and extract it (right-click → Extract All): the .vbox file, the .vdi disk and the .iso image sit side by side, and must stay together.",
    "Double-click the .vbox file, or in VirtualBox: Machine → Add…, and pick it. The grenOS machine appears, already set up: 64-bit, 8 GB of memory, four cores, 256 MB of video memory, a 25 GB disk.",
    "Start it. The first screen asks for your name, then the desktop opens — with no password — and everything you do is kept on the machine's disk.",
    "There is nothing to install: the machine's disk already keeps your files, your settings and your applications. So grenOS does not offer to install itself inside a virtual machine — the “Installer grenOS” icon only appears on a real PC, where it is useful.",
  ],
  vboxNote:
    "A 64-bit machine needs hardware virtualisation (VT-x or AMD-V) enabled in the PC's BIOS. Without the .vbox file: a new Debian (64-bit) machine, 4 GB of memory, a 25 GB disk, the image in the optical drive.",
  usbTitle: "A real PC, from a USB stick",
  usb: [
    "Write the image to a stick with Rufus (DD image mode), balenaEtcher, or the command below on Linux. The stick is wiped.",
    "Boot the PC from the stick. The image boots under BIOS as well as UEFI; Secure Boot must be off.",
    "The desktop appears without installing anything. The “Installer grenOS” icon puts the system on the disk — the installer asks before touching anything.",
  ],
  qemuTitle: "QEMU",
  qemu: "To try it without writing to any disk:",
  source: "Source code",
};

const TEXT = { fr: FR, en: EN };

export function DownloadView({ linux = null, repo }: { linux?: LinuxRelease | null; repo: string }) {
  const { lang } = useLang();
  const c = TEXT[lang];
  const iso = linux ? linux.isoUrl.split("/").pop() : "grenos-linux.iso";

  return (
    <>
      <div className="dl-hero">
        <div className="dl-mark">
          <span className="puce" /> grenOS · Linux
        </div>
        <h1>{c.title}</h1>
        <p>{c.lead}</p>
      </div>

      <section>
        {linux ? (
          <>
            <p className="faint" style={{ marginTop: 0 }}>
              <span className="dot ok" /> <span className="mono">{linux.tag}</span> · {c.published}{" "}
              {linux.publishedAt.slice(0, 10)}
            </p>
            {/* Deux formes de la même image : l'ISO seule, ou la machine toute prête. */}
            <div className="grid cols-2">
              <div className="card">
                <h2>{c.isoTitle}</h2>
                <p className="muted">{c.isoText}</p>
                <a className="dl-button" href={linux.isoUrl}>
                  {c.isoButton}
                </a>
                <div className="faint mono" style={{ marginTop: 8 }}>
                  iso · {go(linux.isoSize)} {c.go}
                </div>
              </div>
              <div className="card">
                <h2>{c.zipTitle}</h2>
                <p className="muted">{c.zipText}</p>
                {linux.zipUrl && (
                  <a className="dl-button" href={linux.zipUrl}>
                    {c.zipButton}
                  </a>
                )}
                <div className="faint mono" style={{ marginTop: 8 }}>
                  zip · {go(linux.zipSize ?? 0)} {c.go}
                </div>
              </div>
            </div>
            <p className="faint" style={{ marginTop: 8 }}>
              {c.checked} <a href={linux.pageUrl}>{linux.tag}</a>
            </p>
          </>
        ) : (
          <div className="card dl-card">
            <p className="muted" style={{ margin: 0 }}>
              {c.none}
            </p>
          </div>
        )}
      </section>

      <section className="grid cols-2">
        <div className="card">
          <h2>{c.vboxTitle}</h2>
          <ol className="dl-steps">
            {c.vbox.map((step) => (
              <li key={step}>{step}</li>
            ))}
          </ol>
          <p className="faint" style={{ marginTop: 10 }}>
            {c.vboxNote}
          </p>
        </div>
        <div className="card">
          <h2>{c.usbTitle}</h2>
          <ol className="dl-steps">
            {c.usb.map((step) => (
              <li key={step}>{step}</li>
            ))}
          </ol>
          <pre className="mono dl-code">sudo dd if={iso} of=/dev/sdX bs=4M status=progress oflag=sync</pre>
        </div>
      </section>

      <section>
        <div className="card">
          <h2>{c.qemuTitle}</h2>
          <p className="muted">{c.qemu}</p>
          <pre className="mono dl-code">qemu-system-x86_64 -m 4096 -smp 2 -cdrom {iso} -boot d</pre>
        </div>
      </section>

      <p className="faint">
        <a href={`https://github.com/${repo}`}>{c.source}</a>
      </p>
    </>
  );
}
