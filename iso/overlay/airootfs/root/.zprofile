# Auto-start the ZenvX session on the live console (tty1).
if [[ -z "$DISPLAY" ]] && [[ "$(tty)" == "/dev/tty1" ]]; then
    exec zenvx-start
fi
