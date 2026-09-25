"use client";

import { useLang } from "@/lib/i18n";

/**
 * L'espace bêta : la toute dernière image, celle qui sort de la construction.
 *
 * Pourquoi une page à part de `/download` : ce ne sont pas les mêmes lecteurs.
 * `/download` s'adresse à quelqu'un qui découvre grenOS et veut l'essayer ;
 * il y trouve la dernière version validée, et rien d'autre. Ici, c'est
 * Grenofar qui vient installer la version d'il y a dix minutes pour voir ce
 * qu'elle vaut — il lui faut la plus récente, les précédentes pour revenir en
 * arrière si celle-ci est mauvaise, et surtout la vérité sur ce qui a été
 * vérifié et ce qui ne l'a pas été.
 *
 * Les pré-publications sont montrées ici, et nulle part ailleurs : une image
 * jugée défectueuse reste téléchargeable pour qui veut comprendre pourquoi,
 * mais elle porte son étiquette.
 */
export interface BetaBuild {
  tag: string;
  publishedAt: string;
  isoUrl: string;
  isoSize: number;
  zipUrl?: string;
  zipSize?: number;
  pageUrl: string;
  prerelease: boolean;
}

const go = (bytes: number) => (bytes / 1024 / 1024 / 1024).toFixed(2);

const quand = (iso: string, lang: "fr" | "en") => {
  if (!iso) return "";
  const d = new Date(iso);
  return d.toLocaleString(lang === "fr" ? "fr-FR" : "en-GB", {
    day: "2-digit",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  });
};

const FR = {
  marque: "grenOS · bêta",
  title: "La dernière image, celle qui vient de sortir",
  lead:
    "Chaque changement du système produit une image, construite puis démarrée pour de vrai par la machine d'intégration. Celle du haut a quelques minutes. C'est ici qu'on vient l'installer pour voir ce qu'elle vaut.",
  derniere: "La plus récente",
  iso: "Télécharger l'ISO",
  zip: "Télécharger pour VirtualBox",
  notes: "Ce que cette image a montré",
  precedentes: "Les précédentes, si celle-ci déçoit",
  aucune: "Aucune image publiée pour l'instant.",
  prepublication: "pré-publication — jugée défectueuse",
  honnete: "Ce que « bêta » veut dire ici",
  honneteTexte:
    "Cette image a été construite, puis démarrée en 1280×720 avec un vrai clic de souris sur la barre. La construction refuse de publier si le bureau ne s'ouvre pas, si une fenêtre déborde de l'écran, si la touche Windows n'ouvre pas le menu, si un clic à côté ne referme pas le panneau de son, si le magasin ne trouve pas son catalogue, si la persistance ne s'active pas, ou si l'image ne démarre pas en UEFI. Mais une machine virtuelle n'a ni carte son, ni carte graphique, ni disque à installer : ce qui suit n'a été vérifié par personne.",
  aEssayer: "Ce que la machine d'intégration ne peut pas essayer",
  liste: [
    "Le son. La cause du silence a été trouvée et corrigée, mais personne n'a encore entendu grenOS.",
    "L'installation sur le disque, menée jusqu'au redémarrage. Le bouton peut enfin démarrer — il ne le pouvait pas avant le 25 septembre.",
    "Une mise à jour lancée depuis une machine déjà installée.",
    "Rocket League, par Steam et Proton Experimental. La page Jeux pose le réglage ; il faut un compte et une vraie carte 3D.",
  ],
  stable: "La page de téléchargement ordinaire",
};

const EN: typeof FR = {
  marque: "grenOS · beta",
  title: "The latest image, fresh from the build",
  lead:
    "Every change to the system produces an image, built and then really booted by the integration machine. The one at the top is minutes old. This is where you come to install it and see what it is worth.",
  derniere: "The most recent",
  iso: "Download the ISO",
  zip: "Download for VirtualBox",
  notes: "What this image showed",
  precedentes: "The previous ones, if this one disappoints",
  aucune: "No image published yet.",
  prepublication: "pre-release — judged faulty",
  honnete: "What “beta” means here",
  honneteTexte:
    "This image was built, then booted at 1280×720 with a real mouse click on the panel. The build refuses to publish if the desktop does not open, if a window overflows the screen, if the Windows key does not open the menu, if a click elsewhere does not close the sound panel, if the store cannot find its catalogue, if persistence does not come up, or if the image does not boot under UEFI. But a virtual machine has no sound card, no graphics card and no disk to install onto: what follows has been checked by nobody.",
  aEssayer: "What the integration machine cannot try",
  liste: [
    "Sound. The cause of the silence was found and fixed, but nobody has heard grenOS yet.",
    "Installing to disk, all the way to a reboot. The button can finally start — it could not before 25 September.",
    "An update launched from an already installed machine.",
    "Rocket League, through Steam and Proton Experimental. The Games page writes the setting; it needs an account and a real 3D card.",
  ],
  stable: "The ordinary download page",
};

const TEXT = { fr: FR, en: EN };

export function BetaView({ builds }: { builds: BetaBuild[] }) {
  const { lang } = useLang();
  const c = TEXT[lang];
  const [derniere, ...precedentes] = builds;

  return (
    <>
      <div className="dl-hero">
        <div className="dl-mark">
          <span className="puce" /> {c.marque}
        </div>
        <h1>{c.title}</h1>
        <p>{c.lead}</p>
      </div>

      {derniere ? (
        <>
          <section>
            <p className="faint" style={{ marginTop: 0 }}>
              <span className={derniere.prerelease ? "dot" : "dot ok"} />{" "}
              <span className="mono">{derniere.tag}</span> · {quand(derniere.publishedAt, lang)}
              {derniere.prerelease && ` · ${c.prepublication}`}
            </p>
            <div className="grid cols-2">
              <div className="card">
                <h2>ISO</h2>
                <a className="dl-button" href={derniere.isoUrl}>
                  {c.iso}
                </a>
                <div className="faint mono" style={{ marginTop: 8 }}>
                  {go(derniere.isoSize)} Go
                </div>
              </div>
              <div className="card">
                <h2>VirtualBox</h2>
                {derniere.zipUrl && (
                  <a className="dl-button" href={derniere.zipUrl}>
                    {c.zip}
                  </a>
                )}
                <div className="faint mono" style={{ marginTop: 8 }}>
                  {go(derniere.zipSize ?? 0)} Go
                </div>
              </div>
            </div>
          </section>

          {/* La partie qui compte : dire ce qui n'a pas ete essaye. Une page
              qui ne promet que du bien fait perdre du temps a celui qui la
              lit, parce qu'il cherche ailleurs la cause de ce qui le gene. */}
          <section>
            <div className="card">
              <h2>{c.honnete}</h2>
              <p className="muted">{c.honneteTexte}</p>
              <h2 style={{ marginTop: 18 }}>{c.aEssayer}</h2>
              <ol className="dl-steps">
                {c.liste.map((ligne) => (
                  <li key={ligne}>{ligne}</li>
                ))}
              </ol>
              <p className="faint" style={{ marginTop: 12 }}>
                <a href={derniere.pageUrl}>{c.notes}</a>
              </p>
            </div>
          </section>

          {precedentes.length > 0 && (
            <section>
              <h2>{c.precedentes}</h2>
              <div className="beta-liste">
                {precedentes.map((b) => (
                  <div className="card beta-ligne" key={b.tag}>
                    <div>
                      <span className={b.prerelease ? "dot" : "dot ok"} />{" "}
                      <span className="mono">{b.tag}</span>
                      <div className="faint" style={{ marginTop: 3 }}>
                        {quand(b.publishedAt, lang)}
                        {b.prerelease && ` · ${c.prepublication}`}
                      </div>
                    </div>
                    <div className="row">
                      <a href={b.isoUrl} className="faint mono">
                        iso · {go(b.isoSize)} Go
                      </a>
                      {b.zipUrl && (
                        <a href={b.zipUrl} className="faint mono" style={{ marginLeft: 12 }}>
                          vbox · {go(b.zipSize ?? 0)} Go
                        </a>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            </section>
          )}
        </>
      ) : (
        <div className="card dl-card">
          <p className="muted" style={{ margin: 0 }}>
            {c.aucune}
          </p>
        </div>
      )}

      <p className="faint">
        <a href="/download">{c.stable}</a>
      </p>
    </>
  );
}
