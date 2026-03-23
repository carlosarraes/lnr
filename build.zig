const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const clap = b.dependency("clap", .{});

    const exe = b.addExecutable(.{
        .name = "lnr",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/main.zig"),
            .target = target,
            .optimize = optimize,
            .imports = &.{
                .{ .name = "clap", .module = clap.module("clap") },
            },
        }),
    });

    b.installArtifact(exe);

    const run_step = b.step("run", "Run lnr");
    const run_cmd = b.addRunArtifact(exe);
    run_step.dependOn(&run_cmd.step);
    run_cmd.step.dependOn(b.getInstallStep());
    if (b.args) |args| {
        run_cmd.addArgs(args);
    }

    const exe_tests = b.addTest(.{
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/main.zig"),
            .target = target,
            .optimize = optimize,
            .imports = &.{
                .{ .name = "clap", .module = clap.module("clap") },
            },
        }),
    });

    const test_step = b.step("test", "Run tests");
    test_step.dependOn(&b.addRunArtifact(exe_tests).step);
}
