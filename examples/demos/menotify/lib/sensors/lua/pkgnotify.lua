--- Return true when a Lua table behaves like an array.
-- @param v any Candidate value.
-- @return boolean True when `v` is a table with numeric indices.
local function is_array(v)
    return type(v) == "table" and rawget(v, 1) ~= nil
end

--- Return the configured track list.
-- @param ctx table MeNotify runtime context.
-- @return table Array of package names to track.
local function tracked_packages(ctx)
    if type(ctx.args.track) == "string" and ctx.args.track ~= "" then
        return { tostring(ctx.args.track) }
    end

    if is_array(ctx.args.track) then
        local out = {}
        for _, name in ipairs(ctx.args.track) do
            if tostring(name) ~= "" then
                out[#out + 1] = tostring(name)
            end
        end
        return out
    end

    return {}
end

--- Return the enabled actions selected in `opts`.
-- @param ctx table MeNotify runtime context.
-- @return table Set-like table keyed by action name.
local function enabled_actions(ctx)
    local out = { added = true, removed = true }

    if not is_array(ctx.opts) or #ctx.opts == 0 then
        return out
    end

    out = {}
    for _, opt in ipairs(ctx.opts) do
        if tostring(opt) == "added" or tostring(opt) == "removed" then
            out[tostring(opt)] = true
        end
    end

    return out
end

--- Return true when the package matches the configured tracking rules.
-- @param item table Package snapshot item.
-- @param tracked table Array of tracked package names.
-- @return boolean True when the package should be considered.
local function is_tracked(item, tracked)
    if #tracked == 0 then
        return true
    end

    for _, name in ipairs(tracked) do
        if item.name == name then
            return true
        end
    end

    return false
end

--- Build a map of relevant installed packages keyed by package id.
-- @param snapshot table Raw `packagekit.packages()` result.
-- @param tracked table Array of tracked package names.
-- @return table Map of package_id to package snapshot entry.
local function package_map(snapshot, tracked)
    local out = {}

    for _, item in ipairs(snapshot or {}) do
        if type(item) == "table" and type(item.package_id) == "string" and is_tracked(item, tracked) then
            out[item.package_id] = {
                package_id = item.package_id,
                name = item.name or "",
                version = item.version or "",
                arch = item.arch or "",
                data = item.data or "",
                summary = item.summary or "",
                info = item.info or 0,
            }
        end
    end

    return out
end

--- Return a stable fingerprint for the current package snapshot.
-- @param packages table Map of package_id to package entry.
-- @return string Stable concatenated fingerprint.
local function snapshot_fingerprint(packages)
    local keys = {}
    for package_id, _ in pairs(packages) do
        keys[#keys + 1] = package_id
    end
    table.sort(keys)
    return table.concat(keys, "\n")
end

--- Emit one Sysinspect event for a package add/remove transition.
-- @param ctx table MeNotify runtime context.
-- @param action string Either "added" or "removed".
-- @param item table Package snapshot item.
-- @return nil
local function emit_entry(ctx, action, item)
    log.info("Package", item.name, "was", action)
    ctx.emit({
        package = item.name,
        version = item.version,
        arch = item.arch,
        source = item.data,
        summary = item.summary,
        package_id = item.package_id,
        info = item.info,
    }, {
        action = action,
        key = item.package_id,
    })
end

return {
    --- Poll installed packages through PackageKit and emit add/remove events.
    -- The first successful poll seeds the local snapshot and emits nothing.
    -- @param ctx table MeNotify runtime context.
    -- @return nil
    tick = function(ctx)
        if not packagekit.available() then
            log.warn("PackageKit is not available on this system")
            return
        end

        local tracked = tracked_packages(ctx)
        local actions = enabled_actions(ctx)
        local current = package_map(packagekit.packages(), tracked)
        local fingerprint = snapshot_fingerprint(current)

        if tostring(ctx.state.get("snapshot_fingerprint") or "") == fingerprint then
            return
        end

        if not ctx.state.has("snapshot") then
            ctx.state.set("snapshot_fingerprint", fingerprint)
            ctx.state.set("snapshot", current)
            log.info("Seeded PackageKit package snapshot")
            return
        end

        local previous = ctx.state.get("snapshot") or {}

        if actions.added then
            for package_id, item in pairs(current) do
                if previous[package_id] == nil then
                    emit_entry(ctx, "added", item)
                end
            end
        end

        if actions.removed then
            for package_id, item in pairs(previous) do
                if current[package_id] == nil then
                    emit_entry(ctx, "removed", item)
                end
            end
        end

        ctx.state.set("snapshot_fingerprint", fingerprint)
        ctx.state.set("snapshot", current)
    end
}
