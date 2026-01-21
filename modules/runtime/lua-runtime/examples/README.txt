These are expanded examples of to let you feel more comfortable with Lua.


hello.lua
=========

  1. Load an extra package, made of two files
  2. Document your whole program and return its manpage
  3. Calculate something and return the result


caller.lua
==========

  1. Execs a shell command
  2. Returns the output as data


reader.lua
==========

  1. Reads a file /etc/os-release
  2. Finds "VERSION"
  3. Returns the result as data

Call example:

$ echo '{"opts":["lines"], "args":{"mod": "caller", "dir": "."}}' | ../../../../target/debug/runtime/lua-runtime | jq
{
  "retcode": 0,
  "message": "Called Lua module successfully.",
  "data": {
    "changed": true,
    "command": "ls -lah . 2>&1",
    "exit_code": 0,
    "output": [
      "total 28K",
      "drwxr-xr-x 3 isbm user 4,0K Jan 21 19:28 .",
      "drwxr-xr-x 4 isbm user 4,0K Jan 21 17:49 ..",
      "-rw-r--r-- 1 isbm user 1,9K Jan 21 19:38 caller.lua",
      "-rw-r--r-- 1 isbm user 1,2K Jan 21 19:34 hello.lua",
      "drwxr-xr-x 3 isbm user 4,0K Jan 21 19:20 lib",
      "-rw-r--r-- 1 isbm user 1,2K Jan 21 19:34 reader.lua",
      "-rw-r--r-- 1 isbm user  442 Jan 21 19:28 README.txt"
    ]
  }
}
