"use client";

import Link from "next/link";
import type { ReactNode } from "react";
import { useLang } from "@/lib/i18n";

/** The frame of a public page: the name, the language, the content. */
export function PublicShell({ children }: { children: ReactNode }) {
  const { lang, setLang } = useLang();

  return (
    <div className="public">
      <header className="public-head">
        <div className="brand" style={{ padding: 0 }}>
          gren<span>OS</span>
        </div>
        <div className="row">
          <Link href="/" className="faint" style={{ marginRight: 10 }}>
            {lang === "fr" ? "Opérateurs" : "Operators"}
          </Link>
          <div className="lang-toggle">
            {(["fr", "en"] as const).map((l) => (
              <button key={l} className={lang === l ? "on" : ""} onClick={() => setLang(l)}>
                {l.toUpperCase()}
              </button>
            ))}
          </div>
        </div>
      </header>
      <main className="public-main">{children}</main>
    </div>
  );
}
