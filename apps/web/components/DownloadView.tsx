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
  nowTitle: "Ce qu'il y a dedans",
  quoi: [
    {
      titre: "Un bureau écrit pour grenOS",
      texte:
        "Notre barre, notre menu, nos Réglages, notre thème et notre fond d'écran. Rien d'emprunté à un autre système.",
    },
    {
      titre: "Fluide sans carte graphique",
      texte:
        "L'écran n'est pas composé par le GPU : le pointeur suit la main, même dans une machine virtuelle sans accélération 3D.",
    },
    {
      titre: "Ton nom dès le premier écran",
      texte: "Il devient ton compte et ton adresse sur la machine : entrer « lea » donne lea@grenos.",
    },
    {
      titre: "Un bureau où l'on pose des choses",
      texte:
        "Fichiers, dossiers et raccourcis restent où tu les laisses. Clic droit pour créer, renommer, changer le fond.",
    },
    {
      titre: "Les applications d'un système complet",
      texte:
        "Firefox et ses onglets, explorateur de fichiers, éditeur de texte, terminal, images, archives, capture d'écran, calculatrice.",
    },
    {
      titre: "GrenPlace",
      texte:
        "Les applications s'installent en un clic, paquets Debian comme Flatpak — Steam compris — dans une fenêtre, jamais dans un terminal.",
    },
    {
      titre: "Des Réglages qui règlent vraiment",
      texte:
        "Jour et nuit, définition de l'écran et images par seconde, sortie audio et volume, mot de passe, mises à jour.",
    },
    {
      titre: "Les raccourcis que tu connais",
      texte:
        "Alt+Tab change de fenêtre, Alt+F4 ferme, Ctrl+Maj+Échap ouvre les tâches, Meta+T bascule le jour et la nuit.",
    },
    {
      titre: "Rien à installer pour commencer",
      texte:
        "La machine livrée avec l'image garde déjà tout sur son disque : fichiers, réglages, applications, mot de passe.",
    },
    {
      titre: "Les mises à jour depuis l'OS",
      texte:
        "Nos nouveautés et les correctifs de sécurité Debian arrivent ensemble, sans jamais retélécharger l'image.",
    },
  ],
  vboxTitle: "Dans VirtualBox",
  vbox: [
    "Télécharger le zip « pour VirtualBox » et l'extraire (clic droit → Extraire tout) : le fichier .vbox, le disque .vdi et l'image .iso sont côte à côte, et doivent le rester.",
    "Double-cliquer sur le fichier .vbox, ou dans VirtualBox : Machine → Ajouter…, puis le choisir. La machine grenOS apparaît, déjà réglée : 64 bits, 4 Go de mémoire, deux cœurs, 128 Mo de mémoire vidéo, un disque de 25 Go.",
    "La démarrer. Le premier écran demande ton nom, puis le bureau s'ouvre — sans mot de passe — et tout ce que tu fais est gardé sur le disque de la machine.",
    "Pour garder ses fichiers et ses comptes : l'icône « Installer grenOS » sur le bureau. L'installation prend une dizaine de minutes, puis la machine démarre sur son disque.",
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
  nowTitle: "What is inside",
  quoi: [
    {
      titre: "A desktop written for grenOS",
      texte:
        "Our own panel, menu, Settings, theme and wallpaper. Nothing borrowed from another system.",
    },
    {
      titre: "Smooth without a graphics card",
      texte:
        "The screen is not composed on the GPU: the pointer follows your hand, even in a virtual machine with no 3D acceleration.",
    },
    {
      titre: "Your name on the very first screen",
      texte:
        "It becomes your account and your address on the machine: typing “lea” gives lea@grenos.",
    },
    {
      titre: "A desktop you can put things on",
      texte:
        "Files, folders and shortcuts stay where you leave them. Right-click to create, rename, change the wallpaper.",
    },
    {
      titre: "The applications of a complete system",
      texte:
        "Firefox with its tabs, a file manager, a text editor, a terminal, images, archives, a screenshot tool, a calculator.",
    },
    {
      titre: "GrenPlace",
      texte:
        "Applications install in one click, Debian packages as well as Flatpaks — Steam included — in a window, never in a terminal.",
    },
    {
      titre: "Settings that actually settle things",
      texte:
        "Day and night, screen resolution and refresh rate, audio output and volume, password, updates.",
    },
    {
      titre: "The shortcuts you already know",
      texte:
        "Alt+Tab switches windows, Alt+F4 closes, Ctrl+Shift+Esc opens the task monitor, Meta+T switches day and night.",
    },
    {
      titre: "Nothing to install to begin with",
      texte:
        "The machine shipped with the image already keeps everything on its disk: files, settings, applications, password.",
    },
    {
      titre: "Updates from inside the OS",
      texte:
        "Our own changes and Debian's security fixes arrive together, without ever downloading the image again.",
    },
  ],
  vboxTitle: "In VirtualBox",
  vbox: [
    "Download the zip “for VirtualBox” and extract it (right-click → Extract All): the .vbox file, the .vdi disk and the .iso image sit side by side, and must stay together.",
    "Double-click the .vbox file, or in VirtualBox: Machine → Add…, and pick it. The grenOS machine appears, already set up: 64-bit, 4 GB of memory, two cores, 128 MB of video memory, a 25 GB disk.",
    "Start it. The first screen asks for your name, then the desktop opens — with no password — and everything you do is kept on the machine's disk.",
    "To keep your files and accounts: the “Installer grenOS” icon on the desktop. Installing takes about ten minutes, then the machine boots from its disk.",
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

      <section>
        <h2>{c.nowTitle}</h2>
        {/* Une grille de points courts : un pavé de deux cents mots, personne
            ne le lit, et c'est pourtant ce que regarde quelqu'un qui hésite. */}
        <div className="dl-quoi">
          {c.quoi.map((point) => (
            <div className="card" key={point.titre}>
              <b>{point.titre}</b>
              <span>{point.texte}</span>
            </div>
          ))}
        </div>
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
