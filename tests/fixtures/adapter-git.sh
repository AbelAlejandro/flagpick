#!/bin/sh
case "$1" in
  commit)
    printf '%s\n' \
      'usage: git commit [<options>]' \
      '' \
      'Options:' \
      '  -m, --message <msg>  commit message' \
      '      --amend          amend the previous commit' \
      '  -a, --all            stage all modified files'
    ;;
  remote)
    printf '%s\n' \
      'usage: git remote [-v | --verbose]' \
      '' \
      'Options:' \
      '  -v, --verbose        be verbose' \
      '      --get-url        show remote URLs'
    ;;
esac
