local M = {}

M.doc = {
  name = "caller",
  version = "0.1.2",
  author = "Gru",
  description = "Executes `ls -lah` on a given directory and returns the output.",

  arguments = {
    {
      name = "dir",
      type = "string",
      required = true,
      description = "Directory path to list"
    }
  },

  options = {
    {
      name = "lines",
      description = "Split stdout into an array of lines"
    }
  },

  examples = {
    {
      description = "List /etc as raw output",
      code = [[
{ "args": { "mod": "caller", "dir": "/etc" } }
      ]]
    },
    {
      description = "List /etc as lines",
      code = [[
{ "args": { "mod": "caller", "dir": "/etc" }, "opts": ["lines"] }
      ]]
    }
  },

  returns = {
    description = "Returns stdout of `ls -lah <dir>`",
    sample = {
      output = { "total 4.0K", "-rw-r--r-- 1 root root ..." }
    }
  }
}

--- Executes `ls -lah` on the provided directory
-- @param req table SysInspect request object
-- @return table data payload
function M.run(req)
  local args = req.args or {}
  local opts = req.opts or {}

  local dir = args.dir
  if not dir or dir == "" then
    error("argument 'dir' is required")
  end

  -- check opts array
  local want_lines = false
  for _, opt in ipairs(opts) do
    if opt == "lines" then
      want_lines = true
      break
    end
  end

  local cmd = "ls -lah " .. dir .. " 2>&1"

  local p = io.popen(cmd, "r")
  if not p then
    error("failed to execute command")
  end

  local output = p:read("*a")
  local ok, _, exit_code = p:close()

  local result_output
  if want_lines then
    result_output = {}
    for line in output:gmatch("([^\n]+)") do
      table.insert(result_output, line)
    end
  else
    result_output = output
  end

  return {
    command = cmd,
    exit_code = exit_code or 0,
    output = result_output
  }
end

return M
