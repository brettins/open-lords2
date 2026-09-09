#!/bin/sh
# Register the keyed JSON merge driver that `.gitattributes` asks for.
#
# Git deliberately will not run a merge driver a repository merely names — that
# would be executing code on checkout — so this is per-clone and opt-in. Without
# it the symbol databases fall back to the ordinary text merge, which is safe
# only because it is loud, and which once produced nine conflict hunks whose
# `ours` bodies sat under the wrong entries' names.
#
# Run from the repository root:  sh tools/symbols/install-merge-driver.sh
set -e
git config merge.l2json.name "keyed merge for the symbol databases"
git config merge.l2json.driver "node tools/symbols/merge-json.js %O %A %B %P"
echo "l2json merge driver registered for this clone:"
git config --get merge.l2json.driver
