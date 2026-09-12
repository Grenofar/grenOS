"use client";

import { useLang, type Lang } from "@/lib/i18n";

/** A published image, as index.json in the Supabase "releases" bucket lists it. */
export interface Build {
  tag: string;
  publishedAt: string;
  size: number;
  url: string;
  vboxUrl?: string;
  screenUrl?: string;
  virtualboxUrl?: string;
  virtualboxSize?: number;
}

const mb = (bytes: number) => (bytes / 1024 / 1024).toFixed(1);

/*
 * The page's copy lives here rather than in the shared dictionary: it is read
 * by people who are not operators, and it changes with what the OS can do.
 * Keep "now" honest — it is the first thing someone booting grenOS reads.
 */
const FR = {
  title: "Télécharger grenOS",
  lead: "grenOS est un système d'exploitation x86_64 écrit en Rust par une équipe d'agents d'IA. Chaque image publiée ici a d'abord été construite, puis démarrée dans QEMU par la CI.",
  pcTitle: "PC, clé USB ou QEMU",
  pcText: "L'image ISO seule : à écrire sur une clé USB pour démarrer un vrai PC, ou à lancer dans QEMU.",
  pcButton: "Télécharger l'ISO",
  vboxText: "L'ISO et sa machine VirtualBox déjà réglée, dans un seul zip : l'extraire, puis double-cliquer sur le fichier .vbox.",
  vboxZipButton: "Télécharger pour VirtualBox",
  vboxOnly: "ou seulement le fichier .vbox",
  preview: "Ce que la CI a vu à l'écran, 25 secondes après le démarrage, dans QEMU :",
  previewAlt: "Capture d'écran de grenOS dans QEMU",
  mb: "Mo",
  build: "Version",
  published: "publiée le",
  all: "Versions précédentes",
  none: "La première image est en cours de publication. Reviens dans quelques minutes.",
  checked: "Vérifié automatiquement dans QEMU. VirtualBox et les vrais PC ne sont pas testés par la CI.",
  nowTitle: "Ce qu'il fait aujourd'hui",
  now: "Il démarre sans menu, directement sur un bureau sombre inspiré de Kali Linux : panneau en haut, menu d'applications avec recherche, fenêtres à ombre portée qu'on déplace, réduit et ferme, et des animations mesurées en millisecondes (donc identiques quel que soit le nombre d'images par seconde). Six applications : Terminal (vraies commandes : ls, cat, écrire, lspci, dmesg, grenfetch), Fichiers (un système de fichiers en mémoire), Navigateur (les pages du système et ses fichiers ; le web attend le pilote réseau), Bloc-notes qui enregistre pour de vrai, Paramètres (système, souris, écran, matériel, réseau, compte, mise à jour) et À propos. Un écran de connexion s'affiche dès qu'un mot de passe est défini dans Paramètres. Éteindre et Redémarrer passent par l'ACPI, le clavier est en AZERTY, F1 ouvre le menu. Et il a le réseau : pilote de carte Intel 8254x, DHCP, DNS, ping et TCP — le navigateur ouvre les pages en http (https attend TLS). Une application Sécurité montre ce qui est réellement en place : NX, écriture du code interdite, SMEP, empreinte du code du noyau vérifiable d'un clic, et une analyse des fichiers qui met en quarantaine ce qu'elle reconnaît.",
  vboxTitle: "VirtualBox",
  vbox: [
    "Télécharger le zip « pour VirtualBox » et l'extraire (clic droit → Extraire tout) : l'ISO et le fichier .vbox sont côte à côte.",
    "Double-cliquer sur le fichier .vbox, ou dans VirtualBox : Machine → Ajouter…, puis le choisir. La machine grenOS apparaît, déjà réglée : 64 bits, 256 Mo, l'ISO dans le lecteur optique.",
    "La démarrer : le bureau s'affiche tout de suite, sans menu de démarrage. Cliquer dans la fenêtre de la machine pour que VirtualBox lui donne la souris ; la touche Ctrl de droite la reprend. Si le pointeur ne bouge pas, ouvrir Paramètres → Souris et clavier : les compteurs disent si les octets arrivent.",
    "Ce qu'il écrit sur le port série est dans C:\\Users\\Public\\Documents\\grenos-serie.txt : grenOS, la mémoire, la pagination, le tas, les périphériques PCI, l'ACPI, puis desktop: drawn.",
  ],
  vboxNote: "Une machine 64 bits demande la virtualisation matérielle (VT-x ou AMD-V) activée dans le BIOS du PC. Sans le fichier .vbox : nouvelle machine Other/Unknown (64-bit), 256 Mo, sans disque dur, l'ISO dans le lecteur optique, port série 1 en « Fichier brut ».",
  qemuTitle: "QEMU",
  qemu: "La sortie série s'affiche dans le terminal :",
  usbTitle: "Un vrai PC, sur clé USB",
  usb: [
    "Écrire l'image sur une clé avec Rufus (mode image DD), balenaEtcher, ou sous Linux la commande ci-dessous. La clé est entièrement effacée.",
    "Démarrer le PC sur la clé, en mode Legacy / BIOS (CSM).",
    "Le bureau s'affiche directement. Souris et clavier ne répondent que s'ils sont PS/2, ou si le BIOS émule le PS/2 pour l'USB : le clavier USB viendra avec le pilote xHCI.",
  ],
  source: "Code source",
};

const EN: typeof FR = {
  title: "Download grenOS",
  lead: "grenOS is an x86_64 operating system written in Rust by a team of AI agents. Every image published here was first built, then booted in QEMU, by CI.",
  pcTitle: "PC, USB stick or QEMU",
  pcText: "The ISO image alone: write it to a USB stick to boot a real PC, or run it in QEMU.",
  pcButton: "Download the ISO",
  vboxText: "The ISO and its ready-made VirtualBox machine, in one zip: extract it, then double-click the .vbox file.",
  vboxZipButton: "Download for VirtualBox",
  vboxOnly: "or just the .vbox file",
  preview: "What CI saw on screen, 25 seconds after boot, in QEMU:",
  previewAlt: "Screenshot of grenOS in QEMU",
  mb: "MB",
  build: "Build",
  published: "published",
  all: "Earlier builds",
  none: "The first image is being published. Come back in a few minutes.",
  checked: "Checked automatically in QEMU. VirtualBox and real PCs are not tested by CI.",
  nowTitle: "What it does today",
  now: "It boots with no menu, straight into a dark desktop in the spirit of Kali Linux: a top panel, an application menu with search, windows with a shadow that you drag, minimise and close, and animations measured in milliseconds, so they look the same at any frame rate. Six applications: Terminal (real commands: ls, cat, écrire, lspci, dmesg, grenfetch), Files (a file system in memory), Browser (the system's own pages and files; the web waits for the network driver), a Notepad that really saves, Settings (system, mouse, screen, hardware, network, account, update) and About. A login screen appears as soon as a password is set in Settings. Shutdown and restart go through ACPI, the keyboard is French AZERTY, and F1 opens the menu. And it has the network: an Intel 8254x driver, DHCP, DNS, ping and TCP — the browser opens http pages for real (https waits for TLS). A Security application shows what is actually enforced: NX, write-protected kernel code, SMEP, a fingerprint of the kernel's own code you can re-check with one click, and a file scan that quarantines what it recognises.",
  vboxTitle: "VirtualBox",
  vbox: [
    "Download the zip “for VirtualBox” and extract it (right-click → Extract All): the ISO and the .vbox file sit side by side.",
    "Double-click the .vbox file, or in VirtualBox: Machine → Add…, and pick it. The grenOS machine appears, already set up: 64-bit, 256 MB, the ISO in the optical drive.",
    "Start it: the desktop appears at once, with no boot menu. Click inside the machine's window so VirtualBox hands it the mouse; the right Ctrl key takes it back. If the pointer will not move, open Settings → Mouse and keyboard: the counters say whether the bytes arrive.",
    "What it writes to the serial port is in C:\\Users\\Public\\Documents\\grenos-serie.txt: grenOS, the memory, paging, the heap, the PCI devices, ACPI, then desktop: drawn.",
  ],
  vboxNote: "A 64-bit machine needs hardware virtualisation (VT-x or AMD-V) enabled in the host PC's firmware. Without the .vbox file: new machine Other/Unknown (64-bit), 256 MB, no hard disk, the ISO in the optical drive, serial port 1 in Raw File mode.",
  qemuTitle: "QEMU",
  qemu: "The serial output appears in the terminal:",
  usbTitle: "A real PC, from a USB stick",
  usb: [
    "Write the image to a stick with Rufus (DD image mode), balenaEtcher, or on Linux the command below. The stick is erased.",
    "Boot the PC from the stick in Legacy / BIOS (CSM) mode.",
    "The desktop appears straight away. The mouse and the keyboard only answer if they are PS/2, or if the firmware emulates PS/2 for USB ones: USB keyboards wait for the xHCI driver.",
  ],
  source: "Source code",
};

const TEXT: Record<Lang, typeof FR> = { fr: FR, en: EN };

export function DownloadView({ builds, repo }: { builds: Build[]; repo: string }) {
  const { lang } = useLang();
  const c = TEXT[lang];
  const build = builds[0] ?? null;
  const earlier = builds.slice(1);

  return (
    <>
      <div className="dl-hero">
        <h1>{c.title}</h1>
        <p>{c.lead}</p>
      </div>

      <section>
        {build ? (
          <>
            <p className="faint" style={{ marginTop: 0 }}>
              <span className="dot ok" /> {c.build} <span className="mono">{build.tag}</span> · {c.published}{" "}
              {build.publishedAt.slice(0, 10)}
            </p>
            {/* Two choices, each enough on its own: the .vbox needs its ISO beside it. */}
            <div className="grid cols-2">
              <div className="card">
                <h2>{c.pcTitle}</h2>
                <p className="muted">{c.pcText}</p>
                <a className="dl-button" href={build.url}>
                  {c.pcButton}
                </a>
                <div className="faint mono" style={{ marginTop: 8 }}>
                  grenos.iso · {mb(build.size)} {c.mb}
                </div>
              </div>
              <div className="card">
                <h2>{c.vboxTitle}</h2>
                <p className="muted">{c.vboxText}</p>
                {build.virtualboxUrl ? (
                  <a className="dl-button" href={build.virtualboxUrl}>
                    {c.vboxZipButton}
                  </a>
                ) : (
                  build.vboxUrl && (
                    <a className="dl-button" href={build.vboxUrl}>
                      {c.vboxZipButton}
                    </a>
                  )
                )}
                <div className="faint" style={{ marginTop: 8 }}>
                  {build.virtualboxSize ? (
                    <span className="mono">
                      zip · {mb(build.virtualboxSize)} {c.mb}
                    </span>
                  ) : null}
                  {build.virtualboxUrl && build.vboxUrl && (
                    <>
                      {" "}
                      · <a href={build.vboxUrl}>{c.vboxOnly}</a>
                    </>
                  )}
                </div>
              </div>
            </div>
          </>
        ) : (
          <div className="card dl-card">
            <p className="muted" style={{ margin: 0 }}>
              {c.none}
            </p>
          </div>
        )}
        <p className="faint" style={{ marginTop: 8 }}>
          {c.checked}
        </p>
        {build?.screenUrl && (
          <figure style={{ margin: "14px 0 0" }}>
            <figcaption className="faint" style={{ marginBottom: 6 }}>
              {c.preview}
            </figcaption>
            <img
              src={build.screenUrl}
              alt={c.previewAlt}
              style={{ maxWidth: "100%", borderRadius: 8, border: "1px solid rgba(127,127,127,.3)" }}
            />
          </figure>
        )}
        {earlier.length > 0 && (
          <details className="faint" style={{ marginTop: 8 }}>
            <summary>{c.all}</summary>
            <ul>
              {earlier.map((b) => (
                <li key={b.tag}>
                  <a href={b.url} className="mono">
                    {b.tag}
                  </a>{" "}
                  {b.vboxUrl && (
                    <>
                      (<a href={b.vboxUrl}>.vbox</a>){" "}
                    </>
                  )}
                  · {(b.size / 1024 / 1024).toFixed(1)} {c.mb} · {b.publishedAt.slice(0, 10)}
                </li>
              ))}
            </ul>
          </details>
        )}
      </section>

      <section>
        <h2>{c.nowTitle}</h2>
        <p className="muted" style={{ fontSize: 13.5 }}>
          {c.now}
        </p>
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
          <h2>{c.qemuTitle}</h2>
          <p className="muted" style={{ marginTop: 0 }}>
            {c.qemu}
          </p>
          <pre className="dl-code">qemu-system-x86_64 -cdrom grenos.iso -serial stdio</pre>

          <h2 style={{ marginTop: 22 }}>{c.usbTitle}</h2>
          <ol className="dl-steps">
            {c.usb.map((step) => (
              <li key={step}>{step}</li>
            ))}
          </ol>
          <pre className="dl-code">sudo dd if=grenos.iso of=/dev/sdX bs=4M status=progress</pre>
        </div>
      </section>

      <p className="faint">
        {c.source} : <a href={`https://github.com/${repo}/tree/main/kernel`}>github.com/{repo}</a>
      </p>
    </>
  );
}
