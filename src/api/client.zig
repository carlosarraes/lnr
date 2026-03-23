const std = @import("std");
const types = @import("types.zig");

const log = std.log.scoped(.api);

const graphql_endpoint = "https://api.linear.app/graphql";

pub const Client = struct {
    allocator: std.mem.Allocator,
    access_token: []const u8,

    pub fn init(allocator: std.mem.Allocator, access_token: []const u8) Client {
        return .{ .allocator = allocator, .access_token = access_token };
    }

    pub fn request(self: *Client, query: []const u8, variables: ?[]const u8) ![]const u8 {
        var client = std.http.Client{ .allocator = self.allocator };
        defer client.deinit();

        var json_aw: std.Io.Writer.Allocating = .init(self.allocator);
        defer json_aw.deinit();
        const jw = &json_aw.writer;

        try jw.writeAll("{\"query\":\"");
        for (query) |c| {
            switch (c) {
                '"' => try jw.writeAll("\\\""),
                '\\' => try jw.writeAll("\\\\"),
                '\n' => try jw.writeAll("\\n"),
                else => try jw.writeByte(c),
            }
        }
        try jw.writeByte('"');
        if (variables) |vars| {
            try jw.writeAll(",\"variables\":");
            try jw.writeAll(vars);
        }
        try jw.writeByte('}');
        try jw.flush();

        var auth_buf: [256]u8 = undefined;
        const auth_header = std.fmt.bufPrint(&auth_buf, "Bearer {s}", .{self.access_token}) catch return error.RequestFailed;

        var aw: std.Io.Writer.Allocating = .init(self.allocator);
        errdefer aw.deinit();

        const result = client.fetch(.{
            .location = .{ .url = graphql_endpoint },
            .method = .POST,
            .payload = json_aw.written(),
            .headers = .{
                .content_type = .{ .override = "application/json" },
                .authorization = .{ .override = auth_header },
            },
            .response_writer = &aw.writer,
        }) catch |err| {
            log.debug("HTTP request failed: {}", .{err});
            return error.RequestFailed;
        };

        if (result.status != .ok) {
            log.debug("API returned status {}", .{result.status});
            if (result.status == .unauthorized) return error.Unauthorized;
            return error.RequestFailed;
        }

        aw.writer.flush() catch return error.RequestFailed;
        return aw.toOwnedSlice() catch return error.RequestFailed;
    }

    pub fn getViewer(self: *Client) !types.Viewer {
        const response = try self.request(@import("queries.zig").viewer, null);
        defer self.allocator.free(response);

        const parsed = std.json.parseFromSlice(
            struct { data: struct { viewer: struct {
                id: []const u8,
                displayName: []const u8,
                email: []const u8,
            } } },
            self.allocator,
            response,
            .{ .ignore_unknown_fields = true, .allocate = .alloc_always },
        ) catch return error.ParseFailed;
        defer parsed.deinit();

        const id = try self.allocator.dupe(u8, parsed.value.data.viewer.id);
        errdefer self.allocator.free(id);
        const display_name = try self.allocator.dupe(u8, parsed.value.data.viewer.displayName);
        errdefer self.allocator.free(display_name);
        const email = try self.allocator.dupe(u8, parsed.value.data.viewer.email);

        return .{ .id = id, .display_name = display_name, .email = email };
    }
};
