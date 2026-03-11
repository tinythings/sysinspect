--- Return true when a Lua table behaves like an array.
-- @param v any Candidate value.
-- @return boolean True when `v` is a table with numeric indices.
local function is_array(v)
    return type(v) == "table" and rawget(v, 1) ~= nil
end

--- Convert a Lua value to a number with a fallback.
-- @param v any Raw Lua value.
-- @param default number Fallback value when conversion fails.
-- @return number Converted number or the fallback.
local function as_number(v, default)
    local n = tonumber(v)
    if n == nil then
        return default
    end
    return n
end

--- Return the configured package list.
-- @param ctx table MeNotify runtime context.
-- @return table Array of package names.
local function configured_packages(ctx)
    if type(ctx.args.packages) == "string" and ctx.args.packages ~= "" then
        return { ctx.args.packages }
    end

    if is_array(ctx.args.packages) then
        local out = {}
        for _, name in ipairs(ctx.args.packages) do
            if tostring(name) ~= "" then
                out[#out + 1] = tostring(name)
            end
        end
        return out
    end

    return {}
end

--- Map PackageKit info enum value to a Sysinspect-friendly action.
-- Enum ordering is taken from PackageKit `PkInfoEnum` in `pk-enum.h`.
-- @param info number PackageKit `info` enum value.
-- @return string|nil Normalized action or nil when the entry is not interesting.
local function action_from_info(info)
    local actions = {
        [1] = "installed",
        [12] = "installed",
        [13] = "removed",
        [19] = "installed",
    }

    return actions[as_number(info, 0)]
end

--- Build a compact unique key for a normalized history entry.
-- @param pkg string Package name.
-- @param action string Normalized action.
-- @param item table PackageKit history item.
-- @return string Compact event identity key.
local function entry_key(pkg, action, item)
    return table.concat({
        pkg,
        action,
        tostring(item.version or ""),
        tostring(item.source or ""),
        tostring(item.timestamp or 0),
        tostring(item["user-id"] or 0),
        tostring(item.info or 0),
    }, "|")
end

--- Normalize raw PackageKit history into event candidates.
-- @param history table Raw `packagekit.history(...)` result.
-- @param packages table Configured package names.
-- @return table Array of normalized event candidates.
local function normalize_history(history, packages)
    local out = {}

    for _, pkg in ipairs(packages) do
        local rows = type(history) == "table" and history[pkg] or nil
        if is_array(rows) then
            for _, item in ipairs(rows) do
                local action = type(item) == "table" and action_from_info(item.info) or nil
                if action ~= nil then
                    out[#out + 1] = {
                        package = pkg,
                        action = action,
                        version = item.version or "",
                        source = item.source or "",
                        timestamp = as_number(item.timestamp, 0),
                        user_id = as_number(item["user-id"], 0),
                        info = as_number(item.info, 0),
                        key = entry_key(pkg, action, item),
                    }
                end
            end
        end
    end

    table.sort(out, function(a, b)
        if a.timestamp ~= b.timestamp then
            return a.timestamp < b.timestamp
        end
        return a.key < b.key
    end)

    return out
end

--- Return a cheap fingerprint for the current normalized history slice.
-- @param entries table Array of normalized history entries.
-- @return string Stable concatenated fingerprint.
local function history_fingerprint(entries)
    local keys = {}
    for _, item in ipairs(entries) do
        keys[#keys + 1] = item.key
    end
    return table.concat(keys, "\n")
end

--- Return a set-like table from an array of keys.
-- @param keys table|nil Array of string keys from VM-local state.
-- @return table Set-like table of existing keys.
local function keyset(keys)
    local out = {}
    if is_array(keys) then
        for _, key in ipairs(keys) do
            out[tostring(key)] = true
        end
    end
    return out
end

--- Return only the entry keys from normalized history.
-- @param entries table Array of normalized history entries.
-- @return table Array of string keys.
local function snapshot_keys(entries)
    local out = {}
    for _, item in ipairs(entries) do
        out[#out + 1] = item.key
    end
    return out
end

--- Emit one Sysinspect event for a normalized package history entry.
-- @param ctx table MeNotify runtime context.
-- @param item table Normalized history entry.
-- @return nil
local function emit_entry(ctx, item)
    log.info("Package", item.package, "was", item.action)
    ctx.emit({
        package = item.package,
        version = item.version,
        source = item.source,
        timestamp = item.timestamp,
        user_id = item.user_id,
        info = item.info,
    }, {
        action = item.action,
        key = item.key,
    })
end

return {
    --- Poll PackageKit package history and emit install/remove/update events.
    -- The first successful poll seeds the local snapshot and emits nothing
    -- unless `bootstrap_emit_existing` is true.
    -- @param ctx table MeNotify runtime context.
    -- @return nil
    tick = function(ctx)
        local packages = configured_packages(ctx)
        if #packages == 0 then
            log.error("pkgnotify requires args.packages")
            return
        end

        if not packagekit.available() then
            log.warn("PackageKit is not available on this system")
            return
        end

        local entries = normalize_history(packagekit.history(packages, as_number(ctx.args.history_count, 20)), packages)
        local fingerprint = history_fingerprint(entries)
        local previous_fingerprint = tostring(ctx.state.get("history_fingerprint") or "")

        if previous_fingerprint == fingerprint then
            return
        end

        local keys = snapshot_keys(entries)
        local seeded = ctx.state.has("history_keys")
        local seen = keyset(ctx.state.get("history_keys"))

        if not seeded then
            ctx.state.set("history_fingerprint", fingerprint)
            ctx.state.set("history_keys", keys)
            log.info("Seeded PackageKit history snapshot for", table.concat(packages, ", "))

            if ctx.args.bootstrap_emit_existing then
                for _, item in ipairs(entries) do
                    emit_entry(ctx, item)
                end
            end
            return
        end

        for _, item in ipairs(entries) do
            if not seen[item.key] then
                emit_entry(ctx, item)
            end
        end

        ctx.state.set("history_fingerprint", fingerprint)
        ctx.state.set("history_keys", keys)
    end
}
