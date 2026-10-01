#!/usr/bin/env bash

set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT

fail() {
  printf 'bash integration test failed: %s\n' "$1" >&2
  exit 1
}

assert_equal() {
  local expected=$1
  local actual=$2
  local message=$3
  [[ $actual == "$expected" ]] || fail "$message"
}

cat >"$test_dir/flagpick" <<'FAKE'
#!/usr/bin/env bash

set -euo pipefail

printf '%s\n' "$*" >"$FLAGPICK_TEST_DIR/argv"
input=$(command cat)
printf '%s' "$input" >"$FLAGPICK_TEST_DIR/stdin"

if [[ ${FLAGPICK_FAKE_STATUS:-0} -ne 0 ]]; then
  exit "$FLAGPICK_FAKE_STATUS"
fi

printf '%s--help' "$input"
FAKE
chmod 700 "$test_dir/flagpick"

export FLAGPICK_TEST_DIR=$test_dir
export PATH="$test_dir:$PATH"
source "$repo_root/integrations/bash/flagpick.bash"

READLINE_LINE=$'printf "$(touch /tmp/flagpick-owned)"\n雪'
READLINE_POINT=${#READLINE_LINE}
original=$READLINE_LINE
_flagpick_widget
assert_equal "${original}--help" "$READLINE_LINE" "successful edit changed the buffer unexpectedly"
assert_equal "${#READLINE_LINE}" "$READLINE_POINT" "successful edit did not move the cursor"
assert_equal "$original" "$(<"$test_dir/stdin")" "stdin transport changed the buffer"
[[ $(<"$test_dir/argv") == 'shell-edit bash --cursor '* ]] || fail "Bash argv was not forwarded"

export FLAGPICK_FAKE_STATUS=130
READLINE_LINE='leave me unchanged'
READLINE_POINT=4
if _flagpick_widget; then
  fail "cancel unexpectedly succeeded"
fi
assert_equal 'leave me unchanged' "$READLINE_LINE" "cancel changed the buffer"

printf 'bash integration checks passed\n'
