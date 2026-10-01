#!/bin/sh
set -eu

if [ "$1" = "run" ] && [ "$2" = "--help" ]; then
    cat <<'EOF'
Usage: docker run [OPTIONS] IMAGE [COMMAND] [ARG...]

Options:
  -d, --detach  Run container in background and print container ID
      --name string  Assign a name to the container
  -p, --publish list  Publish a container's port or ports
      --rm  Automatically remove the container when it exits
EOF
    exit 0
fi

exit 1
