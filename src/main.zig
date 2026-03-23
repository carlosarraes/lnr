const std = @import("std");
const display = @import("render/display.zig");

pub const std_options: std.Options = .{
    .log_level = .warn,
};

pub const commands = struct {
    pub const auth = @import("commands/auth.zig");
    pub const issue = @import("commands/issue.zig");
};

pub const api = struct {
    pub const client = @import("api/client.zig");
    pub const queries = @import("api/queries.zig");
    pub const types = @import("api/types.zig");
};

pub const auth_mod = struct {
    pub const oauth = @import("auth/oauth.zig");
    pub const token = @import("auth/token.zig");
    pub const config = @import("auth/config.zig");
};

pub const render = struct {
    pub const display_mod = @import("render/display.zig");
    pub const issue = @import("render/issue.zig");
    pub const table = @import("render/table.zig");
};

pub fn resolveAlias(comptime map: anytype, arg: []const u8) []const u8 {
    inline for (map) |entry| {
        if (std.mem.eql(u8, arg, entry[0])) return entry[1];
    }
    return arg;
}

const top_aliases = .{
    .{ "i", "issue" },
};

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer std.debug.assert(gpa.deinit() == .ok);
    const allocator = gpa.allocator();

    const args = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, args);

    if (args.len < 2) {
        printUsage();
        return;
    }

    const command = resolveAlias(top_aliases, args[1]);
    const sub_args = args[2..];

    if (std.mem.eql(u8, command, "auth")) {
        commands.auth.run(allocator, sub_args) catch |err| return handleCmdError(err);
    } else if (std.mem.eql(u8, command, "issue")) {
        commands.issue.run(allocator, sub_args) catch |err| return handleCmdError(err);
    } else if (std.mem.eql(u8, command, "help")) {
        printUsage();
    } else {
        var se = display.StdErr.init();
        se.w().print("unknown command: {s}\n\n", .{command}) catch {};
        se.w().flush() catch {};
        printUsage();
    }
}

fn handleCmdError(err: anyerror) void {
    switch (err) {
        error.FileNotFound, error.Unauthorized, error.RefreshFailed, error.RequestFailed => {},
        else => {
            var se = display.StdErr.init();
            se.w().print("error: {s}\n", .{@errorName(err)}) catch {};
            se.w().flush() catch {};
        },
    }
}

fn printUsage() void {
    var so = display.StdOut.init();
    const stdout = so.w();
    stdout.print(
        \\lnr — Linear CLI
        \\
        \\Usage: lnr <command> [options]
        \\
        \\Commands:
        \\  auth    Authenticate with Linear (login, logout, status)
        \\  issue   Manage issues (list, view)
        \\
        \\Aliases:
        \\  i       issue
        \\
        \\Run 'lnr <command> --help' for more information.
        \\
    , .{}) catch {};
    stdout.flush() catch {};
}

test "resolveAlias maps short forms" {
    try std.testing.expectEqualStrings("issue", resolveAlias(top_aliases, "i"));
    try std.testing.expectEqualStrings("auth", resolveAlias(top_aliases, "auth"));
    try std.testing.expectEqualStrings("unknown", resolveAlias(top_aliases, "unknown"));

    const sub_aliases = .{ .{ "ls", "list" }, .{ "l", "list" }, .{ "v", "view" } };
    try std.testing.expectEqualStrings("list", resolveAlias(sub_aliases, "ls"));
    try std.testing.expectEqualStrings("list", resolveAlias(sub_aliases, "l"));
    try std.testing.expectEqualStrings("view", resolveAlias(sub_aliases, "v"));
}
