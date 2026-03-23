const std = @import("std");
const config = @import("config.zig");
const oauth = @import("oauth.zig");
const types = @import("../api/types.zig");

const log = std.log.scoped(.token);

const refresh_buffer_secs: i64 = 300;

pub fn isExpired(expires_at: i64) bool {
    const now = std.time.timestamp();
    return now >= (expires_at - refresh_buffer_secs);
}

pub fn loadValidToken(allocator: std.mem.Allocator) ![]const u8 {
    const cfg = try config.load(allocator);
    defer allocator.free(cfg.refresh_token);

    if (!isExpired(cfg.expires_at)) {
        return cfg.access_token;
    }

    defer allocator.free(cfg.access_token);

    log.debug("token expired, attempting refresh", .{});
    const new_token = try refreshToken(allocator, cfg.refresh_token);
    return new_token;
}

fn refreshToken(allocator: std.mem.Allocator, refresh_tok: []const u8) ![]const u8 {
    var client = std.http.Client{ .allocator = allocator };
    defer client.deinit();

    var payload_aw: std.Io.Writer.Allocating = .init(allocator);
    defer payload_aw.deinit();
    const pw = &payload_aw.writer;
    pw.writeAll("grant_type=refresh_token&client_id=") catch return error.RefreshFailed;
    oauth.urlEncodeValue(pw, oauth.LINEAR_CLIENT_ID) catch return error.RefreshFailed;
    pw.writeAll("&refresh_token=") catch return error.RefreshFailed;
    oauth.urlEncodeValue(pw, refresh_tok) catch return error.RefreshFailed;
    pw.flush() catch return error.RefreshFailed;

    var aw: std.Io.Writer.Allocating = .init(allocator);
    defer aw.deinit();

    const result = client.fetch(.{
        .location = .{ .url = "https://api.linear.app/oauth/token" },
        .method = .POST,
        .payload = payload_aw.written(),
        .headers = .{
            .content_type = .{ .override = "application/x-www-form-urlencoded" },
        },
        .response_writer = &aw.writer,
    }) catch return error.RefreshFailed;

    if (result.status != .ok) {
        log.debug("token refresh failed with status {}", .{result.status});
        return error.RefreshFailed;
    }

    aw.writer.flush() catch return error.RefreshFailed;

    const parsed = std.json.parseFromSlice(
        types.TokenResponse,
        allocator,
        aw.written(),
        .{ .ignore_unknown_fields = true, .allocate = .alloc_always },
    ) catch return error.RefreshFailed;
    defer parsed.deinit();

    const now = std.time.timestamp();
    const new_access = try allocator.dupe(u8, parsed.value.access_token);
    errdefer allocator.free(new_access);

    const new_refresh = if (parsed.value.refresh_token) |rt|
        try allocator.dupe(u8, rt)
    else
        try allocator.dupe(u8, refresh_tok);
    defer allocator.free(new_refresh);

    try config.save(allocator, .{
        .access_token = new_access,
        .refresh_token = new_refresh,
        .expires_at = now + parsed.value.expires_in,
    });

    return new_access;
}

test "isExpired returns true for past timestamps" {
    try std.testing.expect(isExpired(0));
    try std.testing.expect(isExpired(1000));
}

test "isExpired returns false for future timestamps" {
    const far_future = std.time.timestamp() + 10000;
    try std.testing.expect(!isExpired(far_future));
}
