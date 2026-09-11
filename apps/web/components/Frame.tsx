"use client";

import type { ReactNode } from "react";
import { usePathname } from "next/navigation";
import { AuthGate } from "@/components/AuthGate";
import { PublicShell } from "@/components/PublicShell";
import { Shell } from "@/components/Shell";

/**
 * Which pages need an operator, and which are for anyone (D-030).
 *
 * The control plane belongs to the two operators; the OS itself is for
 * everybody. A public page shows no project data — nothing it renders comes
 * from Supabase — so it needs neither a session nor the operators' sidebar.
 */
const PUBLIC = ["/download"];

export function Frame({ children }: { children: ReactNode }) {
  const path = usePathname() ?? "/";
  if (PUBLIC.some((p) => path === p || path.startsWith(`${p}/`))) {
    return <PublicShell>{children}</PublicShell>;
  }
  return (
    <AuthGate>
      <Shell>{children}</Shell>
    </AuthGate>
  );
}
