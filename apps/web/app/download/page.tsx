import type { Metadata } from "next";
import { DownloadView, type Build } from "@/components/DownloadView";

/**
 * The public download page (D-030): the latest image CI built and booted,
 * and how to run it. The image is a GitHub Release asset, which anyone can
 * download without an account — an Actions artifact needs a GitHub login,
 * and expires after thirty days.
 *
 * Read on the server and refreshed every five minutes: the GitHub API allows
 * sixty anonymous requests an hour, and a visitor must never spend them.
 */
export const revalidate = 300;

export const metadata: Metadata = {
  title: "grenOS — download",
  description: "Télécharger grenOS : l'image ISO que la CI a construite et démarrée.",
};

const REPO = "Grenofar/grenOS";

interface Release {
  tag_name: string;
  html_url: string;
  published_at: string;
  assets?: Array<{ name: string; size: number; browser_download_url: string }>;
}

async function latestBuild(): Promise<Build | null> {
  try {
    const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
      headers: { accept: "application/vnd.github+json" },
      next: { revalidate: 300 },
    });
    if (!res.ok) return null;
    const release = (await res.json()) as Release;
    const iso = release.assets?.find((a) => a.name.endsWith(".iso"));
    if (!iso) return null;
    return {
      tag: release.tag_name,
      page: release.html_url,
      publishedAt: release.published_at,
      size: iso.size,
      url: iso.browser_download_url,
    };
  } catch {
    // No release yet, or GitHub unreachable: the page says so and links to
    // the releases list rather than failing.
    return null;
  }
}

export default async function DownloadPage() {
  return <DownloadView build={await latestBuild()} repo={REPO} />;
}
