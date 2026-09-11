"use client";

import { useLang, type Lang } from "@/lib/i18n";

/** The latest published image, as the GitHub Releases API describes it. */
export interface Build {
  tag: string;
  page: string;
  publishedAt: string;
  size: number;
  url: string;
}

/*
 * The page's copy lives here rather than in the shared dictionary: it is read
 * by people who are not operators, and it changes with what the OS can do.
 * Keep "now" honest — it is the first thing someone booting grenOS reads.
 */
const FR = {
  title: "Télécharger grenOS",
  lead: "grenOS est un système d'exploitation x86_64 écrit en Rust par une équipe d'agents d'IA. Chaque image publiée ici a d'abord été construite, puis démarrée dans QEMU par la CI.",
  download: "Télécharger grenos.iso",
  mb: "Mo",
  build: "Version",
  published: "publiée le",
  all: "Toutes les versions",
  none: "La première image est en cours de publication. Reviens dans quelques minutes.",
  checked: "Vérifié automatiquement dans QEMU. VirtualBox et les vrais PC ne sont pas testés par la CI.",
  nowTitle: "Ce qu'il fait aujourd'hui",
  now: "Étape 1 sur 10 : il démarre avec le chargeur Limine, écrit « grenOS » sur le port série COM1, puis s'arrête. Il n'affiche encore rien à l'écran : c'est par le port série qu'on le voit.",
  vboxTitle: "VirtualBox",
  vbox: [
    "Nouvelle machine : type Other, version Other/Unknown (64-bit), 256 Mo de mémoire, sans disque dur.",
    "Configuration → Stockage : mettre grenos.iso dans le lecteur optique.",
    "Configuration → Ports série : activer le port 1 (COM1), mode « Fichier brut » (Raw File), avec un chemin comme C:\\grenos-serie.txt.",
    "Laisser l'EFI désactivé (Configuration → Système).",
    "Démarrer : après le menu de Limine (3 secondes), le fichier contient grenOS. L'écran ne montre rien de plus, c'est normal à cette étape.",
  ],
  vboxNote: "Une machine 64 bits demande la virtualisation matérielle (VT-x ou AMD-V) activée dans le BIOS du PC.",
  qemuTitle: "QEMU",
  qemu: "La sortie série s'affiche dans le terminal :",
  usbTitle: "Un vrai PC, sur clé USB",
  usb: [
    "Écrire l'image sur une clé avec Rufus (mode image DD), balenaEtcher, ou sous Linux la commande ci-dessous. La clé est entièrement effacée.",
    "Démarrer le PC sur la clé, en mode Legacy / BIOS (CSM).",
    "Sans câble série branché sur COM1, on ne voit que le menu de Limine : l'affichage à l'écran viendra plus tard.",
  ],
  source: "Code source",
};

const EN: typeof FR = {
  title: "Download grenOS",
  lead: "grenOS is an x86_64 operating system written in Rust by a team of AI agents. Every image published here was first built, then booted in QEMU, by CI.",
  download: "Download grenos.iso",
  mb: "MB",
  build: "Build",
  published: "published",
  all: "All builds",
  none: "The first image is being published. Come back in a few minutes.",
  checked: "Checked automatically in QEMU. VirtualBox and real PCs are not tested by CI.",
  nowTitle: "What it does today",
  now: "Step 1 of 10: it boots with the Limine bootloader, writes “grenOS” to the COM1 serial port, then halts. It shows nothing on screen yet: the serial port is where you see it.",
  vboxTitle: "VirtualBox",
  vbox: [
    "New machine: type Other, version Other/Unknown (64-bit), 256 MB of memory, no hard disk.",
    "Settings → Storage: put grenos.iso in the optical drive.",
    "Settings → Serial Ports: enable port 1 (COM1), mode Raw File, with a path such as C:\\grenos-serial.txt.",
    "Leave EFI disabled (Settings → System).",
    "Start it: after Limine's menu (3 seconds), the file contains grenOS. The screen shows nothing more, which is expected at this stage.",
  ],
  vboxNote: "A 64-bit machine needs hardware virtualisation (VT-x or AMD-V) enabled in the host PC's firmware.",
  qemuTitle: "QEMU",
  qemu: "The serial output appears in the terminal:",
  usbTitle: "A real PC, from a USB stick",
  usb: [
    "Write the image to a stick with Rufus (DD image mode), balenaEtcher, or on Linux the command below. The stick is erased.",
    "Boot the PC from the stick in Legacy / BIOS (CSM) mode.",
    "Without a serial cable on COM1 you only see Limine's menu: on-screen output comes later.",
  ],
  source: "Source code",
};

const TEXT: Record<Lang, typeof FR> = { fr: FR, en: EN };

export function DownloadView({ build, repo }: { build: Build | null; repo: string }) {
  const { lang } = useLang();
  const c = TEXT[lang];
  const releases = `https://github.com/${repo}/releases`;

  return (
    <>
      <div className="dl-hero">
        <h1>{c.title}</h1>
        <p>{c.lead}</p>
      </div>

      <section>
        <div className="card dl-card">
          {build ? (
            <>
              <div>
                <div className="row">
                  <span className="dot ok" />
                  <b>grenos.iso</b>
                  <span className="faint mono">
                    {(build.size / 1024 / 1024).toFixed(1)} {c.mb}
                  </span>
                </div>
                <div className="faint" style={{ marginTop: 4 }}>
                  {c.build} <span className="mono">{build.tag}</span> · {c.published}{" "}
                  {build.publishedAt.slice(0, 10)}
                </div>
              </div>
              <div className="row">
                <a className="dl-button" href={build.url}>
                  {c.download}
                </a>
                <a href={releases} className="faint" style={{ marginLeft: 6 }}>
                  {c.all}
                </a>
              </div>
            </>
          ) : (
            <p className="muted" style={{ margin: 0 }}>
              {c.none} <a href={releases}>{c.all}</a>
            </p>
          )}
        </div>
        <p className="faint" style={{ marginTop: 8 }}>
          {c.checked}
        </p>
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
