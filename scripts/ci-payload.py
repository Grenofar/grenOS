#!/usr/bin/env python3
"""Construit le corps JSON du verdict de CI, lu sur stdout par le workflow.

Un fichier séparé, et non du code embarqué dans le YAML. Les trois tentatives
précédentes ont échoué exactement là : un programme jq ou un heredoc Python
écrit à l'intérieur d'un bloc `run: |` traverse l'échappement YAML puis celui
du shell, et il suffit d'un antislash mal placé pour produire un fichier vide.
PostgREST répond alors « Empty or invalid json », un message qui envoie
chercher du côté des identifiants alors que le corps était le problème.

Ici il n'y a plus rien à échapper : les valeurs arrivent par l'environnement,
`json.dumps` ne peut produire que du JSON valide, et ce fichier est testable
hors CI.
"""

import datetime
import json
import os
import re
import sys

REQUIRED = ["BRANCH", "SHA", "ST", "RUNID", "URL"]

# Séquences de couleur ANSI : cargo en produit à chaque ligne d'erreur.
ANSI = re.compile(r"\x1b\[[0-9;?]*[A-Za-z]")
# Tout caractère de contrôle sauf tabulation et saut de ligne.
CONTROL = re.compile(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]")

# Ce que la CI a jugé, étape par étape, dans la forme de runs.verdicts
# ({criterion, verdict, evidence}) plus la clé `step`, que le worker lit pour
# savoir jusqu'où une branche est allée et de laquelle repartir
# (worker/src/lineage.ts, D-027). Le Maître et la page /runs les montrent tels
# quels.
STEPS = [
    ("build", "BUILD", "cargo build --release succeeds in kernel/"),
    ("clippy", "CLIPPY", "cargo clippy --release -- -D warnings reports no warning"),
    ("boot", "QEMU", "the QEMU boot prints grenOS on the serial console, without panic or fault"),
    # D-033 : la capture prise à 25 s, jugée par scripts/ci-screen.py.
    ("screen", "SCREEN", "the screen is drawn: at least 3 colours, none on more than 90% of it"),
    # D-035 : QEMU bouge la souris, clique et tape ; le kernel doit le dire.
    (
        "input",
        "INPUT",
        "the kernel decodes the mouse and the keyboard: QEMU injects a move, a click and a key, and the serial log reports the first packet and the first key",
    ),
]
OUTCOMES = {"success": "PASS", "failure": "FAIL"}


def clean(text: str) -> str:
    """Rend un log de build sûr à insérer dans une colonne texte Postgres.

    Trois pièges, tous rencontrés en production :

    - Les octets non-UTF-8 d'un log arrivent dans os.environ en surrogates
      (surrogateescape). json.dumps les encode volontiers en \\udcXX, que
      PostgreSQL refuse — et PostgREST rapporte « Empty or invalid json »,
      un message qui accuse le corps entier plutôt que le caractère fautif.
    - \\u0000 est du JSON parfaitement valide et une valeur impossible dans
      une colonne texte Postgres.
    - Les couleurs ANSI de cargo n'apportent rien au Codeur et gonflent
      l'extrait qu'on lui renvoie.
    """
    text = text.encode("utf-8", "replace").decode("utf-8", "replace")
    text = ANSI.sub("", text)
    return CONTROL.sub("", text)


def verdicts() -> list:
    """Une entrée par étape, ou aucune quand il n'y avait pas de kernel à juger."""
    if os.environ.get("PROBE") != "true":
        return []
    out = []
    for step, var, criterion in STEPS:
        outcome = os.environ.get(var, "")
        out.append(
            {
                "step": step,
                "criterion": criterion,
                # Une étape sautée ou annulée n'a rien jugé : ni PASS ni FAIL.
                "verdict": OUTCOMES.get(outcome, "UNVERIFIABLE"),
                "evidence": f"step outcome: {outcome or 'not run'}",
            }
        )
    return out


def main() -> int:
    missing = [name for name in REQUIRED if not os.environ.get(name)]
    if missing:
        print(f"variables manquantes : {', '.join(missing)}", file=sys.stderr)
        return 1

    try:
        run_id = int(os.environ["RUNID"])
    except ValueError:
        print(f"RUNID n'est pas un entier : {os.environ['RUNID']!r}", file=sys.stderr)
        return 1

    payload = {
        "branch": clean(os.environ["BRANCH"]),
        "commit_sha": clean(os.environ["SHA"]),
        "status": os.environ["ST"],
        # Chaîne vide et absence de panne sont la même chose côté base : la
        # colonne est nullable et un failure vide fausserait le routage du
        # Maître, qui décide de la suite d'après cette valeur.
        "failure": os.environ.get("FL") or None,
        "verdicts": verdicts(),
        "log_excerpt": clean(os.environ.get("LOGTXT", "")),
        "log_url": os.environ["URL"],
        "workflow_run_id": run_id,
        "finished_at": datetime.datetime.now(datetime.timezone.utc).strftime(
            "%Y-%m-%dT%H:%M:%SZ"
        ),
    }

    print(json.dumps(payload))
    return 0


if __name__ == "__main__":
    sys.exit(main())
