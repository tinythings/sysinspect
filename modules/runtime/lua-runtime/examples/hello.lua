local M = {}

-- Module documentation
M.doc = {
  name = "hello",
  version = "0.1.0",
  author = "John Smith",
  description = "Adds two numbers.",

  -- Add name and description
  options = {},

  -- Define arguments
  arguments = {
    { name = "a", type = "number", required = true,  description = "First number" },
    { name = "b", type = "number", required = true,  description = "Second number" },
  },

  -- Provide examples
  examples = {
    {
      description = "Add 1 and 2",
      code = [[
{ "args": { "mod": "test", "a": 1, "b": 2 } }
      ]]
    }
  },

  -- Define return values
  returns = {
    description = "Returns {sum=<number>}",
    sample = { sum = 3 }
  }
}

-- Main function
function M.run(req)
  local a = (req.args and req.args.a) or 0
  local b = (req.args and req.args.b) or 0
  return { sum = a + b }
end

-- Return the module
return M
