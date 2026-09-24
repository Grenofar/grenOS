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
import concurrent.futures
import gzip
import json
import os
import re
import sys
import urllib.error
import urllib.request

# Les index de Debian, pour savoir si un paquet existe vraiment.
DEBIAN = "http://deb.debian.org/debian/dists/trixie"
SECTIONS = ("main", "contrib", "non-free")

BUCKET = "catalogue"
OBJET = "magasin.json"
ICI = os.path.dirname(os.path.abspath(__file__))
SOURCE = os.path.join(ICI, "..", "data", "catalogue.json")

# Les applications qu'on a retirées exprès. Sans cette liste, le contrôle
# ci-dessous refuserait à jamais toute suppression volontaire — il doit
# distinguer « on l'a enlevée » de « on l'a perdue ».
#
# Celles-ci viennent de la tâche 69c84239, publiée par accident depuis une
# branche le 24 septembre au soir. Six sont des outils en ligne de commande, et
# GrenPlace est une boutique à boutons « Installer » : cliquer sur `tree` ne
# montre rien. Les autres font double emploi ou ne servent plus guère.
RETIRES = {
    "gnome-calculator", "cheese", "xsane", "hardinfo",
    "inxi", "lm-sensors", "mc", "ncdu", "rsync", "tree",
}


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


def paquets_de_trixie():
    """Les noms de paquets que Debian publie vraiment, ou None si injoignable.

    Chaque index fait une vingtaine de mégaoctets : une coupure en cours de
    route arrive, et elle ne dit rien du catalogue. On réessaie trois fois,
    puis on rend None — « je n'ai pas pu vérifier » n'est pas « c'est faux »,
    et confondre les deux ferait rougir l'étape pour la panne de quelqu'un
    d'autre.
    """
    noms = set()
    for section in SECTIONS:
        adresse = f"{DEBIAN}/{section}/binary-amd64/Packages.gz"
        for essai in range(3):
            try:
                with urllib.request.urlopen(adresse, timeout=120) as reponse:
                    index = gzip.decompress(reponse.read()).decode("utf-8", "replace")
                break
            except (urllib.error.URLError, OSError, TimeoutError, EOFError) as souci:
                print(f"  {section} : {souci} (essai {essai + 1}/3)")
                index = None
        if index is None:
            return None
        noms.update(m.group(1) for m in re.finditer(r"^Package: (.*)$", index, re.M))
    return noms


def logo_repond(application):
    """Cette adresse de logo rend-elle bien une image ?

    Trois reponses : oui, non, ou muette. « Non » veut dire que l'adresse est
    fausse — 404 ou 410, il n'y a rien la-bas, et c'est notre faute. Tout le
    reste (403, 429, une panne, une coupure) est leur affaire ou celle du
    reseau : cela ne doit pas faire echouer une publication, sinon une
    limitation de debit chez Flathub nous priverait d'un catalogue neuf.
    """
    adresse = application.get("logo") or ""
    if not adresse:
        return application["slug"], "absent"
    requete = urllib.request.Request(adresse, method="HEAD",
                                     headers={"User-Agent": "grenOS"})
    try:
        with urllib.request.urlopen(requete, timeout=25) as reponse:
            return application["slug"], ("oui" if reponse.status == 200
                                         else f"muet ({reponse.status})")
    except urllib.error.HTTPError as souci:
        if souci.code in (404, 410):
            return application["slug"], f"non ({souci.code})"
        return application["slug"], f"muet ({souci.code})"
    except (urllib.error.URLError, OSError, TimeoutError):
        return application["slug"], "muet"


def rien_n_a_disparu(applications, url):
    """Le catalogue a-t-il perdu une application en route ?

    Le 24 septembre, un Coder a **remplacé** les 29 fiches par 17 des siennes,
    alors que sa tâche disait de ne rien enlever. La CI est passée au vert :
    le fichier se lisait, et ses paquets existaient. Rien ne regardait ce qui
    avait disparu.

    On compare donc à ce qui est en ligne. Une boutique ne perd pas ses rayons
    par accident ; si un retrait est voulu, il s'écrit dans RETIRES, et cela se
    voit dans le diff.
    """
    public = f"{url}/storage/v1/object/public/{BUCKET}/{OBJET}"
    try:
        with urllib.request.urlopen(public + "?frais=1", timeout=45) as reponse:
            enligne = json.loads(reponse.read().decode("utf-8"))
    except (urllib.error.URLError, OSError, ValueError, TimeoutError) as souci:
        print(f"catalogue en ligne injoignable ({souci}) : comparaison sautee")
        return

    avant = {a.get("slug") for a in enligne.get("applications", []) if a.get("slug")}
    apres = {a["slug"] for a in applications}
    perdues = sorted(avant - apres - RETIRES)
    if perdues:
        sys.exit("Ces applications etaient en ligne et ne sont plus dans le "
                 "catalogue : " + ", ".join(perdues) + ". Si le retrait est "
                 "voulu, ajoute-les a RETIRES dans ce fichier.")
    print(f"aucune application perdue ({len(avant)} en ligne, {len(apres)} ici)")


def verifier_aux_sources(applications):
    """Chaque fiche tient-elle devant la source qu'elle designe ?"""
    voulus = sorted({a["identifiant"] for a in applications if a["source"] == "apt"})
    if voulus:
        reels = paquets_de_trixie()
        if reels is None:
            print("paquets Debian : les index sont injoignables, controle saute "
                  "(ce n'est pas un defaut du catalogue)")
        else:
            absents = [nom for nom in voulus if nom not in reels]
            print(f"paquets Debian : {len(voulus) - len(absents)}/{len(voulus)} "
                  f"existent dans trixie")
            if absents:
                sys.exit("Ces paquets n'existent pas dans trixie, "
                         "le bouton Installer echouerait : " + ", ".join(absents))

    with concurrent.futures.ThreadPoolExecutor(8) as reunion:
        reponses = sorted(reunion.map(logo_repond, applications))
    faux = [slug for slug, etat in reponses if etat.startswith("non")]
    muets = [f"{slug} {etat[4:]}".strip() for slug, etat in reponses
             if etat.startswith("muet")]
    absents = [slug for slug, etat in reponses if etat == "absent"]
    bons = sum(1 for _, etat in reponses if etat == "oui")
    print(f"logos : {bons}/{len(reponses)} repondent")
    for titre, liste in (("sans logo", absents), ("injoignables", muets)):
        if liste:
            print(f"  {titre} (tuile dessinee a la place) : {', '.join(liste)}")
    if faux:
        sys.exit("Ces adresses de logo ne menent nulle part : " + ", ".join(faux))


def main():
    # `--verifier` contrôle sans publier, et sans avoir besoin d'aucune clé.
    #
    # Le magasin des vraies machines n'appartient pas à une branche. Le
    # 24 septembre à 19h38, la construction d'une branche d'agent a remplacé
    # nos 29 applications par 17 autres, que personne n'avait relues. Vérifier
    # depuis une branche est utile ; publier ne l'est pas.
    verifier_seulement = "--verifier" in sys.argv[1:]
    url = cle = ""
    if not verifier_seulement:
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
        cle_source = (application["source"], application["identifiant"])
        if cle_source in vus:
            sys.exit(f"Deux fiches installent la même chose : {application['identifiant']}")
        vus.add(cle_source)

    # Puis on interroge les sources elles-mêmes. Une fiche peut être bien
    # formée et pourtant désigner un paquet qui n'existe pas.
    verifier_aux_sources(applications)
    rien_n_a_disparu(applications, url or
                     os.environ.get("SUPABASE_URL", "").rstrip("/") or
                     "https://tpqzhzuoyqpfairatdrw.supabase.co")

    if verifier_seulement:
        print("catalogue verifie, rien n'a ete publie (ce n'est pas la branche main)")
        return 0

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
