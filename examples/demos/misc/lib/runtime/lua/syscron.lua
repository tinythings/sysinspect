local M = {}

M.doc = {
  name = "sys.cron",
  version = "0.1.0",
  author = "Bo Maryniuk",
  description = "Manage crontab entries. Inspect, add, remove cron jobs for a given user.",
  options = {
    { name = "check",    description = "List current crontab entries" },
    { name = "present",  description = "Ensure a cron entry exists" },
    { name = "absent",   description = "Remove a cron entry if present" },
    { name = "dry-run",  description = "Show what would be done without changing crontab" },
  },
  arguments = {
    { name = "entry", type = "string", required = false, description = "Cron entry line, e.g. '0 3 * * * /usr/local/bin/backup.sh'" },
    { name = "user",  type = "string", required = false, description = "User whose crontab to manage (default: current user)" },
    { name = "match", type = "string", required = false, description = "Substring to match for removal (used by --absent)" },
  },
  examples = {
    {
      description = "List all cron entries for root",
      code = [[{ "opts": ["check"], "args": { "user": "root" } }]],
    },
    {
      description = "Add a daily backup job",
      code = [[{ "opts": ["present"], "args": { "entry": "0 3 * * * /usr/local/bin/backup.sh", "user": "root" } }]],
    },
    {
      description = "Remove all entries mentioning 'backup'",
      code = [[{ "opts": ["absent"], "args": { "match": "backup", "user": "root" } }]],
    },
  },
  returns = {
    {
      description = "Check returns crontab lines",
      retcode = 0,
      data = { entries = { "0 3 * * * /usr/local/bin/backup.sh", "* * * * * /usr/bin/logger hello" } },
    },
    {
      description = "Mutation returns what changed",
      retcode = 0,
      message = "Cron entry added",
    },
  },
}

local function shell_capture(cmd)
  local f = io.popen(cmd, "r")
  if not f then return nil end
  local out = f:read("*a")
  f:close()
  return out
end

local function crontab_cmd(user, sub)
  local u = (user and #user > 0) and ("-u " .. user) or ""
  return ("crontab " .. u .. " " .. sub)
end

local function read_crontab(user)
  local l = crontab_cmd(user, "-l")
  local out = shell_capture(l .. " 2>/dev/null")
  if not out or #out == 0 then return {} end
  local lines = {}
  for line in out:gmatch("[^\r\n]+") do
    local t = line:match("^%s*(.-)%s*$")
    if #t > 0 and not t:match("^#") then
      lines[#lines + 1] = t
    end
  end
  return lines
end

local function write_crontab(user, lines)
  local tmp = os.tmpname()
  local f = io.open(tmp, "w")
  if not f then return false end
  for _, l in ipairs(lines) do
    f:write(l, "\n")
  end
  f:close()
  local ok = os.execute(crontab_cmd(user, tmp) .. " 2>/dev/null")
  os.remove(tmp)
  return ok == 0
end

--- Check: list current crontab
function M.run(req)
  local opts = req.opts or {}
  local args = req.args or {}
  local name = args.name
  local entry = args.entry or ""
  local match = args.match or ""
  local user = args.user or ""
  local dry_run = false
  for _, o in ipairs(opts) do
    if o == "dry-run" then dry_run = true end
  end

  -- Determine operation
  local op = "check"
  for _, o in ipairs(opts) do
    if o == "check" or o == "present" or o == "absent" then
      op = o
      break
    end
  end

  local lines = read_crontab(user)

  if op == "check" then
    return { entries = lines }
  end

  if op == "present" then
    if #entry == 0 then
      return { retcode = 1, message = "Argument 'entry' is required" }
    end

    for _, l in ipairs(lines) do
      if l == entry then
        return { message = "Entry already present" }
      end
    end

    if dry_run then
      return { message = "[dry-run] would add: " .. entry }
    end

    lines[#lines + 1] = entry
    local ok = write_crontab(user, lines)
    if ok then
      return { message = "Cron entry added" }
    else
      return { retcode = 1, message = "Failed to update crontab" }
    end
  end

  if op == "absent" then
    if #match == 0 then
      return { retcode = 1, message = "Argument 'match' is required for --absent" }
    end

    local found = false
    local new_lines = {}
    for _, l in ipairs(lines) do
      if l:find(match, 1, true) then
        found = true
      else
        new_lines[#new_lines + 1] = l
      end
    end

    if not found then
      return { message = "No matching entries found" }
    end

    if dry_run then
      return { message = "[dry-run] would remove " .. #lines - #new_lines .. " entries matching '" .. match .. "'" }
    end

    local ok = write_crontab(user, new_lines)
    if ok then
      return { message = "Cron entries removed" }
    else
      return { retcode = 1, message = "Failed to update crontab" }
    end
  end

  return { retcode = 1, message = "No operation specified" }
end

return M
