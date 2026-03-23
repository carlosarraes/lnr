const std = @import("std");
const types = @import("../api/types.zig");
const Client = @import("../api/client.zig").Client;
const queries = @import("../api/queries.zig");
const token = @import("../auth/token.zig");
const display = @import("../render/display.zig");
const table = @import("../render/table.zig");
const issue_render = @import("../render/issue.zig");
const resolveAlias = @import("../main.zig").resolveAlias;

const log = std.log.scoped(.issue_cmd);

fn requireAuth(allocator: std.mem.Allocator) ![]const u8 {
    return token.loadValidToken(allocator) catch |err| {
        var se = display.StdErr.init();
        const msg: ?[]const u8 = switch (err) {
            error.FileNotFound => "Not authenticated. Run 'lnr auth login' first.\n",
            error.RefreshFailed => "Session expired. Run 'lnr auth login' to re-authenticate.\n",
            else => null,
        };
        if (msg) |m| {
            se.w().writeAll(m) catch {};
            se.w().flush() catch {};
        }
        return err;
    };
}

fn handleApiError(err: anyerror) void {
    var se = display.StdErr.init();
    const msg = switch (err) {
        error.Unauthorized => "Session expired or invalid. Run 'lnr auth login' to re-authenticate.\n",
        error.RequestFailed => "Request failed. Check your network connection.\n",
        else => "Unexpected error. Try again.\n",
    };
    se.w().writeAll(msg) catch {};
    se.w().flush() catch {};
}

const sub_aliases = .{
    .{ "ls", "list" },
    .{ "l", "list" },
    .{ "v", "view" },
};

pub fn run(allocator: std.mem.Allocator, args: []const [:0]const u8) !void {
    if (args.len == 0) {
        printHelp();
        return;
    }

    const sub = resolveAlias(sub_aliases, args[0]);
    const sub_args = args[1..];

    if (std.mem.eql(u8, sub, "list")) {
        try list(allocator, sub_args);
    } else if (std.mem.eql(u8, sub, "view")) {
        try view(allocator, sub_args);
    } else if (std.mem.eql(u8, sub, "--help") or std.mem.eql(u8, sub, "-h")) {
        printHelp();
    } else {
        var se = display.StdErr.init();
        se.w().print("unknown issue command: {s}\n\n", .{sub}) catch {};
        se.w().flush() catch {};
        printHelp();
    }
}

fn list(allocator: std.mem.Allocator, args: []const [:0]const u8) !void {
    var opts = types.IssueListOpts{};

    var i: usize = 0;
    while (i < args.len) : (i += 1) {
        const arg = args[i];
        if ((std.mem.eql(u8, arg, "--team") or std.mem.eql(u8, arg, "-t")) and i + 1 < args.len) {
            i += 1;
            opts.team = args[i];
        } else if ((std.mem.eql(u8, arg, "--state") or std.mem.eql(u8, arg, "-s")) and i + 1 < args.len) {
            i += 1;
            opts.state = args[i];
        } else if ((std.mem.eql(u8, arg, "--assignee") or std.mem.eql(u8, arg, "-a")) and i + 1 < args.len) {
            i += 1;
            opts.assignee = args[i];
        } else if ((std.mem.eql(u8, arg, "--limit") or std.mem.eql(u8, arg, "-n")) and i + 1 < args.len) {
            i += 1;
            opts.limit = std.fmt.parseInt(u32, args[i], 10) catch 25;
        } else if (std.mem.eql(u8, arg, "--help") or std.mem.eql(u8, arg, "-h")) {
            printListHelp();
            return;
        }
    }

    const access_token = try requireAuth(allocator);
    defer allocator.free(access_token);

    var api_client = Client.init(allocator, access_token);

    const variables = try buildListVariables(allocator, opts);
    defer allocator.free(variables);

    const response = api_client.request(queries.issue_list, variables) catch |err| {
        return handleApiError(err);
    };
    defer allocator.free(response);

    // Arena for parsed issue data — freed all at once
    var arena = std.heap.ArenaAllocator.init(allocator);
    defer arena.deinit();

    const issues = try parseIssueNodes(arena.allocator(), response);

    if (issues.len == 0) {
        var so = display.StdOut.init();
        try so.w().writeAll("No issues found.\n");
        try so.w().flush();
        return;
    }

    try table.printIssueTable(issues);
}

fn view(allocator: std.mem.Allocator, args: []const [:0]const u8) !void {
    if (args.len == 0) {
        var se = display.StdErr.init();
        se.w().writeAll("Usage: lnr issue view <ID>\n") catch {};
        se.w().flush() catch {};
        return;
    }

    const id = types.IssueIdentifier.parse(args[0]) catch {
        var se = display.StdErr.init();
        se.w().print("Invalid issue identifier: {s}\nExpected format: MON-438\n", .{args[0]}) catch {};
        se.w().flush() catch {};
        return;
    };

    const access_token = try requireAuth(allocator);
    defer allocator.free(access_token);

    var api_client = Client.init(allocator, access_token);

    const variables = try std.fmt.allocPrint(
        allocator,
        "{{\"filter\":{{\"team\":{{\"key\":{{\"eq\":\"{s}\"}}}},\"number\":{{\"eq\":{d}}}}}}}",
        .{ id.team_key, id.number },
    );
    defer allocator.free(variables);

    const response = api_client.request(queries.issue_view, variables) catch |err| {
        return handleApiError(err);
    };
    defer allocator.free(response);

    var arena = std.heap.ArenaAllocator.init(allocator);
    defer arena.deinit();
    const arena_alloc = arena.allocator();

    const issues = try parseIssueNodes(arena_alloc, response);
    if (issues.len == 0) {
        var se = display.StdErr.init();
        se.w().print("Issue not found: {s}\n", .{args[0]}) catch {};
        se.w().flush() catch {};
        return;
    }

    try issue_render.printIssueDetail(arena_alloc, issues[0]);
}

fn jsonEscapeString(w: *std.Io.Writer, input: []const u8) !void {
    for (input) |c| {
        switch (c) {
            '"' => try w.writeAll("\\\""),
            '\\' => try w.writeAll("\\\\"),
            '\n' => try w.writeAll("\\n"),
            '\r' => try w.writeAll("\\r"),
            '\t' => try w.writeAll("\\t"),
            else => try w.writeByte(c),
        }
    }
}

fn buildListVariables(allocator: std.mem.Allocator, opts: types.IssueListOpts) ![]const u8 {
    var aw: std.Io.Writer.Allocating = .init(allocator);
    errdefer aw.deinit();
    const w = &aw.writer;

    try w.writeAll("{\"filter\":{");

    var has_filter = false;
    if (opts.team) |t| {
        try w.writeAll("\"team\":{\"key\":{\"eq\":\"");
        try jsonEscapeString(w, t);
        try w.writeAll("\"}}");
        has_filter = true;
    }
    if (opts.state) |s| {
        if (has_filter) try w.writeByte(',');
        try w.writeAll("\"state\":{\"name\":{\"eqCaseInsensitive\":\"");
        try jsonEscapeString(w, s);
        try w.writeAll("\"}}");
        has_filter = true;
    }
    if (opts.assignee) |a| {
        if (has_filter) try w.writeByte(',');
        try w.writeAll("\"assignee\":{\"displayName\":{\"eqCaseInsensitive\":\"");
        try jsonEscapeString(w, a);
        try w.writeAll("\"}}");
    }

    try w.print("}},\"first\":{d}}}", .{opts.limit});
    try w.flush();

    return aw.toOwnedSlice() catch return error.RequestFailed;
}

const JsonIssueNode = struct {
    identifier: []const u8 = "",
    title: []const u8 = "",
    description: ?[]const u8 = null,
    priority: u8 = 0,
    priorityLabel: ?[]const u8 = null,
    dueDate: ?[]const u8 = null,
    estimate: ?f64 = null,
    state: ?struct {
        name: []const u8 = "",
        color: ?[]const u8 = null,
    } = null,
    assignee: ?struct {
        displayName: ?[]const u8 = null,
        email: ?[]const u8 = null,
    } = null,
    team: ?struct {
        name: []const u8 = "",
        key: []const u8 = "",
    } = null,
    project: ?struct {
        name: []const u8 = "",
    } = null,
    labels: ?struct {
        nodes: ?[]struct {
            name: []const u8 = "",
            color: ?[]const u8 = null,
        } = null,
    } = null,
    comments: ?struct {
        nodes: ?[]struct {
            body: []const u8 = "",
            createdAt: []const u8 = "",
            user: ?struct {
                displayName: ?[]const u8 = null,
            } = null,
        } = null,
    } = null,
};

const IssueResponse = struct {
    data: ?struct {
        issues: ?struct {
            nodes: ?[]JsonIssueNode = null,
        } = null,
    } = null,
};

fn parseIssueNodes(allocator: std.mem.Allocator, response: []const u8) ![]types.Issue {
    const parsed = std.json.parseFromSlice(
        IssueResponse,
        allocator,
        response,
        .{ .ignore_unknown_fields = true, .allocate = .alloc_always },
    ) catch return error.ParseFailed;
    defer parsed.deinit();

    const data = parsed.value.data orelse return error.ParseFailed;
    const issues_data = data.issues orelse return error.ParseFailed;
    const nodes = issues_data.nodes orelse return &[_]types.Issue{};

    var issues = try allocator.alloc(types.Issue, nodes.len);
    for (nodes, 0..) |node, idx| {
        issues[idx] = mapNodeToIssue(allocator, node) catch return error.ParseFailed;
    }
    return issues;
}

fn mapNodeToIssue(allocator: std.mem.Allocator, node: JsonIssueNode) !types.Issue {
    var labels: ?[]types.Label = null;
    if (node.labels) |l| {
        if (l.nodes) |label_nodes| {
            var label_list = try allocator.alloc(types.Label, label_nodes.len);
            for (label_nodes, 0..) |ln, idx| {
                label_list[idx] = .{
                    .name = try allocator.dupe(u8, ln.name),
                    .color = if (ln.color) |c| try allocator.dupe(u8, c) else null,
                };
            }
            labels = label_list;
        }
    }

    var comments: ?[]types.Comment = null;
    if (node.comments) |c| {
        if (c.nodes) |comment_nodes| {
            var comment_list = try allocator.alloc(types.Comment, comment_nodes.len);
            for (comment_nodes, 0..) |cn, idx| {
                comment_list[idx] = .{
                    .body = try allocator.dupe(u8, cn.body),
                    .created_at = try allocator.dupe(u8, cn.createdAt),
                    .user = .{
                        .display_name = if (cn.user) |u| if (u.displayName) |dn| try allocator.dupe(u8, dn) else null else null,
                    },
                };
            }
            comments = comment_list;
        }
    }

    return .{
        .identifier = try allocator.dupe(u8, node.identifier),
        .title = try allocator.dupe(u8, node.title),
        .description = if (node.description) |d| try allocator.dupe(u8, d) else null,
        .state = if (node.state) |s| .{
            .name = try allocator.dupe(u8, s.name),
            .color = if (s.color) |c| try allocator.dupe(u8, c) else null,
        } else .{ .name = try allocator.dupe(u8, "Unknown") },
        .priority = node.priority,
        .priority_label = if (node.priorityLabel) |pl| try allocator.dupe(u8, pl) else null,
        .assignee = if (node.assignee) |a| .{
            .display_name = if (a.displayName) |dn| try allocator.dupe(u8, dn) else null,
            .email = if (a.email) |e| try allocator.dupe(u8, e) else null,
        } else null,
        .team = if (node.team) |t| .{
            .name = try allocator.dupe(u8, t.name),
            .key = try allocator.dupe(u8, t.key),
        } else null,
        .project_name = if (node.project) |p| try allocator.dupe(u8, p.name) else null,
        .labels = labels,
        .due_date = if (node.dueDate) |d| try allocator.dupe(u8, d) else null,
        .estimate = node.estimate,
        .comments = comments,
    };
}

fn printHelp() void {
    var so = display.StdOut.init();
    so.w().print(
        \\Usage: lnr issue <command> [options]
        \\
        \\Commands:
        \\  list    List issues (aliases: ls, l)
        \\  view    View issue details (alias: v)
        \\
        \\Run 'lnr issue <command> --help' for more information.
        \\
    , .{}) catch {};
    so.w().flush() catch {};
}

fn printListHelp() void {
    var so = display.StdOut.init();
    so.w().print(
        \\Usage: lnr issue list [options]
        \\
        \\Options:
        \\  -t, --team <TEAM>         Filter by team key
        \\  -s, --state <STATE>       Filter by state name
        \\  -a, --assignee <NAME>     Filter by assignee name
        \\  -n, --limit <N>           Max results (default: 25)
        \\  -h, --help                Show this help
        \\
    , .{}) catch {};
    so.w().flush() catch {};
}
