const std = @import("std");
const types = @import("../api/types.zig");
const display = @import("display.zig");

pub fn printIssueTable(issues: []const types.Issue) !void {
    var so = display.StdOut.init();
    const stdout = so.w();

    try display.colorPrint(stdout, .bold, "{s:<12}{s:<40}{s:<16}{s:<12}{s}\n", .{
        "ID", "Title", "State", "Priority", "Assignee",
    });
    try display.separator(stdout, 90);

    for (issues) |issue| {
        try stdout.print("{s:<12}", .{issue.identifier});

        const title = display.truncate(issue.title, 37);
        try stdout.print("{s:<40}", .{title});

        const state_col = display.stateColor(issue.state.name);
        try display.colorPrint(stdout, state_col, "{s:<16}", .{issue.state.name});

        const prio_col = display.priorityColor(issue.priority);
        const prio_label = issue.priority_label orelse "None";
        try display.colorPrint(stdout, prio_col, "{s:<12}", .{prio_label});

        if (issue.assignee) |assignee| {
            try stdout.print("{s}", .{assignee.display_name orelse "Unassigned"});
        } else {
            try display.colorWrite(stdout, .dim, "Unassigned");
        }
        try stdout.writeByte('\n');
    }
    try stdout.flush();
}
