const std = @import("std");
const types = @import("../api/types.zig");
const display = @import("display.zig");

pub fn printIssueDetail(allocator: std.mem.Allocator, issue: types.Issue) !void {
    var so = display.StdOut.init();
    const stdout = so.w();

    try display.colorPrint(stdout, .bold, "{s}  {s}", .{ issue.identifier, issue.title });
    const state_col = display.stateColor(issue.state.name);
    try stdout.writeAll("  ");
    try display.colorPrint(stdout, state_col, "{s}", .{issue.state.name});
    try stdout.writeByte('\n');

    try display.separator(stdout, 70);

    if (issue.team) |team| {
        try display.colorPrint(stdout, .dim, "Team: ", .{});
        try stdout.print("{s}    ", .{team.name});
    }

    try display.colorPrint(stdout, .dim, "Priority: ", .{});
    const prio_col = display.priorityColor(issue.priority);
    try display.colorPrint(stdout, prio_col, "{s}", .{issue.priority_label orelse "None"});
    try stdout.writeAll("    ");

    try display.colorPrint(stdout, .dim, "Assignee: ", .{});
    if (issue.assignee) |assignee| {
        try stdout.print("{s}", .{assignee.display_name orelse "Unassigned"});
    } else {
        try stdout.writeAll("Unassigned");
    }
    try stdout.writeByte('\n');

    if (issue.labels) |labels| {
        if (labels.len > 0) {
            try display.colorPrint(stdout, .dim, "Labels: ", .{});
            for (labels, 0..) |label, idx| {
                if (idx > 0) try stdout.writeAll(", ");
                try stdout.print("{s}", .{label.name});
            }
            try stdout.writeByte('\n');
        }
    }

    if (issue.due_date) |due| {
        try display.colorPrint(stdout, .dim, "Due: ", .{});
        try stdout.print("{s}\n", .{due});
    }

    if (issue.project_name) |proj| {
        try display.colorPrint(stdout, .dim, "Project: ", .{});
        try stdout.print("{s}\n", .{proj});
    }

    if (issue.description) |desc| {
        try stdout.writeAll("\n");
        try display.colorPrint(stdout, .bold, "Description:\n", .{});

        const stripped = try display.stripMarkdown(allocator, desc);
        defer allocator.free(stripped);

        var lines = std.mem.splitScalar(u8, stripped, '\n');
        while (lines.next()) |line| {
            try stdout.print("  {s}\n", .{line});
        }
    }

    if (issue.comments) |comments| {
        if (comments.len > 0) {
            try stdout.writeAll("\n");
            try display.colorPrint(stdout, .bold, "Comments ({d}):\n", .{comments.len});

            for (comments) |comment| {
                const author = comment.user.display_name orelse "Unknown";
                try display.colorPrint(stdout, .dim, "  ┃ ", .{});
                try display.colorPrint(stdout, .cyan, "{s}", .{author});
                try display.colorPrint(stdout, .dim, " · {s}\n", .{comment.created_at});

                const body = try display.stripMarkdown(allocator, comment.body);
                defer allocator.free(body);

                var lines = std.mem.splitScalar(u8, body, '\n');
                while (lines.next()) |line| {
                    try display.colorPrint(stdout, .dim, "  ┃ ", .{});
                    try stdout.print("{s}\n", .{line});
                }
                try display.colorPrint(stdout, .dim, "  ┃\n", .{});
            }
        }
    }

    try stdout.flush();
}
