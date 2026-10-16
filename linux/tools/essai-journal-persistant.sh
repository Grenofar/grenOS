#!/bin/sh
# Exercer le verdict du journal persistant, DANS LES DEUX SENS.
#
# Le bloc est EXTRAIT DU WORKFLOW, jamais recopie : une copie et son original
# finissent par diverger, et c est alors la copie qu on croit.
racine=$(cd "$(dirname "$0")/../.." && pwd)
flux="$racine/.github/workflows/linux.yml"
ici=$(cd "$(dirname "$0")" && pwd)
echecs=0

# Les lignes du workflow, desindentees de dix espaces.
sed -n '/^          JOURNAL=\$(grep -a/,/^          fi$/p' "$flux" \
  | sed 's/^          //' > "$ici/frag-journal.sh"
if [ ! -s "$ici/frag-journal.sh" ]; then
  echo "le bloc n a pas ete trouve dans le workflow — l essai ne juge rien"
  exit 1
fi
echo "  bloc extrait du workflow : $(wc -l < "$ici/frag-journal.sh") lignes"

juger() {
  printf '%s\n' "$2" > "$ici/serial-uefi.log"
  RUNNER_TEMP="$ici" /bin/sh "$ici/frag-journal.sh" 2>&1
}

verifier() {
  sortie=$(juger "$1" "$2")
  if printf '%s' "$sortie" | grep -q "$3"; then
    printf '  [ok] %-46s -> %s\n' "$1" "$3"
  else
    printf '  [KO] %-46s attendu %s, obtenu :\n%s\n' "$1" "$3" "$sortie"
    echecs=$((echecs + 1))
  fi
}

verifier "deux demarrages connus : accepte" \
  "grenos: journal : persistant, 2 demarrage(s) connu(s)" "encore lisible"
verifier "douze demarrages connus : accepte" \
  "grenos: journal : persistant, 12 demarrage(s) connu(s)" "encore lisible"
verifier "un seul demarrage : AVERTIT" \
  "grenos: journal : persistant, 1 demarrage(s) connu(s)" "::warning::"
verifier "zero demarrage : AVERTIT" \
  "grenos: journal : persistant, 0 demarrage(s) connu(s)" "::warning::"
verifier "journal volatile : AVERTIT" \
  "grenos: journal : volatile — ce qui s'ecrit maintenant disparait" "::warning::"
verifier "aucune ligne : AVERTIT" \
  "grenos: session de bureau ouverte" "::warning::"
verifier "aucune ligne : le dit au lieu de se taire" \
  "grenos: session de bureau ouverte" "n'a rien dit"

rm -f "$ici/frag-journal.sh" "$ici/serial-uefi.log"
echo
if [ "$echecs" -gt 0 ]; then
  echo "$echecs cas ont echoue."
  exit 1
fi
echo "les 7 cas passent, dont 4 refus."
