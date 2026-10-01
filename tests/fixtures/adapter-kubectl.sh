#!/bin/sh
set -eu

if [ "$1" = "get" ] && [ "$2" = "--help" ]; then
    cat <<'EOF'
Usage: kubectl get RESOURCE

Options:
  -A, --all-namespaces  If present, list the requested object(s) across all namespaces
  -o, --output string  Output format
      --show-labels  When printing the default columns, show the labels
  -w, --watch  After listing/getting the requested object, watch for changes
EOF
    exit 0
fi

exit 1
