#!/usr/bin/env python3
"""grenos-steam-shaders, sans Steam (28 novembre).

    python3 linux/tools/essai-steam-shaders.py

« Processing Vulkan shaders (0%) » : Steam rejouait 1,8 million de pipelines
de BeamNG.drive avant chaque lancement, 15 fils à 90 %. Le veilleur coupe le
pré-cache et la compilation en fond dans config.vdf, Steam fermé. Ici : la
lecture du format de Steam (avec un extrait du vrai fichier de Grenofar), le
réglage à la bonne place et rien d'autre, et une passe complète — rien tant
que Steam tourne, une fois seulement, jamais remis si la personne rallume le
pré-cache, rien sur un fichier illisible.
"""
import importlib.machinery
import importlib.util
import os
import sys
import tempfile

ICI = os.path.dirname(os.path.abspath(__file__))
CHEMIN = os.path.join(ICI, "../config/includes.chroot/usr/lib/grenos/grenos-steam-shaders")
chargeur = importlib.machinery.SourceFileLoader("grenos_steam_shaders", CHEMIN)
spec = importlib.util.spec_from_loader("grenos_steam_shaders", chargeur)
m = importlib.util.module_from_spec(spec)
chargeur.exec_module(m)

FAUTES = 0
CAS = 0


def verifier(vrai, texte):
    global FAUTES, CAS
    CAS += 1
    if vrai:
        print(f"  [ok] {texte}")
    else:
        print(f"  [FAUTE] {texte}")
        FAUTES += 1


# Extrait du config.vdf de Grenofar, le 28 novembre, tel que Steam l'écrit :
# une valeur sur plusieurs lignes, des \\ et des \" dans les valeurs.
VRAI = '''"InstallConfigStore"
{
\t"Software"
\t{
\t\t"Valve"
\t\t{
\t\t\t"Steam"
\t\t\t{
\t\t\t\t"AutoUpdateWindowEnabled"\t\t"0"
\t\t\t\t"ipv6check_http_state"\t\t"good"
\t\t\t\t"ShaderCacheManager"
\t\t\t\t{
\t\t\t\t\t"HasCurrentBucket"\t\t"1"
\t\t\t\t\t"ProcessingQueue"\t\t"552990;284160;"
\t\t\t\t\t"App"
\t\t\t\t\t{
\t\t\t\t\t\t"284160"
\t\t\t\t\t\t{
\t\t\t\t\t\t\t"ShaderCacheSize"\t\t"5687827778"
\t\t\t\t\t\t}
\t\t\t\t\t}
\t\t\t\t\t"CommittedDepotManifests"
\t\t\t\t\t{
\t\t\t\t\t\t"284160"\t\t"1547454_6475200005299449441_5685669642_fozpipelinesv6\\\\steam_pipeline_cache.foz"
\t\t\t\t\t}
\t\t\t\t}
\t\t\t\t"ipv6check_udp_state"\t\t"good"
\t\t\t\t"Tools"
\t\t\t\t{
\t\t\t\t\t"284160"
\t\t\t\t\t{
\t\t\t\t\t\t"SizeOnDisk"\t\t"0"
\t\t\t\t\t}
\t\t\t\t}
\t\t\t}
\t\t}
\t}
\t"SDL_GamepadBind"\t\t"03000000de280000ff11000001000000,Steam Virtual Gamepad,a:b0,
030000005e040000120b000017050000,Xbox Series X Controller,a:b0,b:b1,
"
\t"WebStorage"
\t{
\t\t"continuously_render_store"\t\t"false"
\t\t"DownloadsStoreRecentlyCompleted"\t\t"{\\"version\\":2,\\"data\\":[{\\"appid\\":228980}]}"
\t}
}
'''
LIGNES = '\t\t\t\t\t"DisableShaderCache"\t\t"1"\n\t\t\t\t\t"EnableShaderBackgroundProcessing"\t\t"0"\n'


def valeur(texte, nom):
    _, valeurs = m.lire_vdf(texte)
    place = valeurs.get(m.BLOC + (nom.lower(),))
    return texte[place[0]:place[1]] if place else None


print("steam shaders :")
blocs, valeurs = m.lire_vdf(VRAI)
verifier(m.STEAM_BLOC in blocs and m.BLOC in blocs, "les blocs Steam et ShaderCacheManager trouvés")
verifier(valeur(VRAI, "ProcessingQueue") == "552990;284160;", "une valeur du bloc, par son chemin")
verifier(valeur(VRAI, "processingqueue") == "552990;284160;", "les noms de Steam sans la casse")
verifier(("installconfigstore", "sdl_gamepadbind") in valeurs and ("installconfigstore", "webstorage") in blocs,
         "une valeur sur plusieurs lignes et des \\\" ne coupent pas la lecture")
verifier(valeur(VRAI, "DisableShaderCache") is None, "le vrai fichier : aucun des deux réglages (pré-cache allumé)")

nouveau, change = m.regler(VRAI)
verifier(change and valeur(nouveau, "DisableShaderCache") == "1"
         and valeur(nouveau, "EnableShaderBackgroundProcessing") == "0", "pré-cache et compilation en fond coupés")
verifier('"ShaderCacheManager"\n\t\t\t\t{\n' + LIGNES + '\t\t\t\t\t"HasCurrentBucket"' in nouveau,
         "en tête du bloc ShaderCacheManager, à la tabulation de Steam")
verifier(nouveau.replace(LIGNES, "", 1) == VRAI, "rien d'autre ne bouge")
verifier(m.regler(nouveau) == (nouveau, False), "une seconde fois : rien à changer")

allume = VRAI.replace('"HasCurrentBucket"\t\t"1"\n',
                      '"HasCurrentBucket"\t\t"1"\n\t\t\t\t\t"enableshaderbackgroundprocessing"\t\t"1"\n')
regle, change = m.regler(allume)
verifier(change and valeur(regle, "EnableShaderBackgroundProcessing") == "0"
         and regle.lower().count('"enableshaderbackgroundprocessing"') == 1,
         "une valeur déjà là (casse de Steam ou non) : changée, pas doublée")
verifier(valeur(regle, "DisableShaderCache") == "1", "et celle qui manque, ajoutée")

sans = VRAI[:VRAI.index('\t\t\t\t"ShaderCacheManager"')] + VRAI[VRAI.index('\t\t\t\t"ipv6check_udp_state"'):]
cree, change = m.regler(sans)
verifier(change and valeur(cree, "DisableShaderCache") == "1"
         and valeur(cree, "EnableShaderBackgroundProcessing") == "0", "sans bloc ShaderCacheManager (Steam neuf) : il est créé")
verifier('\t\t\t{\n\t\t\t\t"ShaderCacheManager"\n\t\t\t\t{\n\t\t\t\t\t"DisableShaderCache"' in cree,
         "dans le bloc Steam, à sa tabulation")

ailleurs = VRAI.replace('\t"WebStorage"\n\t{\n',
                        '\t"WebStorage"\n\t{\n\t\t"ShaderCacheManager"\n\t\t{\n\t\t\t"DisableShaderCache"\t\t"1"\n\t\t}\n')
verifier(m.regler(ailleurs)[1], "un ShaderCacheManager hors de Software › Valve › Steam ne compte pas")

for mauvais, quoi in (("", "vide"), ('"autre"\n{\n}\n', "un autre fichier"),
                      ('"InstallConfigStore"\n{\n\t"Software"\n\t{\n', "coupé"),
                      (VRAI.replace('"AutoUpdateWindowEnabled"', 'AutoUpdateWindowEnabled'), "un mot sans guillemets"),
                      (VRAI + "}\n", "une accolade en trop")):
    try:
        m.regler(mauvais)
        verifier(False, f"illisible ({quoi}) : refusé")
    except ValueError:
        verifier(True, f"illisible ({quoi}) : refusé")

with tempfile.TemporaryDirectory() as d:
    steam, etat = f"{d}/Steam", f"{d}/etat/steam-shaders"
    config = f"{steam}/{m.CONFIG}"
    appels = []

    def passe(tourne=False):
        def regarde():
            appels.append(1)
            return tourne
        return m.passe(steam=steam, etat=etat, tourne=regarde)

    verifier(passe() is None and not os.path.exists(etat), "Steam jamais ouvert (pas de config.vdf) : on attend, rien n'est noté")
    os.makedirs(f"{steam}/config")
    open(config, "w").write(VRAI)
    verifier(passe(tourne=True) is None and open(config).read() == VRAI and not os.path.exists(etat),
             "Steam ouvert : rien n'est écrit (il réécrirait config.vdf)")
    verifier(passe() == "écrit" and valeur(open(config).read(), "DisableShaderCache") == "1", "Steam fermé : écrit")
    verifier(open(f"{config}.avant-grenos").read() == VRAI, "l'ancien config.vdf gardé à côté")
    verifier(os.path.exists(etat), "noté comme fait")

    # La personne rallume le pré-cache : Steam réécrit config.vdf avec "0".
    rallume = VRAI.replace('"HasCurrentBucket"\t\t"1"\n',
                           '"HasCurrentBucket"\t\t"1"\n\t\t\t\t\t"DisableShaderCache"\t\t"0"\n')
    open(config, "w").write(rallume)
    verifier(passe() == "fait" and open(config).read() == rallume, "rallumé par la personne : jamais recoupé")

    # Déjà coupé dans Steam : noté, rien d'écrit, Steam pas même regardé.
    os.remove(etat)
    os.remove(f"{config}.avant-grenos")
    open(config, "w").write(nouveau)
    appels.clear()
    verifier(passe() == "déjà" and open(config).read() == nouveau and os.path.exists(etat)
             and not os.path.exists(f"{config}.avant-grenos") and not appels,
             "déjà coupé : noté, config.vdf n'est pas touché")

    # Un config.vdf illisible : on n'y touche pas, et on réessaiera.
    os.remove(etat)
    open(config, "w").write('"InstallConfigStore"\n{\n')
    try:
        passe()
        verifier(False, "config.vdf illisible : refusé")
    except ValueError:
        verifier(open(config).read() == '"InstallConfigStore"\n{\n' and not os.path.exists(etat),
                 "config.vdf illisible : rien n'est écrit ni noté")

if FAUTES:
    print(f"steam shaders : {FAUTES} faute(s) sur {CAS} cas")
    sys.exit(1)
print(f"steam shaders : {CAS} cas, 0 faute")
