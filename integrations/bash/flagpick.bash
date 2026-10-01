# Flagpick Bash Readline integration.
# Source this file after the `flagpick` binary is available on PATH.

function _flagpick_widget() {
  local result
  result="$({
    printf '%s' "$READLINE_LINE" | command flagpick shell-edit bash --cursor "$READLINE_POINT"
    local exit_code=$?
    printf '\x1f'
    return "$exit_code"
  })" || return

  READLINE_LINE="${result%$'\x1f'}"
  READLINE_POINT=${#READLINE_LINE}
}

if [[ $- == *i* ]]; then
  bind -x '"\C-g":_flagpick_widget'
fi
