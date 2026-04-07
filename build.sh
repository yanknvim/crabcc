CC="riscv64-none-elf-gcc"

cargo run -- "$@" > test.S
$CC -o test test.S
spike $(which pk) test
echo $?

