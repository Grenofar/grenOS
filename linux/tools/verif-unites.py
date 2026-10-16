#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Vérifie les unités systemd du chroot.

Pour chaque *.service et *.timer sous linux/config/includes.chroot/etc/systemd/system/
(et uniquement là, pas les répertoires .d), le script refuse (exit 1, une ligne
::error file=<path>::<reason> par défaut) quand :
- le fichier ne possède pas de section [Unit], ou un .service sans [Service],
  ou un .timer sans [Timer] ;
- une clé apparaît en dehors de toute section ;
- ExecStart, ExecStop ou ExecStartPre désigne un chemin absolu sous /usr/ qui
  n'existe pas dans le chroot linux/config/includes.chroot/usr/ (après avoir
  retiré un éventuel '-' ou '+' et ignoré les arguments) ;
- le fichier est un .service sans section [Install] ET sans directive WantedBy
  (dans [Install]) : cela produit une ligne ::warning mais ne fait pas exit 1.

Si aucun défaut n'est trouvé, il affiche exactement :
unites : N fichier(s) lus, chaque Exec livre existe
où N est le nombre de fichiers lus.

Le script accepte en argument des chemins spécifiques à vérifier ; sinon il
parcourt l'arborescence ci-dessus. Tout le texte affiché est en français avec
ses accents. Seule la bibliothèque standard est utilisée.
"""
import os
import sys
import configparser

CHROOT_BASE = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "config",
    "includes.chroot",
)


def _iter_unit_files(root):
    """Yield absolute paths to *.service and *.timer files under root,
    excluding any *.d subdirectories."""
    systemd_dir = os.path.join(root, "etc", "systemd", "system")
    if not os.path.isdir(systemd_dir):
        return
    for name in sorted(os.listdir(systemd_dir)):
        if name.endswith((".service", ".timer")):
            path = os.path.join(systemd_dir, name)
            if os.path.isfile(path):
                yield path


def _map_usr_path(abs_path):
    """Map an absolute /usr/ path to its location inside the chroot.
    Returns None if the path is not under /usr/."""
    if not abs_path.startswith("/usr/"):
        return None
    rest = abs_path[len("/usr/") :]
    return os.path.join(CHROOT_BASE, "usr", rest)


def _parse_exec(value):
    """Strip a leading '-' or '+' and return the first token (the path)."""
    if not value:
        return ""
    if value[0] in ("-", "+"):
        value = value[1:]
    # Keep only the first word (the executable path), ignore arguments.
    return value.split()[0] if value.split() else ""


def _check_file(path):
    """Return a list of error strings, and a warning string or None."""
    errors = []
    warning = None

    try:
        with open(path, "r", encoding="utf-8") as f:
            content = f.read()
    except OSError as e:
        return [f"impossible de lire le fichier — {e}"], None

    # configparser with empty_lines_in_values=False to detect keys outside sections.
    parser = configparser.ConfigParser(empty_lines_in_values=False)
    try:
        parser.read_string(content)
    except configparser.Error as e:
        return [f"clé en dehors de toute section — {e}"], None

    # Check for [Unit] section.
    if not parser.has_section("Unit"):
        errors.append("section [Unit] manquante")

    # Determine expected section based on file extension.
    if path.endswith(".service"):
        expected = "Service"
    elif path.endswith(".timer"):
        expected = "Timer"
    else:
        # Should not happen due to caller filter.
        expected = None

    if expected and not parser.has_section(expected):
        errors.append(f"section [{expected}] manquante")

    # Validate Exec* directives.
    for section in parser.sections():
        if not section:
            # Ignore the default section (should be empty due to empty_lines_in_values=False).
            continue
        for key in ("ExecStart", "ExecStop", "ExecStartPre"):
            if parser.has_option(section, key):
                raw = parser.get(section, key)
                exec_path = _parse_exec(raw)
                if exec_path:
                    mapped = _map_usr_path(exec_path)
                    if mapped is not None and not os.path.exists(mapped):
                        errors.append(
                            f"ExecStart pointe vers un fichier absent dans le chroot — {exec_path}"
                        )

    # Special warning for .service without [Install] and without WantedBy.
    if path.endswith(".service"):
        has_install = parser.has_section("Install")
        wanted_by = False
        if has_install:
            try:
                wanted_by = parser.has_option("Install", "WantedBy")
            except configparser.Error:
                pass
        if not has_install and not wanted_by:
            warning = f"::warning::{os.path.relpath(path, os.path.dirname(CHROOT_BASE))} : le service ne possède ni [Install] ni WantedBy"

    return errors, warning


def main(argv):
    # Determine which files to check.
    if argv[1:]:
        files_to_check = argv[1:]
    else:
        files_to_check = list(_iter_unit_files(CHROOT_BASE))

    if not files_to_check:
        print("unites : 0 fichier(s) lu, chaque Exec livre existe")
        return 0

    fault_count = 0
    for f in files_to_check:
        rel_path = os.path.relpath(f, os.path.dirname(CHROOT_BASE))
        errors, warning = _check_file(f)
        if warning:
            print(warning)
        for err in errors:
            print(f"::error file={rel_path}::{err}")
            fault_count += 1

    if fault_count == 0:
        print(f"unites : {len(files_to_check)} fichier(s) lus, chaque Exec livre existe")
        return 0
    else:
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
