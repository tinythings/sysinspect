0. Ensure you have "wasm-opt" from  Binaryen:

   apt install binaryen <ENTER>


1. Build it:

   make <ENTER>


2. From here, go to target/release and add "./lib" with the sysinspect:

   cd target/release <ENTER>
   sysinspect module -Alp ./lib <ENTER>


3. Refresh your cluster:

   sysinspect --sync <YES-ALSO-ENTER>



Done.
