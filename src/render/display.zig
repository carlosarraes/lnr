const std = @import("std");

var no_color_cached: enum { unknown, yes, no } = .unknown;

fn isNoColor() bool {
    if (no_color_cached == .unknown) {
        no_color_cached = if (std.posix.getenv("NO_COLOR") != null) .yes else .no;
    }
    return no_color_cached == .yes;
}

pub const StdOut = struct {
    buf: [4096]u8 = undefined,
    writer: std.fs.File.Writer = undefined,

    pub fn init() StdOut {
        var s: StdOut = .{};
        s.writer = std.fs.File.stdout().writer(&s.buf);
        return s;
    }

    pub fn w(self: *StdOut) *std.Io.Writer {
        return &self.writer.interface;
    }
};

pub const StdErr = struct {
    buf: [4096]u8 = undefined,
    writer: std.fs.File.Writer = undefined,

    pub fn init() StdErr {
        var s: StdErr = .{};
        s.writer = std.fs.File.stderr().writer(&s.buf);
        return s;
    }

    pub fn w(self: *StdErr) *std.Io.Writer {
        return &self.writer.interface;
    }
};

pub const Color = enum {
    reset,
    bold,
    dim,
    red,
    green,
    yellow,
    blue,
    cyan,
    white,

    pub fn code(self: Color) []const u8 {
        if (isNoColor()) return "";
        return switch (self) {
            .reset => "\x1b[0m",
            .bold => "\x1b[1m",
            .dim => "\x1b[2m",
            .red => "\x1b[31m",
            .green => "\x1b[32m",
            .yellow => "\x1b[33m",
            .blue => "\x1b[34m",
            .cyan => "\x1b[36m",
            .white => "\x1b[37m",
        };
    }
};

pub fn colorWrite(writer: *std.Io.Writer, color: Color, text: []const u8) !void {
    if (isNoColor()) {
        try writer.writeAll(text);
    } else {
        try writer.writeAll(color.code());
        try writer.writeAll(text);
        try writer.writeAll(Color.reset.code());
    }
}

pub fn colorPrint(writer: *std.Io.Writer, color: Color, comptime fmt: []const u8, args: anytype) !void {
    if (isNoColor()) {
        try writer.print(fmt, args);
    } else {
        try writer.writeAll(color.code());
        try writer.print(fmt, args);
        try writer.writeAll(Color.reset.code());
    }
}

pub fn stateColor(state_name: []const u8) Color {
    if (std.ascii.indexOfIgnoreCase(state_name, "done") != null or
        std.ascii.indexOfIgnoreCase(state_name, "complete") != null)
        return .green;
    if (std.ascii.indexOfIgnoreCase(state_name, "progress") != null or
        std.ascii.indexOfIgnoreCase(state_name, "started") != null or
        std.ascii.indexOfIgnoreCase(state_name, "review") != null)
        return .yellow;
    if (std.ascii.indexOfIgnoreCase(state_name, "cancel") != null)
        return .red;
    return .blue;
}

pub fn priorityColor(priority: u8) Color {
    return switch (priority) {
        1 => .red,
        2 => .yellow,
        3 => .white,
        else => .dim,
    };
}

pub fn separator(writer: *std.Io.Writer, width: usize) !void {
    var i: usize = 0;
    while (i < width) : (i += 1) {
        try writer.writeAll("─");
    }
    try writer.writeByte('\n');
}

pub fn truncate(text: []const u8, max_width: usize) []const u8 {
    if (text.len <= max_width) return text;
    if (max_width < 3) return text[0..max_width];
    return text[0 .. max_width - 3];
}

pub fn stripMarkdown(allocator: std.mem.Allocator, input: []const u8) ![]const u8 {
    var result: std.ArrayList(u8) = .empty;
    errdefer result.deinit(allocator);

    var i: usize = 0;
    while (i < input.len) {
        const c = input[i];
        switch (c) {
            '#' => {
                while (i < input.len and input[i] == '#') : (i += 1) {}
                if (i < input.len and input[i] == ' ') i += 1;
            },
            '*' => {
                if (i + 1 < input.len and input[i + 1] == '*') {
                    i += 2;
                } else {
                    i += 1;
                }
            },
            '`' => {
                if (i + 2 < input.len and input[i + 1] == '`' and input[i + 2] == '`') {
                    i += 3;
                    while (i + 2 < input.len) : (i += 1) {
                        if (input[i] == '`' and input[i + 1] == '`' and input[i + 2] == '`') {
                            i += 3;
                            break;
                        }
                        try result.append(allocator, input[i]);
                    }
                } else {
                    i += 1;
                }
            },
            '[' => {
                const close = std.mem.indexOfScalarPos(u8, input, i, ']');
                if (close) |ci| {
                    if (ci + 1 < input.len and input[ci + 1] == '(') {
                        try result.appendSlice(allocator, input[i + 1 .. ci]);
                        const paren_close = std.mem.indexOfScalarPos(u8, input, ci + 2, ')');
                        i = if (paren_close) |pc| pc + 1 else ci + 1;
                    } else {
                        try result.append(allocator, c);
                        i += 1;
                    }
                } else {
                    try result.append(allocator, c);
                    i += 1;
                }
            },
            '>' => {
                if (i == 0 or (i > 0 and input[i - 1] == '\n')) {
                    i += 1;
                    if (i < input.len and input[i] == ' ') i += 1;
                } else {
                    try result.append(allocator, c);
                    i += 1;
                }
            },
            else => {
                try result.append(allocator, c);
                i += 1;
            },
        }
    }

    return result.toOwnedSlice(allocator);
}

test "stripMarkdown removes headers" {
    const allocator = std.testing.allocator;
    const result = try stripMarkdown(allocator, "## Hello World");
    defer allocator.free(result);
    try std.testing.expectEqualStrings("Hello World", result);
}

test "stripMarkdown removes bold" {
    const allocator = std.testing.allocator;
    const result = try stripMarkdown(allocator, "this is **bold** text");
    defer allocator.free(result);
    try std.testing.expectEqualStrings("this is bold text", result);
}

test "stripMarkdown converts links" {
    const allocator = std.testing.allocator;
    const result = try stripMarkdown(allocator, "see [Linear](https://linear.app) for details");
    defer allocator.free(result);
    try std.testing.expectEqualStrings("see Linear for details", result);
}

test "truncate shortens text" {
    try std.testing.expectEqualStrings("Hello", truncate("Hello World!", 8));
    try std.testing.expectEqualStrings("Hi", truncate("Hi", 10));
}
