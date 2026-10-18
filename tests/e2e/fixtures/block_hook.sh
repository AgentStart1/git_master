#!/bin/sh
# Bound the pause even if pytest exits before releasing its fixture.
case "$1" in committed|aborted) exit 0 ;; esac
test -n "$GIT_MASTER_E2E_GATE" || exit 1
: > "$GIT_MASTER_E2E_GATE/entered"
attempt=0
while test ! -f "$GIT_MASTER_E2E_GATE/release"; do
    attempt=$((attempt + 1))
    test "$attempt" -lt 600 || exit 1
    sleep 0.05
done
