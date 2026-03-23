const std = @import("std");
const oauth = @import("../auth/oauth.zig");
const config = @import("../auth/config.zig");
const token = @import("../auth/token.zig");
const Client = @import("../api/client.zig").Client;
const display = @import("../render/display.zig");

pub fn run(allocator: std.mem.Allocator, args: []const [:0]const u8) !void {
    if (args.len == 0) {
        printHelp();
        return;
    }

    const sub = args[0];
    if (std.mem.eql(u8, sub, "login")) {
        try login(allocator);
    } else if (std.mem.eql(u8, sub, "logout")) {
        try logout(allocator);
    } else if (std.mem.eql(u8, sub, "status")) {
        try status(allocator);
    } else if (std.mem.eql(u8, sub, "--help") or std.mem.eql(u8, sub, "-h")) {
        printHelp();
    } else {
        var se = display.StdErr.init();
        se.w().print("unknown auth command: {s}\n\n", .{sub}) catch {};
        se.w().flush() catch {};
        printHelp();
    }
}

fn login(allocator: std.mem.Allocator) !void {
    var so = display.StdOut.init();
    const stdout = so.w();

    var verifier: [43]u8 = undefined;
    oauth.generateVerifier(&verifier);

    var challenge: [43]u8 = undefined;
    oauth.generateChallenge(&verifier, &challenge);

    var state: [32]u8 = undefined;
    oauth.generateState(&state);

    const url = try oauth.buildAuthorizeUrl(allocator, &state, &challenge);
    defer allocator.free(url);

    try stdout.writeAll("Opening browser for Linear authentication...\n");
    try stdout.flush();

    oauth.openBrowser(url) catch {
        try stdout.print("Could not open browser. Visit this URL manually:\n{s}\n", .{url});
        try stdout.flush();
    };

    const code = try oauth.waitForCallback(allocator, &state);
    defer allocator.free(code);

    try stdout.writeAll("Exchanging authorization code...\n");
    try stdout.flush();

    const result = try oauth.exchangeCode(allocator, code, &verifier);
    defer allocator.free(result.access_token);
    defer allocator.free(result.refresh_token);

    const now = std.time.timestamp();
    try config.save(allocator, .{
        .access_token = result.access_token,
        .refresh_token = result.refresh_token,
        .expires_at = now + result.expires_in,
    });

    try stdout.writeAll("Authenticated successfully!\n");
    try stdout.flush();
}

fn logout(allocator: std.mem.Allocator) !void {
    var so = display.StdOut.init();
    try config.delete(allocator);
    try so.w().writeAll("Logged out successfully.\n");
    try so.w().flush();
}

fn status(allocator: std.mem.Allocator) !void {
    var so = display.StdOut.init();
    const stdout = so.w();

    const access_token = token.loadValidToken(allocator) catch |err| {
        const msg = switch (err) {
            error.FileNotFound => "Not authenticated. Run 'lnr auth login' to authenticate.\n",
            error.RefreshFailed => "Session expired. Run 'lnr auth login' to re-authenticate.\n",
            else => "Authentication error. Run 'lnr auth login' to re-authenticate.\n",
        };
        try stdout.writeAll(msg);
        try stdout.flush();
        return;
    };
    defer allocator.free(access_token);

    var api_client = Client.init(allocator, access_token);
    const viewer = api_client.getViewer() catch |err| {
        const msg = switch (err) {
            error.Unauthorized => "Token is invalid. Run 'lnr auth login' to re-authenticate.\n",
            error.RequestFailed => "Failed to reach Linear API. Check your network connection.\n",
            else => "Failed to fetch user info. Run 'lnr auth login' to re-authenticate.\n",
        };
        try stdout.writeAll(msg);
        try stdout.flush();
        return;
    };
    defer allocator.free(viewer.id);
    defer allocator.free(viewer.display_name);
    defer allocator.free(viewer.email);

    try stdout.print("Logged in as: {s} ({s})\n", .{ viewer.display_name, viewer.email });
    try stdout.flush();
}

fn printHelp() void {
    var so = display.StdOut.init();
    so.w().print(
        \\Usage: lnr auth <command>
        \\
        \\Commands:
        \\  login     Authenticate with Linear via OAuth
        \\  logout    Remove stored credentials
        \\  status    Show current authentication status
        \\
    , .{}) catch {};
    so.w().flush() catch {};
}
