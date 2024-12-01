# `frots`: File rotation from standard input

[![crates.io](https://img.shields.io/crates/v/frots.svg)](https://crates.io/crates/frots)

The only file rotation tool I'm aware of that *actually handles stdout*.

`logrotate` (which you really ought to use if you can) happens to use `rename()`
under the hood to move the primary logfile into rotation, which confuses programs
already writing to it (they will now be writing to the rotated file, which is weird).
`frots` was primarily made to solve that problem.

Reads standard input into the specified `file` (`-f`) until:
- Standard input reaches EOF. This is the normal, and we exit normally.
- Unrecoverable errors occur. In this case, we display an error message
  and exit with a returncode of 1.

If `file` grows to reach or exceed the `limit` (`-s`), then:
1. Synchronize `file` with the disk.
2. In the range `R`..1 (where `R` is `-r`, `--num-rotate`),
   rename `file.N` to `file.N+1` if `N + 1 < R`. (I.e., rotate the files.)
3. Rename `file` `file.1`
4. Create or open the specified file and continue writing to it.

Example usage:
```sh
# Two files (one active, one rotated) of 1GB each; Verbose output along with "tee"ing
some-prog | frots -f /var/log/prog/a.log -s 1G -r 2 --tee -v
```

Notes:
- "Rename" file operations mean "in place" renaming, as-if with `rename()`, not copy-and-move.
- "Synchronize" file operations mean to-disk synchronization, as-if with `fsync()`.

```
Usage: frots [OPTIONS] --file-path <FILE_PATH> --file-sz-lim <FILE_SZ_LIM>

Options:
  -f, --file-path <FILE_PATH>
          The name of the file to write to

  -s, --file-sz-lim <FILE_SZ_LIM>
          The size limit of the file in a byte-unit format (1KB, 3M, 4G, etc.) before rotation

  -r, --num-rotate <NUM_ROTATE>
          The number of files to rotate through (see '--help' for more), must be >= 1
          
          [default: 1]

      --b-is-bits
          If "b" means bytes or bits for the `file_sz_lim`, i.e., if 1Kb = 8192b or 1024B

      --tee
          Whether to "tee" stdin to stdout as well as to `file` (just like `tee(1)`)

  -v, --verbose
          Verbose output, as-if by setting `RUST_LOG=info` in the environment

  -h, --help
          Print help (see a summary with '-h')
```
