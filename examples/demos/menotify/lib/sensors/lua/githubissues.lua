--- Convert a Lua value into a number and fall back to a default value.
-- @param v any Raw Lua value that might hold a number-like string or number.
-- @param default number Fallback value when conversion fails.
-- @return number Converted numeric value or the provided default.
local function as_number(v, default)
    local n = tonumber(v)
    if n == nil then
        return default
    end
    return n
end

--- Build a printable repository name from the configured owner and repo.
-- @param ctx table MeNotify runtime context.
-- @return string Repository name in the form "owner/repo".
local function repo_name(ctx)
    return string.format("%s/%s", ctx.args.owner, ctx.args.repo)
end

--- Return true when the GitHub API item is a real issue and not a pull request.
-- @param item table GitHub API entry from the issues listing.
-- @return boolean True for real issues, false for pull requests or malformed items.
local function is_issue(item)
    return type(item) == "table" and item.pull_request == nil and item.number ~= nil
end

--- Build the GitHub issues listing URL from the configured arguments.
-- @param ctx table MeNotify runtime context.
-- @return string Fully qualified GitHub issues API URL.
local function request_url(ctx)
    return string.format(
        "%s/repos/%s/%s/issues?state=%s&sort=created&direction=desc&per_page=%d",
        ctx.args.api or "https://api.github.com",
        ctx.args.owner,
        ctx.args.repo,
        ctx.args.state or "open",
        as_number(ctx.args.per_page, 20)
    )
end

--- Build HTTP headers for the GitHub API request.
-- @param ctx table MeNotify runtime context.
-- @return table HTTP headers table for http.get().
local function request_headers(ctx)
    local headers = {
        ["Accept"] = "application/vnd.github+json",
        ["User-Agent"] = ctx.args.user_agent or "sysinspect-menotify-githubissues"
    }

    if ctx.args.token ~= nil and tostring(ctx.args.token) ~= "" then
        headers["Authorization"] = "Bearer " .. tostring(ctx.args.token)
    end

    return headers
end

--- Return the highest issue number present in the current API response.
-- @param items table Array-like GitHub issues response payload.
-- @return number Highest issue number found, or 0 when none are present.
local function highest_issue_number(items)
    local maxn = 0

    for _, item in ipairs(items or {}) do
        if is_issue(item) and as_number(item.number, 0) > maxn then
            maxn = as_number(item.number, 0)
        end
    end

    return maxn
end

--- Emit one Sysinspect event for a newly discovered GitHub issue.
-- @param ctx table MeNotify runtime context.
-- @param issue table GitHub issue object from the API response.
-- @return nil
local function emit_issue(ctx, issue)
    local number = as_number(issue.number, 0)

    log.info("New issue here:", "#" .. tostring(number), issue.title or "")
    ctx.emit({
        number = number,
        title = issue.title or "",
        body = issue.body or "",
        state = issue.state or "",
        user = issue.user and issue.user.login or "",
        html_url = issue.html_url or "",
        api_url = issue.url or "",
        created_at = issue.created_at or "",
        updated_at = issue.updated_at or "",
    }, {
        action = "opened",
        key = tostring(number),
    })
end

return {
  --- Poll the configured GitHub repository and emit one event per new issue.
  -- The first successful poll only seeds the local state cursor unless
  -- `bootstrap_emit_existing` is enabled in the sensor arguments.
  -- @param ctx table MeNotify runtime context.
  -- @return nil
    tick = function(ctx)
        if ctx.args.owner == nil or ctx.args.repo == nil then
            log.error("githubissues requires args.owner and args.repo")
            return
        end

        local rsp = http.get(request_url(ctx), {
            headers = request_headers(ctx),
            parse_json = true,
            timeout = as_number(ctx.args.timeout, 30),
        })

        if not rsp.ok then
            log.error("GitHub issues poll failed for", repo_name(ctx), "with HTTP status", rsp.status)
            return
        end

        if type(rsp.json) ~= "table" then
            log.error("GitHub issues poll returned no JSON array for", repo_name(ctx))
            return
        end

        local seeded = ctx.state.has("last_seen_number")
        local last_seen = as_number(ctx.state.get("last_seen_number"), 0)
        local current_max = highest_issue_number(rsp.json)

        if not seeded then
            ctx.state.set("last_seen_number", current_max)
            log.info("Seeded GitHub issue cursor for", repo_name(ctx), "at", current_max)

            if ctx.args.bootstrap_emit_existing then
                for i = #rsp.json, 1, -1 do
                    local issue = rsp.json[i]
                    if is_issue(issue) then
                        emit_issue(ctx, issue)
                    end
                end
            end
            return
        end

        for i = #rsp.json, 1, -1 do
            local issue = rsp.json[i]
            local number = is_issue(issue) and as_number(issue.number, 0) or 0

            if number > last_seen then
                emit_issue(ctx, issue)
                last_seen = number
            end
        end

        if current_max > as_number(ctx.state.get("last_seen_number"), 0) then
            ctx.state.set("last_seen_number", current_max)
        end
    end
}
