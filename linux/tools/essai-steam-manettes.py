#!/usr/bin/env python3
"""grenos-steam-manettes, sans X ni Steam (27 novembre).

    python3 linux/tools/essai-steam-manettes.py

Les jeux Steam ne recevaient pas la manette : Steam Flatpak ne rattache pas
la fenêtre active à son jeu et reste sur la configuration « Bureau ». Le
veilleur le fait à sa place. Ici, ce qui décide : l'id d'un jeu ou d'un
raccourci, le jeu d'un processus (environnement, reaper, parents), la fenêtre
(STEAM_GAME, processus, classe), ce qu'on écrit à Steam en entrant et en
sortant d'un jeu, et le tube de Steam (avec et sans Steam au bout).
"""
import importlib.machinery
import importlib.util
import os
import sys
import tempfile
import threading

ICI = os.path.dirname(os.path.abspath(__file__))
CHEMIN = os.path.join(ICI, "../config/includes.chroot/usr/lib/grenos/grenos-steam-manettes")
chargeur = importlib.machinery.SourceFileLoader("grenos_steam_manettes", CHEMIN)
spec = importlib.util.spec_from_loader("grenos_steam_manettes", chargeur)
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


print("steam manettes :")
verifier(m.id_de("767560") == 767560, "un jeu Steam : son appid")
verifier(m.id_de(str((3456789012 << 32) | 0x02000000)) == 3456789012, "un raccourci non-Steam : l'appid en haut de l'id de jeu")
verifier(m.id_de("0") is None and m.id_de("") is None and m.id_de("abc") is None, "0, vide, illisible : rien")

with tempfile.TemporaryDirectory() as proc:
    def processus(pid, ppid, env=b"", cmd=b"x"):
        os.makedirs(f"{proc}/{pid}")
        open(f"{proc}/{pid}/environ", "wb").write(env)
        open(f"{proc}/{pid}/cmdline", "wb").write(cmd)
        open(f"{proc}/{pid}/stat", "w").write(f"{pid} (nom (avec) parenthèses) S {ppid} 0 0")
    processus(10, 1, cmd=b"reaper\0SteamLaunch\0AppId=767560\0--\0proton")
    processus(20, 10, env=b"HOME=/h\0SteamClientLaunch=1\0SteamGameId=767560\0SteamAppId=767560\0SteamOverlayGameId=767560\0")
    processus(30, 20, env=b"HOME=/h\0")                       # un enfant qui a perdu l'environnement
    processus(40, 1, env=b"HOME=/h\0DISPLAY=:0\0")             # Claude, le bureau
    processus(50, 1, env=b"SteamClientLaunch=1\0SteamAppId=0\0SteamGameId=" + str((123 << 32) | 0x02000000).encode() + b"\0")
    # Rocket League par Heroic (umu) : SteamGameId posé, mais pas par Steam.
    processus(60, 1, env=b"UMU_ID=umu-252950\0STORE=egs\0SteamAppId=252950\0SteamGameId=252950\0STEAM_COMPAT_APP_ID=252950\0")
    verifier(m.jeu_du_processus(20, proc) == 767560, "le jeu (wine) : SteamGameId de son environnement")
    verifier(m.jeu_du_processus(30, proc) == 767560, "un enfant sans l'environnement : ses parents jusqu'au reaper")
    verifier(m.jeu_du_processus(40, proc) is None, "une application du bureau : pas un jeu")
    verifier(m.jeu_du_processus(50, proc) == 123, "un raccourci non-Steam (SteamAppId=0) : son id de jeu")
    verifier(m.jeu_du_processus(99, proc) is None, "un processus disparu : rien, sans erreur")
    verifier(m.jeu_du_processus(60, proc) is None, "Rocket League lancé par Heroic (umu) : pas un jeu de Steam, rien imposé")

verifier(m.cible(767560, 767560) == 767560, "War Robots : STEAM_GAME")
verifier(m.cible(0, 767560) == 767560, "un jeu de Steam sans overlay : son processus")
verifier(m.cible(0, None) is None, "Rocket League par Heroic (classe steam_app_252950, pas de STEAM_GAME) : rien")
verifier(m.cible(769, None) == m.STEAM_UI, "la Big Picture : 769")
verifier(m.cible(m.BUREAU, None) is None, "STEAM_GAME 413080 (le Bureau) : pas un jeu")

e = m.Etat()
verifier(e.changer(None) == [], "au démarrage sur le bureau : rien à dire à Steam")
verifier(e.changer(767560) == [767560], "le jeu au premier plan : Steam Input le suit")
verifier(e.changer(767560) == [], "encore le jeu (STEAM_GAME posé après coup) : rien de plus")
verifier(e.changer(252950) == [252950], "un autre jeu : il le suit")
verifier(e.changer(None) == [m.BUREAU, 0], "on quitte le jeu : le Bureau tout de suite, puis la main rendue")
verifier(e.changer(None) == [], "le bureau encore : rien")
verifier(e.changer(m.STEAM_UI) == [m.STEAM_UI] and e.changer(None) == [m.BUREAU, 0], "la Big Picture, puis le bureau")

with tempfile.TemporaryDirectory() as d:
    tube = os.path.join(d, "steam.pipe")
    os.mkfifo(tube)
    verifier(m.envoyer(767560, (tube,)) is False, "Steam pas lancé (personne au bout du tube) : faux, sans bloquer")
    recu = []
    lecteur = os.open(tube, os.O_RDONLY | os.O_NONBLOCK)
    verifier(m.envoyer(767560, (os.path.join(d, "absent"), tube)), "Steam lancé : écrit (le premier tube absent est sauté)")
    recu.append(os.read(lecteur, 4096))
    os.close(lecteur)
    verifier(recu[0] == b"'steam' 'steam://forceinputappid/767560'\n",
             "la ligne exacte qu'un second « steam » écrit (ExecCommandLine de Steam)")

if FAUTES:
    print(f"steam manettes : {FAUTES} faute(s) sur {CAS} cas")
    sys.exit(1)
print(f"steam manettes : {CAS} cas, 0 faute")
