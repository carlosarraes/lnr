const std = @import("std");

pub const IssueIdentifier = struct {
    team_key: []const u8,
    number: u32,

    pub fn parse(input: []const u8) error{InvalidFormat}!IssueIdentifier {
        const sep = std.mem.indexOfScalar(u8, input, '-') orelse return error.InvalidFormat;
        if (sep == 0 or sep == input.len - 1) return error.InvalidFormat;

        const key = input[0..sep];
        for (key) |c| {
            if (!std.ascii.isAlphabetic(c)) return error.InvalidFormat;
        }

        const num_str = input[sep + 1 ..];
        const number = std.fmt.parseInt(u32, num_str, 10) catch return error.InvalidFormat;

        return .{ .team_key = key, .number = number };
    }
};

pub const User = struct {
    id: ?[]const u8 = null,
    display_name: ?[]const u8 = null,
    email: ?[]const u8 = null,
};

pub const State = struct {
    name: []const u8,
    color: ?[]const u8 = null,
};

pub const Team = struct {
    name: []const u8,
    key: []const u8,
};

pub const Label = struct {
    name: []const u8,
    color: ?[]const u8 = null,
};

pub const Comment = struct {
    body: []const u8,
    created_at: []const u8,
    user: User,
};

pub const Issue = struct {
    identifier: []const u8,
    title: []const u8,
    description: ?[]const u8 = null,
    state: State,
    priority: u8 = 0,
    priority_label: ?[]const u8 = null,
    assignee: ?User = null,
    team: ?Team = null,
    project_name: ?[]const u8 = null,
    labels: ?[]Label = null,
    due_date: ?[]const u8 = null,
    estimate: ?f64 = null,
    comments: ?[]Comment = null,
};

pub const Viewer = struct {
    id: []const u8,
    display_name: []const u8,
    email: []const u8,
};

pub const IssueListOpts = struct {
    team: ?[]const u8 = null,
    state: ?[]const u8 = null,
    assignee: ?[]const u8 = null,
    limit: u32 = 25,
};

pub const TokenResponse = struct {
    access_token: []const u8,
    refresh_token: ?[]const u8 = null,
    expires_in: i64 = 3600,
};

test "IssueIdentifier.parse valid" {
    const id = try IssueIdentifier.parse("MON-438");
    try std.testing.expectEqualStrings("MON", id.team_key);
    try std.testing.expectEqual(@as(u32, 438), id.number);
}

test "IssueIdentifier.parse long number" {
    const id = try IssueIdentifier.parse("AB-123456");
    try std.testing.expectEqualStrings("AB", id.team_key);
    try std.testing.expectEqual(@as(u32, 123456), id.number);
}

test "IssueIdentifier.parse long key" {
    const id = try IssueIdentifier.parse("INFRA-1");
    try std.testing.expectEqualStrings("INFRA", id.team_key);
    try std.testing.expectEqual(@as(u32, 1), id.number);
}

test "IssueIdentifier.parse invalid" {
    try std.testing.expectError(error.InvalidFormat, IssueIdentifier.parse("MON"));
    try std.testing.expectError(error.InvalidFormat, IssueIdentifier.parse("-438"));
    try std.testing.expectError(error.InvalidFormat, IssueIdentifier.parse("MON-"));
    try std.testing.expectError(error.InvalidFormat, IssueIdentifier.parse("123-456"));
    try std.testing.expectError(error.InvalidFormat, IssueIdentifier.parse("MON-abc"));
}
