#!/usr/bin/env bash
# Download a pinned, reproducible corpus of real-world .docx files from public test suites.
#
#   tools/corpus/fetch.sh OUT_DIR
#
# Every source is a sparse, blob-filtered, depth-1 git fetch of one commit (by SHA), so the set
# never moves. Files are copied to OUT_DIR/<source>/<upstream path with / replaced by __> and listed in
# OUT_DIR/manifest.tsv (file, source, upstream URL, bytes). Files over 20 MB are skipped.
# Needs only git and coreutils. Expected total: 547 files (see the counts below).
set -euo pipefail

out=${1:?usage: fetch.sh OUT_DIR}
max_bytes=$((20 * 1024 * 1024))
mkdir -p "$out"
out=$(cd "$out" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
manifest="$out/manifest.tsv"
printf 'file\tsource\turl\tbytes\n' > "$manifest"

# fetch NAME REPO SHA DIR... : sparse checkout of DIRs of REPO at SHA into $work/NAME.
fetch() {
  local name=$1 repo=$2 sha=$3
  shift 3
  local dir="$work/$name"
  echo "== $name: $repo@$sha ($*)" >&2
  git init -q "$dir"
  git -C "$dir" remote add origin "https://github.com/$repo.git"
  git -C "$dir" sparse-checkout set --no-cone "$@" >/dev/null
  local try
  for try in 1 2 3; do
    if git -C "$dir" fetch -q --depth 1 --filter=blob:none origin "$sha"; then break; fi
    [ "$try" = 3 ] && { echo "fetch of $repo failed" >&2; exit 1; }
    sleep 10
  done
  git -C "$dir" -c advice.detachedHead=false checkout -q FETCH_HEAD
  test "$(git -C "$dir" rev-parse HEAD)" = "$sha"
}

# take NAME REPO SHA DIR EVERY EXT... : copy files under $work/NAME/DIR with one of the extensions,
# in byte order, keeping every EVERY-th one (1 = all).
take() {
  local name=$1 repo=$2 sha=$3 sub=$4 every=$5
  shift 5
  local pattern="" ext n=0 kept=0
  for ext in "$@"; do pattern="$pattern${pattern:+|}$ext"; done
  mkdir -p "$out/$name"
  while IFS= read -r -d '' rel; do
    n=$((n + 1))
    [ $(((n - 1) % every)) -eq 0 ] || continue
    local src="$work/$name/$sub/$rel"
    local bytes
    bytes=$(stat -c %s "$src")
    if [ "$bytes" -gt "$max_bytes" ]; then echo "skip (>20 MB): $sub/$rel" >&2; continue; fi
    local flat="$name/${sub//\//__}__${rel//\//__}"
    cp "$src" "$out/$flat"
    printf '%s\t%s\t%s\t%s\n' "$flat" "$name" "https://github.com/$repo/blob/$sha/$sub/$rel" "$bytes" >> "$manifest"
    kept=$((kept + 1))
  done < <(cd "$work/$name/$sub" && find . -type f -print0 | sed -z 's|^\./||' | grep -zEi "\.($pattern)\$" | LC_ALL=C sort -z)
  echo "   $name/$sub: kept $kept of $n" >&2
}

# Apache POI test data: Word files collected from bug reports (138: docx 130, docm 4, dotx 4).
POI=519e0a3c3945364e8ea75c172fe1a15fb6d4a202
fetch poi apache/poi "$POI" /test-data/document/
take poi apache/poi "$POI" test-data/document 1 docx docm dotx dotm

# LibreOffice Writer OOXML import tests (164) and every 7th export test (1380 -> 198).
LO=40a48f569f9461343861b45fb5696a63c60af048
fetch lo LibreOffice/core "$LO" /sw/qa/extras/ooxmlimport/data/ /sw/qa/extras/ooxmlexport/data/
take lo LibreOffice/core "$LO" sw/qa/extras/ooxmlimport/data 1 docx docm dotx
take lo LibreOffice/core "$LO" sw/qa/extras/ooxmlexport/data 7 docx docm dotx

# python-docx feature and unit test files (45).
PD=e45454602b53e8e572b179ccf1c91093ec9f4ed7
fetch python-docx python-openxml/python-docx "$PD" /features/steps/test_files/ /tests/test_files/
take python-docx python-openxml/python-docx "$PD" features/steps/test_files 1 docx
take python-docx python-openxml/python-docx "$PD" tests/test_files 1 docx

# ONLYOFFICE sample documents (2; the per-language blank templates are skipped).
OO=0a52fb76fa53fddf7d70da28878537cde3d29421
fetch onlyoffice ONLYOFFICE/document-templates "$OO" /sample/ /sample-mobile/
take onlyoffice ONLYOFFICE/document-templates "$OO" sample 1 docx
take onlyoffice ONLYOFFICE/document-templates "$OO" sample-mobile 1 docx

echo "corpus: $(($(wc -l < "$manifest") - 1)) files in $out" >&2
