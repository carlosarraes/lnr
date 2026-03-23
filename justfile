default:
    just --list

build:
    zig build
    cp zig-out/bin/lnr ~/.local/bin/

release:
    zig build -Doptimize=ReleaseSmall
    cp zig-out/bin/lnr ~/.local/bin/

run *ARGS:
    zig build run -- {{ARGS}}

test:
    zig build test --summary all

clean:
    rm -rf zig-out .zig-cache
