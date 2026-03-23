const std = @import("std");

const log = std.log.scoped(.config);

const config_dir_name = "lnr";
const config_file_name = "config.json";

pub const StoredConfig = struct {
    access_token: []const u8,
    refresh_token: []const u8,
    expires_at: i64,
};

pub fn getConfigDir(allocator: std.mem.Allocator) ![]const u8 {
    if (std.posix.getenv("XDG_CONFIG_HOME")) |xdg| {
        return std.fmt.allocPrint(allocator, "{s}/{s}", .{ xdg, config_dir_name });
    }
    if (std.posix.getenv("HOME")) |home| {
        return std.fmt.allocPrint(allocator, "{s}/.config/{s}", .{ home, config_dir_name });
    }
    return error.FileNotFound;
}

pub fn getConfigPath(allocator: std.mem.Allocator) ![]const u8 {
    const dir = try getConfigDir(allocator);
    defer allocator.free(dir);
    return std.fmt.allocPrint(allocator, "{s}/{s}", .{ dir, config_file_name });
}

pub fn load(allocator: std.mem.Allocator) !StoredConfig {
    const path = try getConfigPath(allocator);
    defer allocator.free(path);

    const file = std.fs.cwd().openFile(path, .{}) catch return error.FileNotFound;
    defer file.close();

    const contents = file.readToEndAlloc(allocator, 1024 * 16) catch return error.ParseError;
    defer allocator.free(contents);

    const parsed = std.json.parseFromSlice(
        StoredConfig,
        allocator,
        contents,
        .{ .ignore_unknown_fields = true, .allocate = .alloc_always },
    ) catch return error.ParseError;
    defer parsed.deinit();

    return .{
        .access_token = try allocator.dupe(u8, parsed.value.access_token),
        .refresh_token = try allocator.dupe(u8, parsed.value.refresh_token),
        .expires_at = parsed.value.expires_at,
    };
}

pub fn save(allocator: std.mem.Allocator, cfg: StoredConfig) !void {
    const path = try getConfigPath(allocator);
    defer allocator.free(path);

    // Extract dir from path for makePath
    const dir_end = std.mem.lastIndexOfScalar(u8, path, '/') orelse return error.WriteError;
    const dir_path = path[0..dir_end];

    std.fs.cwd().makePath(dir_path) catch |err| {
        log.err("failed to create config dir: {}", .{err});
        return error.WriteError;
    };

    const file = std.fs.cwd().createFile(path, .{ .mode = 0o600 }) catch |err| {
        log.err("failed to create config file: {}", .{err});
        return error.WriteError;
    };
    defer file.close();

    var write_buf: [4096]u8 = undefined;
    var fw = file.writer(&write_buf);
    const w = &fw.interface;
    w.print("{f}", .{std.json.fmt(cfg, .{})}) catch return error.WriteError;
    w.flush() catch return error.WriteError;
}

pub fn delete(allocator: std.mem.Allocator) !void {
    const path = try getConfigPath(allocator);
    defer allocator.free(path);

    std.fs.cwd().deleteFile(path) catch |err| {
        if (err == error.FileNotFound) return;
        log.err("failed to delete config: {}", .{err});
        return error.WriteError;
    };
}
