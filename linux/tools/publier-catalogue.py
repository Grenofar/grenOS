#!/usr/bin/env python3
"""Publie le catalogue de GrenPlace dans notre backend.

Le magasin de grenOS ne connaît pas la liste des applications : il la demande.
Ce script dépose `linux/data/catalogue.json` dans le bucket public
`catalogue` de Supabase, à l'adresse que les machines interrogent. Changer le
catalogue ne demande donc ni nouvelle image, ni mise à jour du système : la
fiche change, et tout le monde la voit.

Ce qui est publié n'est qu'une fiche par application — nom, description,
adresse du logo, et d'où elle s'installe vraiment (Flathub ou les dépôts
Debian). Aucune application n'est stockée chez nous, et aucun logo non plus :
seulement son adresse.

    python3 linux/tools/publier-catalogue.py
"""
import json
import os
import sys
import urllib.error
import urllib.request

BUCKET = "catalogue"
OBJET = "magasin.json"
ICI = os.path.dirname(os.path.abspath(__file__))
SOURCE = os.path.join(ICI, "..", "data", "catalogue.json")


def secrets():
    """L'adresse du projet et la clé de service, jamais affichées."""
    url = os.environ.get("SUPABASE_URL", "")
    cle = os.environ.get("SUPABASE_SERVICE_ROLE_KEY", "")
    if not url or not cle:
        racine = os.path.join(ICI, "..", "..", ".env.local")
        if os.path.exists(racine):
            for ligne in open(racine, encoding="utf-8"):
                if ligne.startswith("SUPABASE_URL=") and not url:
                    url = ligne.split("=", 1)[1].strip()
                elif ligne.startswith("SUPABASE_SERVICE_ROLE_KEY=") and not cle:
                    cle = ligne.split("=", 1)[1].strip()
    if not url or not cle:
        sys.exit("Il manque SUPABASE_URL ou SUPABASE_SERVICE_ROLE_KEY.")
    return url.rstrip("/"), cle


def appeler(methode, adresse, cle, corps=None, entetes=None):
    requete = urllib.request.Request(adresse, method=methode, data=corps)
    requete.add_header("Authorization", f"Bearer {cle}")
    requete.add_header("apikey", cle)
    for nom, valeur in (entetes or {}).items():
        requete.add_header(nom, valeur)
    try:
        with urllib.request.urlopen(requete, timeout=60) as reponse:
            return reponse.status, reponse.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as souci:
        return souci.code, souci.read().decode("utf-8", "replace")


def main():
    url, cle = secrets()

    with open(SOURCE, encoding="utf-8") as fichier:
        catalogue = json.load(fichier)
    applications = catalogue.get("applications", [])
    print(f"{len(applications)} applications dans le catalogue")

    # Un catalogue mal formé mettrait un magasin vide entre les mains de tout
    # le monde : on vérifie chaque fiche avant de publier.
    vus = set()
    for application in applications:
        for champ in ("slug", "nom", "rayon", "description", "source", "identifiant"):
            if not application.get(champ):
                sys.exit(f"Fiche incomplète : {application.get('slug', '?')} sans {champ}")
        if application["source"] not in ("flatpak", "apt"):
            sys.exit(f"Source inconnue pour {application['slug']} : {application['source']}")
        if application["slug"] in vus:
            sys.exit(f"Deux fiches portent le même nom court : {application['slug']}")
        vus.add(application["slug"])

    # Le bucket, créé une seule fois, public en lecture.
    statut, _ = appeler("POST", f"{url}/storage/v1/bucket", cle,
                        json.dumps({"id": BUCKET, "name": BUCKET, "public": True}).encode(),
                        {"Content-Type": "application/json"})
    print("bucket :", "créé" if statut == 200 else f"déjà là ({statut})")

    corps = json.dumps(catalogue, ensure_ascii=False, indent=1).encode("utf-8")
    statut, reponse = appeler("POST", f"{url}/storage/v1/object/{BUCKET}/{OBJET}", cle, corps,
                              {"Content-Type": "application/json",
                               "Cache-Control": "max-age=300",
                               "x-upsert": "true"})
    if statut not in (200, 201):
        sys.exit(f"Dépôt refusé ({statut}) : {reponse}")

    public = f"{url}/storage/v1/object/public/{BUCKET}/{OBJET}"
    print("publié :", public)

    # On relit ce qu'on vient de publier : une publication qu'on ne vérifie pas
    # n'est pas une publication.
    with urllib.request.urlopen(public, timeout=30) as reponse:
        relu = json.loads(reponse.read().decode("utf-8"))
    print(f"relu depuis l'adresse publique : {len(relu.get('applications', []))} applications")
    return 0


if __name__ == "__main__":
    sys.exit(main())
