#!/usr/bin/env python3
"""Vérifie que ci-payload.py produit du JSON qu'une base Postgres accepte.

Ce test existe parce que le contraire a coûté une soirée : la CI produisait un
corps que `json.loads` acceptait sans broncher et que PostgREST rejetait avec
« Empty or invalid json ». Un JSON valide n'est pas forcément un JSON insérable.
"""

import json
import sys

sys.path.insert(0, __file__.rsplit("/", 1)[0].rsplit("\\", 1)[0])

from importlib import import_module

payload_mod = import_module("ci-payload".replace("-", "_")) if False else None

# Import direct : le nom du fichier contient un tiret.
import importlib.util
import os

spec = importlib.util.spec_from_file_location(
    "ci_payload", os.path.join(os.path.dirname(__file__), "ci-payload.py")
)
ci = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ci)

failures = []


def check(label, condition, detail=""):
    if condition:
        print(f"  OK   {label}")
    else:
        failures.append(label)
        print(f"  ECHEC {label} {detail}")


print("clean() :")

# cargo colore chaque diagnostic ; ces séquences n'apprennent rien au Codeur.
ansi = "\x1b[0m\x1b[1m\x1b[38;5;9merror\x1b[0m: build failed"
check("supprime les couleurs ANSI", ci.clean(ansi) == "error: build failed", repr(ci.clean(ansi)))

# Un NUL est du JSON valide et une valeur impossible dans une colonne texte.
check("supprime le NUL", "\x00" not in ci.clean("avant\x00apres"))

# Un log non-UTF-8 arrive en surrogates ; json.dumps les encode volontiers en
# \udcXX, que Postgres refuse.
surrogate = "octet invalide \udcb0 ici"
cleaned = ci.clean(surrogate)
check("remplace les surrogates", all(not (0xD800 <= ord(c) <= 0xDFFF) for c in cleaned))
check(
    "le resultat est encodable en UTF-8 strict",
    (lambda: (cleaned.encode("utf-8"), True)[1])(),
)

# Ce qui compte pour le Codeur doit survivre intact.
keep = "error: can't find library `grenos_kernel`\n  --> src/main.rs:12\ttab"
check("preserve le message, les sauts de ligne et les tabulations", ci.clean(keep) == keep)

print("\npayload complet :")
os.environ.update(
    {
        "BRANCH": "agent/b6dcd08c",
        "SHA": "c6b3288",
        "ST": "failed",
        "FL": "compile_error",
        "RUNID": "34489116426",
        "URL": "https://github.com/Grenofar/grenOS/actions/runs/1",
        # Pas de NUL ici : Windows refuse un octet nul dans une variable
        # d environnement, alors que le runner Linux l accepte. clean() est
        # teste sur ce cas juste au-dessus, la ou ca compte.
        "LOGTXT": ansi + chr(10) + keep + chr(10) + surrogate,
    }
)

import io
from contextlib import redirect_stdout


def run_payload():
    buf = io.StringIO()
    with redirect_stdout(buf):
        code = ci.main()
    return code, buf.getvalue()


code, raw = run_payload()
check("sort en 0", code == 0)
try:
    doc = json.loads(raw)
    check("JSON parsable", True)
except Exception as exc:  # noqa: BLE001
    check("JSON parsable", False, str(exc))
    doc = {}

check("9 champs", len(doc) == 9, str(sorted(doc)))
check("workflow_run_id est un entier", isinstance(doc.get("workflow_run_id"), int))
check("aucun surrogate dans la sortie", "\\udc" not in raw)
check("aucun NUL echappe dans la sortie", "\\u0000" not in raw)

# failure vide doit devenir null : le Maître route d'après cette valeur.
os.environ["FL"] = ""
check("failure vide devient null", json.loads(run_payload()[1])["failure"] is None)

# Les étapes jugées : c'est d'elles que le worker déduit la branche la plus
# avancée d'une mission (worker/src/lineage.ts).
print("\nverdicts par etape :")
os.environ.update({"PROBE": "true", "BUILD": "success", "CLIPPY": "failure", "QEMU": "skipped", "SCREEN": ""})
steps = json.loads(run_payload()[1])["verdicts"]
check("une entree par etape", [s.get("step") for s in steps] == ["build", "clippy", "boot", "screen"], str(steps))
check(
    "success -> PASS, failure -> FAIL, sautee ou absente -> UNVERIFIABLE",
    [s.get("verdict") for s in steps] == ["PASS", "FAIL", "UNVERIFIABLE", "UNVERIFIABLE"],
    str(steps),
)
check(
    "chaque entree a la forme de runs.verdicts",
    all({"criterion", "verdict", "evidence"} <= set(s) for s in steps),
)
os.environ["PROBE"] = "false"
check("pas de kernel -> aucun verdict d'etape", json.loads(run_payload()[1])["verdicts"] == [])

# Une variable manquante doit échouer en le disant, pas produire un corps vide.
os.environ["BRANCH"] = ""
check("variable manquante -> code 1", ci.main() == 1)

print()
if failures:
    print(f"{len(failures)} echec(s)")
    sys.exit(1)
print("tout passe")
