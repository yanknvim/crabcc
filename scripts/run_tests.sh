#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
TEST_DIR="${ROOT_DIR}/tests"
OUT_DIR="${ROOT_DIR}/target/c-tests"

if [ -z "${CC:-}" ] || [ "${CC}" = "gcc" ]; then
  if command -v riscv64-none-elf-gcc >/dev/null 2>&1; then
    CC=riscv64-none-elf-gcc
  else
    CC=gcc
  fi
fi
SPIKE=${SPIKE:-spike}
PK=${PK:-}

if [ -z "${PK}" ]; then
  PK=$(command -v pk || true)
fi

if [ -z "${PK}" ]; then
  echo "pk not found in PATH. Set PK=/path/to/pk if needed."
  exit 1
fi

mkdir -p "${OUT_DIR}"

shopt -s nullglob
tests=("${TEST_DIR}"/*.c)
if [ ${#tests[@]} -eq 0 ]; then
  echo "No C tests found in ${TEST_DIR}".
  exit 1
fi

for test in "${tests[@]}"; do
  name=$(basename "${test}" .c)
  asm="${OUT_DIR}/${name}.S"
  bin="${OUT_DIR}/${name}"
  expect=0
  expect_file="${TEST_DIR}/${name}.expect"
  if [ -f "${expect_file}" ]; then
    expect=$(tr -d ' \t\n\r' < "${expect_file}")
  fi
  echo "[test] ${name}"
  cargo run -- "${test}" > "${asm}"
  env -u NIX_LDFLAGS -u NIX_LDFLAGS_COMPILE "${CC}" -o "${bin}" "${asm}" 2> >(
    while IFS= read -r line; do
      case "${line}" in
        *"warning: -z relro ignored"*|*"warning: -z now ignored"*)
          ;;
        *)
          echo "${line}" >&2
          ;;
      esac
    done
  )
  set +e
  "${SPIKE}" "${PK}" "${bin}"
  rc=$?
  set -e
  if [ "${rc}" -ne "${expect}" ]; then
    echo "[fail] ${name}: expected ${expect}, got ${rc}"
    exit 1
  fi
  echo "[ok] ${name}"
done
