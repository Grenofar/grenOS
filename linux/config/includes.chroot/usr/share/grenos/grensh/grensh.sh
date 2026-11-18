# grensh — le langage du terminal grenOS.
#
# Grenofar, le 18 novembre : « créer aussi le langage bash de grenOS comme
# Windows PowerShell, ajoute des commandes dont grenget ».
#
# CE QUE C'EST : bash, plus une couche grenOS. On ne remplace PAS bash — tout
# tutoriel Linux du monde doit continuer de marcher dans ce terminal. On y
# ajoute :
#
#   - des commandes en français, au format Verbe-Nom de PowerShell
#     (`Installer-App`, `Afficher-Systeme`…), avec la complétion par Tab ;
#   - deux programmes : `gren` (le système) et `grenget` (les applications) ;
#   - une invite lisible : qui, où, branche git, et le code d'erreur quand la
#     dernière commande a échoué.
#
# Chargé par ~/.bashrc (la ligne est posée une fois par grenos-session).
# Rien ici ne lance de programme à l'ouverture du terminal : l'invite lit la
# branche git dans .git/HEAD au lieu d'appeler `git`, pour rester instantanée.

# Seulement en interactif : un script bash qui source ~/.bashrc ne doit
# jamais voir notre invite ni nos fonctions.
case $- in
    *i*) ;;
    *) return 0 2>/dev/null || exit 0 ;;
esac

GRENSH_VERSION=1

# ---- Les couleurs (palette « nuit » de grenosui.py) --------------------------
_g_c() { printf '\001\033[38;2;%sm\002' "$1"; }
_G_BLEU=$(_g_c "96;165;250")
_G_VIOLET=$(_g_c "167;139;250")
_G_GRIS=$(_g_c "148;161;186")
_G_VERT=$(_g_c "43;196;138")
_G_ROUGE=$(_g_c "239;77;94")
_G_BLANC=$(_g_c "238;242;250")
_G_RAZ=$'\001\033[0m\002'
_G_GRAS=$'\001\033[1m\002'

# ---- L'invite ------------------------------------------------------------------
# La branche git, lue à la main : remonter jusqu'au .git, lire HEAD.
_grensh_branche() {
    local d=$PWD
    while [ -n "$d" ]; do
        if [ -f "$d/.git/HEAD" ]; then
            local tete
            read -r tete < "$d/.git/HEAD" 2>/dev/null || return
            case $tete in
                "ref: refs/heads/"*) printf '%s' "${tete#ref: refs/heads/}" ;;
                *) printf '%s' "${tete:0:7}" ;;
            esac
            return
        fi
        [ "$d" = "/" ] && return
        d=${d%/*}
        [ -z "$d" ] && d=/
    done
}

_grensh_invite() {
    local code=$?
    local ou=${PWD/#$HOME/\~}
    local branche
    branche=$(_grensh_branche)
    local ligne="${_G_VIOLET}╭─${_G_RAZ} ${_G_GRAS}${_G_BLEU}${USER}${_G_RAZ}${_G_GRIS}@${HOSTNAME%%.*}${_G_RAZ}  ${_G_GRAS}${_G_BLANC}${ou}${_G_RAZ}"
    [ -n "$branche" ] && ligne+="  ${_G_VIOLET}⎇ ${branche}${_G_RAZ}"
    [ "$code" -ne 0 ] && ligne+="  ${_G_ROUGE}✗ ${code}${_G_RAZ}"
    PS1="${ligne}\n${_G_VIOLET}╰─${_G_RAZ}${_G_BLEU}❯${_G_RAZ} "
    # Le titre de l'onglet : le dossier courant.
    printf '\033]0;%s\007' "$ou"
}
PROMPT_COMMAND=_grensh_invite
PS2="${_G_VIOLET}  ┆${_G_RAZ} "

# ---- Le confort ----------------------------------------------------------------
shopt -s autocd cdspell checkwinsize histappend 2>/dev/null
HISTSIZE=10000
HISTFILESIZE=20000
HISTCONTROL=ignoreboth:erasedups

alias ls='ls --color=auto --group-directories-first'
alias ll='ls -lh'
alias la='ls -lhA'
alias grep='grep --color=auto'
alias ..='cd ..'
alias ...='cd ../..'

# Les mots français de tous les jours. Courts, et sans masquer une commande
# Linux existante (vérifié : aucun de ces noms n'est un paquet Debian courant).
alias liste='ls -lh --color=auto --group-directories-first'
alias aller='cd'
alias ici='pwd'
alias effacer='clear'
alias historique='history'

# ---- Les commandes Verbe-Nom, à la PowerShell ---------------------------------
# Chacune est un raccourci vers `gren` ou `grenget` : une seule implémentation,
# deux façons de l'appeler. `Get-Help` a son équivalent : Obtenir-Aide.
Obtenir-Aide()        { gren aide "$@"; }
Afficher-Systeme()    { gren info "$@"; }
Afficher-Memoire()    { gren memoire "$@"; }
Liberer-Memoire()     { gren liberer "$@"; }
Afficher-Reseau()     { gren reseau "$@"; }
Changer-Theme()       { gren theme "$@"; }
Changer-Accent()      { gren accent "$@"; }
Mettre-AJour()        { gren maj "$@"; }
Ouvrir-App()          { gren ouvrir "$@"; }
Sauvegarder-Fichiers() { gren sauvegarde "$@"; }
Demander-IA()         { gren ia "$@"; }
# `corrige` sans rien : la commande d'avant (l'avant-dernière ligne de
# l'historique — la dernière, c'est `corrige` lui-même). Rien n'est relancé.
Corriger-Commande() {
    if [ "$#" -gt 0 ]; then gren corrige "$@"; return; fi
    gren corrige "$(fc -ln -2 -2 2>/dev/null | sed 's/^[[:space:]]*//')"
}
alias corrige='Corriger-Commande'
alias ia='Demander-IA'

Chercher-App()        { grenget chercher "$@"; }
Installer-App()       { grenget installer "$@"; }
Desinstaller-App()    { grenget enlever "$@"; }
Lister-Apps()         { grenget liste "$@"; }
Afficher-App()        { grenget info "$@"; }
Proposer-App()        { grenget proposer "$@"; }

# ---- La langue (19 novembre) ---------------------------------------------------
# L'anglais par défaut, le français au choix : le réglage de grenOS, sinon
# $LANG — la même règle que gren et grenget. La complétion propose les mots
# de la langue choisie ; les DEUX marchent toujours une fois tapés.
_grensh_langue() {
    local v
    v=$(cat "${XDG_CONFIG_HOME:-$HOME/.config}/grenos/langue" 2>/dev/null)
    case $v in en|fr) echo "$v"; return ;; esac
    case $LANG in fr*) echo fr ;; *) echo en ;; esac
}

# ---- La complétion par Tab -----------------------------------------------------
_grensh_completer_grenget() {
    local mot=${COMP_WORDS[COMP_CWORD]}
    if [ "$COMP_CWORD" -eq 1 ]; then
        if [ "$(_grensh_langue)" = fr ]; then
            COMPREPLY=($(compgen -W "chercher installer enlever liste info maj proposer aide" -- "$mot"))
        else
            COMPREPLY=($(compgen -W "search install remove list info update suggest help" -- "$mot"))
        fi
        return
    fi
    case ${COMP_WORDS[1]} in
        installer|info|enlever|install|remove|uninstall|show)
            COMPREPLY=($(compgen -W "$(grenget --slugs 2>/dev/null)" -- "$mot")) ;;
    esac
}
complete -F _grensh_completer_grenget grenget

_grensh_completer_slug() {
    local mot=${COMP_WORDS[COMP_CWORD]}
    COMPREPLY=($(compgen -W "$(grenget --slugs 2>/dev/null)" -- "$mot"))
}
complete -F _grensh_completer_slug Installer-App Afficher-App Desinstaller-App

_grensh_completer_gren() {
    local mot=${COMP_WORDS[COMP_CWORD]}
    if [ "$COMP_CWORD" -eq 1 ]; then
        if [ "$(_grensh_langue)" = fr ]; then
            COMPREPLY=($(compgen -W "aide info memoire liberer reseau theme accent maj ouvrir sauvegarde ia corrige version" -- "$mot"))
        else
            COMPREPLY=($(compgen -W "help info memory free network theme accent update open backup ai fix version" -- "$mot"))
        fi
        return
    fi
    case ${COMP_WORDS[1]} in
        theme)  COMPREPLY=($(compgen -W "nuit jour night day" -- "$mot")) ;;
        accent) COMPREPLY=($(compgen -W "bleu violet vert orange rose cyan blue purple green pink" -- "$mot")) ;;
        sauvegarde|backup) COMPREPLY=($(compgen -W "etat activer desactiver maintenant status on off now" -- "$mot")) ;;
    esac
}
complete -F _grensh_completer_gren gren
complete -W "nuit jour" Changer-Theme
complete -W "etat activer desactiver maintenant" Sauvegarder-Fichiers
complete -W "bleu violet vert orange rose cyan" Changer-Accent

# ---- L'accueil ----------------------------------------------------------------
# Une ligne, pas une affiche : on ouvre un terminal pour travailler.
if [ -z "$GRENSH_ACCUEILLI" ]; then
    export GRENSH_ACCUEILLI=1
    if [ "$(_grensh_langue)" = fr ]; then
        printf '\033[38;2;167;139;250m●\033[0m \033[1mgrenOS\033[0m \033[38;2;148;161;186m— tape \033[38;2;96;165;250mgren aide\033[38;2;148;161;186m pour les commandes, \033[38;2;96;165;250mgrenget chercher\033[38;2;148;161;186m pour une application.\033[0m\n'
    else
        printf '\033[38;2;167;139;250m●\033[0m \033[1mgrenOS\033[0m \033[38;2;148;161;186m— type \033[38;2;96;165;250mgren help\033[38;2;148;161;186m for the commands, \033[38;2;96;165;250mgrenget search\033[38;2;148;161;186m for an app.\033[0m\n'
    fi
fi
