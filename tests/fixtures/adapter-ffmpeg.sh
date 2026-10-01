#!/bin/sh
set -eu

if [ "$1" = "-h" ] && [ "$2" = "long" ]; then
    cat <<'EOF'
ffmpeg version 6.1.1
Options:
  -i url  input file name
  -c[:stream_specifier] codec  select video codec
  -crf integer  constant rate factor
  -y  overwrite output files
EOF
    exit 0
fi

exit 1
