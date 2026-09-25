import type { Metadata } from "next";
import { DownloadView, type LinuxRelease } from "@/components/DownloadView";

/**
 * La page publique de telechargement : l'image Linux que la CI a construite et
 * demarree. Elle depasse le gigaoctet, donc elle est publiee en Release GitHub
 * plutot que dans Supabase Storage, plafonne a 1 Go.
 *
 * Lue sur le serveur et rafraichie toutes les cinq minutes.
 */
export const revalidate = 60;

export const metadata: Metadata = {
  title: "grenOS — download",
  description: "Télécharger grenOS : l'image ISO que la CI a construite et démarrée.",
};

const REPO = "Grenofar/grenOS";


/**
 * L'edition Linux : une image de plus d'un gigaoctet, donc publiee en Release
 * GitHub (Supabase est plafonne a 1 Go) par .github/workflows/linux.yml. On
 * prend la derniere release dont l'etiquette commence par "linux-".
 */
async function linuxRelease(): Promise<LinuxRelease | null> {
  try {
    const res = await fetch(`https://api.github.com/repos/${REPO}/releases?per_page=20`, {
      headers: { accept: "application/vnd.github+json" },
      // Soixante secondes, comme la page elle-meme. Elles disaient 60 et 300 :
      // c'est le cache de CETTE requete qui l'emporte, donc la page pouvait
      // annoncer une image vieille de cinq minutes alors qu'une plus recente
      // etait publiee. Grenofar l'a vu trois fois dans la journee, et la
      // troisieme il a cru que la page etait restee bloquee sur une vieille
      // version. Deux nombres qui devraient etre egaux ne doivent pas etre
      // ecrits deux fois.
      next: { revalidate: 60 },
    });
    if (!res.ok) return null;
    const releases: unknown = await res.json();
    if (!Array.isArray(releases)) return null;
    for (const release of releases as Array<Record<string, unknown>>) {
      const tag = typeof release.tag_name === "string" ? release.tag_name : "";
      if (!tag.startsWith("linux-")) continue;
      // Une version marquee brouillon ou pre-publication a ete jugee
      // defectueuse : elle reste telechargeable pour qui la cherche, mais la
      // page ne la propose pas. Le 18 septembre, une image ou personne ne
      // pouvait se connecter a ete publiee ; c'est ainsi qu'on l'ecarte.
      if (release.draft === true || release.prerelease === true) continue;
      const assets = Array.isArray(release.assets) ? (release.assets as Array<Record<string, unknown>>) : [];
      const pick = (end: string) =>
        assets.find((a) => typeof a.name === "string" && (a.name as string).endsWith(end));
      const iso = pick(".iso");
      const zip = pick("-virtualbox.zip");
      if (!iso) continue;
      return {
        tag,
        publishedAt: typeof release.published_at === "string" ? release.published_at : "",
        isoUrl: String(iso.browser_download_url ?? ""),
        isoSize: Number(iso.size ?? 0),
        zipUrl: zip ? String(zip.browser_download_url ?? "") : undefined,
        zipSize: zip ? Number(zip.size ?? 0) : undefined,
        pageUrl: typeof release.html_url === "string" ? release.html_url : "",
      };
    }
    return null;
  } catch {
    return null;
  }
}

export default async function DownloadPage() {
  return <DownloadView linux={await linuxRelease()} repo={REPO} />;
}
