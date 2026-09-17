import type { Metadata } from "next";
import { DownloadView, type Build, type LinuxRelease } from "@/components/DownloadView";

/**
 * The public download page (D-030, D-032): the images CI built and booted,
 * and how to run them. They live in Supabase Storage, in the public
 * "releases" bucket: anyone can download them without an account, and
 * without going through GitHub. release.yml puts them there, with index.json
 * listing the ten latest, newest first.
 *
 * Read on the server and refreshed every minute, as long as Storage caches
 * the index itself.
 */
export const revalidate = 60;

export const metadata: Metadata = {
  title: "grenOS — download",
  description: "Télécharger grenOS : l'image ISO que la CI a construite et démarrée.",
};

const REPO = "Grenofar/grenOS";

// The project URL is public (it is in verify.yml too); the variable wins.
const SUPABASE_URL = process.env.NEXT_PUBLIC_SUPABASE_URL ?? "https://tpqzhzuoyqpfairatdrw.supabase.co";
const RELEASES = `${SUPABASE_URL}/storage/v1/object/public/releases`;

/** One entry of index.json, as release.yml writes it. */
interface Entry {
  build: string;
  path: string;
  commit: string;
  size: number;
  published_at: string;
  /** The VirtualBox machine that boots `path` (D-033), when published with it. */
  vbox?: string | null;
  /** What CI saw on screen, when the kernel drew something. */
  screen?: string | null;
  /** The ISO and its .vbox in one zip: the VirtualBox choice. */
  virtualbox?: string | null;
  virtualbox_size?: number | null;
}

async function publishedBuilds(): Promise<Build[]> {
  try {
    const res = await fetch(`${RELEASES}/index.json`, { next: { revalidate: 60 } });
    if (!res.ok) return [];
    const index: unknown = await res.json();
    if (!Array.isArray(index)) return [];
    return (index as Entry[])
      .filter((e) => typeof e?.path === "string" && typeof e?.build === "string")
      .map((e) => ({
        tag: e.build,
        publishedAt: e.published_at,
        size: e.size,
        // ?download makes Storage send the file as an attachment, under this name.
        url: `${RELEASES}/${e.path}?download=grenos-${e.build}.iso`,
        // The .vbox names the ISO by this same file name: both land side by side.
        vboxUrl: e.vbox ? `${RELEASES}/${e.vbox}?download=grenos-${e.build}.vbox` : undefined,
        screenUrl: e.screen ? `${RELEASES}/${e.screen}` : undefined,
        virtualboxUrl: e.virtualbox ? `${RELEASES}/${e.virtualbox}?download=grenos-${e.build}-virtualbox.zip` : undefined,
        virtualboxSize: e.virtualbox_size ?? undefined,
      }));
  } catch {
    // Nothing published yet, or Storage unreachable: the page says so.
    return [];
  }
}

/**
 * L'edition Linux : une image de plus d'un gigaoctet, donc publiee en Release
 * GitHub (Supabase est plafonne a 1 Go) par .github/workflows/linux.yml. On
 * prend la derniere release dont l'etiquette commence par "linux-".
 */
async function linuxRelease(): Promise<LinuxRelease | null> {
  try {
    const res = await fetch(`https://api.github.com/repos/${REPO}/releases?per_page=20`, {
      headers: { accept: "application/vnd.github+json" },
      next: { revalidate: 300 },
    });
    if (!res.ok) return null;
    const releases: unknown = await res.json();
    if (!Array.isArray(releases)) return null;
    for (const release of releases as Array<Record<string, unknown>>) {
      const tag = typeof release.tag_name === "string" ? release.tag_name : "";
      if (!tag.startsWith("linux-")) continue;
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
  const [builds, linux] = await Promise.all([publishedBuilds(), linuxRelease()]);
  return <DownloadView builds={builds} linux={linux} repo={REPO} />;
}
