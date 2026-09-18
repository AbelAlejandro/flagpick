# Flagpick Phase A ZLE integration.
# Source this file after the `flagpick` binary is available on PATH.

function _flagpick_edit() {
  local mode="$1"
  local result
  local -a flagpick_args=(shell-edit zsh --cursor "$CURSOR")

  if [[ "$mode" == "at-cursor" ]]; then
    flagpick_args+=(--at-cursor)
  fi

  result="$(
    printf '%s' "$BUFFER" | command flagpick "${flagpick_args[@]}"
    local exit_code=$?
    (( exit_code == 0 )) || exit "$exit_code"
    # A sentinel prevents command substitution from stripping buffer-ending newlines.
    printf '\x1f'
  )" || return

  BUFFER="${result%$'\x1f'}"
  CURSOR=${#BUFFER}
  if [[ -o interactive ]]; then
    zle redisplay
  fi
  return 0
}

function flagpick-widget() {
  _flagpick_edit append
}

function flagpick-widget-at-cursor() {
  _flagpick_edit at-cursor
}

if [[ -o interactive ]]; then
  zle -N flagpick-widget
  zle -N flagpick-widget-at-cursor
  bindkey '^G' flagpick-widget
fi
