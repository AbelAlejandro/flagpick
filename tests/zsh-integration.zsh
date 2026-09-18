#!/usr/bin/env zsh

emulate -L zsh
setopt errexit nounset pipefail

repo_root=${0:A:h:h}
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT

function fail() {
  print -u2 -r -- "zsh integration test failed: $1"
  exit 1
}

function assert_equal() {
  local expected="$1"
  local actual="$2"
  local message="$3"
  [[ "$actual" == "$expected" ]] || fail "$message"
}

cat >| "$test_dir/flagpick" <<'FAKE'
#!/usr/bin/env zsh
emulate -L zsh
setopt nounset pipefail

print -r -- "${(j:|:)@}" >| "$FLAGPICK_TEST_DIR/argv"
input="$(command cat; printf '\x1f')"
input="${input%$'\x1f'}"
printf '%s' "$input" >| "$FLAGPICK_TEST_DIR/stdin"

exit_code=${FLAGPICK_FAKE_STATUS:-0}
(( exit_code == 0 )) || exit "$exit_code"

cursor=0
at_cursor=0
while (( $# > 0 )); do
  case "$1" in
    --cursor)
      cursor="$2"
      shift 2
      ;;
    --at-cursor)
      at_cursor=1
      shift
      ;;
    *)
      shift
      ;;
  esac
done

if (( at_cursor )); then
  prefix=${input[1,cursor]}
  suffix=${input[cursor + 1,-1]}
  printf '%s' "${prefix}--help${suffix}"
elif [[ -z "$input" || "$input" == *[[:space:]] ]]; then
  printf '%s' "${input}--help"
else
  printf '%s' "${input} --help"
fi
FAKE
chmod 700 "$test_dir/flagpick"

export FLAGPICK_TEST_DIR="$test_dir"
export PATH="$test_dir:$PATH"
source "$repo_root/integrations/zsh/flagpick.zsh"

marker="$test_dir/owned"
original="print -r -- '\$(touch $marker)' \`touch $marker\`;:"$'\n雪\n\n'
BUFFER="$original"
CURSOR=${#BUFFER}
flagpick-widget
assert_equal "${original}--help" "$BUFFER" "successful append did not preserve multiline Unicode buffer"
assert_equal "${#BUFFER}" "$CURSOR" "successful append did not move cursor to the result end"
[[ ! -e "$marker" ]] || fail "shell-like buffer text was executed"
captured="$(command cat "$test_dir/stdin"; printf '\x1f')"
captured="${captured%$'\x1f'}"
assert_equal "$original" "$captured" "stdin transport changed the original buffer"
[[ "$(<"$test_dir/argv")" == "shell-edit|zsh|--cursor|${#original}" ]] || fail "append argv was not forwarded exactly"

original=$'ab雪\n\n'
BUFFER="$original"
CURSOR=2
flagpick-widget-at-cursor
assert_equal $'ab--help雪\n\n' "$BUFFER" "at-cursor edit lost Unicode or trailing newlines"
[[ "$(<"$test_dir/argv")" == "shell-edit|zsh|--cursor|2|--at-cursor" ]] || fail "at-cursor argv was not forwarded exactly"

original=$'left\x1fright'
BUFFER="$original"
CURSOR=${#BUFFER}
flagpick-widget
assert_equal "${original} --help" "$BUFFER" "sentinel-like buffer data was not preserved"

for exit_code in 130 1; do
  export FLAGPICK_FAKE_STATUS=$exit_code
  BUFFER=$'leave me\nunchanged'
  CURSOR=4
  if flagpick-widget; then
    fail "status $exit_code unexpectedly succeeded"
  fi
  assert_equal $'leave me\nunchanged' "$BUFFER" "status $exit_code changed BUFFER"
done

print -r -- "zsh integration checks passed"
