#!/usr/bin/env python3
"""Vérifier une proposition grenget (une issue du formulaire grenget.yml).

    python3 scripts/grenget-verifier.py verifier corps.md   → rapport Markdown, code 0 si valable
    python3 scripts/grenget-verifier.py fiche corps.md      → la fiche JSON du catalogue

Ce que la vérification automatique contrôle, et seulement cela :
  - les champs obligatoires sont là, l'identifiant a une forme saine (la même
    règle que GrenPlace et grenget : il ne doit jamais pouvoir passer pour
    une option d'apt ou de flatpak) ;
  - l'application n'est pas déjà au catalogue ;
  - elle EXISTE à la source : Flathub (avec son icône 128 px, que GrenPlace
    affiche) ou les paquets Debian trixie.

Ce qu'elle ne juge pas : si l'application a sa place dans grenOS. Ça, c'est
la relecture humaine, et le label « acceptee » qui la conclut.
"""
import json
import os
import re
import sys
import urllib.error
import urllib.request

RACINE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CATALOGUE = os.path.join(RACINE, "linux", "data", "catalogue.json")
RAYONS = ("Bureautique", "Création", "Développement", "Intelligence artificielle", "Internet", "Jeux", "Système")
IDENTIFIANT_SAIN = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._+-]*$")
ICONES = "https://dl.flathub.org/repo/appstream/x86_64/icons/128x128/"

CHAMPS = {
    "Nom de l'application": "nom",
    "Source": "source",
    "Identifiant": "identifiant",
    "Rayon": "rayon",
    "Description (en français, une phrase)": "description",
    "Site officiel": "site",
}


def lire_formulaire(corps):
    """Les issues de formulaire s'écrivent « ### Libellé » puis la valeur."""
    valeurs, cle = {}, None
    for ligne in corps.replace("\r", "").split("\n"):
        if ligne.startswith("### "):
            cle = CHAMPS.get(ligne[4:].strip())
            if cle:
                valeurs[cle] = ""
            continue
        if cle is not None:
            valeurs[cle] = (valeurs[cle] + "\n" + ligne).strip()
    return {k: ("" if v == "_No response_" else v) for k, v in valeurs.items()}


def http(url, methode="GET"):
    try:
        requete = urllib.request.Request(url, method=methode, headers={"User-Agent": "grenget-verifier"})
        with urllib.request.urlopen(requete, timeout=25) as r:
            return r.status, (r.read().decode("utf-8", "replace") if methode == "GET" else "")
    except urllib.error.HTTPError as e:
        return e.code, ""
    except Exception:  # noqa: BLE001 — réseau
        return 0, ""


def verifier(v):
    """(valable, lignes du rapport, données Flathub éventuelles)."""
    rapport, bon, flathub = [], True, {}

    def faute(texte):
        nonlocal bon
        bon = False
        rapport.append(f"- ❌ {texte}")

    for cle in ("nom", "source", "identifiant", "rayon", "description"):
        if not v.get(cle):
            faute(f"le champ « {cle} » est vide")
    ident = v.get("identifiant", "")
    if ident and not IDENTIFIANT_SAIN.match(ident):
        faute(f"l'identifiant `{ident}` contient des caractères refusés")
    if v.get("source") not in ("flatpak", "apt"):
        faute("la source doit être flatpak ou apt")
    if v.get("rayon") and v["rayon"] not in RAYONS:
        faute(f"le rayon « {v['rayon']} » n'existe pas")
    if len(v.get("description", "")) > 160:
        faute("la description dépasse 160 caractères : une phrase suffit")

    try:
        fiches = json.load(open(CATALOGUE, encoding="utf-8"))["applications"]
    except (OSError, ValueError):
        fiches = []
    if ident and any(f["identifiant"] == ident for f in fiches):
        faute(f"`{ident}` est déjà au catalogue")

    if bon and v["source"] == "flatpak":
        code, texte = http(f"https://flathub.org/api/v2/appstream/{ident}")
        if code != 200 or not texte.strip() or texte.strip() == "null":
            faute(f"`{ident}` est introuvable sur Flathub")
        else:
            flathub = json.loads(texte)
            rapport.append(f"- ✅ existe sur Flathub : **{flathub.get('name', ident)}** ({flathub.get('developer_name') or 'éditeur inconnu'})")
            if http(ICONES + ident + ".png", "HEAD")[0] == 200:
                rapport.append("- ✅ icône 128 px disponible (GrenPlace l'affichera)")
            else:
                faute("pas d'icône 128 px sur Flathub : la tuile de GrenPlace serait vide")
    elif bon and v["source"] == "apt":
        code, texte = http(f"https://qa.debian.org/madison.php?package={ident}&s=trixie&text=on")
        if code == 200 and ident in texte:
            rapport.append(f"- ✅ paquet présent dans Debian trixie : `{texte.split('|')[1].strip() if '|' in texte else ident}`")
        else:
            faute(f"le paquet `{ident}` est introuvable dans Debian trixie")
    if bon:
        rapport.insert(0, "- ✅ formulaire complet, identifiant sain, pas de doublon")
    return bon, rapport, flathub


def fiche(v, flathub):
    ident = v["identifiant"]
    slug = re.sub(r"[^a-z0-9]+", "-", v["nom"].lower()).strip("-") or ident.split(".")[-1].lower()
    return {
        "slug": slug, "nom": v["nom"], "rayon": v["rayon"],
        "editeur": (flathub.get("developer_name") if flathub else "") or v["nom"],
        "description": v["description"].replace("\n", " "),
        "logo": (ICONES + ident + ".png") if v["source"] == "flatpak" else "",
        "source": v["source"], "identifiant": ident,
        "site": v.get("site") or ((flathub.get("urls") or {}).get("homepage", "") if flathub else "")
                or (f"https://flathub.org/apps/{ident}" if v["source"] == "flatpak" else f"https://packages.debian.org/trixie/{ident}"),
    }


def main():
    action, chemin = sys.argv[1], sys.argv[2]
    v = lire_formulaire(open(chemin, encoding="utf-8").read())
    bon, rapport, flathub = verifier(v)
    if action == "verifier":
        print("### Vérification automatique\n")
        print("\n".join(rapport))
        print("\n" + ("**Valable** : la proposition attend maintenant sa relecture. 🌱" if bon
                      else "**À corriger** : modifie la proposition, la vérification repassera toute seule."))
        return 0 if bon else 1
    if action == "fiche":
        if not bon:
            return 1
        print(json.dumps(fiche(v, flathub), ensure_ascii=False))
        return 0
    return 2


if __name__ == "__main__":
    sys.exit(main())
