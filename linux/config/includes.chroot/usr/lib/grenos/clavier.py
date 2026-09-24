"""Le clavier : quelle disposition, et où l'on se trouve.

Un clavier français, belge ou québécois ne place ni les lettres ni les accents
au même endroit. Se tromper de disposition, c'est ne plus pouvoir taper son
propre mot de passe — raison pour laquelle on le demande au tout premier
écran, avant même qu'il y ait un mot de passe.

Chaque entrée donne : le nom du pays tel qu'on le dit, la disposition XKB, la
variante, et le fuseau horaire qui va avec. Le fuseau n'est qu'une proposition
de départ ; les Réglages permettent d'en changer.
"""

# nom affiché, disposition, variante, fuseau proposé
PAYS = [
    ("France (AZERTY)", "fr", "", "Europe/Paris"),
    ("France (AZERTY, clavier récent)", "fr", "oss", "Europe/Paris"),
    ("Belgique (AZERTY)", "be", "", "Europe/Brussels"),
    ("Suisse romande (QWERTZ)", "ch", "fr", "Europe/Zurich"),
    ("Luxembourg", "fr", "", "Europe/Luxembourg"),
    ("Canada — Québec", "ca", "", "America/Montreal"),
    ("Canada — anglais", "ca", "eng", "America/Toronto"),
    ("Maroc", "ma", "french", "Africa/Casablanca"),
    ("Sénégal", "fr", "", "Africa/Dakar"),
    ("La Réunion", "fr", "", "Indian/Reunion"),
    ("États-Unis (QWERTY)", "us", "", "America/New_York"),
    ("Royaume-Uni (QWERTY)", "gb", "", "Europe/London"),
    ("Allemagne (QWERTZ)", "de", "", "Europe/Berlin"),
    ("Espagne", "es", "", "Europe/Madrid"),
    ("Italie", "it", "", "Europe/Rome"),
    ("Portugal", "pt", "", "Europe/Lisbon"),
]

DEFAUT = 0


def ecrire_systeme(disposition, variante):
    """Écrit le clavier du système, celui que toute session reprendra."""
    contenu = (
        '# Écrit par grenOS, au premier démarrage.\n'
        'XKBMODEL="pc105"\n'
        f'XKBLAYOUT="{disposition}"\n'
        f'XKBVARIANT="{variante}"\n'
        'XKBOPTIONS=""\n'
        'BACKSPACE="guess"\n'
    )
    with open("/etc/default/keyboard", "w", encoding="utf-8") as fichier:
        fichier.write(contenu)


def appliquer_maintenant(disposition, variante):
    """Change le clavier de l'écran en cours, sans attendre un redémarrage."""
    import subprocess
    commande = ["setxkbmap", disposition]
    if variante:
        commande += [variante]
    subprocess.run(commande, check=False, capture_output=True)
