Good news:
     Runtimes are just a regular standalone modules

Each runtime has just as same interaction as a regular module,
except it has additional information, such as:

1. id (i.e. string of runtime identification: "wasm", "lua", "python" etc)
2. Additional targeting. A regular module has just opts and args, but a runtime
   must additionally know what its module is targeted

Different news:
     Runtimes need extra management for their modules,
     so package manager must take care of it.
