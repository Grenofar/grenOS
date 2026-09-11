import type { Metadata } from "next";
import { DownloadView, type Build } from "@/components/DownloadView";

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
      }));
  } catch {
    // Nothing published yet, or Storage unreachable: the page says so.
    return [];
  }
}

export default async function DownloadPage() {
  return <DownloadView builds={await publishedBuilds()} repo={REPO} />;
}
