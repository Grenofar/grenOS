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
import sys

REQUIRED = ["BRANCH", "SHA", "ST", "RUNID", "URL"]


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
        "branch": os.environ["BRANCH"],
        "commit_sha": os.environ["SHA"],
        "status": os.environ["ST"],
        # Chaîne vide et absence de panne sont la même chose côté base : la
        # colonne est nullable et un failure vide fausserait le routage du
        # Maître, qui décide de la suite d'après cette valeur.
        "failure": os.environ.get("FL") or None,
        "log_excerpt": os.environ.get("LOGTXT", ""),
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
