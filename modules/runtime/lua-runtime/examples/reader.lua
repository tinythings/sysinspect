local M = {}

-- Module documentation
M.doc = {
  name = "reader",
  version = "0.1.0",
  author = "Bo Maryniuk",
  description = "Reads /etc/os-release and returns VERSION.",
  arguments = {},
  examples = {
    {
      description = "Read OS version",
      code = [[
{ "args": { "mod": "reader" } }
      ]]
    }
  },
  returns = {
    description = "Returns detected OS version",
    sample = { version = "12 (bookworm)" }
  }
}

--- Function to read /etc/os-release and extract VERSION
-- @return string|nil VERSION value or nil if not found
local function read_os_release()
  local f, err = io.open("/etc/os-release", "r")
  if not f then
    error("failed to open /etc/os-release: " .. tostring(err))
  end

  local version = nil
  for line in f:lines() do
    local v = line:match('^VERSION="?([^"]+)"?')
    if v then
      version = v
      break
    end
  end

  f:close()
  return version
end

--- Main function
-- @param _req table Request object (not used)
-- @return table Result containing the OS version
function M.run(_req)
  local version = read_os_release()

  if not version then
    error("VERSION not found in /etc/os-release")
  end

  return {
    version = version
  }
end

return M
