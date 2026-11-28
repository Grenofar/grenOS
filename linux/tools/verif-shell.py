#!/usr/bin/env python3
"""Refuser un « \\n » littéral hors guillemets dans nos scripts shell.

POURQUOI CE FICHIER EXISTE — et c'est une panne, pas une précaution.

Le 14 octobre, la construction `38588801230` est morte sur cette ligne :

    grenos: ces icones manquent en 24x24 : n

Une icône nommée « n ». La cause est à deux caractères : une continuation de
ligne dont l'anti-slash avait été mangé, et qui est donc arrivée dans le
fichier sous la forme `\\n` littéral :

    for nom in grenos … grenos-theme \\n           grenos-hub …; do

Pour le shell, `\\n` hors guillemets vaut simplement `n`. La boucle a donc
cherché une icône appelée `n`, ne l'a pas trouvée, et a refusé l'image —
dix-huit minutes après le début de la construction.

`bash -n` ne voit rien : la ligne est du shell parfaitement valide. C'est
exactement pour cela qu'un garde est nécessaire ici et pas ailleurs.

**C'est la septième fois que ce piège mord ce dépôt**, toujours de la même
façon : une édition par heredoc avale les anti-slashs. La règle est écrite
depuis des semaines — « pour toute édition contenant des échappements :
l'outil Edit, ou un fichier fragment, jamais un heredoc » — et je l'ai violée
deux fois dans la même séance. Une règle qu'on relit ne vaut pas une machine
qui refuse.

CE QUE LE GARDE REGARDE, ET CE QU'IL NE REGARDE PAS

Un `\\n` **entre guillemets** est normal et fréquent : `printf '\\n'`,
`sed 's/\\n//'`, `tr -d '\\n'`. Il est donc ignoré, sinon cent faux noieraient
les vrais — la même leçon que `verif-accents.py`, qui ne lit que ce qui
s'affiche.

Hors guillemets, en revanche, `\\n` et `\\t` n'ont aucun usage légitime dans
nos fichiers : ce sont toujours les restes d'une continuation de ligne
abîmée. Un anti-slash en **fin** de ligne, lui, est une vraie continuation et
reste parfaitement permis.

    python3 linux/tools/verif-shell.py [fichiers…]
"""
import os
import sys

RACINE = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

#: Là où vivent nos scripts shell. Les programmes de `usr/bin` sont en Python,
#: sauf quelques-uns : on les reconnaît à leur première ligne.
DOSSIERS = (
    "linux/config/hooks/normal",
    "linux/tools",
    "scripts",
    "linux/config/includes.chroot/usr/bin",
    "linux/config/includes.chroot/usr/lib/grenos",
)

SUFFIXES = (".sh", ".hook.chroot")
ENTETES = ("#!/bin/sh", "#!/bin/bash", "#!/usr/bin/env sh",
           "#!/usr/bin/env bash")

MAUVAIS = ("n", "t")


def est_du_shell(chemin):
    """Un fichier de shell, par son suffixe ou par sa première ligne."""
    if chemin.endswith(SUFFIXES):
        return True
    try:
        with open(chemin, encoding="utf-8", errors="replace") as fichier:
            premiere = fichier.readline()
    except OSError:
        return False
    return premiere.startswith(ENTETES)


def fautes_de_la_ligne(ligne):
    """Les colonnes où un échappement traîne hors de tout guillemet.

    L'analyse est volontairement simple — une ligne à la fois, sans suivre les
    chaînes sur plusieurs lignes. Elle suffit pour ce qu'on cherche, et une
    analyse compliquée qu'on ne comprend plus est pire qu'une analyse simple
    dont on connaît les limites.
    """
    trouvees = []
    citation = None          # None, "'" ou '"'
    # LA SUBSTITUTION DE COMMANDE OUVRE UN CONTEXTE NEUF, Y COMPRIS AU MILIEU
    # D'UNE CHAÎNE. C'est ce que la première version ignorait, et elle a
    # aussitôt accusé deux lignes parfaitement correctes de la sonde :
    #
    #     dire "maj: depot INJOIGNABLE : $(tr -d "\n" < /tmp/grenos-apt)"
    #
    # Elle lisait le guillemet de `tr -d "` comme la FERMETURE de celui de
    # `dire "`, et voyait donc le `\n` « hors guillemets ». C'est exactement la
    # faute de `verif-commandes.py` lisant `import sys` comme un programme : le
    # garde avait raison d'être strict, c'est sa lecture qui était fausse.
    #
    # Les contextes s'empilent donc. Les apostrophes inverses ne sont pas
    # traitées : nous n'en écrivons aucune, et un analyseur compliqué qu'on ne
    # comprend plus est pire qu'un analyseur simple dont on connaît les bords.
    pile = []
    indice = 0
    while indice < len(ligne):
        caractere = ligne[indice]

        # `$(` ouvre une substitution — même à l'intérieur de guillemets
        # doubles, où elle est parfaitement licite. Entre apostrophes, non :
        # là, rien n'est interprété.
        if citation != "'" and ligne.startswith("$(", indice):
            pile.append(citation)
            citation = None
            indice += 2
            continue
        if caractere == ")" and pile and citation is None:
            citation = pile.pop()
            indice += 1
            continue

        if citation == "'":
            # Entre apostrophes, RIEN n'est échappé : même l'anti-slash est
            # un caractère ordinaire. On avance jusqu'à l'apostrophe fermante.
            if caractere == "'":
                citation = None
            indice += 1
            continue

        if citation == '"':
            if caractere == "\\":
                indice += 2      # l'échappement est légitime ici
                continue
            if caractere == '"':
                citation = None
            indice += 1
            continue

        # Hors guillemets.
        if caractere == "#" and (indice == 0 or ligne[indice - 1].isspace()):
            break                # un commentaire : plus rien à juger
        if caractere in "'\"":
            citation = caractere
            indice += 1
            continue
        if caractere == "\\":
            suivant = ligne[indice + 1] if indice + 1 < len(ligne) else ""
            if suivant == "":
                break            # anti-slash en fin de ligne : continuation
            if suivant in MAUVAIS:
                trouvees.append((indice + 1, "\\" + suivant))
            indice += 2
            continue
        indice += 1
    return trouvees


def lire(chemin):
    try:
        with open(chemin, encoding="utf-8", errors="replace") as fichier:
            return fichier.read().splitlines()
    except OSError:
        return []


def juger(chemins):
    """Rend la liste des fautes : (fichier, ligne, colonne, texte)."""
    fautes = []
    for chemin in chemins:
        for numero, ligne in enumerate(lire(chemin), 1):
            for colonne, texte in fautes_de_la_ligne(ligne):
                fautes.append((chemin, numero, colonne, texte))
    return fautes


def rassembler():
    chemins = []
    for dossier in DOSSIERS:
        complet = os.path.join(RACINE, dossier)
        if not os.path.isdir(complet):
            continue
        for nom in sorted(os.listdir(complet)):
            chemin = os.path.join(complet, nom)
            if os.path.isfile(chemin) and est_du_shell(chemin):
                chemins.append(chemin)
    return chemins


def sans_droit_d_executer():
    """Les scripts (#!) de usr/lib/grenos et usr/bin que Git garde en 100644.

    23 novembre : grenos-pilote-signe, écrit sous Windows, est arrivé dans
    l'image sans son bit d'exécution — « Permission denied » au clic de
    Grenofar sur « Préparer le matériel ». L'essai en CI ne l'avait pas vu :
    il faisait lui-même chmod. Les .py se lancent par python3, pas par eux-mêmes.
    """
    import subprocess
    try:
        sortie = subprocess.run(
            ["git", "ls-files", "-s", "--", "linux/config/includes.chroot/usr/lib/grenos",
             "linux/config/includes.chroot/usr/bin"],
            cwd=RACINE, capture_output=True, text=True, timeout=30).stdout
    except (OSError, subprocess.SubprocessError):
        return []
    fautes = []
    for ligne in sortie.splitlines():
        mode, _, chemin = ligne.split(None, 2)
        chemin = chemin.split("\t", 1)[-1]
        if mode != "100644" or chemin.endswith(".py"):
            continue
        try:
            with open(os.path.join(RACINE, chemin), "rb") as f:
                if f.read(2) == b"#!":
                    fautes.append(chemin)
        except OSError:
            pass
    return fautes


def detect_virt_avec_repli(chemins):
    """Les lignes « $(systemd-detect-virt … || echo none) » : (fichier, ligne).

    systemd-detect-virt écrit « none » ET sort en 1 sur une vraie machine ; le
    repli en ajoute un second, et le résultat n'est JAMAIS « none ». Payé deux
    fois : picom éteint sur tout vrai PC (25 novembre), puis le 28 tout vrai PC
    pris pour une machine virtuelle dans grenos-session — 181 ms de réserve sur
    le son, la molette sans défilement fluide. Lire `$?` ou `|| true`, jamais
    `|| echo`.
    """
    fautes = []
    for chemin in chemins:
        for numero, ligne in enumerate(lire(chemin), 1):
            code = ligne.split("#", 1)[0]
            if "systemd-detect-virt" in code and "|| echo" in code:
                fautes.append((chemin, numero))
    return fautes


def syntaxe_cassee(chemins):
    """Les scripts que leur propre shell refuse de lire : (fichier, message).

    28 novembre : grenos-maj-verif sortait en 2 depuis le 24 septembre,
    « Syntax error: end of file unexpected ». Un commentaire coupé en deux par
    un vrai retour à la ligne avait laissé sa seconde moitié sans « # », avec
    un accent grave jamais refermé. Le minuteur le lançait chaque jour, il
    mourait avant d'écrire quoi que ce soit, et personne ne voyait rien : ni le
    bouton « N mises à jour », ni le mode automatique n'ont jamais rien
    compté. `sh -n` le dit en une ligne ; il n'était lancé sur aucun script.

    Chaque script est lu par le shell de sa première ligne (dash pour
    #!/bin/sh, comme sur la machine), un .sh sans en-tête par sh.
    """
    import subprocess
    fautes = []
    for chemin in chemins:
        try:
            with open(chemin, encoding="utf-8", errors="replace") as fichier:
                premiere = fichier.readline()
        except OSError:
            continue
        shell = "bash" if "bash" in premiere.split("\n")[0] else "sh"
        try:
            sortie = subprocess.run([shell, "-n", chemin], capture_output=True,
                                    text=True, timeout=30)
        except (OSError, subprocess.SubprocessError) as souci:
            fautes.append((chemin, str(souci)))
            continue
        if sortie.returncode != 0:
            fautes.append((chemin, (sortie.stderr or sortie.stdout).strip()))
    return fautes


def main():
    non_executables = [] if sys.argv[1:] else sans_droit_d_executer()
    for chemin in non_executables:
        print("%s : script sans droit d'execution dans Git (100644) — "
              "« Permission denied » sur la machine. git update-index --chmod=+x"
              % chemin, file=sys.stderr)
    if non_executables:
        return 1
    chemins = sys.argv[1:] or rassembler()
    illisibles = syntaxe_cassee(chemins)
    for chemin, message in illisibles:
        relatif = os.path.relpath(chemin, RACINE).replace(os.sep, "/")
        print("%s : le shell refuse de le lire — %s" % (relatif, message),
              file=sys.stderr)
    if illisibles:
        print("shell : %d script(s) qui ne demarreront jamais (sh -n / bash -n)"
              % len(illisibles), file=sys.stderr)
        return 1
    replis = detect_virt_avec_repli(chemins)
    for chemin, numero in replis:
        relatif = os.path.relpath(chemin, RACINE).replace(os.sep, "/")
        print("%s:%d: « systemd-detect-virt || echo » — sur un vrai PC il "
              "ecrit deja « none » et sort en 1 : le repli en ajoute un second, "
              "et la machine passe pour virtuelle" % (relatif, numero), file=sys.stderr)
    if replis:
        return 1
    fautes = juger(chemins)
    for chemin, numero, colonne, texte in fautes:
        relatif = os.path.relpath(chemin, RACINE).replace(os.sep, "/")
        print("%s:%d:%d: « %s » hors guillemets — une continuation de ligne "
              "abimee ?" % (relatif, numero, colonne, texte), file=sys.stderr)
    if fautes:
        print("shell : %d echappement(s) qui ne veulent rien dire. Le shell "
              "les lit comme une lettre ordinaire, et la ligne fait autre "
              "chose que ce qu'elle a l'air de faire." % len(fautes),
              file=sys.stderr)
        return 1
    print("shell : %d fichier(s) lus, aucun echappement egare hors guillemets"
          % len(chemins))
    return 0


if __name__ == "__main__":
    sys.exit(main())
