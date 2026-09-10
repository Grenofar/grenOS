"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import type { ReactNode } from "react";
import { supabase } from "@/lib/supabase";
import { useLang } from "@/lib/i18n";

export function Shell({ children }: { children: ReactNode }) {
  const { t, lang, setLang } = useLang();
  const path = usePathname();

  const links = [
    { href: "/", key: "nav.dashboard" },
    { href: "/roadmap", key: "nav.roadmap" },
    { href: "/missions", key: "nav.missions" },
    { href: "/runs", key: "nav.runs" },
    { href: "/agents", key: "nav.agents" },
  ];

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">
          gren<span>OS</span>
        </div>

        <nav className="nav">
          {links.map((l) => (
            <Link
              key={l.href}
              href={l.href}
              className={path === l.href ? "active" : ""}
            >
              {t(l.key)}
            </Link>
          ))}
        </nav>

        <div className="spacer" />

        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <div className="lang-toggle">
            {(["fr", "en"] as const).map((l) => (
              <button
                key={l}
                className={lang === l ? "on" : ""}
                onClick={() => setLang(l)}
              >
                {l.toUpperCase()}
              </button>
            ))}
          </div>
          <button
            className="ghost"
            onClick={() => supabase().auth.signOut()}
            style={{ fontSize: 11, padding: "5px 9px" }}
          >
            {t("auth.signout")}
          </button>
        </div>
      </aside>

      <main className="main">{children}</main>
    </div>
  );
}
