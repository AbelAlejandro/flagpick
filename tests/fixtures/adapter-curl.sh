#!/bin/sh
printf '%s\n' \
  'Usage: curl [options...] <url>' \
  '' \
  'Options:' \
  '     --help             This help' \
  '     --proxy <host>     Use proxy' \
  ' -L, --location         Follow redirects' \
  ' -m, --max-time <sec>   Maximum transfer time'
