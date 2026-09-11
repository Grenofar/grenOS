import type { Metadata } from "next";
import { LangProvider } from "@/lib/i18n";
import { Frame } from "@/components/Frame";
import "./globals.css";

export const metadata: Metadata = {
  title: "grenOS — control plane",
  description: "Poste de pilotage de l'équipe d'agents qui construit grenOS.",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="fr">
      <body>
        <LangProvider>
          <Frame>{children}</Frame>
        </LangProvider>
      </body>
    </html>
  );
}
