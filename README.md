cultivate/jj-vfs
=======
An experimental vfs backend for [jj](https://martinvonz.github.io/jj/)

WARNING: EXTREMELY EXPERIMENTAL

TODO:
* snapshot logic
* mount working copy directory with .jj/ passthrough
* use xfstests for correctness.
* persistent store for daemon, currently all in-memory.
* split daemon into a local daemon and a remote server.
* basically rewrite the whole thing because it's unorganized and kludgy.
