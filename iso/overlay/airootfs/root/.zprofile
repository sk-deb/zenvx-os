# ZenvX OS P1 — start the session on the live console (tty1).
# No `exec`: if the session exits we fall back to a shell instead of looping.
if [[ "$(tty)" == /dev/tty1 ]]; then
    zenvx-start
fi
