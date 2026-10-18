#!/bin/sh
# Synthetic diagnostics exercise bounded UI layout without a network service.
i=0
while test "$i" -lt 120; do
    echo "Diagnostic $i: rejected synthetic push; inspect repository permissions and retry." >&2
    i=$((i + 1))
done
echo "FINAL_DIAGNOSTIC_MARKER" >&2
exit 1
