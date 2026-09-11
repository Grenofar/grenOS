import { effectiveFiles, pinnedBeforeRust185, RECENT_NIGHTLY } from "./preflight.ts";

/**
 * Crate versions checked against crates.io before a commit (D-026, D-027).
 *
 * The most common invention in kernel code is a version number: mission 1
 * pinned `limine = "0.11"` when the newest release was 0.6.5, and cargo gave
 * up before compiling a single line. Whether a published version satisfies a
 * requirement is not a matter of judgement, so it belongs to pre-flight.
 *
 * So is the edition a release is built with. Cargo refuses the manifest of an
 * edition-2024 crate under a toolchain older than Rust 1.85, again before
 * compiling anything, and the limine crate is edition 2024 from 0.6 on while
 * mission 1's toolchain pinned a nightly of November 2024.
 *
 * Only certainties. A requirement form this does not model, or a crates.io
 * that does not answer, is left for CI to judge.
 */

type Change = { path: string; content: string | null };

/**
 * Published, non-yanked versions of a crate; `null` when the crate does not
 * exist; `undefined` when that could not be established.
 */
export type Lookup = (name: string) => Promise<string[] | null | undefined>;

/** A published, non-yanked release, with the edition its manifest declares. */
export interface Release {
  num: string;
  edition: string | null;
}

/** Releases of a crate, with the same `null` and `undefined` as Lookup. */
export type Releases = (name: string) => Promise<Release[] | null | undefined>;

export interface Requirement {
  name: string;
  req: string;
}

const DEPENDENCY_SECTION =
  /^\s*\[(?:target\.[^\]]+\.)?(?:dependencies|build-dependencies|dev-dependencies)\]\s*$/;

/** Registry requirements of a manifest: path and git dependencies are not ours. */
export function requirements(manifest: string): Requirement[] {
  const out: Requirement[] = [];
  let inDependencies = false;

  for (const line of manifest.split("\n")) {
    if (/^\s*\[/.test(line)) {
      inDependencies = DEPENDENCY_SECTION.test(line);
      continue;
    }
    if (!inDependencies) continue;

    const simple = line.match(/^\s*([A-Za-z0-9_-]+)\s*=\s*"([^"]+)"/);
    if (simple) {
      out.push({ name: simple[1]!, req: simple[2]! });
      continue;
    }

    const table = line.match(/^\s*([A-Za-z0-9_-]+)\s*=\s*\{(.*)\}/);
    if (table && !/\b(path|git)\s*=/.test(table[2]!)) {
      const version = table[2]!.match(/\bversion\s*=\s*"([^"]+)"/)?.[1];
      const renamed = table[2]!.match(/\bpackage\s*=\s*"([^"]+)"/)?.[1];
      if (version) out.push({ name: renamed ?? table[1]!, req: version });
    }
  }
  return out;
}

/**
 * Whether `version` satisfies a cargo requirement: caret (the default), tilde
 * or exact. `undefined` for forms not modelled here — ranges, wildcards,
 * pre-release requirements — which are left to cargo.
 */
export function satisfies(version: string, req: string): boolean | undefined {
  const r = req.trim();
  if (r === "*") return true;

  const m = r.match(/^([=^~]?)\s*(\d+)(?:\.(\d+))?(?:\.(\d+))?$/);
  if (!m) return undefined;

  const v = parse(version);
  // A plain requirement never selects a pre-release.
  if (!v || version.includes("-")) return false;

  const op = m[1] || "^";
  const major = Number(m[2]);
  const minor = m[3] === undefined ? undefined : Number(m[3]);
  const patch = m[4] === undefined ? undefined : Number(m[4]);
  const low = [major, minor ?? 0, patch ?? 0];

  let high: number[];
  if (op === "=") {
    if (patch !== undefined) return compare(v, low) === 0;
    high = minor === undefined ? [major + 1, 0, 0] : [major, minor + 1, 0];
  } else if (op === "~") {
    high = minor === undefined ? [major + 1, 0, 0] : [major, minor + 1, 0];
  } else if (major > 0 || minor === undefined) {
    high = [major + 1, 0, 0];
  } else if (minor > 0 || patch === undefined) {
    high = [0, minor + 1, 0];
  } else {
    high = [0, 0, (patch ?? 0) + 1];
  }
  return compare(v, low) >= 0 && compare(v, high) < 0;
}

export async function checkDependencies(changes: Change[], lookup: Lookup): Promise<string[]> {
  const problems: string[] = [];

  for (const { path, content } of changes) {
    if (content === null || !path.endsWith("Cargo.toml")) continue;

    for (const { name, req } of requirements(content)) {
      const versions = await lookup(name);
      if (versions === undefined) continue;
      if (versions === null) {
        problems.push(`${path}: the crate "${name}" does not exist on crates.io.`);
        continue;
      }

      const verdicts = versions.map((v) => satisfies(v, req));
      if (verdicts.some((x) => x === undefined)) continue;
      if (verdicts.some(Boolean)) continue;

      const newest = versions
        .filter((v) => !v.includes("-") && parse(v))
        .sort((a, b) => compare(parse(b)!, parse(a)!))[0];
      problems.push(
        `${path}: no published version of ${name} matches "${req}"` +
          (newest ? ` — the newest is ${newest}` : "") +
          `. Consult https://crates.io/api/v1/crates/${name} and pin a version that exists.`,
      );
    }
  }
  return problems;
}

/**
 * Dependencies built with edition 2024 under a toolchain pinned before Rust
 * 1.85. Judged for every crate whose manifest or toolchain file the answer
 * changes, on the release cargo would select: the newest that satisfies the
 * requirement. A Cargo.lock can select another one, so a crate that has one
 * is left to CI, and so is a branch that could not be read.
 */
export async function checkEditions(
  changes: Change[],
  branch: Change[] | null,
  releases: Releases,
): Promise<string[]> {
  if (branch === null) return [];
  const files = effectiveFiles(changes, branch);
  const problems: string[] = [];

  const roots = new Set<string>();
  for (const { path } of changes) {
    const m = path.match(/^(?:(.+?)\/)?(?:Cargo\.toml|rust-toolchain\.toml)$/);
    if (m) roots.add(m[1] ?? "");
  }

  for (const root of roots) {
    const prefix = root ? `${root}/` : "";
    const manifest = files.get(`${prefix}Cargo.toml`);
    const toolchain = files.get(`${prefix}rust-toolchain.toml`);
    const pinned = toolchain ? pinnedBeforeRust185(toolchain) : null;
    if (!manifest || !pinned || files.has(`${prefix}Cargo.lock`)) continue;

    for (const { name, req } of requirements(manifest)) {
      const published = await releases(name);
      if (!published) continue;
      const selected = newestMatching(published, req);
      if (!selected || selected.edition !== "2024") continue;

      const fallback = published
        .filter((r) => r.edition !== null && r.edition !== "2024" && !r.num.includes("-") && parse(r.num))
        .sort((a, b) => compare(parse(b.num)!, parse(a.num)!))[0];
      problems.push(
        `${prefix}Cargo.toml: ${name} ${selected.num}, which cargo selects for "${req}", is built with edition 2024 and needs Rust 1.85, ` +
          `but ${prefix}rust-toolchain.toml pins ${pinned}. Pin a recent dated nightly (${RECENT_NIGHTLY} exists, with clippy)` +
          (fallback
            ? `, or require ${name} = "${fallback.num}", the newest release built with edition ${fallback.edition}.`
            : "."),
      );
    }
  }
  return problems;
}

/** The release cargo selects for a requirement; null when this module cannot tell. */
function newestMatching(published: Release[], req: string): Release | null {
  const judged = published.map((r) => ({ r, ok: satisfies(r.num, req) }));
  if (judged.some((j) => j.ok === undefined)) return null;
  return (
    judged
      .filter((j) => j.ok)
      .map((j) => j.r)
      .sort((a, b) => compare(parse(b.num)!, parse(a.num)!))[0] ?? null
  );
}

const cache = new Map<string, { at: number; releases: Release[] | null }>();
const CACHE_MS = 60 * 60 * 1000;

/** The real lookup: crates.io, cached for an hour per crate. */
export const crateReleases: Releases = async (name) => {
  const hit = cache.get(name);
  if (hit && Date.now() - hit.at < CACHE_MS) return hit.releases;

  try {
    const res = await fetch(`https://crates.io/api/v1/crates/${encodeURIComponent(name)}`, {
      // crates.io refuses requests without one.
      headers: { "user-agent": "grenOS-worker (https://github.com/Grenofar/grenOS)" },
      signal: AbortSignal.timeout(15_000),
    });
    if (res.status === 404) {
      cache.set(name, { at: Date.now(), releases: null });
      return null;
    }
    if (!res.ok) return undefined;

    const data = (await res.json()) as {
      crate?: { max_stable_version?: string };
      versions?: Array<{ num: string; yanked: boolean; edition?: string | null }> | null;
    };
    const releases = data.versions
      ? data.versions.filter((v) => !v.yanked).map((v) => ({ num: v.num, edition: v.edition ?? null }))
      : data.crate?.max_stable_version
        ? [{ num: data.crate.max_stable_version, edition: null }]
        : undefined;
    if (!releases) return undefined;

    cache.set(name, { at: Date.now(), releases });
    return releases;
  } catch {
    return undefined;
  }
};

/** Published versions only, as checkDependencies reads them. */
export const crateVersions: Lookup = async (name) => {
  const releases = await crateReleases(name);
  return releases ? releases.map((r) => r.num) : releases;
};

function parse(version: string): number[] | null {
  const m = version.match(/^(\d+)\.(\d+)\.(\d+)/);
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null;
}

function compare(a: number[], b: number[]): number {
  for (let i = 0; i < 3; i++) {
    const d = (a[i] ?? 0) - (b[i] ?? 0);
    if (d !== 0) return d;
  }
  return 0;
}
