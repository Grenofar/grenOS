import type { Metadata } from "next";
import { BetaView, type BetaBuild } from "@/components/BetaView";

/**
 * L'espace bêta : toutes les images récentes, la plus fraîche en tête.
 *
 * `/download` ne montre que la dernière image saine, et c'est ce qu'il faut à
 * quelqu'un qui découvre grenOS. Ici on montre la suite complète, y compris
 * les pré-publications : c'est la page de celui qui installe la version d'il y
 * a dix minutes pour la mettre à l'épreuve, et qui doit pouvoir revenir à la
 * précédente sans aller la chercher sur GitHub.
 *
 * Rafraîchie souvent : une image sort à chaque changement du système, et une
 * page bêta qui montre celle d'hier ne sert à rien.
 */
export const revalidate = 60;

export const metadata: Metadata = {
  title: "grenOS — bêta",
  description:
    "Les dernières images de grenOS, la plus récente en tête : construites et démarrées par la CI, jamais essayées par quelqu'un.",
};

const REPO = "Grenofar/grenOS";
const COMBIEN = 8;

async function builds(): Promise<BetaBuild[]> {
  try {
    const res = await fetch(`https://api.github.com/repos/${REPO}/releases?per_page=30`, {
      headers: { accept: "application/vnd.github+json" },
      next: { revalidate: 60 },
    });
    if (!res.ok) return [];
    const releases: unknown = await res.json();
    if (!Array.isArray(releases)) return [];

    const sortie: BetaBuild[] = [];
    for (const release of releases as Array<Record<string, unknown>>) {
      const tag = typeof release.tag_name === "string" ? release.tag_name : "";
      if (!tag.startsWith("linux-")) continue;
      // Un brouillon n'est pas une image : c'est une publication interrompue.
      // Une pré-publication, si : elle a ete jugee defectueuse, et la page le
      // dit au lieu de la cacher.
      if (release.draft === true) continue;
      const assets = Array.isArray(release.assets)
        ? (release.assets as Array<Record<string, unknown>>)
        : [];
      const pick = (end: string) =>
        assets.find((a) => typeof a.name === "string" && (a.name as string).endsWith(end));
      const iso = pick(".iso");
      const zip = pick("-virtualbox.zip");
      if (!iso) continue;
      sortie.push({
        tag,
        publishedAt: typeof release.published_at === "string" ? release.published_at : "",
        isoUrl: String(iso.browser_download_url ?? ""),
        isoSize: Number(iso.size ?? 0),
        zipUrl: zip ? String(zip.browser_download_url ?? "") : undefined,
        zipSize: zip ? Number(zip.size ?? 0) : undefined,
        pageUrl: typeof release.html_url === "string" ? release.html_url : "",
        prerelease: release.prerelease === true,
      });
      if (sortie.length >= COMBIEN) break;
    }
    return sortie;
  } catch {
    return [];
  }
}

export default async function BetaPage() {
  return <BetaView builds={await builds()} />;
}
