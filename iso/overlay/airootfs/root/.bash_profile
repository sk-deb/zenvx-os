# Auto-start the ZenvX session on the live console (tty1).
[ -z "$DISPLAY" ] && [ "$XDG_VTNR" = 1 ] && exec zenvx-start
