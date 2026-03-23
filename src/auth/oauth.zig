const std = @import("std");
const config = @import("config.zig");
const types = @import("../api/types.zig");

const log = std.log.scoped(.oauth);

pub const LINEAR_CLIENT_ID = "85d20567e5636f5dccc9a369bb6a16dc";
const redirect_uri = "http://127.0.0.1:8484/callback";
const authorize_url = "https://linear.app/oauth/authorize";
const token_url = "https://api.linear.app/oauth/token";

pub const OAuthResult = struct {
    access_token: []const u8,
    refresh_token: []const u8,
    expires_in: i64,
};

pub fn generateVerifier(buf: *[43]u8) void {
    var random_bytes: [32]u8 = undefined;
    std.crypto.random.bytes(&random_bytes);
    _ = std.base64.url_safe_no_pad.Encoder.encode(buf, &random_bytes);
}

pub fn generateChallenge(verifier: []const u8, buf: *[43]u8) void {
    var hash: [32]u8 = undefined;
    std.crypto.hash.sha2.Sha256.hash(verifier, &hash, .{});
    _ = std.base64.url_safe_no_pad.Encoder.encode(buf, &hash);
}

pub fn generateState(buf: *[32]u8) void {
    var random_bytes: [16]u8 = undefined;
    std.crypto.random.bytes(&random_bytes);
    buf.* = std.fmt.bytesToHex(random_bytes, .lower);
}

pub fn buildAuthorizeUrl(allocator: std.mem.Allocator, state: []const u8, challenge: []const u8) ![]const u8 {
    return std.fmt.allocPrint(allocator, "{s}?client_id={s}&redirect_uri={s}&response_type=code&scope=read,write&state={s}&code_challenge={s}&code_challenge_method=S256&prompt=consent", .{
        authorize_url,
        LINEAR_CLIENT_ID,
        redirect_uri,
        state,
        challenge,
    });
}

pub fn openBrowser(url: []const u8) !void {
    const argv = switch (@import("builtin").os.tag) {
        .linux => &[_][]const u8{ "xdg-open", url },
        .macos => &[_][]const u8{ "open", url },
        .windows => &[_][]const u8{ "cmd", "/c", "start", url },
        else => return error.OAuthFailed,
    };
    var child = std.process.Child.init(argv, std.heap.page_allocator);
    _ = child.spawnAndWait() catch return error.OAuthFailed;
}

const error_page = "<html><body><h1>Authentication Failed</h1><p>Something went wrong. Please try again.</p></body></html>";
const success_page = "<html><body><h1>Success!</h1><p>You can close this window.</p></body></html>";

fn respondError(request_obj: *std.http.Server.Request) void {
    request_obj.respond(error_page, .{ .status = .bad_request, .keep_alive = false }) catch {};
}

pub fn waitForCallback(allocator: std.mem.Allocator, expected_state: []const u8) ![]const u8 {
    const address = std.net.Address.parseIp("127.0.0.1", 8484) catch unreachable;
    var server = address.listen(.{ .reuse_address = true }) catch return error.OAuthFailed;
    defer server.deinit();

    log.info("waiting for OAuth callback on :8484...", .{});

    const conn = server.accept() catch return error.OAuthFailed;
    defer conn.stream.close();

    var read_buf: [4096]u8 = undefined;
    var write_buf: [4096]u8 = undefined;
    var stream_reader = conn.stream.reader(&read_buf);
    var stream_writer = conn.stream.writer(&write_buf);
    var http_server = std.http.Server.init(stream_reader.interface(), &stream_writer.interface);

    var request_obj = http_server.receiveHead() catch return error.OAuthFailed;

    const target = request_obj.head.target;

    if (extractParam(target, "error")) |oauth_err| {
        log.err("OAuth error: {s}", .{oauth_err});
        respondError(&request_obj);
        return error.OAuthFailed;
    }

    const raw_code = extractParam(target, "code") orelse {
        respondError(&request_obj);
        return error.OAuthFailed;
    };

    const state = extractParam(target, "state") orelse {
        respondError(&request_obj);
        return error.InvalidState;
    };

    if (!std.mem.eql(u8, state, expected_state)) {
        log.err("state mismatch in OAuth callback", .{});
        respondError(&request_obj);
        return error.InvalidState;
    }

    const code_buf = try allocator.dupe(u8, raw_code);
    const decoded_code = std.Uri.percentDecodeInPlace(code_buf);
    const code_owned = try allocator.dupe(u8, decoded_code);
    allocator.free(code_buf);

    request_obj.respond(success_page, .{ .status = .ok, .keep_alive = false }) catch {};

    return code_owned;
}

pub fn urlEncodeValue(w: *std.Io.Writer, value: []const u8) !void {
    for (value) |c| {
        if (std.ascii.isAlphanumeric(c) or c == '-' or c == '_' or c == '.' or c == '~') {
            try w.writeByte(c);
        } else {
            try w.print("%{X:0>2}", .{c});
        }
    }
}

pub fn exchangeCode(allocator: std.mem.Allocator, code: []const u8, verifier: []const u8) !OAuthResult {
    var client = std.http.Client{ .allocator = allocator };
    defer client.deinit();

    var payload_aw: std.Io.Writer.Allocating = .init(allocator);
    defer payload_aw.deinit();
    const pw = &payload_aw.writer;

    try pw.writeAll("grant_type=authorization_code&client_id=");
    try urlEncodeValue(pw, LINEAR_CLIENT_ID);
    try pw.writeAll("&redirect_uri=");
    try urlEncodeValue(pw, redirect_uri);
    try pw.writeAll("&code=");
    try urlEncodeValue(pw, code);
    try pw.writeAll("&code_verifier=");
    try urlEncodeValue(pw, verifier);
    try pw.flush();

    var aw: std.Io.Writer.Allocating = .init(allocator);
    errdefer aw.deinit();

    const result = client.fetch(.{
        .location = .{ .url = token_url },
        .method = .POST,
        .payload = payload_aw.written(),
        .headers = .{
            .content_type = .{ .override = "application/x-www-form-urlencoded" },
        },
        .response_writer = &aw.writer,
    }) catch return error.OAuthFailed;

    if (result.status != .ok) {
        log.err("token exchange failed with status {}", .{result.status});
        return error.OAuthFailed;
    }

    aw.writer.flush() catch return error.OAuthFailed;
    const response_body = aw.written();

    const parsed = std.json.parseFromSlice(
        types.TokenResponse,
        allocator,
        response_body,
        .{ .ignore_unknown_fields = true, .allocate = .alloc_always },
    ) catch return error.OAuthFailed;
    defer parsed.deinit();

    const access = try allocator.dupe(u8, parsed.value.access_token);
    errdefer allocator.free(access);
    const refresh = if (parsed.value.refresh_token) |rt| try allocator.dupe(u8, rt) else try allocator.dupe(u8, "");

    return .{ .access_token = access, .refresh_token = refresh, .expires_in = parsed.value.expires_in };
}

fn extractParam(target: []const u8, name: []const u8) ?[]const u8 {
    const query_start = std.mem.indexOfScalar(u8, target, '?') orelse return null;
    var rest = target[query_start + 1 ..];
    while (rest.len > 0) {
        const pair_end = std.mem.indexOfScalar(u8, rest, '&') orelse rest.len;
        const pair = rest[0..pair_end];
        const eq = std.mem.indexOfScalar(u8, pair, '=') orelse {
            rest = if (pair_end < rest.len) rest[pair_end + 1 ..] else rest[rest.len..];
            continue;
        };
        if (std.mem.eql(u8, pair[0..eq], name)) {
            return pair[eq + 1 ..];
        }
        rest = if (pair_end < rest.len) rest[pair_end + 1 ..] else rest[rest.len..];
    }
    return null;
}

test "extractParam" {
    try std.testing.expectEqualStrings("abc123", extractParam("/callback?code=abc123&state=xyz", "code").?);
    try std.testing.expectEqualStrings("xyz", extractParam("/callback?code=abc123&state=xyz", "state").?);
    try std.testing.expect(extractParam("/callback?code=abc123", "state") == null);
    try std.testing.expect(extractParam("/callback", "code") == null);
}
