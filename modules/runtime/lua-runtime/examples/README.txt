Welcome!
These are expanded examples of to let you feel more comfortable with Lua
hacking (if that's the word).

To use these, you have to compile and install Lua runtime. So typically
if you build the whole thing using "make", you will have in your
target/<debug|release>/runtime/lua-runtime binary. Then you have to
install it, by issuing this command:

1. sysinspect module -A --path /to/your/target/release/runtime/lua-runtime --name runtime.lua-runtime --descr "Lua runtime"

   This will put Lua runtime into your package manager repository on
   SysMaster side.

2. Sync your cluster:

   sysinspect --sync


You're good to go! Now you need to install these modules. Current implementation
allows you to install Lua modules as a library to "lua-runtime" module.

To install these into your environment, do the following (assuming you
are literally HERE in the current directory):

1. Important to add $PATH_TO_HERE/lib (or just ./lib). This way the
   entire structure inside "lib" will be preserved (and this is the
   most important step):

   sysinspect module -A --path ./lib -l

2. Sync the cluster by issuing --sync:

   sysinspect --sync

3. You can verify if that landed correctly:

   sysinslect module -Ll

You should see something like this (among other stuff, if any):

 Type    Name                                    OS   Arch    SHA256        
 ────────────────────────────────────────────────────────────────────────── 
 script  runtime/lua54/caller.lua                Any  noarch  7aff...d8c5   
 script  runtime/lua54/hello.lua                 Any  noarch  22ce...f2e1   
 script  runtime/lua54/reader.lua                Any  noarch  8ce3...0135   
 script  runtime/lua54/site-lua/mathx/extra.lua  Any  noarch  92ce...79e3   
 script  runtime/lua54/site-lua/mathx/init.lua   Any  noarch  f636...f314   


4. To run them, add some caller to your model, something like:
------------------------------
entities:
  - foo

actions:
  my-example:
    descr: Call some Lua stuff
    module: runtime.lua-runtime
    bind:
      - foo
    state:
      $:
        opts:
	args:
	  mod: reader
------------------------------

This will call "reader.lua" module. It supposed to read your /etc/os-release
and extract VERSION tag, returning it. To execute that, call this:

  sysinspect yourmodel/foo 'yourminion'

...where "yourmodel" is the model you use and "yourminion" is the hostname.
Or use "*" to wakeup the entire cluster. :-)


Below are modules description:

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
